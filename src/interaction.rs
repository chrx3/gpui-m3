//! M3 interaction states: hover layer and a ripple that starts where you press.
//!
//! GPUI only clips to rectangles, so the ripple is drawn as the intersection
//! of the circle with the component's rounded rectangle (both convex).

use std::{cell::Cell, f32::consts::FRAC_PI_2, rc::Rc, time::Instant};

use gpui::{
    App, Bounds, Corners, Div, ElementId, Entity, FocusHandle, Hsla, InteractiveElement,
    IntoElement, MouseButton, PathBuilder, Pixels, Point, Stateful, StatefulInteractiveElement,
    Styled, Window, canvas, div, fill, point, px,
};

use crate::motion::{self, Spring, animate_f32, cubic_bezier, key};
use crate::theme::alpha;

const RIPPLE_GROW: f32 = 0.5;
const RIPPLE_FADE: f32 = 0.4;

#[derive(Clone, Debug)]
struct Ripple {
    origin: Point<Pixels>,
    start: Instant,
    released: Option<Instant>,
}

/// Pointer state of a component, persisted across frames.
#[derive(Debug, Default)]
pub struct Interaction {
    pub hovered: bool,
    pub pressed: bool,
    ripples: Vec<Ripple>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl Interaction {
    fn release(&mut self) {
        self.pressed = false;
        let now = Instant::now();
        for r in &mut self.ripples {
            r.released.get_or_insert(now);
        }
    }
}

/// Gets (or creates) the interaction state of component `id`.
pub fn use_interaction(id: &ElementId, window: &mut Window, cx: &mut App) -> Entity<Interaction> {
    window.use_keyed_state(key(id, "interaction"), cx, |_, _| Interaction::default())
}

/// Wires hover, press and ripples to an element. With `ripple = false` it only tracks the state.
pub fn interactive(
    el: Stateful<Div>,
    state: &Entity<Interaction>,
    enabled: bool,
    ripple: bool,
) -> Stateful<Div> {
    if !enabled {
        return el;
    }
    let hover = state.clone();
    let down = state.clone();
    let up = state.clone();
    let up_out = state.clone();
    el.cursor_pointer()
        .on_hover(move |hovered, _, cx| {
            hover.update(cx, |s, cx| {
                s.hovered = *hovered;
                cx.notify();
            })
        })
        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
            down.update(cx, |s, cx| {
                s.pressed = true;
                if ripple {
                    let origin = s.bounds.get().origin;
                    s.ripples.push(Ripple {
                        origin: event.position - origin,
                        start: Instant::now(),
                        released: None,
                    });
                }
                cx.notify();
            })
        })
        .on_mouse_up(MouseButton::Left, move |_, _, cx| {
            up.update(cx, |s, cx| {
                s.release();
                cx.notify();
            })
        })
        .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
            up_out.update(cx, |s, cx| {
                if s.pressed {
                    s.release();
                    cx.notify();
                }
            })
        })
}

/// State layer: hover (8 % of the content color) and ripples (14 %).
/// Must be the component's first child so it sits beneath text and icons.
pub fn state_layer(
    id: &ElementId,
    state: &Entity<Interaction>,
    radii: Corners<Pixels>,
    content: Hsla,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let hovered = state.read(cx).hovered;
    let hover = animate_f32(
        key(id, "hover"),
        Spring::EFFECTS,
        if hovered { 1. } else { 0. },
        window,
        cx,
    );
    let scale = motion::MotionSettings::get(cx).time_scale.max(0.01);
    let now = Instant::now();

    // Live ripples: grow for 0.5 s and fade out over 0.4 s after release.
    let ripples = state.update(cx, |s, _| {
        s.ripples.retain(|r| {
            r.released
                .is_none_or(|t| now.duration_since(t).as_secs_f32() / scale < RIPPLE_FADE)
        });
        s.ripples.clone()
    });
    if !ripples.is_empty() {
        window.request_animation_frame();
    }
    let bounds_cell = state.read(cx).bounds.clone();

    canvas(
        move |bounds, _, _| bounds_cell.set(bounds),
        move |bounds, _, window, _| {
            if hover > 0.001 {
                window.paint_quad(fill(bounds, alpha(content, 0.08 * hover)).corner_radii(radii));
            }
            if ripples.is_empty() {
                return;
            }
            let outline = rounded_rect(bounds, radii);
            let w = f32::from(bounds.size.width);
            let h = f32::from(bounds.size.height);
            let full = (w * w + h * h).sqrt();
            let grow = cubic_bezier(0.2, 0., 0., 1.);
            for r in &ripples {
                let t = now.duration_since(r.start).as_secs_f32() / scale;
                let radius = full * grow((t / RIPPLE_GROW).min(1.));
                let fade = r.released.map_or(1., |rel| {
                    1. - (now.duration_since(rel).as_secs_f32() / scale / RIPPLE_FADE).min(1.)
                });
                if radius < 0.5 || fade <= 0. {
                    continue;
                }
                let center = bounds.origin + r.origin;
                let circle = circle_points(center, radius);
                let clipped = clip_convex(&circle, &outline);
                if clipped.len() < 3 {
                    continue;
                }
                let mut builder = PathBuilder::fill();
                builder.add_polygon(&clipped, true);
                if let Ok(path) = builder.build() {
                    window.paint_path(path, alpha(content, 0.14 * fade));
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// Keyboard focus of a component, persisted across frames.
pub struct FocusState {
    pub handle: FocusHandle,
    /// Focus came from a pointer press: the ring stays hidden (like CSS `:focus-visible`).
    by_pointer: bool,
}

/// Gets (or creates) the focus handle of component `id` and whether its focus ring should show:
/// focused and reached with the keyboard, not with a click.
pub fn use_focus(
    id: &ElementId,
    window: &mut Window,
    cx: &mut App,
) -> (Entity<FocusState>, FocusHandle, bool) {
    let state = window.use_keyed_state(key(id, "focus"), cx, |_, cx| FocusState {
        handle: cx.focus_handle().tab_stop(true),
        by_pointer: false,
    });
    let handle = state.read(cx).handle.clone();
    let focused = handle.is_focused(window);
    if !focused && state.read(cx).by_pointer {
        state.update(cx, |s, _| s.by_pointer = false);
    }
    let visible = focused && !state.read(cx).by_pointer;
    (state, handle, visible)
}

/// Makes an element focusable with Tab, hides the ring after clicks and shows it again on the
/// next key press. Pair it with [`use_focus`] and [`focus_ring`].
///
/// A click does not move the keyboard focus to the element, so expanding a card does not take it
/// away from a composer; Tab and Shift+Tab reach it.
pub fn focusable(
    el: Stateful<Div>,
    state: &Entity<FocusState>,
    handle: &FocusHandle,
) -> Stateful<Div> {
    let down = state.clone();
    let key_down = state.clone();
    el.track_focus(handle)
        // Runs before GPUI's own focus-on-click, which skips prevented events.
        .capture_any_mouse_down(|_, window, _| window.prevent_default())
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            down.update(cx, |s, cx| {
                s.by_pointer = true;
                cx.notify();
            })
        })
        .on_key_down(move |_, _, cx| {
            key_down.update(cx, |s, cx| {
                if s.by_pointer {
                    s.by_pointer = false;
                    cx.notify();
                }
            })
        })
}

/// M3 focus indicator: a 2 px ring in the primary color, 2 px outside the component's corners.
/// Fades in with the effects spring; it is drawn only while `visible` or fading out.
pub fn focus_ring(
    id: &ElementId,
    visible: bool,
    radii: Corners<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> Option<impl IntoElement + use<>> {
    let p = animate_f32(
        key(id, "focus-ring"),
        Spring::EFFECTS,
        if visible { 1. } else { 0. },
        window,
        cx,
    );
    if p <= 0.001 {
        return None;
    }
    let color = alpha(crate::theme::Theme::of(cx).primary, p.clamp(0., 1.));
    let grow = |r: Pixels| r + px(3.);
    Some(
        div()
            .absolute()
            .top(px(-3.))
            .left(px(-3.))
            .right(px(-3.))
            .bottom(px(-3.))
            .border_2()
            .border_color(color)
            .rounded_tl(grow(radii.top_left))
            .rounded_tr(grow(radii.top_right))
            .rounded_br(grow(radii.bottom_right))
            .rounded_bl(grow(radii.bottom_left)),
    )
}

/// Absolute container that fills its parent (for indicators and layers).
pub fn overlay() -> Div {
    div().absolute().top_0().left_0().size_full()
}

fn circle_points(center: Point<Pixels>, radius: f32) -> Vec<Point<Pixels>> {
    let n = 48;
    (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            point(
                center.x + px(radius * a.cos()),
                center.y + px(radius * a.sin()),
            )
        })
        .collect()
}

/// Rounded rectangle as a convex polygon (clockwise on screen).
pub fn rounded_rect(bounds: Bounds<Pixels>, radii: Corners<Pixels>) -> Vec<Point<Pixels>> {
    let x0 = f32::from(bounds.origin.x);
    let y0 = f32::from(bounds.origin.y);
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    let max = w.min(h) / 2.;
    let r = |v: Pixels| f32::from(v).clamp(0., max);
    // Center and start angle of each corner, going clockwise from the top left.
    let corners = [
        (
            x0 + r(radii.top_left),
            y0 + r(radii.top_left),
            r(radii.top_left),
            2. * FRAC_PI_2,
        ),
        (
            x0 + w - r(radii.top_right),
            y0 + r(radii.top_right),
            r(radii.top_right),
            3. * FRAC_PI_2,
        ),
        (
            x0 + w - r(radii.bottom_right),
            y0 + h - r(radii.bottom_right),
            r(radii.bottom_right),
            0.,
        ),
        (
            x0 + r(radii.bottom_left),
            y0 + h - r(radii.bottom_left),
            r(radii.bottom_left),
            FRAC_PI_2,
        ),
    ];
    let steps = 10;
    let mut points = Vec::with_capacity(4 * (steps + 1));
    for (cx, cy, radius, start) in corners {
        for i in 0..=steps {
            let a = start + FRAC_PI_2 * i as f32 / steps as f32;
            points.push(point(px(cx + radius * a.cos()), px(cy + radius * a.sin())));
        }
    }
    points
}

/// Sutherland–Hodgman: clips `subject` against the convex polygon `clip`.
fn clip_convex(subject: &[Point<Pixels>], clip: &[Point<Pixels>]) -> Vec<Point<Pixels>> {
    let f = |p: Point<Pixels>| (f32::from(p.x), f32::from(p.y));
    let mut output: Vec<(f32, f32)> = subject.iter().map(|&p| f(p)).collect();
    let clip: Vec<(f32, f32)> = clip.iter().map(|&p| f(p)).collect();
    // Clip orientation, to know which side is "inside".
    let area: f32 = (0..clip.len())
        .map(|i| {
            let (a, b) = (clip[i], clip[(i + 1) % clip.len()]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum();
    let sign = area.signum();
    for i in 0..clip.len() {
        if output.is_empty() {
            break;
        }
        let (a, b) = (clip[i], clip[(i + 1) % clip.len()]);
        if (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4 {
            continue;
        }
        let inside =
            |p: (f32, f32)| sign * ((b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)) >= 0.;
        let cross = |p: (f32, f32), q: (f32, f32)| {
            let (dx, dy) = (q.0 - p.0, q.1 - p.1);
            let (ex, ey) = (b.0 - a.0, b.1 - a.1);
            let denom = dx * ey - dy * ex;
            let t = ((a.0 - p.0) * ey - (a.1 - p.1) * ex) / denom;
            (p.0 + dx * t, p.1 + dy * t)
        };
        let input = std::mem::take(&mut output);
        for j in 0..input.len() {
            let cur = input[j];
            let prev = input[(j + input.len() - 1) % input.len()];
            match (inside(cur), inside(prev)) {
                (true, true) => output.push(cur),
                (true, false) => {
                    output.push(cross(prev, cur));
                    output.push(cur);
                }
                (false, true) => output.push(cross(prev, cur)),
                (false, false) => {}
            }
        }
    }
    output
        .into_iter()
        .map(|(x, y)| point(px(x), px(y)))
        .collect()
}
