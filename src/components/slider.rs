use std::rc::Rc;

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use super::{ChangeHandler, RoundedExt, corners};
use crate::interaction::{interactive, use_interaction};
use crate::motion::{Spring, animate, animate_color, key};
use crate::theme::Theme;

/// Stepped slider (the reference design's effort picker): the current stop becomes a vertical bar
/// and the preceding stops are filled with the primary color. With no value (`None`) no stop
/// is marked, e.g. for a "default" state.
#[derive(IntoElement)]
pub struct StopSlider {
    id: ElementId,
    stops: usize,
    value: Option<usize>,
    compact: bool,
    on_change: Option<ChangeHandler<usize>>,
}

impl StopSlider {
    pub fn new(id: impl Into<ElementId>, stops: usize, value: impl Into<Option<usize>>) -> Self {
        Self {
            id: id.into(),
            stops,
            value: value.into(),
            compact: false,
            on_change: None,
        }
    }

    /// Menu variant (15 × 12 stops).
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
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
        let mut row = div().flex().flex_none().items_center().h(px(height));
        for i in 0..self.stops {
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
            let stop = div()
                .id(id.clone())
                .flex_none()
                .mx(px(margin + 1.5))
                .w(px(w.max(1.)))
                .h(px(h.max(1.)))
                .corners(corners([r0.max(0.), r1.max(0.), r2.max(0.), r3.max(0.)]))
                .bg(color);
            let mut stop = interactive(stop, &state, true, false);
            if let Some(f) = self.on_change.clone() {
                stop = stop.on_click(move |_, w, cx| f(i, w, cx));
            }
            row = row.child(stop);
        }
        row
    }
}
