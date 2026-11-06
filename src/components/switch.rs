use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use super::ChangeHandler;
use super::Icon;
use super::RoundedExt;
use crate::interaction::{interactive, keyboard_focus, overlay, use_interaction};
use crate::motion::{Spring, animate, animate_color, animate_f32, key};
use crate::theme::{Theme, alpha};

/// M3 switch: the thumb grows when turned on and stretches while pressed. With
/// [`Switch::icons`] the thumb carries a check when on and an x when off.
#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    checked: bool,
    compact: bool,
    disabled: bool,
    /// Icons of the thumb when on and when off.
    icons: Option<(SharedString, SharedString)>,
    on_toggle: Option<ChangeHandler<bool>>,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            compact: false,
            disabled: false,
            icons: None,
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

    /// Icons in the thumb: a check when on and an x when off. As in M3, the off thumb is then
    /// as big as the on one so the icon fits; without icons the sizes are unchanged.
    pub fn icons(self, icons: bool) -> Self {
        if icons {
            self.thumb_icons("check", "x")
        } else {
            Self {
                icons: None,
                ..self
            }
        }
    }

    /// Custom icons (names of the icon library) for the thumb when on and when off.
    pub fn thumb_icons(
        mut self,
        on: impl Into<SharedString>,
        off: impl Into<SharedString>,
    ) -> Self {
        self.icons = Some((on.into(), off.into()));
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
        let (w, h, mut off, on, held, mut left_off, left_on) = if self.compact {
            (36., 22., 10., 16., 18., 6., 17.)
        } else {
            (44., 26., 12., 20., 22., 7., 21.)
        };
        if self.icons.is_some() {
            (off, left_off) = (on, (h - on) / 2.);
        }
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

        let mut thumb = div()
            .absolute()
            // The 2 px border already offsets the content; compensate for it.
            .left(px(left - 2.))
            .top(px((h - size) / 2. - 2.))
            .size(px(size))
            .corners(super::all(px(size / 2.)))
            .bg(thumb);
        if let Some((icon_on, icon_off)) = &self.icons {
            // Both icons ride the thumb: the active one fades and grows in with the thumb's spring.
            let on_p = animate_f32(
                key(&self.id, "icon"),
                Spring::SPATIAL,
                if self.checked { 1. } else { 0. },
                window,
                cx,
            );
            let icon_size = on * 0.7;
            let layer = |name: &SharedString, color, p: f32| {
                overlay().flex().items_center().justify_center().child(
                    Icon::new(name.clone())
                        .size(px(icon_size))
                        .color(alpha(color, p.clamp(0., 1.)))
                        .scale((0.4 + 0.6 * p.max(0.)) * size / on),
                )
            };
            thumb = thumb
                .child(layer(icon_off, s.surface_container_highest, 1. - on_p))
                .child(layer(icon_on, s.on_primary_container, on_p));
        }
        // The border-less wrapper is what the focus ring is measured from.
        let el = div()
            .id(self.id.clone())
            .relative()
            .flex_none()
            .w(px(w))
            .h(px(h))
            .when(!enabled, |d| d.opacity(0.38))
            .child(
                div()
                    .relative()
                    .size_full()
                    .rounded(px(h / 2.))
                    .bg(track)
                    .border_2()
                    .border_color(outline)
                    .child(thumb),
            );
        let mut el = interactive(el, &state, enabled, false);
        if let (Some(f), true) = (self.on_toggle, enabled) {
            let checked = self.checked;
            el = keyboard_focus(el, &self.id, super::all(px(h / 2.)), false, window, cx);
            el = el.on_click(move |_, w, cx| f(!checked, w, cx));
        }
        el
    }
}
