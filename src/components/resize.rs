//! Draggable dividers: [`ResizeHandle`] (the terminal's top edge, a panel's side) and
//! [`Splitter`], two panes with a handle between them.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Axis, Bounds, ElementId, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, RenderOnce, Styled,
    Window, canvas, div, px,
};

use super::{ChangeHandler, child_id};
use crate::motion::{Spring, animate_f32, key};
use crate::theme::{Theme, alpha};

/// Thickness of the area that reacts to the pointer.
const HIT: f32 = 8.;

#[derive(Default)]
struct Drag {
    /// Pointer position along the axis and size when the drag started.
    start: Option<(Pixels, Pixels)>,
    hovered: bool,
    last: Pixels,
}

/// A thin strip you drag to resize something, with a minimum, a maximum and callbacks.
///
/// [`ResizeHandle::height`] resizes vertically (a bottom panel such as a terminal: drag its top
/// edge up to grow it); [`ResizeHandle::width`] resizes horizontally (a side panel). The size is
/// yours: pass it in and store the value `on_resize` gives you.
///
/// The handle is an 8 px strip that fills its parent along the other axis. Place it yourself,
/// usually absolutely on the edge it resizes, or let [`Splitter`] place it. On hover and while
/// dragging a 2 px line in the primary color shows (the reference design's `.term-resize`); [`ResizeHandle::pill`]
/// shows the M3 pane drag handle instead.
#[derive(IntoElement)]
pub struct ResizeHandle {
    id: ElementId,
    axis: Axis,
    size: Pixels,
    min: Pixels,
    max: Pixels,
    invert: bool,
    pill: bool,
    on_resize: Option<ChangeHandler<Pixels>>,
    on_resize_end: Option<ChangeHandler<Pixels>>,
}

impl ResizeHandle {
    /// Resizes a height. Dragging down grows it, unless [`ResizeHandle::invert`] is set.
    pub fn height(id: impl Into<ElementId>, size: Pixels) -> Self {
        Self::new(id, Axis::Vertical, size)
    }

    /// Resizes a width. Dragging right grows it, unless [`ResizeHandle::invert`] is set.
    pub fn width(id: impl Into<ElementId>, size: Pixels) -> Self {
        Self::new(id, Axis::Horizontal, size)
    }

    fn new(id: impl Into<ElementId>, axis: Axis, size: Pixels) -> Self {
        Self {
            id: id.into(),
            axis,
            size,
            min: px(0.),
            max: Pixels::MAX,
            invert: false,
            pill: false,
            on_resize: None,
            on_resize_end: None,
        }
    }

    pub fn min(mut self, min: Pixels) -> Self {
        self.min = min;
        self
    }

    pub fn max(mut self, max: Pixels) -> Self {
        self.max = max;
        self
    }

    /// Dragging toward the start (up or left) grows the size: for a panel docked at the bottom
    /// or on the right.
    pub fn invert(mut self, invert: bool) -> Self {
        self.invert = invert;
        self
    }

    /// Shows the M3 drag handle (a 4 × 40 px pill that widens while dragged) instead of a line.
    pub fn pill(mut self, pill: bool) -> Self {
        self.pill = pill;
        self
    }

    /// Receives the new size, clamped between the minimum and the maximum, while dragging.
    pub fn on_resize(mut self, f: impl Fn(Pixels, &mut Window, &mut App) + 'static) -> Self {
        self.on_resize = Some(Rc::new(f));
        self
    }

    /// Receives the final size when the pointer is released (to persist it).
    pub fn on_resize_end(mut self, f: impl Fn(Pixels, &mut Window, &mut App) + 'static) -> Self {
        self.on_resize_end = Some(Rc::new(f));
        self
    }
}

/// The size for a pointer that moved `delta` along the axis since the drag started.
pub(crate) fn resized(
    start: Pixels,
    delta: Pixels,
    invert: bool,
    min: Pixels,
    max: Pixels,
) -> Pixels {
    let delta = if invert { -delta } else { delta };
    (start + delta).max(min).min(max.max(min)).round()
}

impl RenderOnce for ResizeHandle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let drag = window.use_keyed_state(key(&self.id, "drag"), cx, |_, _| Drag::default());
        let (dragging, hovered) = {
            let d = drag.read(cx);
            (d.start.is_some(), d.hovered)
        };
        let active = dragging || hovered;
        let show = animate_f32(
            key(&self.id, "show"),
            Spring::EFFECTS,
            if active { 1. } else { 0. },
            window,
            cx,
        );
        let vertical = self.axis == Axis::Vertical;
        let indicator = if self.pill {
            let long = animate_f32(
                key(&self.id, "long"),
                Spring::SPATIAL_FAST,
                if dragging { 52. } else { 40. },
                window,
                cx,
            );
            let thick = animate_f32(
                key(&self.id, "thick"),
                Spring::SPATIAL_FAST,
                if dragging { 6. } else { 4. },
                window,
                cx,
            );
            let color = if active { s.on_surface } else { s.outline };
            let pill = div().rounded(px(3.)).bg(color);
            let pill = if vertical {
                pill.w(px(long)).h(px(thick))
            } else {
                pill.w(px(thick)).h(px(long))
            };
            div()
                .absolute()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(pill)
        } else {
            let line = div()
                .absolute()
                .bg(alpha(s.primary, 0.8 * show.clamp(0., 1.)));
            if vertical {
                line.left_0().right_0().top(px(HIT / 2. - 1.)).h(px(2.))
            } else {
                line.top_0().bottom_0().left(px(HIT / 2. - 1.)).w(px(2.))
            }
        };

        let (size, min, max, invert, axis) =
            (self.size, self.min, self.max, self.invert, self.axis);
        let along = move |p: gpui::Point<Pixels>| if axis == Axis::Vertical { p.y } else { p.x };
        let on_resize = self.on_resize;
        let on_end = self.on_resize_end;
        let listener = canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                let d = drag.clone();
                window.on_mouse_event(move |e: &MouseDownEvent, phase, _, cx| {
                    if phase.bubble()
                        && e.button == MouseButton::Left
                        && bounds.contains(&e.position)
                    {
                        d.update(cx, |d, cx| {
                            d.start = Some((along(e.position), size));
                            d.last = size;
                            cx.notify();
                        });
                    }
                });
                let d = drag.clone();
                let on_resize = on_resize.clone();
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                    if !phase.bubble() {
                        return;
                    }
                    let inside = bounds.contains(&e.position);
                    let (start, was_hovered) = {
                        let d = d.read(cx);
                        (d.start, d.hovered)
                    };
                    if inside != was_hovered {
                        d.update(cx, |d, cx| {
                            d.hovered = inside;
                            cx.notify();
                        });
                    }
                    if let Some((origin, start_size)) = start {
                        if e.pressed_button != Some(MouseButton::Left) {
                            return;
                        }
                        let next =
                            resized(start_size, along(e.position) - origin, invert, min, max);
                        let changed = d.read(cx).last != next;
                        if changed {
                            d.update(cx, |d, _| d.last = next);
                            if let Some(f) = &on_resize {
                                f(next, window, cx);
                            }
                        }
                    }
                });
                let d = drag.clone();
                let on_end = on_end.clone();
                window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                    if !phase.bubble() {
                        return;
                    }
                    let Some(_) = d.read(cx).start else {
                        return;
                    };
                    let last = d.read(cx).last;
                    d.update(cx, |d, cx| {
                        d.start = None;
                        cx.notify();
                    });
                    if let Some(f) = &on_end {
                        f(last, window, cx);
                    }
                });
            },
        )
        .absolute()
        .size_full();

        let strip = div()
            .id(child_id(&self.id, "strip"))
            .relative()
            .flex_none()
            .child(indicator)
            .child(listener);
        let strip = if vertical {
            strip.w_full().h(px(HIT)).cursor_row_resize()
        } else {
            strip.h_full().w(px(HIT)).cursor_col_resize()
        };
        strip.when(dragging, |d| d.occlude())
    }
}

/// Two panes with a [`ResizeHandle`] between them. The first pane gets the size you keep; the
/// second takes the rest. Use [`Splitter::invert`] to size the second pane instead (a terminal
/// under an editor, an inspector on the right).
#[derive(IntoElement)]
pub struct Splitter {
    id: ElementId,
    axis: Axis,
    size: Pixels,
    min: Pixels,
    max: Pixels,
    invert: bool,
    pill: bool,
    first: Option<AnyElement>,
    second: Option<AnyElement>,
    on_resize: Option<ChangeHandler<Pixels>>,
    on_resize_end: Option<ChangeHandler<Pixels>>,
}

impl Splitter {
    /// Panes side by side; `size` is the width of the first one.
    pub fn horizontal(id: impl Into<ElementId>, size: Pixels) -> Self {
        Self::new(id, Axis::Horizontal, size)
    }

    /// Panes one above the other; `size` is the height of the first one.
    pub fn vertical(id: impl Into<ElementId>, size: Pixels) -> Self {
        Self::new(id, Axis::Vertical, size)
    }

    fn new(id: impl Into<ElementId>, axis: Axis, size: Pixels) -> Self {
        Self {
            id: id.into(),
            axis,
            size,
            min: px(48.),
            max: Pixels::MAX,
            invert: false,
            pill: false,
            first: None,
            second: None,
            on_resize: None,
            on_resize_end: None,
        }
    }

    pub fn first(mut self, pane: impl IntoElement) -> Self {
        self.first = Some(pane.into_any_element());
        self
    }

    pub fn second(mut self, pane: impl IntoElement) -> Self {
        self.second = Some(pane.into_any_element());
        self
    }

    /// Minimum size of the sized pane (48 px by default).
    pub fn min(mut self, min: Pixels) -> Self {
        self.min = min;
        self
    }

    pub fn max(mut self, max: Pixels) -> Self {
        self.max = max;
        self
    }

    /// `size` belongs to the second pane, and the first one takes the rest.
    pub fn invert(mut self, invert: bool) -> Self {
        self.invert = invert;
        self
    }

    /// Shows the M3 pill drag handle (see [`ResizeHandle::pill`]).
    pub fn pill(mut self, pill: bool) -> Self {
        self.pill = pill;
        self
    }

    pub fn on_resize(mut self, f: impl Fn(Pixels, &mut Window, &mut App) + 'static) -> Self {
        self.on_resize = Some(Rc::new(f));
        self
    }

    pub fn on_resize_end(mut self, f: impl Fn(Pixels, &mut Window, &mut App) + 'static) -> Self {
        self.on_resize_end = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Splitter {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let vertical = self.axis == Axis::Vertical;
        let mut handle = ResizeHandle::new(child_id(&self.id, "handle"), self.axis, self.size)
            .min(self.min)
            .max(self.max)
            .invert(self.invert)
            .pill(self.pill);
        if let Some(f) = self.on_resize {
            handle = handle.on_resize(move |v, w, cx| f(v, w, cx));
        }
        if let Some(f) = self.on_resize_end {
            handle = handle.on_resize_end(move |v, w, cx| f(v, w, cx));
        }
        let sized = |pane: Option<AnyElement>| {
            let d = div().flex_none().overflow_hidden().children(pane);
            if vertical {
                d.w_full().h(self.size)
            } else {
                d.h_full().w(self.size)
            }
        };
        let rest = |pane: Option<AnyElement>| {
            div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .children(pane)
        };
        let (first, second) = if self.invert {
            (rest(self.first), sized(self.second))
        } else {
            (sized(self.first), rest(self.second))
        };
        div()
            .flex()
            .size_full()
            .when(vertical, |d| d.flex_col())
            .child(first)
            .child(handle)
            .child(second)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resizing_clamps_and_inverts() {
        assert_eq!(
            resized(px(200.), px(30.), false, px(120.), px(400.)),
            px(230.)
        );
        // A bottom panel: dragging its top edge up (negative delta) grows it.
        assert_eq!(
            resized(px(200.), px(-30.), true, px(120.), px(400.)),
            px(230.)
        );
        assert_eq!(
            resized(px(200.), px(500.), false, px(120.), px(400.)),
            px(400.)
        );
        assert_eq!(
            resized(px(200.), px(-500.), false, px(120.), px(400.)),
            px(120.)
        );
        // A maximum below the minimum (a tiny window) keeps the minimum.
        assert_eq!(
            resized(px(200.), px(0.), false, px(120.), px(80.)),
            px(120.)
        );
    }
}
