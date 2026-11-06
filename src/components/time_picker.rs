//! Time picker: a clock dial, or two numeric fields, in 24 h or 12 h, docked in place or modal.
//!
//! The dial's maths (pointer angle to hour or minute, the rings of a 24 h dial, where the
//! selector rests) are plain functions with unit tests.

use std::fmt;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, Entity, FocusHandle, HitboxBehavior, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement,
    PathBuilder, Pixels, RenderOnce, SharedString, Stateful, StatefulInteractiveElement, Styled,
    Window, canvas, div, point, px,
};

use super::overlays::{ground, over};
use super::{Button, ChangeHandler, ClickHandler, Dialog, Icon, all, child_id};
use crate::interaction::{focusable, interactive, state_layer, use_focus, use_interaction};
use crate::motion::{Spring, animate, animate_color, key};
use crate::theme::type_scale::{BODY_LARGE, BODY_SMALL, LABEL_LARGE, LABEL_MEDIUM, TypeScaleExt};
use crate::theme::{Scheme, Theme, alpha, font_family};

// ---------------------------------------------------------------------------------------------
// Time and dial maths
// ---------------------------------------------------------------------------------------------

/// A time of day. Displays as `HH:MM` (24 h).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time {
    /// 0 to 23.
    pub hour: u8,
    /// 0 to 59.
    pub minute: u8,
}

/// Before or after noon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Meridiem {
    Am,
    Pm,
}

impl Time {
    /// A time, or `None` when the hour or the minute are out of range.
    pub fn new(hour: u8, minute: u8) -> Option<Self> {
        (hour < 24 && minute < 60).then_some(Self { hour, minute })
    }

    /// The hour on a 12 h clock (1 to 12) and whether it is before or after noon.
    pub fn to_12h(self) -> (u8, Meridiem) {
        let meridiem = if self.hour < 12 { Meridiem::Am } else { Meridiem::Pm };
        (if matches!(self.hour % 12, 0) { 12 } else { self.hour % 12 }, meridiem)
    }

    /// The time of a 12 h clock reading; an hour outside 1 to 12 is taken modulo 12.
    pub fn from_12h(hour: u8, meridiem: Meridiem, minute: u8) -> Self {
        let pm = if meridiem == Meridiem::Pm { 12 } else { 0 };
        Self { hour: hour % 12 + pm, minute: minute.min(59) }
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

/// What the dial is choosing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Hour,
    Minute,
}

const DIAL: f32 = 256.;
const SELECTOR: f32 = 48.;
/// Distance from the center to the numbers of the outer ring, and of the inner ring (24 h).
const OUTER_RING: f32 = 104.;
const INNER_RING: f32 = 68.;

/// Angle of a pointer `(dx, dy)` away from the dial's center, in degrees clockwise from twelve
/// o'clock: 0 up, 90 right, 180 down, 270 left.
fn pointer_angle(dx: f32, dy: f32) -> f32 {
    dx.atan2(-dy).to_degrees().rem_euclid(360.)
}

/// The hour under a pointer. On a 12 h dial it is 1 to 12; on a 24 h dial the outer ring is 1 to
/// 12 (12 on top) and the inner one 13 to 23 with 0 on top.
fn hour_at(dx: f32, dy: f32, twenty_four: bool) -> u8 {
    let slot = ((pointer_angle(dx, dy) / 30.).round() as u32 % 12) as u8;
    let inner = twenty_four && dx.hypot(dy) < (OUTER_RING + INNER_RING) / 2.;
    match (inner, slot) {
        (false, 0) => 12,
        (false, slot) => slot,
        (true, 0) => 0,
        (true, slot) => 12 + slot,
    }
}

/// The minute under a pointer, snapped to `step` minutes (1 snaps to every minute).
fn minute_at(dx: f32, dy: f32, step: u8) -> u8 {
    let step = f32::from(step.clamp(1, 30));
    let snapped = (pointer_angle(dx, dy) / (6. * step)).round() * step;
    snapped as u32 as u8 % 60
}

/// Where the selector rests for an hour: its angle in degrees clockwise from the top, and whether
/// it sits on the inner ring.
fn hour_angle(hour: u8, twenty_four: bool) -> (f32, bool) {
    let inner = twenty_four && (hour == 0 || hour > 12);
    (f32::from(hour % 12) * 30., inner)
}

/// The time after moving the part the dial is choosing by `delta` steps. An hour wraps around the
/// day, a minute around the hour (without carrying into the hour).
fn nudge(time: Time, part: Part, delta: i32, minute_step: u8) -> Time {
    match part {
        Part::Hour => Time { hour: (i32::from(time.hour) + delta).rem_euclid(24) as u8, ..time },
        Part::Minute => {
            let step = i32::from(minute_step.max(1));
            Time { minute: (i32::from(time.minute) + delta * step).rem_euclid(60) as u8, ..time }
        }
    }
}

/// The value of a field after typing a digit: the old last digit moves to the tens, and a number
/// beyond `max` starts again from the digit. Never below `min`.
fn type_digit(current: u8, digit: u8, min: u8, max: u8) -> u8 {
    let next = (current % 10) * 10 + digit;
    (if next > max { digit } else { next }).clamp(min, max)
}

/// The value of a field after Backspace: the last digit goes. Never below `min`.
fn erase_digit(current: u8, min: u8) -> u8 {
    (current / 10).max(min)
}

/// The range of a field: the hour of a 12 h clock is 1 to 12, of a 24 h clock 0 to 23.
fn field_range(part: Part, twenty_four: bool) -> (u8, u8) {
    match (part, twenty_four) {
        (Part::Hour, false) => (1, 12),
        (Part::Hour, true) => (0, 23),
        (Part::Minute, _) => (0, 59),
    }
}

/// The number a field shows.
fn field_value(time: Time, part: Part, twenty_four: bool) -> u8 {
    match (part, twenty_four) {
        (Part::Hour, false) => time.to_12h().0,
        (Part::Hour, true) => time.hour,
        (Part::Minute, _) => time.minute,
    }
}

/// The time after a field takes a number (the 12 h hour keeps its side of noon).
fn with_field(time: Time, part: Part, value: u8, twenty_four: bool) -> Time {
    match (part, twenty_four) {
        (Part::Hour, false) => Time::from_12h(value, time.to_12h().1, time.minute),
        (Part::Hour, true) => Time { hour: value.min(23), ..time },
        (Part::Minute, _) => Time { minute: value.min(59), ..time },
    }
}

/// The time after Up or Down on a field: it goes round its own range, without carrying (the 12 h
/// hour goes from 12 to 1 and keeps its side of noon).
fn nudge_field(time: Time, part: Part, delta: i32, twenty_four: bool) -> Time {
    let (min, max) = field_range(part, twenty_four);
    let size = i32::from(max - min) + 1;
    let moved = (i32::from(field_value(time, part, twenty_four) - min) + delta).rem_euclid(size);
    with_field(time, part, moved as u8 + min, twenty_four)
}

// ---------------------------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------------------------

/// The words of a time picker. [`TimeNames::english`] is the default.
#[derive(Clone, Debug)]
pub struct TimeNames {
    pub select_time: SharedString,
    pub enter_time: SharedString,
    pub hour: SharedString,
    pub minute: SharedString,
    pub am: SharedString,
    pub pm: SharedString,
    pub ok: SharedString,
    pub cancel: SharedString,
}

impl Default for TimeNames {
    fn default() -> Self {
        Self::english()
    }
}

impl TimeNames {
    pub fn english() -> Self {
        Self {
            select_time: "Select time".into(),
            enter_time: "Enter time".into(),
            hour: "Hour".into(),
            minute: "Minute".into(),
            am: "AM".into(),
            pm: "PM".into(),
            ok: "OK".into(),
            cancel: "Cancel".into(),
        }
    }

    pub fn spanish() -> Self {
        Self {
            select_time: "Elige la hora".into(),
            enter_time: "Escribe la hora".into(),
            hour: "Hora".into(),
            minute: "Minuto".into(),
            am: "a. m.".into(),
            pm: "p. m.".into(),
            ok: "Aceptar".into(),
            cancel: "Cancelar".into(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------------------------

/// What the picker remembers between frames. The time itself belongs to the app.
struct State {
    part: Part,
    input: bool,
    dragging: bool,
    /// Last time seen or sent: keys typed between two frames build on it.
    time: Time,
    /// Angle the selector was drawn at, to turn the short way round.
    angle: f32,
}

type TimeEmit = Rc<dyn Fn(Time, &mut Window, &mut App)>;

/// Material 3 time picker.
///
/// It does not keep the time: pass it with [`TimePicker::value`] and update your own state in
/// [`TimePicker::on_change`]. The dial picks the hour and goes on to the minutes when the pointer
/// is released; the header tokens switch between them, and the keyboard button swaps the dial for
/// two numeric fields (type digits, or use the arrows).
///
/// A modal picker covers the window with a scrim and has OK and Cancel buttons
/// ([`TimePicker::on_confirm`], [`TimePicker::on_cancel`]; Escape and a click on the scrim cancel).
/// Render a modal one only while it is open, and drive its exit with [`TimePicker::exit`] like a
/// [`Dialog`].
#[derive(IntoElement)]
pub struct TimePicker {
    id: ElementId,
    modal: bool,
    value: Time,
    twenty_four: bool,
    minute_step: u8,
    input_mode: bool,
    start_with_input: bool,
    title: Option<SharedString>,
    names: TimeNames,
    shown: f32,
    on_change: Option<ChangeHandler<Time>>,
    on_confirm: Option<ClickHandler>,
    on_cancel: Option<ClickHandler>,
}

impl TimePicker {
    fn with_modal(id: impl Into<ElementId>, modal: bool) -> Self {
        Self {
            id: id.into(),
            modal,
            value: Time::default(),
            twenty_four: false,
            minute_step: 1,
            input_mode: true,
            start_with_input: false,
            title: None,
            names: TimeNames::english(),
            shown: 1.,
            on_change: None,
            on_confirm: None,
            on_cancel: None,
        }
    }

    /// A picker drawn in place.
    pub fn docked(id: impl Into<ElementId>) -> Self {
        Self::with_modal(id, false)
    }

    /// A picker over a scrim, with OK and Cancel buttons.
    pub fn modal(id: impl Into<ElementId>) -> Self {
        Self::with_modal(id, true)
    }

    /// The time shown.
    pub fn value(mut self, value: Time) -> Self {
        self.value = value;
        self
    }

    /// A 24 h dial with two rings, instead of the 12 h dial with an AM / PM switch (the default).
    pub fn twenty_four_hour(mut self, twenty_four: bool) -> Self {
        self.twenty_four = twenty_four;
        self
    }

    /// Minutes the dial snaps to while dragging (1 by default).
    pub fn minute_step(mut self, step: u8) -> Self {
        self.minute_step = step.clamp(1, 30);
        self
    }

    /// Shows the button that swaps the dial for numeric fields (on by default).
    pub fn input_mode(mut self, input_mode: bool) -> Self {
        self.input_mode = input_mode;
        self
    }

    /// Opens with the numeric fields instead of the dial.
    pub fn start_with_input(mut self, start_with_input: bool) -> Self {
        self.start_with_input = start_with_input;
        self
    }

    /// Replaces the small title above the time.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The words of the picker (English by default).
    pub fn names(mut self, names: TimeNames) -> Self {
        self.names = names;
        self
    }

    /// Draws a modal picker while it leaves: `progress` goes from 1 to 0, as
    /// [`crate::motion::presence`] returns it.
    pub fn exit(mut self, progress: f32) -> Self {
        self.shown = progress.clamp(0., 1.);
        self
    }

    /// Called with the time every time the dial, a field or the AM / PM switch changes it.
    pub fn on_change(mut self, f: impl Fn(Time, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    /// Called by the OK button (and Enter) of a modal picker.
    pub fn on_confirm(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_confirm = Some(Rc::new(f));
        self
    }

    /// Called by the Cancel button, Escape and a click on the scrim of a modal picker.
    pub fn on_cancel(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(f));
        self
    }
}

/// One of the two numbers of the header: the button that picks what the dial chooses, and the
/// field that takes typed digits in input mode.
#[allow(clippy::too_many_arguments)]
fn token(
    id: &ElementId,
    part: Part,
    text: String,
    active: bool,
    input: bool,
    s: &Scheme,
    window: &mut Window,
    cx: &mut App,
) -> (Stateful<Div>, FocusHandle) {
    let token_id = key(id, if part == Part::Hour { "hour" } else { "minute" });
    let interaction = use_interaction(&token_id, window, cx);
    let (focus, handle, _) = use_focus(&token_id, window, cx);
    let typing = input && handle.is_focused(window);
    let (bg, fg) = if active || typing {
        (s.primary_container, s.on_primary_container)
    } else {
        (s.surface_container_highest, s.on_surface)
    };
    let bg = animate_color(key(&token_id, "bg"), Spring::EFFECTS, bg, window, cx);
    let layer = state_layer(&token_id, &interaction, all(px(8.)), fg, window, cx);
    let el = div()
        .id(token_id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(px(96.))
        .h(px(if input { 72. } else { 80. }))
        .rounded(px(8.))
        .bg(bg)
        .text_color(fg)
        .text_size(px(if input { 45. } else { 57. }))
        .line_height(px(64.))
        .when(typing, |d| d.border_2().border_color(s.primary))
        .child(layer)
        .child(text);
    (focusable(interactive(el, &interaction, true, true), &focus, &handle), handle)
}

impl RenderOnce for TimePicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let id = self.id.clone();
        let twenty_four = self.twenty_four;
        let start_with_input = self.start_with_input;
        let value = self.value;
        let state = window.use_keyed_state(key(&id, "state"), cx, move |_, _| State {
            part: Part::Hour,
            input: start_with_input,
            dragging: false,
            time: value,
            angle: 0.,
        });
        state.update(cx, |st, _| st.time = value);
        let focus = window.use_keyed_state(key(&id, "focus"), cx, |_, cx| cx.focus_handle());
        let handle = focus.read(cx).clone();
        if self.modal {
            let asked = window.use_keyed_state(key(&id, "autofocus"), cx, |_, _| false);
            if !*asked.read(cx) {
                asked.update(cx, |asked, _| *asked = true);
                window.focus(&handle);
            }
        }
        let (part, input, dragging) = {
            let st = state.read(cx);
            (st.part, st.input, st.dragging)
        };

        let emit: TimeEmit = {
            let state = state.clone();
            let on_change = self.on_change.clone();
            Rc::new(move |time, window, cx| {
                state.update(cx, |st, _| st.time = time);
                if let Some(f) = &on_change {
                    f(time, window, cx);
                }
            })
        };

        let meridiem = value.to_12h().1;
        let confirm = self.modal.then(|| self.on_confirm.clone()).flatten();
        let cancel = self.modal.then(|| self.on_cancel.clone()).flatten();
        let minute_step = self.minute_step;

        // ----- header: the two numbers, and AM / PM
        let attach = |token: Stateful<Div>, part_of: Part, field: FocusHandle| {
            let (state, emit) = (state.clone(), emit.clone());
            let (click_state, click_field) = (state.clone(), field);
            token
                .on_click(move |_, window, cx| {
                    let typing = click_state.update(cx, |st, cx| {
                        st.part = part_of;
                        cx.notify();
                        st.input
                    });
                    if typing {
                        window.focus(&click_field);
                    }
                })
                .on_key_down(move |event, window, cx| {
                    let (time, typing) = {
                        let st = state.read(cx);
                        (st.time, st.input)
                    };
                    if !typing {
                        return;
                    }
                    let key = event.keystroke.key.as_str();
                    let (min, max) = field_range(part_of, twenty_four);
                    let current = field_value(time, part_of, twenty_four);
                    let number = match key {
                        "up" => nudge_field(time, part_of, 1, twenty_four),
                        "down" => nudge_field(time, part_of, -1, twenty_four),
                        "backspace" => with_field(time, part_of, erase_digit(current, min), twenty_four),
                        digit if digit.len() == 1 && digit.as_bytes()[0].is_ascii_digit() => {
                            let typed = type_digit(current, digit.as_bytes()[0] - b'0', min, max);
                            with_field(time, part_of, typed, twenty_four)
                        }
                        _ => return,
                    };
                    cx.stop_propagation();
                    state.update(cx, |st, cx| {
                        st.part = part_of;
                        cx.notify();
                    });
                    emit(number, window, cx);
                })
        };
        let (hour_token, hour_field) = token(
            &id,
            Part::Hour,
            format!("{:02}", field_value(value, Part::Hour, twenty_four)),
            part == Part::Hour,
            input,
            &s,
            window,
            cx,
        );
        let (minute_token, minute_field) = token(
            &id,
            Part::Minute,
            format!("{:02}", value.minute),
            part == Part::Minute,
            input,
            &s,
            window,
            cx,
        );
        let hour_token = attach(hour_token, Part::Hour, hour_field.clone());
        let minute_token = attach(minute_token, Part::Minute, minute_field);

        let with_label = |token: Stateful<Div>, label: SharedString| {
            div().flex().flex_col().gap(px(4.)).child(token).when(input, |d| {
                d.child(div().type_style(BODY_SMALL).text_color(s.on_surface_variant).child(label))
            })
        };
        let token_height = px(if input { 72. } else { 80. });
        let colon = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(24.))
            .h(token_height)
            .text_size(px(if input { 45. } else { 57. }))
            .line_height(px(64.))
            .text_color(s.on_surface)
            .child(":");

        let meridiem_toggle = (!twenty_four).then(|| {
            let segment = |which: Meridiem, label: SharedString, window: &mut Window, cx: &mut App| {
                let segment_id = key(&id, if which == Meridiem::Am { "am" } else { "pm" });
                let interaction = use_interaction(&segment_id, window, cx);
                let (bg, fg) = if meridiem == which {
                    (s.tertiary_container, s.on_tertiary_container)
                } else {
                    (alpha(s.tertiary_container, 0.), s.on_surface_variant)
                };
                let bg = animate_color(key(&segment_id, "bg"), Spring::EFFECTS, bg, window, cx);
                let layer = state_layer(&segment_id, &interaction, all(px(0.)), fg, window, cx);
                let (state, emit) = (state.clone(), emit.clone());
                let el = div()
                    .id(segment_id)
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_1()
                    .bg(bg)
                    .text_color(fg)
                    .type_emphasized(LABEL_LARGE)
                    .child(layer)
                    .child(label);
                interactive(el, &interaction, true, false).on_click(move |_, window, cx| {
                    let time = state.read(cx).time;
                    emit(Time::from_12h(time.to_12h().0, which, time.minute), window, cx);
                })
            };
            div()
                .flex()
                .flex_col()
                .flex_none()
                .overflow_hidden()
                .w(px(52.))
                .h(token_height)
                .ml(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(s.outline)
                .child(segment(Meridiem::Am, self.names.am.clone(), window, cx))
                .child(div().h(px(1.)).w_full().bg(s.outline))
                .child(segment(Meridiem::Pm, self.names.pm.clone(), window, cx))
        });

        let header = div()
            .flex()
            .items_start()
            .when(twenty_four, |d| d.justify_center())
            .child(with_label(hour_token, self.names.hour.clone()))
            .child(colon)
            .child(with_label(minute_token, self.names.minute.clone()))
            .children(meridiem_toggle);

        // ----- surface
        let title = self.title.clone().unwrap_or_else(|| {
            if input { self.names.enter_time.clone() } else { self.names.select_time.clone() }
        });
        let on_key = {
            let (state, emit) = (state.clone(), emit.clone());
            let (confirm, cancel) = (confirm.clone(), cancel.clone());
            move |event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut App| {
                match event.keystroke.key.as_str() {
                    "escape" => {
                        if let Some(cancel) = &cancel {
                            cx.stop_propagation();
                            cancel(&ClickEvent::default(), window, cx);
                        }
                    }
                    "enter" => {
                        if let Some(confirm) = &confirm {
                            cx.stop_propagation();
                            confirm(&ClickEvent::default(), window, cx);
                        }
                    }
                    key @ ("up" | "down" | "left" | "right") => {
                        let (time, part, input) = {
                            let st = state.read(cx);
                            (st.time, st.part, st.input)
                        };
                        if input {
                            return;
                        }
                        cx.stop_propagation();
                        let delta = if matches!(key, "up" | "right") { 1 } else { -1 };
                        emit(nudge(time, part, delta, minute_step), window, cx);
                    }
                    _ => {}
                }
            }
        };
        let mut surface = div()
            .id(key(&id, "root"))
            .track_focus(&handle)
            .flex()
            .flex_col()
            .w_full()
            .px(px(24.))
            .pt(px(24.))
            .pb(px(20.))
            .text_color(s.on_surface)
            .font_family(font_family())
            .on_key_down(on_key)
            .child(
                div()
                    .h(px(20.))
                    .mb(px(20.))
                    .type_style(LABEL_MEDIUM)
                    .text_color(s.on_surface_variant)
                    .child(title),
            )
            .child(header);

        if !input {
            let dial = self.dial(&state, &emit, part, dragging, value, &s, window, cx);
            surface = surface.child(div().flex().justify_center().mt(px(36.)).child(dial));
        }

        // ----- actions: the keyboard button, and OK / Cancel
        let toggle = self.input_mode.then(|| {
            let state = state.clone();
            let toggle_id = key(&id, "toggle");
            let interaction = use_interaction(&toggle_id, window, cx);
            let layer =
                state_layer(&toggle_id, &interaction, all(px(20.)), s.on_surface_variant, window, cx);
            let el = div()
                .id(toggle_id)
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(40.))
                .rounded(px(20.))
                .child(layer)
                .child(
                    Icon::new(if input { "gauge" } else { "pen" })
                        .size(px(20.))
                        .color(s.on_surface_variant),
                );
            interactive(el, &interaction, true, true).on_click(move |_, window, cx| {
                let typing = state.update(cx, |st, cx| {
                    st.input = !st.input;
                    cx.notify();
                    st.input
                });
                if typing {
                    window.focus(&hour_field);
                }
            })
        });
        let mut actions = div()
            .flex()
            .items_center()
            .justify_between()
            .mt(px(if input { 24. } else { 16. }))
            .child(div().children(toggle));
        if self.modal {
            let cancel_button = Button::new(child_id(&id, "cancel"), self.names.cancel.clone()).text();
            let ok_button = Button::new(child_id(&id, "ok"), self.names.ok.clone()).text();
            actions = actions.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(match cancel.clone() {
                        Some(f) => cancel_button.on_click(move |e, w, cx| f(e, w, cx)),
                        None => cancel_button,
                    })
                    .child(match confirm {
                        Some(f) => ok_button.on_click(move |e, w, cx| f(e, w, cx)),
                        None => ok_button,
                    }),
            );
        }
        surface = surface.child(actions);

        if self.modal {
            let dialog = Dialog::new(key(&id, "dialog"))
                .width(px(328.))
                .padding(px(0.))
                .exit(self.shown)
                .child(surface);
            return match cancel {
                Some(f) => dialog.on_dismiss(move |e, w, cx| f(e, w, cx)),
                None => dialog,
            }
            .into_any_element();
        }

        let background = if s.surface.a < 1. {
            over(s.surface_container_high, ground(&s))
        } else {
            s.surface_container_high
        };
        div()
            .flex_none()
            .w(px(328.))
            .rounded(px(28.))
            .bg(background)
            .child(surface)
            .into_any_element()
    }
}

/// The text of a dial number, drawn at a place on the dial.
struct DialNumber {
    label: String,
    x: f32,
    y: f32,
    size: f32,
}

impl TimePicker {
    /// The clock dial: the numbers, the selector that follows the pointer and the hand to it.
    #[allow(clippy::too_many_arguments)]
    fn dial(
        &self,
        state: &Entity<State>,
        emit: &TimeEmit,
        part: Part,
        dragging: bool,
        value: Time,
        s: &Scheme,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let id = &self.id;
        let twenty_four = self.twenty_four;
        let minute_step = self.minute_step;
        let half = DIAL / 2.;

        // Where the selector rests: an angle turned the short way from where it was, and a radius.
        let (target_angle, inner) = match part {
            Part::Hour => hour_angle(field_value(value, Part::Hour, twenty_four), twenty_four),
            Part::Minute => (f32::from(value.minute) * 6., false),
        };
        let target_angle = {
            let last = state.read(cx).angle;
            let angle = target_angle + 360. * ((last - target_angle) / 360.).round();
            state.update(cx, |st, _| st.angle = angle);
            angle
        };
        let [angle, radius] = animate(
            key(id, "hand"),
            if dragging { Spring::EFFECTS_FAST } else { Spring::EFFECTS },
            [target_angle, if inner { INNER_RING } else { OUTER_RING }],
            window,
            cx,
        );
        let at = |angle: f32, radius: f32| {
            let radians = angle.to_radians();
            (half + radius * radians.sin(), half - radius * radians.cos())
        };
        let (selector_x, selector_y) = at(angle, radius);

        // The numbers.
        let mut numbers = Vec::new();
        for slot in 0..12u8 {
            let slot_angle = f32::from(slot) * 30.;
            let (x, y) = at(slot_angle, OUTER_RING);
            let label = match part {
                Part::Minute => format!("{:02}", slot * 5),
                Part::Hour if slot == 0 => "12".to_string(),
                Part::Hour => slot.to_string(),
            };
            numbers.push(DialNumber { label, x, y, size: 16. });
            if part == Part::Hour && twenty_four {
                let (x, y) = at(slot_angle, INNER_RING);
                let label = if slot == 0 { "00".to_string() } else { (12 + slot).to_string() };
                numbers.push(DialNumber { label, x, y, size: 14. });
            }
        }

        let mut dial = div()
            .relative()
            .flex_none()
            .size(px(DIAL))
            .rounded(px(half))
            .bg(s.surface_container_highest);

        // The hand, from the center to the selector. The same layer takes the pointer.
        let (pointer_state, pointer_emit) = (state.clone(), emit.clone());
        let hand_color = s.primary;
        dial = dial.child(
            canvas(
                |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                move |bounds, hitbox, window, _| {
                    let center = bounds.center();
                    let (from_x, from_y) = (f32::from(center.x), f32::from(center.y));
                    let (to_x, to_y) = (from_x + selector_x - half, from_y + selector_y - half);
                    let length = (to_x - from_x).hypot(to_y - from_y).max(0.001);
                    // Half the 2 px width of the hand, across it.
                    let (across_x, across_y) = (-(to_y - from_y) / length, (to_x - from_x) / length);
                    let mut hand = PathBuilder::fill();
                    hand.add_polygon(
                        &[
                            point(px(from_x + across_x), px(from_y + across_y)),
                            point(px(to_x + across_x), px(to_y + across_y)),
                            point(px(to_x - across_x), px(to_y - across_y)),
                            point(px(from_x - across_x), px(from_y - across_y)),
                        ],
                        true,
                    );
                    if let Ok(path) = hand.build() {
                        window.paint_path(path, hand_color);
                    }

                    let pick = {
                        let (state, emit) = (pointer_state.clone(), pointer_emit);
                        Rc::new(move |position: gpui::Point<Pixels>, window: &mut Window, cx: &mut App| {
                            let dx = f32::from(position.x - center.x);
                            let dy = f32::from(position.y - center.y);
                            let (time, part) = {
                                let st = state.read(cx);
                                (st.time, st.part)
                            };
                            let next = match part {
                                Part::Hour => with_field(
                                    time,
                                    Part::Hour,
                                    hour_at(dx, dy, twenty_four),
                                    twenty_four,
                                ),
                                Part::Minute => Time { minute: minute_at(dx, dy, minute_step), ..time },
                            };
                            if next != time {
                                emit(next, window, cx);
                            }
                        })
                    };
                    let reach = f32::from(bounds.size.width) / 2.;
                    window.on_mouse_event({
                        let (state, pick) = (pointer_state.clone(), pick.clone());
                        move |e: &MouseDownEvent, phase, window, cx| {
                            let inside = f32::from(e.position.x - center.x)
                                .hypot(f32::from(e.position.y - center.y))
                                <= reach;
                            if phase.bubble()
                                && e.button == MouseButton::Left
                                && inside
                                && hitbox.is_hovered(window)
                            {
                                state.update(cx, |st, cx| {
                                    st.dragging = true;
                                    cx.notify();
                                });
                                pick(e.position, window, cx);
                            }
                        }
                    });
                    window.on_mouse_event({
                        let state = pointer_state.clone();
                        move |e: &MouseMoveEvent, phase, window, cx| {
                            if phase.bubble() && e.pressed_button.is_some() && state.read(cx).dragging {
                                pick(e.position, window, cx);
                            }
                        }
                    });
                    // After the hour comes the minute.
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                        if phase.bubble() {
                            pointer_state.update(cx, |st, cx| {
                                if st.dragging {
                                    st.dragging = false;
                                    if st.part == Part::Hour {
                                        st.part = Part::Minute;
                                    }
                                    cx.notify();
                                }
                            });
                        }
                    });
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );

        // The selector: a circle with the number it rests on drawn over it, in on_primary.
        let minute_dot = part == Part::Minute && !matches!(value.minute % 5, 0);
        dial = dial
            .child(
                div()
                    .absolute()
                    .left(px(half - 4.))
                    .top(px(half - 4.))
                    .size(px(8.))
                    .rounded(px(4.))
                    .bg(s.primary),
            )
            .child(
                div()
                    .absolute()
                    .left(px(selector_x - SELECTOR / 2.))
                    .top(px(selector_y - SELECTOR / 2.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(SELECTOR))
                    .rounded(px(SELECTOR / 2.))
                    .bg(s.primary)
                    .when(minute_dot, |d| d.child(div().size(px(8.)).rounded(px(4.)).bg(s.on_primary))),
            );
        for number in numbers {
            let under_selector =
                (number.x - selector_x).hypot(number.y - selector_y) < SELECTOR / 2. - 4.;
            dial = dial.child(
                div()
                    .absolute()
                    .left(px(number.x - 20.))
                    .top(px(number.y - 20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(40.))
                    .type_style(BODY_LARGE)
                    .text_size(px(number.size))
                    .text_color(if under_selector { s.on_primary } else { s.on_surface })
                    .child(number.label),
            );
        }
        dial.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pointer `radius` away from the center at `degrees` clockwise from the top.
    fn at(degrees: f32, radius: f32) -> (f32, f32) {
        let radians = degrees.to_radians();
        (radius * radians.sin(), -radius * radians.cos())
    }

    #[test]
    fn angles_run_clockwise_from_the_top() {
        let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
        assert!(close(pointer_angle(0., -10.), 0.));
        assert!(close(pointer_angle(10., 0.), 90.));
        assert!(close(pointer_angle(0., 10.), 180.));
        assert!(close(pointer_angle(-10., 0.), 270.));
        assert!(close(pointer_angle(10., -10.), 45.));
        assert!(close(pointer_angle(-10., -10.), 315.));
        // The center has no direction but must not blow up.
        assert!((0. ..360.).contains(&pointer_angle(0., 0.)));
    }

    #[test]
    fn a_12_hour_dial_gives_one_to_twelve() {
        for hour in 1..=12u8 {
            let (dx, dy) = at(f32::from(hour % 12) * 30., OUTER_RING);
            assert_eq!(hour_at(dx, dy, false), hour);
            // Near the center is still the same hour on a 12 h dial.
            let (dx, dy) = at(f32::from(hour % 12) * 30., 30.);
            assert_eq!(hour_at(dx, dy, false), hour);
        }
    }

    #[test]
    fn hours_snap_to_the_nearest_number() {
        // 14 degrees is nearer 0 (12) than 30 (1); 16 is nearer 1.
        let (dx, dy) = at(14., OUTER_RING);
        assert_eq!(hour_at(dx, dy, false), 12);
        let (dx, dy) = at(16., OUTER_RING);
        assert_eq!(hour_at(dx, dy, false), 1);
        // Just left of the top rounds up to 12 from the other side.
        let (dx, dy) = at(-14., OUTER_RING);
        assert_eq!(hour_at(dx, dy, false), 12);
        let (dx, dy) = at(-16., OUTER_RING);
        assert_eq!(hour_at(dx, dy, false), 11);
        let (dx, dy) = at(359., OUTER_RING);
        assert_eq!(hour_at(dx, dy, false), 12);
    }

    #[test]
    fn a_24_hour_dial_has_two_rings() {
        // Outer ring: 12 on top, then 1 to 11.
        let outer: Vec<u8> = (0..12u8)
            .map(|slot| {
                let (dx, dy) = at(f32::from(slot) * 30., OUTER_RING);
                hour_at(dx, dy, true)
            })
            .collect();
        assert_eq!(outer, [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
        // Inner ring: 0 on top, then 13 to 23.
        let inner: Vec<u8> = (0..12u8)
            .map(|slot| {
                let (dx, dy) = at(f32::from(slot) * 30., INNER_RING);
                hour_at(dx, dy, true)
            })
            .collect();
        assert_eq!(inner, [0, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23]);
    }

    #[test]
    fn the_rings_split_halfway() {
        let middle = (OUTER_RING + INNER_RING) / 2.;
        let (dx, dy) = at(90., middle - 1.);
        assert_eq!(hour_at(dx, dy, true), 15);
        let (dx, dy) = at(90., middle + 1.);
        assert_eq!(hour_at(dx, dy, true), 3);
        // Outside the dial it is still the outer ring.
        let (dx, dy) = at(90., 200.);
        assert_eq!(hour_at(dx, dy, true), 3);
    }

    #[test]
    fn minutes_follow_the_angle() {
        for (degrees, minute) in [(0., 0), (90., 15), (180., 30), (270., 45), (6., 1), (354., 59)] {
            let (dx, dy) = at(degrees, OUTER_RING);
            assert_eq!(minute_at(dx, dy, 1), minute, "{degrees} degrees");
        }
        // Just before the top comes back to 0, not 60.
        let (dx, dy) = at(359., OUTER_RING);
        assert_eq!(minute_at(dx, dy, 1), 0);
        let (dx, dy) = at(2.9, OUTER_RING);
        assert_eq!(minute_at(dx, dy, 1), 0);
        let (dx, dy) = at(3.1, OUTER_RING);
        assert_eq!(minute_at(dx, dy, 1), 1);
    }

    #[test]
    fn minutes_snap_to_the_step() {
        let (dx, dy) = at(100., OUTER_RING);
        assert_eq!(minute_at(dx, dy, 1), 17);
        assert_eq!(minute_at(dx, dy, 5), 15);
        assert_eq!(minute_at(dx, dy, 10), 20);
        let (dx, dy) = at(358., OUTER_RING);
        assert_eq!(minute_at(dx, dy, 5), 0);
        let (dx, dy) = at(176., OUTER_RING);
        assert_eq!(minute_at(dx, dy, 15), 30);
    }

    #[test]
    fn the_selector_rests_where_the_pointer_found_the_hour() {
        for twenty_four in [false, true] {
            for hour in 0..24u8 {
                let shown = if twenty_four { hour } else { Time::new(hour, 0).unwrap().to_12h().0 };
                let (angle, inner) = hour_angle(shown, twenty_four);
                let radius = if inner { INNER_RING } else { OUTER_RING };
                let (dx, dy) = at(angle, radius);
                assert_eq!(hour_at(dx, dy, twenty_four), shown, "hour {hour}, 24h {twenty_four}");
            }
        }
    }

    #[test]
    fn times_move_between_clocks() {
        assert_eq!(Time::new(0, 5).unwrap().to_12h(), (12, Meridiem::Am));
        assert_eq!(Time::new(12, 0).unwrap().to_12h(), (12, Meridiem::Pm));
        assert_eq!(Time::new(13, 30).unwrap().to_12h(), (1, Meridiem::Pm));
        assert_eq!(Time::new(23, 59).unwrap().to_12h(), (11, Meridiem::Pm));
        assert_eq!(Time::from_12h(12, Meridiem::Am, 5), Time { hour: 0, minute: 5 });
        assert_eq!(Time::from_12h(12, Meridiem::Pm, 0), Time { hour: 12, minute: 0 });
        assert_eq!(Time::from_12h(1, Meridiem::Pm, 30), Time { hour: 13, minute: 30 });
        for hour in 0..24 {
            let time = Time::new(hour, 7).unwrap();
            let (h, m) = time.to_12h();
            assert_eq!(Time::from_12h(h, m, 7), time);
        }
        assert!(Time::new(24, 0).is_none());
        assert!(Time::new(0, 60).is_none());
        assert_eq!(Time::new(9, 5).unwrap().to_string(), "09:05");
        assert!(Time::new(9, 5) < Time::new(9, 6));
    }

    #[test]
    fn arrows_wrap_inside_the_part() {
        let time = Time { hour: 23, minute: 59 };
        assert_eq!(nudge(time, Part::Hour, 1, 1), Time { hour: 0, minute: 59 });
        assert_eq!(nudge(time, Part::Minute, 1, 1), Time { hour: 23, minute: 0 });
        assert_eq!(nudge(Time { hour: 0, minute: 0 }, Part::Hour, -1, 1), Time { hour: 23, minute: 0 });
        assert_eq!(nudge(Time { hour: 9, minute: 58 }, Part::Minute, 1, 5), Time { hour: 9, minute: 3 });
        // The 12 h hour field goes round 1 to 12 and keeps its side of noon.
        let afternoon = Time { hour: 12, minute: 10 };
        assert_eq!(nudge_field(afternoon, Part::Hour, 1, false), Time { hour: 13, minute: 10 });
        assert_eq!(nudge_field(Time { hour: 23, minute: 10 }, Part::Hour, 1, false), Time { hour: 12, minute: 10 });
        assert_eq!(nudge_field(Time { hour: 0, minute: 10 }, Part::Hour, -1, false), Time { hour: 11, minute: 10 });
        assert_eq!(nudge_field(Time { hour: 23, minute: 10 }, Part::Hour, 1, true), Time { hour: 0, minute: 10 });
        assert_eq!(nudge_field(Time { hour: 5, minute: 0 }, Part::Minute, -1, true), Time { hour: 5, minute: 59 });
    }

    #[test]
    fn typing_digits_builds_a_number_inside_the_range() {
        // 24 h hour: 0, 9 gives 09; 1, 5 gives 15; 2, 5 starts again from 5.
        assert_eq!(type_digit(14, 0, 0, 23), 0);
        assert_eq!(type_digit(0, 9, 0, 23), 9);
        assert_eq!(type_digit(1, 1, 0, 23), 11);
        assert_eq!(type_digit(9, 1, 0, 23), 1);
        assert_eq!(type_digit(2, 5, 0, 23), 5);
        // 12 h hour never goes below 1 or above 12.
        assert_eq!(type_digit(12, 0, 1, 12), 1);
        assert_eq!(type_digit(1, 3, 1, 12), 3);
        assert_eq!(type_digit(1, 1, 1, 12), 11);
        assert_eq!(type_digit(1, 2, 1, 12), 12);
        // Minutes.
        assert_eq!(type_digit(4, 5, 0, 59), 45);
        assert_eq!(type_digit(5, 9, 0, 59), 59);
        assert_eq!(type_digit(6, 0, 0, 59), 0);
        assert_eq!(erase_digit(45, 0), 4);
        assert_eq!(erase_digit(7, 0), 0);
        assert_eq!(erase_digit(7, 1), 1);
    }

    #[test]
    fn fields_read_and_write_their_part() {
        let time = Time { hour: 15, minute: 42 };
        assert_eq!(field_value(time, Part::Hour, true), 15);
        assert_eq!(field_value(time, Part::Hour, false), 3);
        assert_eq!(field_value(time, Part::Minute, false), 42);
        assert_eq!(with_field(time, Part::Hour, 4, false), Time { hour: 16, minute: 42 });
        assert_eq!(with_field(time, Part::Hour, 12, false), Time { hour: 12, minute: 42 });
        assert_eq!(with_field(time, Part::Hour, 7, true), Time { hour: 7, minute: 42 });
        assert_eq!(with_field(time, Part::Minute, 5, true), Time { hour: 15, minute: 5 });
        assert_eq!(field_range(Part::Hour, false), (1, 12));
        assert_eq!(field_range(Part::Hour, true), (0, 23));
    }
}
