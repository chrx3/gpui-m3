use std::f32::consts::TAU;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px,
};

use super::{ClickHandler, Icon, RoundedExt, all};
use crate::interaction::{interactive, state_layer, use_interaction};
use crate::motion::{Spring, animate_color, animate_f32, continuous_at, key};
use crate::shapes::Tone;
use crate::theme::{FONT_FAMILY, MONO_FONT_FAMILY, Theme, alpha};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ChipKind {
    /// Outlined (actions and suggestions).
    #[default]
    Assist,
    /// Filled with the primary container (the project chip in the reference design).
    Filled,
    /// Toggleable: when activated it goes from 8 px corners to a pill and fills in.
    Filter,
    /// Outlined, with a trailing "x" that calls [`Chip::on_remove`] (attachments, tags).
    Input,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ChipSize {
    /// 24 px (tool cards, inline actions).
    Small,
    /// 32 px.
    #[default]
    Medium,
}

/// 32 px chip with 8 px corners (per M3). It rounds with the spring on press.
#[derive(IntoElement)]
pub struct Chip {
    id: ElementId,
    label: SharedString,
    sublabel: Option<SharedString>,
    icon: Option<SharedString>,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
    kind: ChipKind,
    tone: Option<Tone>,
    size: ChipSize,
    selected: bool,
    show_check: bool,
    muted: bool,
    mono: bool,
    inert: bool,
    pulse_icon: bool,
    max_width: Option<Pixels>,
    on_click: Option<ClickHandler>,
    on_remove: Option<ClickHandler>,
}

impl Chip {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            sublabel: None,
            icon: None,
            leading: None,
            trailing: None,
            kind: ChipKind::Assist,
            tone: None,
            size: ChipSize::Medium,
            selected: false,
            show_check: true,
            muted: false,
            mono: false,
            inert: false,
            pulse_icon: false,
            max_width: None,
            on_click: None,
            on_remove: None,
        }
    }

    /// Input chip: outlined, with a trailing "x" (see [`Chip::on_remove`]).
    pub fn input(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self::new(id, label).kind(ChipKind::Input)
    }

    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Leading element instead of the icon (a color dot, an [`super::Avatar`]…).
    pub fn leading(mut self, el: impl IntoElement) -> Self {
        self.leading = Some(el.into_any_element());
        self
    }

    /// Element after the label (e.g. a small chevron for a picker).
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing = Some(el.into_any_element());
        self
    }

    /// Secondary text after the label, in the outline color (e.g. ":12-30" line ranges).
    pub fn sublabel(mut self, sublabel: impl Into<SharedString>) -> Self {
        self.sublabel = Some(sublabel.into());
        self
    }

    pub fn kind(mut self, kind: ChipKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn filled(self) -> Self {
        self.kind(ChipKind::Filled)
    }

    /// Toggleable chip with its state.
    pub fn filter(mut self, selected: bool) -> Self {
        self.kind = ChipKind::Filter;
        self.selected = selected;
        self
    }

    /// Fills the chip with a tonal container and drops the border (permission mode, files).
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    pub fn size(mut self, size: ChipSize) -> Self {
        self.size = size;
        self
    }

    /// Whether a selected filter chip replaces its icon with a check (default `true`).
    pub fn show_check(mut self, show: bool) -> Self {
        self.show_check = show;
        self
    }

    /// Text and icon in the variant color (secondary information such as the branch).
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// Label in the monospace font.
    pub fn mono(mut self, mono: bool) -> Self {
        self.mono = mono;
        self
    }

    /// Display-only chip: no hover, ripple or pointer cursor.
    pub fn inert(mut self, inert: bool) -> Self {
        self.inert = inert;
        self
    }

    /// The icon pulses (opacity 1 → 0.45 → 1 every 1.4 s), e.g. while agents are running.
    pub fn pulse_icon(mut self, pulse: bool) -> Self {
        self.pulse_icon = pulse;
        self
    }

    /// Maximum width; the label is truncated with an ellipsis.
    pub fn max_w(mut self, max_width: Pixels) -> Self {
        self.max_width = Some(max_width);
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }

    /// Called by the "x" of an input chip.
    pub fn on_remove(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_remove = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Chip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let reactive = !self.inert;
        let state = use_interaction(&self.id, window, cx);
        let pressed = state.read(cx).pressed && reactive;
        let on = self.kind == ChipKind::Filter && self.selected;
        // (height, text size, icon size, gap, side padding)
        let (height, text_size, icon_size, gap, pad) = match self.size {
            ChipSize::Small => (24., 12., 13., 5., 8.),
            ChipSize::Medium => (32., 12.5, 16., 7., 12.),
        };

        let radius = match (pressed, on) {
            (true, _) | (false, true) => height / 2.,
            (false, false) => 8.,
        };
        let radius = px(animate_f32(
            key(&self.id, "radius"),
            Spring::SPATIAL_FAST,
            radius,
            window,
            cx,
        ));
        // Press shrink (scale 0.96 in the reference design): approximated by reducing the height.
        let shrink = animate_f32(
            key(&self.id, "shrink"),
            Spring::SPATIAL_FAST,
            if pressed { 1. } else { 0. },
            window,
            cx,
        );

        let (bg, fg, border, icon_color, weight) = match (self.kind, self.tone) {
            (ChipKind::Filter, _) if on => (
                s.primary_container,
                s.on_primary_container,
                alpha(s.outline_variant, 0.),
                s.on_primary_container,
                600.,
            ),
            (_, Some(tone)) => {
                let (bg, fg) = tone.colors(&s);
                (bg, fg, alpha(bg, 0.), fg, 500.)
            }
            (ChipKind::Assist | ChipKind::Input, None) => (
                alpha(s.surface, 0.),
                s.on_surface,
                s.outline_variant,
                s.primary,
                500.,
            ),
            (ChipKind::Filled, None) => (
                s.primary_container,
                s.on_primary_container,
                alpha(s.primary_container, 0.),
                s.on_primary_container,
                600.,
            ),
            (ChipKind::Filter, None) => (
                alpha(s.surface, 0.),
                s.on_surface_variant,
                s.outline_variant,
                s.on_surface_variant,
                500.,
            ),
        };
        let (fg, icon_color) = if self.muted && !on {
            (s.on_surface_variant, s.on_surface_variant)
        } else {
            (fg, icon_color)
        };
        let bg = animate_color(key(&self.id, "bg"), Spring::EFFECTS, bg, window, cx);
        let fg = animate_color(key(&self.id, "fg"), Spring::EFFECTS, fg, window, cx);
        let border = animate_color(key(&self.id, "border"), Spring::EFFECTS, border, window, cx);
        let icon_color = animate_color(
            key(&self.id, "icon"),
            Spring::EFFECTS,
            icon_color,
            window,
            cx,
        );
        let icon_color = if self.pulse_icon {
            let t = continuous_at(30., window, cx);
            alpha(icon_color, 0.725 + 0.275 * (t / 1.4 * TAU).cos())
        } else {
            icon_color
        };
        let layer = state_layer(&self.id, &state, all(radius), fg, window, cx);

        let has_leading = self.icon.is_some() || self.leading.is_some() || on;
        let mut face = div()
            .relative()
            .flex()
            .items_center()
            .gap(px(gap))
            .h(px(height - shrink * 2.))
            .pl(px(if has_leading { pad - 4. } else { pad }))
            .pr(px(if self.kind == ChipKind::Input {
                4.
            } else {
                pad
            }))
            .corners(all(radius))
            .bg(bg)
            .border_1()
            .border_color(border)
            .text_color(fg)
            .font_family(FONT_FAMILY)
            .text_size(px(text_size))
            .font_weight(FontWeight(weight))
            .when_some(self.max_width, |d, w| d.max_w(w))
            .child(layer);
        if on && self.show_check {
            face = face.child(Icon::new("check").size(px(icon_size)).color(fg));
        } else if let Some(leading) = self.leading {
            face = face.child(leading);
        } else if let Some(icon) = self.icon {
            let color = if on { fg } else { icon_color };
            face = face.child(Icon::new(icon).size(px(icon_size)).color(color));
        }
        let truncate = self.max_width.is_some();
        let label = div()
            .flex()
            .items_baseline()
            .whitespace_nowrap()
            .when(truncate, |d| d.min_w_0().overflow_hidden())
            .when(!truncate, |d| d.flex_none())
            .when(self.mono, |d| {
                d.font_family(MONO_FONT_FAMILY)
                    .font_weight(FontWeight(400.))
            })
            .child(
                div()
                    .when(truncate, |d| d.min_w_0().overflow_hidden().text_ellipsis())
                    .child(self.label),
            )
            .children(
                self.sublabel
                    .map(|sub| div().flex_none().text_color(s.outline).child(sub)),
            );
        face = face.child(label);
        if let Some(trailing) = self.trailing {
            face = face.child(div().flex_none().child(trailing));
        }
        if self.kind == ChipKind::Input {
            let remove_id = super::child_id(&self.id, "remove");
            let remove_state = use_interaction(&remove_id, window, cx);
            let remove_layer = state_layer(&remove_id, &remove_state, all(px(9.)), fg, window, cx);
            let remove = div()
                .id(remove_id)
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(18.))
                .mr(px(2.))
                .rounded(px(9.))
                .child(remove_layer)
                .child(Icon::new("x").size(px(12.)).color(s.on_surface_variant))
                // Keeps the press from also rippling the whole chip.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
            let mut remove = interactive(remove, &remove_state, true, true);
            if let Some(f) = self.on_remove {
                remove = remove.on_click(move |e, w, cx| {
                    cx.stop_propagation();
                    f(e, w, cx)
                });
            }
            face = face.child(remove);
        }

        let el = div()
            .id(self.id.clone())
            .flex()
            .flex_none()
            .items_center()
            .h(px(height))
            .when_some(self.max_width, |d, w| d.max_w(w))
            .child(face);
        let mut el = interactive(el, &state, reactive, reactive);
        if let (Some(f), true) = (self.on_click, reactive) {
            el = el.on_click(move |e, w, cx| f(e, w, cx));
        }
        el
    }
}

/// Split button (the model picker in the reference design): a main action and a trailing button
/// that, when open, becomes a primary-colored circle and rotates its arrow.
#[derive(IntoElement)]
pub struct SplitButton {
    id: ElementId,
    label: SharedString,
    sublabel: Option<SharedString>,
    open: bool,
    on_click: Option<ClickHandler>,
    on_toggle: Option<ClickHandler>,
}

impl SplitButton {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            sublabel: None,
            open: false,
            on_click: None,
            on_toggle: None,
        }
    }

    /// Secondary text next to the label (e.g. the effort level).
    pub fn sublabel(mut self, sublabel: impl Into<SharedString>) -> Self {
        self.sublabel = Some(sublabel.into());
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }

    pub fn on_toggle(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for SplitButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let main_id = super::child_id(&self.id, "main");
        let trail_id = super::child_id(&self.id, "trail");
        let main_state = use_interaction(&main_id, window, cx);
        let trail_state = use_interaction(&trail_id, window, cx);
        let main_pressed = main_state.read(cx).pressed;
        let trail_pressed = trail_state.read(cx).pressed;

        let inner_main = if main_pressed { 8. } else { 4. };
        let main_r = crate::motion::animate(
            key(&main_id, "radius"),
            Spring::SPATIAL_FAST,
            [16., inner_main, inner_main, 16.],
            window,
            cx,
        );
        let inner_trail = if self.open {
            16.
        } else if trail_pressed {
            8.
        } else {
            4.
        };
        let trail_r = crate::motion::animate(
            key(&trail_id, "radius"),
            Spring::SPATIAL_FAST,
            [inner_trail, 16., 16., inner_trail],
            window,
            cx,
        );
        let rotation = animate_f32(
            key(&trail_id, "rot"),
            Spring::SPATIAL,
            if self.open { std::f32::consts::PI } else { 0. },
            window,
            cx,
        );
        let (trail_bg, trail_fg) = if self.open {
            (s.primary, s.on_primary)
        } else {
            (s.secondary_container, s.on_secondary_container)
        };
        let trail_bg = animate_color(key(&trail_id, "bg"), Spring::EFFECTS, trail_bg, window, cx);
        let trail_fg = animate_color(key(&trail_id, "fg"), Spring::EFFECTS, trail_fg, window, cx);

        let main_layer = state_layer(
            &main_id,
            &main_state,
            super::corners(main_r),
            s.on_secondary_container,
            window,
            cx,
        );
        let trail_layer = state_layer(
            &trail_id,
            &trail_state,
            super::corners(trail_r),
            trail_fg,
            window,
            cx,
        );

        let mut main = div()
            .id(main_id.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(32.))
            .pl(px(14.))
            .pr(px(10.))
            .corners(super::corners(main_r))
            .bg(s.secondary_container)
            .text_color(s.on_secondary_container)
            .font_weight(FontWeight(600.))
            .child(main_layer)
            .child(self.label);
        if let Some(sub) = self.sublabel {
            main = main.child(div().font_weight(FontWeight(500.)).opacity(0.75).child(sub));
        }
        let mut main = interactive(main, &main_state, true, true);
        if let Some(f) = self.on_click {
            main = main.on_click(move |e, w, cx| f(e, w, cx));
        }

        let trail = div()
            .id(trail_id.clone())
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .size(px(32.))
            .corners(super::corners(trail_r))
            .bg(trail_bg)
            .child(trail_layer)
            .child(
                Icon::new("chevron-down")
                    .size(px(16.))
                    .color(trail_fg)
                    .rotate(rotation),
            );
        let mut trail = interactive(trail, &trail_state, true, true);
        if let Some(f) = self.on_toggle {
            trail = trail.on_click(move |e, w, cx| f(e, w, cx));
        }

        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(2.))
            .font_family(FONT_FAMILY)
            .text_size(px(12.5))
            .child(main)
            .child(trail)
    }
}

/// Badge: a dot (no text), a pill with a short number, or a tonal text label ("all",
/// "3 changes", "in background").
#[derive(IntoElement)]
pub struct Badge {
    label: Option<SharedString>,
    tone: Option<Tone>,
    mono: bool,
    height: Pixels,
}

impl Badge {
    /// Numeric badge (tertiary container).
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: Some(label.into()),
            tone: None,
            mono: false,
            height: px(18.),
        }
    }

    pub fn dot() -> Self {
        Self {
            label: None,
            ..Self::new("")
        }
    }

    /// Text label on a tonal container (11 px, weight 500).
    pub fn tonal(tone: Tone, label: impl Into<SharedString>) -> Self {
        Self {
            tone: Some(tone),
            ..Self::new(label)
        }
    }

    /// Label in the monospace font.
    pub fn mono(mut self, mono: bool) -> Self {
        self.mono = mono;
        self
    }

    /// Height (18 px by default; 20 for a question header).
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let Some(label) = self.label else {
            return div().flex_none().size(px(6.)).rounded(px(3.)).bg(s.error);
        };
        let (bg, fg, weight, pad, min_w) = match self.tone {
            Some(tone) => {
                let (bg, fg) = tone.colors(&s);
                (bg, fg, 500., 6., 0.)
            }
            None => (s.tertiary_container, s.on_tertiary_container, 700., 5., 18.),
        };
        let height = f32::from(self.height);
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .min_w(px(min_w))
            .h(self.height)
            .px(px(pad))
            .rounded(px(height / 2.))
            .bg(bg)
            .text_color(fg)
            .font_family(if self.mono {
                MONO_FONT_FAMILY
            } else {
                FONT_FAMILY
            })
            .text_size(px(11.))
            .font_weight(FontWeight(if self.mono { 400. } else { weight }))
            .whitespace_nowrap()
            .child(label)
    }
}
