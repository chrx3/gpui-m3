//! Selection marks: radio buttons and checkboxes.

use std::rc::Rc;

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use super::{ChangeHandler, Icon};
use crate::interaction::{interactive, use_interaction};
use crate::motion::{Spring, animate_color, animate_f32, key};
use crate::theme::{Theme, alpha};

/// Radio button (the reference design's model picker): a 2 px outline ring whose border
/// thickens to 5 px in the primary color when selected.
#[derive(IntoElement)]
pub struct Radio {
    id: ElementId,
    selected: bool,
    size: Pixels,
    disabled: bool,
    on_change: Option<ChangeHandler<bool>>,
}

impl Radio {
    pub fn new(id: impl Into<ElementId>, selected: bool) -> Self {
        Self {
            id: id.into(),
            selected,
            size: px(16.),
            disabled: false,
            on_change: None,
        }
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Makes it clickable on its own. Receives `true` (a radio never unselects itself).
    pub fn on_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Radio {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let border = animate_f32(
            key(&self.id, "border"),
            Spring::SPATIAL_FAST,
            if self.selected { 5. } else { 2. },
            window,
            cx,
        );
        let color = if self.disabled {
            alpha(s.on_surface, 0.38)
        } else if self.selected {
            s.primary
        } else {
            s.outline
        };
        let color = animate_color(key(&self.id, "color"), Spring::EFFECTS, color, window, cx);
        let size = f32::from(self.size);
        let ring = div()
            .id(self.id.clone())
            .flex_none()
            .size(self.size)
            .rounded(px(size / 2.))
            .border(px(border.clamp(1., size / 2.)))
            .border_color(color);
        let Some(f) = self.on_change.filter(|_| !self.disabled) else {
            return ring;
        };
        let state = use_interaction(&self.id, window, cx);
        interactive(ring, &state, true, false).on_click(move |_, w, cx| f(true, w, cx))
    }
}

/// M3 checkbox: an 18 px box that fills with the primary color and shows a check.
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    disabled: bool,
    on_change: Option<ChangeHandler<bool>>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            disabled: false,
            on_change: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Receives the new state.
    pub fn on_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let (bg, border, fg) = match (self.disabled, self.checked) {
            (true, true) => (
                alpha(s.on_surface, 0.38),
                alpha(s.on_surface, 0.),
                s.surface,
            ),
            (true, false) => (
                alpha(s.on_surface, 0.),
                alpha(s.on_surface, 0.38),
                s.surface,
            ),
            (false, true) => (s.primary, alpha(s.primary, 0.), s.on_primary),
            (false, false) => (alpha(s.primary, 0.), s.on_surface_variant, s.on_primary),
        };
        let bg = animate_color(key(&self.id, "bg"), Spring::EFFECTS, bg, window, cx);
        let border = animate_color(key(&self.id, "border"), Spring::EFFECTS, border, window, cx);
        let mark = animate_f32(
            key(&self.id, "mark"),
            Spring::SPATIAL_FAST,
            if self.checked { 1. } else { 0. },
            window,
            cx,
        );
        let state = use_interaction(&self.id, window, cx);
        let el = div()
            .id(self.id.clone())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(18.))
            .rounded(px(3.))
            .bg(bg)
            .border_2()
            .border_color(border)
            .child(
                Icon::new("check")
                    .size(px(14.))
                    .color(alpha(fg, mark.clamp(0., 1.)))
                    .scale(0.4 + 0.6 * mark.max(0.)),
            );
        let enabled = !self.disabled && self.on_change.is_some();
        let mut el = interactive(el, &state, enabled, false);
        if let (Some(f), true) = (self.on_change, enabled) {
            let checked = self.checked;
            el = el.on_click(move |_, w, cx| f(!checked, w, cx));
        }
        el
    }
}
