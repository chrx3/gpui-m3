use std::ops::RangeInclusive;
use std::rc::Rc;

use gpui::{
    App, Bounds, Div, ElementId, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, deferred, div, px,
};

use super::{ChangeHandler, Icon, RoundedExt, all, child_id, corners};
use crate::interaction::{focus_ring, focusable, interactive, use_focus, use_interaction};
use crate::motion::{Spring, animate, animate_color, animate_f32, key};
use crate::theme::type_scale::{LABEL_MEDIUM, TypeScaleExt};
use crate::theme::{Scheme, Theme, alpha};

/// Stepped slider (an effort picker): the current stop becomes a vertical bar
/// and the preceding stops are filled with the primary color. With no value (`None`) no stop
/// is marked, e.g. for a "default" state. Arrow keys move the focused slider (Shift: ten stops),
/// Home and End jump to the ends.
#[derive(IntoElement)]
pub struct StopSlider {
    id: ElementId,
    stops: usize,
    value: Option<usize>,
    compact: bool,
    vertical: bool,
    on_change: Option<ChangeHandler<usize>>,
}

impl StopSlider {
    pub fn new(id: impl Into<ElementId>, stops: usize, value: impl Into<Option<usize>>) -> Self {
        Self {
            id: id.into(),
            stops,
            value: value.into(),
            compact: false,
            vertical: false,
            on_change: None,
        }
    }

    /// Menu variant (15 × 12 stops).
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    /// Vertical orientation: the first stop is at the bottom.
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    pub fn on_change(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for StopSlider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let (stop_w, stop_h, height) = if self.compact {
            (15., 12., 24.)
        } else {
            (18., 14., 28.)
        };
        let vertical = self.vertical;
        let mut row = div().relative().flex().flex_none().items_center();
        row = if vertical {
            row.flex_col().w(px(height))
        } else {
            row.h(px(height))
        };
        let order: Vec<usize> = if vertical {
            (0..self.stops).rev().collect()
        } else {
            (0..self.stops).collect()
        };
        for i in order {
            let id = ElementId::NamedInteger(format!("{}-stop", self.id).into(), i as u64);
            let state = use_interaction(&id, window, cx);
            let hovered = state.read(cx).hovered;
            let current = self.value == Some(i);
            let (first, last) = (i == 0, i + 1 == self.stops);
            // (width, height, side margin, radii)
            let (w, h, margin, r) = if current {
                (5., height, 3., [3.; 4])
            } else {
                let (a, b) = (stop_h / 2., 4.);
                let r = [
                    if first { a } else { b },
                    if last { a } else { b },
                    if last { a } else { b },
                    if first { a } else { b },
                ];
                (stop_w, if hovered { stop_h + 4. } else { stop_h }, 0., r)
            };
            let [w, h, margin, r0, r1, r2, r3] = animate(
                key(&id, "geom"),
                Spring::SPATIAL_FAST,
                [w, h, margin, r[0], r[1], r[2], r[3]],
                window,
                cx,
            );
            let color = if self.value.is_some_and(|value| i <= value) {
                s.primary
            } else {
                s.secondary_container
            };
            let color = animate_color(key(&id, "bg"), Spring::EFFECTS_SLOW, color, window, cx);
            let stop = div().id(id.clone()).flex_none().bg(color);
            // `w` and `h` are along and across the slider; the first stop is at the bottom when
            // vertical, so its rounded corners (r0 and r3) move there.
            let stop = if vertical {
                stop.my(px(margin + 1.5))
                    .h(px(w.max(1.)))
                    .w(px(h.max(1.)))
                    .corners(corners([r1, r2, r3, r0]))
            } else {
                stop.mx(px(margin + 1.5))
                    .w(px(w.max(1.)))
                    .h(px(h.max(1.)))
                    .corners(corners([r0, r1, r2, r3]))
            };
            let mut stop = interactive(stop, &state, true, false);
            if let Some(f) = self.on_change.clone() {
                stop = stop.on_click(move |_, w, cx| f(i, w, cx));
            }
            row = row.child(stop);
        }
        let Some(f) = self.on_change else {
            return row.into_any_element();
        };
        let row_id = child_id(&self.id, "row");
        let (state, handle, ring_visible) = use_focus(&row_id, window, cx);
        let ring = focus_ring(&row_id, ring_visible, all(px(height / 2.)), window, cx);
        let (stops, value) = (self.stops, self.value);
        focusable(row.id(row_id), &state, &handle)
            .on_key_down(move |e: &KeyDownEvent, window, cx| {
                let Some(nav) = key_nav(e.keystroke.key.as_str()) else {
                    return;
                };
                cx.stop_propagation();
                let jump = if e.keystroke.modifiers.shift { 10 } else { 1 };
                f(stop_target(value, nav, jump, stops), window, cx);
            })
            .children(ring)
            .into_any_element()
    }
}

/// The stop an arrow key reaches; with no value, forward starts at the first stop and back at the last.
fn stop_target(value: Option<usize>, nav: KeyNav, jump: usize, stops: usize) -> usize {
    let last = stops.saturating_sub(1);
    match (nav, value) {
        (KeyNav::Start, _) => 0,
        (KeyNav::End, _) => last,
        (KeyNav::Forward, None) => 0,
        (KeyNav::Back, None) => last,
        (KeyNav::Forward, Some(v)) => (v + jump).min(last),
        (KeyNav::Back, Some(v)) => v.saturating_sub(jump),
    }
}


/// Size of a [`Slider`]: `Standard` is the compact look the slider always had; the others are
/// the Expressive sizes (track 16, 24, 40, 56 and 96 dp, handle 4 dp wide with a 6 dp gap).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SliderSize {
    #[default]
    Standard,
    Xs,
    S,
    M,
    L,
    Xl,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Metrics {
    track: f32,
    handle_h: f32,
    handle_w: f32,
    gap: f32,
    /// (icon size, inset from the track start); only the two large sizes carry an icon.
    icon: Option<(f32, f32)>,
}

impl SliderSize {
    fn metrics(self) -> Metrics {
        let (track, handle_h, gap, icon) = match self {
            SliderSize::Standard => (16., 28., 4., None),
            SliderSize::Xs => (16., 44., 6., None),
            SliderSize::S => (24., 44., 6., None),
            SliderSize::M => (40., 52., 6., None),
            SliderSize::L => (56., 68., 6., Some((24., 16.))),
            SliderSize::Xl => (96., 108., 6., Some((32., 24.))),
        };
        Metrics {
            track,
            handle_h,
            handle_w: HANDLE_W,
            gap,
            icon,
        }
    }
}

const HANDLE_W: f32 = 4.;
const PRESSED_HANDLE_W: f32 = 2.;
const INNER_RADIUS: f32 = 2.;
const TICK_SIZE: f32 = 4.;
const BUBBLE_H: f32 = 28.;
const BUBBLE_GAP: f32 = 4.;
const BUBBLE_RISE: f32 = 8.;

/// Moves a thumb to a value and commits the new values.
type ApplyThumb = Rc<dyn Fn(Thumb, f32, &mut Window, &mut App)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Thumb {
    Low,
    High,
}

impl Thumb {
    fn index(self) -> usize {
        match self {
            Thumb::Low => 0,
            Thumb::High => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyNav {
    Back,
    Forward,
    Start,
    End,
}

fn key_nav(key: &str) -> Option<KeyNav> {
    match key {
        "left" | "down" => Some(KeyNav::Back),
        "right" | "up" => Some(KeyNav::Forward),
        "home" => Some(KeyNav::Start),
        "end" => Some(KeyNav::End),
        _ => None,
    }
}

/* ─────────── Pure maths ─────────── */

fn fraction_of(value: f32, min: f32, max: f32) -> f32 {
    ((value - min) / (max - min).max(f32::EPSILON)).clamp(0., 1.)
}

fn value_at(fraction: f32, min: f32, max: f32) -> f32 {
    min + fraction.clamp(0., 1.) * (max - min)
}

/// Rounds to the nearest multiple of `step` counted from `min`, and keeps the value in range.
fn snap(value: f32, min: f32, max: f32, step: Option<f32>) -> f32 {
    let value = match step.filter(|s| *s > 0.) {
        Some(step) => min + ((value - min) / step).round() * step,
        None => value,
    };
    value.max(min).min(max)
}

/// Distance from the minimum end of the track (left, or bottom when vertical) to the pointer.
fn along_from_pointer(pointer: f32, origin: f32, length: f32, vertical: bool) -> f32 {
    if vertical {
        origin + length - pointer
    } else {
        pointer - origin
    }
}

/// `lead` is the distance from the track start to the handle's center at the minimum.
fn fraction_at(along: f32, usable: f32, lead: f32) -> f32 {
    ((along - lead) / usable.max(1.)).clamp(0., 1.)
}

fn thumb_center(fraction: f32, usable: f32, lead: f32) -> f32 {
    lead + usable * fraction
}

fn nearest_thumb(value: f32, low: f32, high: f32) -> Thumb {
    if low >= high {
        return if value > high { Thumb::High } else { Thumb::Low };
    }
    if (value - low).abs() <= (high - value).abs() {
        Thumb::Low
    } else {
        Thumb::High
    }
}

/// Moves one thumb to `value`. In a range the thumbs keep `min_gap` between them.
fn set_thumb(
    values: [f32; 2],
    thumb: Thumb,
    value: f32,
    min: f32,
    max: f32,
    min_gap: f32,
    range: bool,
) -> [f32; 2] {
    let mut next = values;
    next[thumb.index()] = match (range, thumb) {
        (false, _) => value.max(min).min(max),
        (true, Thumb::Low) => value.min(values[1] - min_gap).max(min).min(max),
        (true, Thumb::High) => value.max(values[0] + min_gap).min(max).max(min),
    };
    next
}

fn key_target(
    current: f32,
    nav: KeyNav,
    amount: f32,
    ten_steps: bool,
    snap_to: Option<f32>,
    min: f32,
    max: f32,
) -> f32 {
    let amount = if ten_steps { amount * 10. } else { amount };
    match nav {
        KeyNav::Start => min,
        KeyNav::End => max,
        KeyNav::Back => snap(current - amount, min, max, snap_to),
        KeyNav::Forward => snap(current + amount, min, max, snap_to),
    }
}

/// How a piece of track ends: fully rounded at the ends of the slider, with a small radius next
/// to a handle, or square where the active and inactive tracks meet (the centered slider's origin).
#[derive(Clone, Copy, Debug, PartialEq)]
enum End {
    Outer,
    Gap,
    Flat,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Seg {
    from: f32,
    to: f32,
    start: End,
    end: End,
}

/// The track between `0` and `length` minus the handle slots, optionally cut at one point.
fn segments(length: f32, slots: &[(f32, f32)], cut: Option<f32>) -> Vec<Seg> {
    let mut slots = slots.to_vec();
    slots.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut free = Vec::new();
    let (mut pos, mut start) = (0., End::Outer);
    for (a, b) in slots {
        if a > pos + 0.01 {
            let end = if a >= length - 0.01 { End::Outer } else { End::Gap };
            free.push(Seg { from: pos, to: a.min(length), start, end });
        }
        if b > pos {
            pos = b;
            start = End::Gap;
        }
    }
    if pos < length - 0.01 {
        free.push(Seg { from: pos, to: length, start, end: End::Outer });
    }
    let mut out = Vec::with_capacity(free.len() + 1);
    for seg in free {
        match cut {
            Some(c) if c > seg.from + 0.01 && c < seg.to - 0.01 => {
                out.push(Seg { to: c, end: End::Flat, ..seg });
                out.push(Seg { from: c, start: End::Flat, ..seg });
            }
            _ => out.push(seg),
        }
    }
    out
}

fn end_radius(end: End, track: f32) -> f32 {
    match end {
        End::Outer => track / 2.,
        End::Gap => INNER_RADIUS,
        End::Flat => 0.,
    }
}

/// [top-left, top-right, bottom-right, bottom-left] of a piece with the given end radii.
fn seg_radii(start: f32, end: f32, vertical: bool) -> [f32; 4] {
    if vertical {
        [end, end, start, start]
    } else {
        [start, end, end, start]
    }
}

fn default_label(value: f32) -> String {
    let text = format!("{value:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".into() } else { text.into() }
}

/* ─────────── Shared slider ─────────── */

/// What `Slider` and `RangeSlider` have in common; `values[1]` is only used by the range.
#[derive(Clone)]
struct Core {
    id: ElementId,
    values: [f32; 2],
    range_mode: bool,
    centered: bool,
    range: RangeInclusive<f32>,
    step: Option<f32>,
    min_gap: f32,
    length: Pixels,
    vertical: bool,
    size: SliderSize,
    icon: Option<SharedString>,
    ticks: usize,
    indicator: bool,
    format: Option<Rc<dyn Fn(f32) -> String>>,
    disabled: bool,
    commit: Option<ChangeHandler<[f32; 2]>>,
}

impl Core {
    fn new(id: ElementId, values: [f32; 2], range_mode: bool, range: RangeInclusive<f32>) -> Self {
        Self {
            id,
            values,
            range_mode,
            centered: false,
            range,
            step: None,
            min_gap: 0.,
            length: px(240.),
            vertical: false,
            size: SliderSize::Standard,
            icon: None,
            ticks: 0,
            indicator: false,
            format: None,
            disabled: false,
            commit: None,
        }
    }

    /// Amount an arrow key moves: the step, else one stop, else 1 % of the range.
    fn key_amount(&self) -> f32 {
        let span = *self.range.end() - *self.range.start();
        match (self.step.filter(|s| *s > 0.), self.ticks) {
            (Some(step), _) => step,
            (None, 0) => span / 100.,
            (None, n) => span / n as f32,
        }
    }
}

macro_rules! slider_options {
    ($ty:ident) => {
        impl $ty {
            /// Values snap to multiples of `step` counted from the start of the range.
            pub fn step(mut self, step: f32) -> Self {
                self.core.step = Some(step);
                self
            }

            /// Width (240 px by default); the height when the slider is vertical.
            pub fn width(mut self, width: Pixels) -> Self {
                self.core.length = width;
                self
            }

            /// Height of a vertical slider (240 px by default); the width otherwise.
            pub fn height(mut self, height: Pixels) -> Self {
                self.core.length = height;
                self
            }

            /// Vertical orientation: the bottom is the minimum.
            pub fn vertical(mut self) -> Self {
                self.core.vertical = true;
                self
            }

            pub fn disabled(mut self, disabled: bool) -> Self {
                self.core.disabled = disabled;
                self
            }

            pub fn size(mut self, size: SliderSize) -> Self {
                self.core.size = size;
                self
            }

            /// Bubble with the value above the handle (right of it when vertical) while it is
            /// pressed, dragged or has keyboard focus.
            pub fn value_indicator(mut self, show: bool) -> Self {
                self.core.indicator = show;
                self
            }

            /// Text of the value indicator (the value with up to two decimals by default).
            pub fn format(mut self, f: impl Fn(f32) -> String + 'static) -> Self {
                self.core.format = Some(Rc::new(f));
                self
            }

            /// Icon at the start of the track; only drawn on the `L` and `Xl` sizes.
            pub fn icon(mut self, name: impl Into<SharedString>) -> Self {
                self.core.icon = Some(name.into());
                self
            }

            /// Draws `n + 1` stop indicators along the track. They do not snap by themselves:
            /// combine with [`step`](Self::step) to land on them.
            pub fn ticks(mut self, n: usize) -> Self {
                self.core.ticks = n;
                self
            }
        }
    };
}

/// Continuous M3 slider: a track split by a handle. Click or drag anywhere on it; values snap to
/// [`Slider::step`]. Arrow keys move the focused handle by one step (Shift: ten), Home and End
/// jump to the ends.
#[derive(IntoElement)]
pub struct Slider {
    core: Core,
}

impl Slider {
    pub fn new(id: impl Into<ElementId>, value: f32, range: RangeInclusive<f32>) -> Self {
        Self {
            core: Core::new(id.into(), [value, value], false, range),
        }
    }

    /// The active track runs from the middle of the range to the handle.
    pub fn centered(mut self) -> Self {
        self.core.centered = true;
        self
    }

    pub fn on_change(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.core.commit = Some(Rc::new(move |values: [f32; 2], w, cx| f(values[0], w, cx)));
        self
    }
}

slider_options!(Slider);

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        render_core(self.core, window, cx)
    }
}

/// Slider with two handles that select `(low, high)` inside a range. The handles cannot cross:
/// they keep at least [`RangeSlider::min_gap`] between them. A click on the track moves the
/// nearest handle.
#[derive(IntoElement)]
pub struct RangeSlider {
    core: Core,
}

impl RangeSlider {
    pub fn new(
        id: impl Into<ElementId>,
        low: f32,
        high: f32,
        range: RangeInclusive<f32>,
    ) -> Self {
        Self {
            core: Core::new(id.into(), [low, high], true, range),
        }
    }

    /// Smallest distance between the two values, in value units (0 by default).
    pub fn min_gap(mut self, gap: f32) -> Self {
        self.core.min_gap = gap.max(0.);
        self
    }

    pub fn on_change(mut self, f: impl Fn((f32, f32), &mut Window, &mut App) + 'static) -> Self {
        self.core.commit = Some(Rc::new(move |values: [f32; 2], w, cx| {
            f((values[0], values[1]), w, cx)
        }));
        self
    }
}

slider_options!(RangeSlider);

impl RenderOnce for RangeSlider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        render_core(self.core, window, cx)
    }
}

/// Rectangle `from..to` along the slider and `cross..cross + cross_len` across it.
fn along_rect(
    from: f32,
    to: f32,
    cross: f32,
    cross_len: f32,
    length: f32,
    vertical: bool,
) -> Div {
    let base = div().absolute();
    let long = (to - from).max(0.);
    if vertical {
        base.left(px(cross)).top(px(length - to.max(from))).w(px(cross_len)).h(px(long))
    } else {
        base.left(px(from)).top(px(cross)).w(px(long)).h(px(cross_len))
    }
}

fn value_bubble(
    text: String,
    center: f32,
    p: f32,
    handle_h: f32,
    length: f32,
    vertical: bool,
    s: &Scheme,
) -> impl IntoElement + use<> {
    let body = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(px(BUBBLE_H))
        .min_w(px(BUBBLE_H))
        .px(px(8.))
        .rounded(px(12.))
        .bg(s.inverse_surface)
        .text_color(s.inverse_on_surface)
        .type_emphasized(LABEL_MEDIUM)
        .whitespace_nowrap()
        .child(text);
    let rise = (1. - p) * BUBBLE_RISE;
    let holder = div().absolute().flex().opacity(p.clamp(0., 1.));
    let holder = if vertical {
        holder
            .items_center()
            .left(px(handle_h + BUBBLE_GAP - rise))
            .top(px(length - center - BUBBLE_H / 2.))
            .h(px(BUBBLE_H))
    } else {
        holder
            .justify_center()
            .left(px(center - 60.))
            .w(px(120.))
            .top(px(-BUBBLE_H - BUBBLE_GAP + rise))
            .h(px(BUBBLE_H))
    };
    deferred(holder.child(body)).with_priority(1)
}

fn render_core(core: Core, window: &mut Window, cx: &mut App) -> Div {
    let s = *Theme::of(cx);
    let m = core.size.metrics();
    let (min, max) = (*core.range.start(), *core.range.end());
    let two = core.range_mode;
    let vertical = core.vertical;
    let id = core.id.clone();
    let last = if two { 1 } else { 0 };

    let target = [
        fraction_of(core.values[0], min, max),
        fraction_of(core.values[last], min, max),
    ];
    let fractions = animate(key(&id, "value"), Spring::SPATIAL_FAST, target, window, cx)
        .map(|f| f.clamp(0., 1.));
    let drag = window.use_keyed_state(key(&id, "drag"), cx, |_, _| None::<Thumb>);
    let dragging = *drag.read(cx);
    let handle_w = animate(
        key(&id, "handle-w"),
        Spring::SPATIAL_FAST,
        [Thumb::Low, Thumb::High].map(|t| {
            if core.size != SliderSize::Standard && dragging == Some(t) {
                PRESSED_HANDLE_W
            } else {
                m.handle_w
            }
        }),
        window,
        cx,
    );

    let (active, inactive, handle) = if core.disabled {
        (
            alpha(s.on_surface, 0.38),
            alpha(s.on_surface, 0.12),
            alpha(s.on_surface, 0.38),
        )
    } else {
        (s.primary, s.secondary_container, s.primary)
    };
    let (tick_active, tick_inactive, icon_active, icon_inactive) = if core.disabled {
        (s.surface, alpha(s.on_surface, 0.38), s.surface, alpha(s.on_surface, 0.38))
    } else {
        (s.on_primary, s.on_secondary_container, s.on_primary, s.on_secondary_container)
    };

    let length = f32::from(core.length);
    let lead = m.gap + m.handle_w / 2.;
    let usable = (length - m.handle_w - 2. * m.gap).max(1.);
    let slot = m.handle_w / 2. + m.gap;
    let centers: Vec<f32> = fractions[..=last]
        .iter()
        .map(|f| thumb_center(*f, usable, lead))
        .collect();
    let slots: Vec<(f32, f32)> = centers.iter().map(|c| (c - slot, c + slot)).collect();
    let origin = thumb_center(0.5, usable, lead);
    let (active_from, active_to) = if two {
        (centers[0].min(centers[1]), centers[0].max(centers[1]))
    } else if core.centered {
        (origin.min(centers[0]), origin.max(centers[0]))
    } else {
        (0., centers[0])
    };
    let is_active = |at: f32| at >= active_from - 0.01 && at <= active_to + 0.01;

    let cross_len = m.handle_h;
    let track_cross = (cross_len - m.track) / 2.;
    let (width, height) = if vertical { (cross_len, length) } else { (length, cross_len) };
    let mut el = div().relative().flex_none().w(px(width)).h(px(height));

    let commit = core.commit.clone().filter(|_| !core.disabled);
    if let Some(commit) = commit.clone() {
        let values = core.values;
        let step = core.step;
        let min_gap = core.min_gap;
        let apply: ApplyThumb =
            Rc::new(move |thumb, v, w, cx| {
                commit(set_thumb(values, thumb, v, min, max, min_gap, two), w, cx)
            });
        let to_value = move |pointer: gpui::Point<Pixels>, bounds: Bounds<Pixels>| {
            let (pos, from, len) = if vertical {
                (pointer.y, bounds.origin.y, bounds.size.height)
            } else {
                (pointer.x, bounds.origin.x, bounds.size.width)
            };
            let along =
                along_from_pointer(f32::from(pos), f32::from(from), f32::from(len), vertical);
            snap(value_at(fraction_at(along, usable, lead), min, max), min, max, step)
        };
        let drag_state = drag.clone();
        el = el.cursor_pointer().child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let (down, down_state) = (apply.clone(), drag_state.clone());
                    window.on_mouse_event(move |e: &MouseDownEvent, phase, w, cx| {
                        if phase.bubble()
                            && e.button == MouseButton::Left
                            && bounds.contains(&e.position)
                        {
                            let v = to_value(e.position, bounds);
                            let thumb = if two {
                                nearest_thumb(v, values[0], values[1])
                            } else {
                                Thumb::Low
                            };
                            down_state.update(cx, |d, cx| {
                                *d = Some(thumb);
                                cx.notify();
                            });
                            down(thumb, v, w, cx);
                        }
                    });
                    let (moving, move_state) = (apply.clone(), drag_state.clone());
                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, w, cx| {
                        let thumb = *move_state.read(cx);
                        if let (true, Some(thumb), true) =
                            (phase.bubble(), thumb, e.pressed_button.is_some())
                        {
                            moving(thumb, to_value(e.position, bounds), w, cx);
                        }
                    });
                    let up_state = drag_state.clone();
                    window.on_mouse_event(move |_: &MouseUpEvent, _, _, cx| {
                        if up_state.read(cx).is_some() {
                            up_state.update(cx, |d, cx| {
                                *d = None;
                                cx.notify();
                            });
                        }
                    });
                },
            )
            .absolute()
            .size_full(),
        );
    }

    let cut = core.centered.then_some(origin).filter(|_| !two);
    for seg in segments(length, &slots, cut) {
        let color = if is_active((seg.from + seg.to) / 2.) { active } else { inactive };
        let radii = seg_radii(
            end_radius(seg.start, m.track),
            end_radius(seg.end, m.track),
            vertical,
        );
        el = el.child(
            along_rect(seg.from, seg.to, track_cross, m.track, length, vertical)
                .corners(corners(radii))
                .bg(color),
        );
    }

    if core.ticks > 0 {
        for i in 0..=core.ticks {
            let c = thumb_center(i as f32 / core.ticks as f32, usable, lead);
            let covered = slots
                .iter()
                .any(|(a, b)| c + TICK_SIZE / 2. > *a && c - TICK_SIZE / 2. < *b);
            if covered {
                continue;
            }
            let color = if is_active(c) { tick_active } else { tick_inactive };
            el = el.child(
                along_rect(
                    c - TICK_SIZE / 2.,
                    c + TICK_SIZE / 2.,
                    (cross_len - TICK_SIZE) / 2.,
                    TICK_SIZE,
                    length,
                    vertical,
                )
                .rounded(px(TICK_SIZE / 2.))
                .bg(color),
            );
        }
    }

    if let (Some(name), Some((size, inset))) = (core.icon.clone(), m.icon) {
        let (from, to) = (inset, inset + size);
        let hidden = slots.iter().any(|(a, b)| to > *a && from < *b);
        let shown = animate_f32(
            key(&id, "icon"),
            Spring::EFFECTS_FAST,
            if hidden { 0. } else { 1. },
            window,
            cx,
        );
        let color = animate_color(
            key(&id, "icon-color"),
            Spring::EFFECTS,
            if is_active((from + to) / 2.) { icon_active } else { icon_inactive },
            window,
            cx,
        );
        if shown > 0.01 {
            el = el.child(
                along_rect(from, to, (cross_len - size) / 2., size, length, vertical).child(
                    Icon::new(name)
                        .size(px(size))
                        .color(alpha(color, shown.clamp(0., 1.))),
                ),
            );
        }
    }

    for (i, thumb) in [Thumb::Low, Thumb::High].into_iter().take(centers.len()).enumerate() {
        let thumb_id = child_id(&id, if i == 0 { "thumb-low" } else { "thumb-high" });
        let c = centers[i];
        let w = handle_w[i].max(1.);
        let mut handle_el = along_rect(c - w / 2., c + w / 2., 0., cross_len, length, vertical)
            .id(thumb_id.clone())
            .rounded(px(w / 2.))
            .bg(handle);
        let mut keyboard_focused = false;
        if let Some(commit) = commit.clone() {
            let (state, focus, ring_visible) = use_focus(&thumb_id, window, cx);
            keyboard_focused = ring_visible;
            let ring = focus_ring(&thumb_id, ring_visible, all(px(w / 2.)), window, cx);
            let values = core.values;
            let (min_gap, amount, snap_to) = (core.min_gap, core.key_amount(), core.step);
            handle_el = focusable(handle_el, &state, &focus)
                .on_key_down(move |e: &KeyDownEvent, window, cx| {
                    let Some(nav) = key_nav(e.keystroke.key.as_str()) else {
                        return;
                    };
                    cx.stop_propagation();
                    let v = key_target(
                        values[thumb.index()],
                        nav,
                        amount,
                        e.keystroke.modifiers.shift,
                        snap_to,
                        min,
                        max,
                    );
                    commit(set_thumb(values, thumb, v, min, max, min_gap, two), window, cx);
                })
                .children(ring);
        }
        el = el.child(handle_el);

        let show = core.indicator && (keyboard_focused || dragging == Some(thumb));
        let p = animate_f32(
            key(&thumb_id, "bubble"),
            Spring::SPATIAL_FAST,
            if show { 1. } else { 0. },
            window,
            cx,
        );
        if core.indicator && (show || p > 0.01) {
            let value = core.values[thumb.index().min(last)];
            let text = match &core.format {
                Some(f) => f(value),
                None => default_label(value),
            };
            el = el.child(value_bubble(text, c, p, cross_len, length, vertical, &s));
        }
    }
    el
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn value_and_fraction_round_trip() {
        assert!(close(fraction_of(5., 0., 10.), 0.5));
        assert!(close(fraction_of(-1., 0., 10.), 0.));
        assert!(close(fraction_of(11., 0., 10.), 1.));
        assert!(close(value_at(0.25, -1., 1.), -0.5));
        for v in [-3., 0., 1.7, 4.] {
            assert!(close(value_at(fraction_of(v, -3., 4.), -3., 4.), v));
        }
    }

    #[test]
    fn pointer_maps_to_a_fraction_and_inverts_when_vertical() {
        let (usable, lead) = (200., 6.);
        // Horizontal: the cursor measured from the left edge.
        let along = along_from_pointer(106., 0., 212., false);
        assert!(close(fraction_at(along, usable, lead), 0.5));
        // Vertical: the bottom (largest y) is the minimum.
        let bottom = along_from_pointer(212., 0., 212., true);
        let top = along_from_pointer(0., 0., 212., true);
        assert!(close(fraction_at(bottom, usable, lead), 0.));
        assert!(close(fraction_at(top, usable, lead), 1.));
        assert!(close(fraction_at(along_from_pointer(106., 0., 212., true), usable, lead), 0.5));
        // Outside the track it clamps.
        assert!(close(fraction_at(-30., usable, lead), 0.));
        assert!(close(fraction_at(900., usable, lead), 1.));
    }

    #[test]
    fn thumb_center_is_the_inverse_of_fraction_at() {
        let (usable, lead) = (200., 6.);
        for f in [0., 0.3, 1.] {
            assert!(close(fraction_at(thumb_center(f, usable, lead), usable, lead), f));
        }
    }

    #[test]
    fn snapping_follows_the_step_from_the_minimum() {
        assert!(close(snap(0.34, 0., 1., Some(0.1)), 0.3));
        assert!(close(snap(0.36, 0., 1., Some(0.1)), 0.4));
        assert!(close(snap(1.7, 1., 5., Some(0.5)), 1.5));
        assert!(close(snap(9., 0., 5., Some(2.)), 5.));
        assert!(close(snap(0.123, 0., 1., None), 0.123));
        assert!(close(snap(7., 0., 5., None), 5.));
        assert!(close(snap(-7., 0., 5., Some(0.)), 0.));
    }

    #[test]
    fn nearest_thumb_picks_the_closer_one() {
        assert_eq!(nearest_thumb(10., 20., 80.), Thumb::Low);
        assert_eq!(nearest_thumb(45., 20., 80.), Thumb::Low);
        assert_eq!(nearest_thumb(55., 20., 80.), Thumb::High);
        assert_eq!(nearest_thumb(95., 20., 80.), Thumb::High);
        // Together: the side the click lands on decides, so they can separate again.
        assert_eq!(nearest_thumb(10., 50., 50.), Thumb::Low);
        assert_eq!(nearest_thumb(90., 50., 50.), Thumb::High);
    }

    #[test]
    fn thumbs_keep_the_gap_and_never_cross() {
        let v = [20., 60.];
        assert_eq!(set_thumb(v, Thumb::Low, 70., 0., 100., 5., true), [55., 60.]);
        assert_eq!(set_thumb(v, Thumb::Low, -10., 0., 100., 5., true), [0., 60.]);
        assert_eq!(set_thumb(v, Thumb::High, 10., 0., 100., 5., true), [20., 25.]);
        assert_eq!(set_thumb(v, Thumb::High, 140., 0., 100., 5., true), [20., 100.]);
        assert_eq!(set_thumb(v, Thumb::High, 40., 0., 100., 0., true), [20., 40.]);
        // A single slider only clamps to the range.
        assert_eq!(set_thumb([30., 30.], Thumb::Low, 140., 0., 100., 5., false)[0], 100.);
    }

    #[test]
    fn keys_move_by_a_step_ten_with_shift_and_jump_to_the_ends() {
        let go = |nav, shift| key_target(50., nav, 1., shift, Some(1.), 0., 100.);
        assert!(close(go(KeyNav::Forward, false), 51.));
        assert!(close(go(KeyNav::Back, false), 49.));
        assert!(close(go(KeyNav::Forward, true), 60.));
        assert!(close(go(KeyNav::Back, true), 40.));
        assert!(close(go(KeyNav::Start, false), 0.));
        assert!(close(go(KeyNav::End, false), 100.));
        assert!(close(key_target(99., KeyNav::Forward, 5., true, Some(1.), 0., 100.), 100.));
        assert_eq!(key_nav("left"), Some(KeyNav::Back));
        assert_eq!(key_nav("up"), Some(KeyNav::Forward));
        assert_eq!(key_nav("a"), None);
    }

    #[test]
    fn stop_slider_keys_stay_inside_the_stops() {
        assert_eq!(stop_target(Some(2), KeyNav::Forward, 1, 5), 3);
        assert_eq!(stop_target(Some(4), KeyNav::Forward, 1, 5), 4);
        assert_eq!(stop_target(Some(1), KeyNav::Back, 10, 5), 0);
        assert_eq!(stop_target(None, KeyNav::Forward, 1, 5), 0);
        assert_eq!(stop_target(None, KeyNav::Back, 1, 5), 4);
        assert_eq!(stop_target(Some(2), KeyNav::End, 1, 5), 4);
    }

    #[test]
    fn track_pieces_leave_room_for_the_handle() {
        let pieces = segments(100., &[(40., 52.)], None);
        assert_eq!(
            pieces,
            vec![
                Seg { from: 0., to: 40., start: End::Outer, end: End::Gap },
                Seg { from: 52., to: 100., start: End::Gap, end: End::Outer },
            ]
        );
        // A handle at the start leaves only the far piece.
        let pieces = segments(100., &[(0., 12.)], None);
        assert_eq!(pieces, vec![Seg { from: 12., to: 100., start: End::Gap, end: End::Outer }]);
        // Two handles make three pieces, whatever their order.
        assert_eq!(segments(100., &[(60., 72.), (20., 32.)], None).len(), 3);
    }

    #[test]
    fn centered_track_is_cut_at_the_origin() {
        let pieces = segments(100., &[(70., 82.)], Some(50.));
        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces[0].end, End::Flat);
        assert_eq!(pieces[1], Seg { from: 50., to: 70., start: End::Flat, end: End::Gap });
        // A handle sitting on the origin needs no cut.
        assert_eq!(segments(100., &[(44., 56.)], Some(50.)).len(), 2);
    }

    #[test]
    fn end_radii_and_vertical_corner_order() {
        assert!(close(end_radius(End::Outer, 40.), 20.));
        assert!(close(end_radius(End::Gap, 40.), 2.));
        assert!(close(end_radius(End::Flat, 40.), 0.));
        assert_eq!(seg_radii(8., 2., false), [8., 2., 2., 8.]);
        // Vertical: the start is the bottom edge.
        assert_eq!(seg_radii(8., 2., true), [2., 2., 8., 8.]);
    }

    #[test]
    fn metrics_keep_the_old_look_by_default() {
        let m = SliderSize::default().metrics();
        assert_eq!((m.track, m.handle_h, m.handle_w, m.gap), (16., 28., 4., 4.));
        let tracks = [SliderSize::Xs, SliderSize::S, SliderSize::M, SliderSize::L, SliderSize::Xl]
            .map(|s| s.metrics().track);
        assert_eq!(tracks, [16., 24., 40., 56., 96.]);
        assert!(SliderSize::M.metrics().icon.is_none() && SliderSize::L.metrics().icon.is_some());
    }

    #[test]
    fn labels_drop_trailing_zeros() {
        assert_eq!(default_label(3.), "3");
        assert_eq!(default_label(0.35), "0.35");
        assert_eq!(default_label(1.5), "1.5");
        assert_eq!(default_label(-0.001), "0");
    }
}
