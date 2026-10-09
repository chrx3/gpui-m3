use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px,
};

use super::{ClickHandler, Icon, RoundedExt, all};
use crate::interaction::{interactive, state_layer, use_interaction};
use crate::motion::{Spring, animate_color, animate_f32, key};
use crate::theme::{FONT_FAMILY, Theme, alpha};

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

/// 32 px chip with 8 px corners (per M3). It rounds with the spring on press.
#[derive(IntoElement)]
pub struct Chip {
    id: ElementId,
    label: SharedString,
    icon: Option<SharedString>,
    leading: Option<AnyElement>,
    kind: ChipKind,
    selected: bool,
    on_click: Option<ClickHandler>,
    on_remove: Option<ClickHandler>,
}

impl Chip {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            leading: None,
            kind: ChipKind::Assist,
            selected: false,
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
        let state = use_interaction(&self.id, window, cx);
        let pressed = state.read(cx).pressed;
        let on = self.kind == ChipKind::Filter && self.selected;

        let radius = match (pressed, on) {
            (true, _) | (false, true) => 16.,
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

        let (bg, fg, border, icon_color, weight) = match self.kind {
            ChipKind::Assist | ChipKind::Input => (
                alpha(s.surface, 0.),
                s.on_surface,
                s.outline_variant,
                s.primary,
                500.,
            ),
            ChipKind::Filled => (
                s.primary_container,
                s.on_primary_container,
                alpha(s.primary_container, 0.),
                s.on_primary_container,
                600.,
            ),
            ChipKind::Filter if on => (
                s.primary_container,
                s.on_primary_container,
                alpha(s.outline_variant, 0.),
                s.on_primary_container,
                600.,
            ),
            ChipKind::Filter => (
                alpha(s.surface, 0.),
                s.on_surface_variant,
                s.outline_variant,
                s.on_surface_variant,
                500.,
            ),
        };
        let bg = animate_color(key(&self.id, "bg"), Spring::EFFECTS, bg, window, cx);
        let fg = animate_color(key(&self.id, "fg"), Spring::EFFECTS, fg, window, cx);
        let border = animate_color(key(&self.id, "border"), Spring::EFFECTS, border, window, cx);
        let layer = state_layer(&self.id, &state, all(radius), fg, window, cx);

        let mut face = div()
            .relative()
            .flex()
            .items_center()
            .gap(px(7.))
            .h(px(32. - shrink * 2.))
            .pl(px(if self.icon.is_some() || self.leading.is_some() || on {
                8.
            } else {
                12.
            }))
            .pr(px(if self.kind == ChipKind::Input {
                4.
            } else {
                12.
            }))
            .corners(all(radius))
            .bg(bg)
            .border_1()
            .border_color(border)
            .text_color(fg)
            .font_family(FONT_FAMILY)
            .text_size(px(12.5))
            .font_weight(FontWeight(weight))
            .child(layer);
        if on {
            face = face.child(Icon::new("check").size(px(16.)).color(fg));
        } else if let Some(leading) = self.leading {
            face = face.child(leading);
        } else if let Some(icon) = self.icon {
            face = face.child(Icon::new(icon).size(px(16.)).color(icon_color));
        }
        face = face.child(self.label);
        if self.kind == ChipKind::Input {
            let remove_id = super::child_id(&self.id, "remove");
            let remove_state = use_interaction(&remove_id, window, cx);
            let remove_layer = state_layer(&remove_id, &remove_state, all(px(12.)), fg, window, cx);
            let remove = div()
                .id(remove_id)
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .size(px(24.))
                .rounded(px(12.))
                .child(remove_layer)
                .child(Icon::new("x").size(px(14.)).color(s.on_surface_variant))
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
            .h(px(32.))
            .child(face);
        let mut el = interactive(el, &state, true, true);
        if let Some(f) = self.on_click {
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

/// Badge: a dot (no text) or a pill with a short number.
#[derive(IntoElement)]
pub struct Badge {
    label: Option<SharedString>,
}

impl Badge {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: Some(label.into()),
        }
    }

    pub fn dot() -> Self {
        Self { label: None }
    }
}

impl RenderOnce for Badge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        match self.label {
            None => div().flex_none().size(px(6.)).rounded(px(3.)).bg(s.error),
            Some(label) => div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .min_w(px(18.))
                .h(px(18.))
                .px(px(5.))
                .rounded(px(9.))
                .bg(s.tertiary)
                .text_color(s.surface)
                .font_family(FONT_FAMILY)
                .text_size(px(11.))
                .font_weight(FontWeight(700.))
                .child(label),
        }
    }
}
