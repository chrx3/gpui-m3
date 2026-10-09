use std::ops::RangeInclusive;
use std::rc::Rc;

use gpui::{
    App, Bounds, ElementId, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, RenderOnce, StatefulInteractiveElement,
    Styled, Window, canvas, div, px,
};

use super::{ChangeHandler, RoundedExt, corners};
use crate::interaction::{interactive, use_interaction};
use crate::motion::{Spring, animate, animate_color, animate_f32, key};
use crate::theme::{Theme, alpha};

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

/// Continuous M3 slider: a 16 px track split by a 4 × 28 px handle with 4 px gaps on each side.
/// Click or drag anywhere on it; values snap to [`Slider::step`].
#[derive(IntoElement)]
pub struct Slider {
    id: ElementId,
    value: f32,
    range: RangeInclusive<f32>,
    step: Option<f32>,
    width: Pixels,
    disabled: bool,
    on_change: Option<ChangeHandler<f32>>,
}

impl Slider {
    pub fn new(id: impl Into<ElementId>, value: f32, range: RangeInclusive<f32>) -> Self {
        Self {
            id: id.into(),
            value,
            range,
            step: None,
            width: px(240.),
            disabled: false,
            on_change: None,
        }
    }

    pub fn step(mut self, step: f32) -> Self {
        self.step = Some(step);
        self
    }

    /// Width (240 px by default).
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

const HANDLE_W: f32 = 4.;
const HANDLE_GAP: f32 = 4.;

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let (min, max) = (*self.range.start(), *self.range.end());
        let span = (max - min).max(f32::EPSILON);
        let fraction = ((self.value - min) / span).clamp(0., 1.);
        let fraction = animate_f32(
            key(&self.id, "value"),
            Spring::SPATIAL_FAST,
            fraction,
            window,
            cx,
        )
        .clamp(0., 1.);
        let dragging = window.use_keyed_state(key(&self.id, "drag"), cx, |_, _| false);
        let (active, inactive, handle) = if self.disabled {
            (
                alpha(s.on_surface, 0.38),
                alpha(s.on_surface, 0.12),
                alpha(s.on_surface, 0.38),
            )
        } else {
            (s.primary, s.secondary_container, s.primary)
        };
        let width = f32::from(self.width);
        let usable = width - HANDLE_W - 2. * HANDLE_GAP;
        let left = usable * fraction;
        let right = usable - left;
        let track = |w: f32, color, outer_left: bool| {
            let (l, r) = if outer_left { (8., 2.) } else { (2., 8.) };
            div()
                .flex_none()
                .w(px(w.max(0.)))
                .h(px(16.))
                .corners(corners([l, r, r, l]))
                .bg(color)
        };
        let mut el = div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .w(self.width)
            .h(px(28.))
            .child(track(left, active, true))
            .child(
                div()
                    .flex_none()
                    .mx(px(HANDLE_GAP))
                    .w(px(HANDLE_W))
                    .h(px(28.))
                    .rounded(px(2.))
                    .bg(handle),
            )
            .child(track(right, inactive, false));
        if let (Some(f), false) = (self.on_change, self.disabled) {
            let step = self.step;
            let to_value = move |x: Pixels, bounds: Bounds<Pixels>| {
                let t = f32::from(x - bounds.origin.x - px(HANDLE_GAP + HANDLE_W / 2.)) / usable;
                let mut v = min + t.clamp(0., 1.) * span;
                if let Some(step) = step.filter(|s| *s > 0.) {
                    v = min + ((v - min) / step).round() * step;
                }
                v.clamp(min, max)
            };
            el = el.cursor_pointer().child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let (down, moving, up) = (f.clone(), f.clone(), dragging.clone());
                        let drag = dragging.clone();
                        window.on_mouse_event(move |e: &MouseDownEvent, phase, w, cx| {
                            if phase.bubble()
                                && e.button == MouseButton::Left
                                && bounds.contains(&e.position)
                            {
                                drag.update(cx, |d, _| *d = true);
                                down(to_value(e.position.x, bounds), w, cx);
                            }
                        });
                        let drag = dragging.clone();
                        window.on_mouse_event(move |e: &MouseMoveEvent, phase, w, cx| {
                            if phase.bubble() && *drag.read(cx) && e.pressed_button.is_some() {
                                moving(to_value(e.position.x, bounds), w, cx);
                            }
                        });
                        window.on_mouse_event(move |_: &MouseUpEvent, _, _, cx| {
                            up.update(cx, |d, _| *d = false);
                        });
                    },
                )
                .absolute()
                .size_full(),
            );
        }
        el
    }
}
