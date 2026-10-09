use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use super::ChangeHandler;
use super::RoundedExt;
use crate::interaction::{interactive, use_interaction};
use crate::motion::{Spring, animate, animate_color, key};
use crate::theme::Theme;

/// M3 switch: the thumb grows when turned on and stretches while pressed.
#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    checked: bool,
    compact: bool,
    disabled: bool,
    on_toggle: Option<ChangeHandler<bool>>,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            compact: false,
            disabled: false,
            on_toggle: None,
        }
    }

    /// Menu variant (36 × 22 instead of 44 × 26).
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_toggle(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let enabled = !self.disabled;
        let state = use_interaction(&self.id, window, cx);
        let pressed = state.read(cx).pressed && enabled;

        // (width, height, thumb off, thumb on, thumb pressed, left offset off, left offset on)
        let (w, h, off, on, held, left_off, left_on) = if self.compact {
            (36., 22., 10., 16., 18., 6., 17.)
        } else {
            (44., 26., 12., 20., 22., 7., 21.)
        };
        let (size, left) = match (self.checked, pressed) {
            (false, false) => (off, left_off),
            (true, false) => (on, left_on),
            (false, true) => (held, 2.),
            (true, true) => (held, w - held - 2.),
        };
        let [size, left] = animate(
            key(&self.id, "thumb"),
            Spring::SPATIAL,
            [size, left],
            window,
            cx,
        );

        let (track, thumb, outline) = if self.checked {
            (s.primary, s.on_primary, s.primary)
        } else {
            (s.surface_container_highest, s.outline, s.outline)
        };
        let track = animate_color(key(&self.id, "track"), Spring::EFFECTS, track, window, cx);
        let thumb = animate_color(
            key(&self.id, "thumb-color"),
            Spring::EFFECTS,
            thumb,
            window,
            cx,
        );
        let outline = animate_color(
            key(&self.id, "outline"),
            Spring::EFFECTS,
            outline,
            window,
            cx,
        );

        let el = div()
            .id(self.id.clone())
            .relative()
            .flex_none()
            .w(px(w))
            .h(px(h))
            .rounded(px(h / 2.))
            .bg(track)
            .border_2()
            .border_color(outline)
            .when(!enabled, |d| d.opacity(0.38))
            .child(
                div()
                    .absolute()
                    // The 2 px border already offsets the content; compensate for it.
                    .left(px(left - 2.))
                    .top(px((h - size) / 2. - 2.))
                    .size(px(size))
                    .corners(super::all(px(size / 2.)))
                    .bg(thumb),
            );
        let mut el = interactive(el, &state, enabled, false);
        if let (Some(f), true) = (self.on_toggle, enabled) {
            let checked = self.checked;
            el = el.on_click(move |_, w, cx| f(!checked, w, cx));
        }
        el
    }
}
