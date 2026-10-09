//! Shapes, loading indicators and progress indicators.

use std::f32::consts::{PI, TAU};

use gpui::{
    AnyElement, App, Bounds, ElementId, FontWeight, Hsla, IntoElement, ParentElement, PathBuilder,
    Pixels, Point, RenderOnce, SharedString, Styled, Window, canvas, div, point, px,
};

use crate::interaction::rounded_rect;
use crate::motion::{
    Spring, animate, animate_color, animate_f32, continuous, continuous_at, cubic_bezier, key,
};
use crate::shapes::{LOADING_SEQUENCE, Morph, ShapeName, Tone, avatar_for};
use crate::theme::{FONT_FAMILY, Theme};

/// A shape from the M3 shape library. `morph` makes it breathe between two shapes and `spin` rotates it.
#[derive(IntoElement)]
pub struct Shape {
    shape: ShapeName,
    morph: Option<(ShapeName, f32)>,
    spin: Option<f32>,
    size: Pixels,
    color: Option<Hsla>,
    child: Option<AnyElement>,
}

impl Shape {
    pub fn new(shape: ShapeName) -> Self {
        Self {
            shape,
            morph: None,
            spin: None,
            size: px(48.),
            color: None,
            child: None,
        }
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// Breathes toward `other` and back, in a cycle of `period` seconds (the reference design uses 7 s).
    pub fn breathe(mut self, other: ShapeName, period: f32) -> Self {
        self.morph = Some((other, period));
        self
    }

    /// Rotates one full turn every `period` seconds (negative: counterclockwise).
    pub fn spin(mut self, period: f32) -> Self {
        self.spin = Some(period);
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }
}

impl RenderOnce for Shape {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = self.color.unwrap_or(Theme::of(cx).primary_container);
        let animated = self.morph.is_some() || self.spin.is_some();
        // Ambient motion: it is slow, so 30 fps is enough.
        let t = if animated {
            continuous_at(30., window, cx)
        } else {
            0.
        };
        let ease = cubic_bezier(0.42, 0., 0.58, 1.);
        let morph = match self.morph {
            Some((other, period)) => {
                let phase = (t / period).fract();
                let tri = if phase < 0.5 {
                    phase * 2.
                } else {
                    2. - phase * 2.
                };
                Morph {
                    from: self.shape,
                    to: other,
                    t: ease(tri),
                    rotation: 0.,
                }
            }
            None => Morph::still(self.shape),
        };
        let rotation = self.spin.map_or(0., |p| (t / p).fract() * TAU);
        let morph = Morph { rotation, ..morph };

        div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(self.size)
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| morph.paint(bounds, color, window),
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .children(self.child)
    }
}

/// Avatar with the initial inside an M3 shape; shape and tone are stable for each name.
#[derive(IntoElement)]
pub struct Avatar {
    name: SharedString,
    size: Pixels,
}

impl Avatar {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            size: px(22.),
        }
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Avatar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (shape, tone) = avatar_for(&self.name);
        let (bg, fg) = tone.colors(Theme::of(cx));
        let initial: String = self
            .name
            .trim()
            .chars()
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        let size = f32::from(self.size);
        Shape::new(shape).size(self.size).color(bg).child(
            div()
                .text_color(fg)
                .font_family(FONT_FAMILY)
                .text_size(px((size * 0.5).max(11.)))
                .font_weight(FontWeight(800.))
                .child(initial),
        )
    }
}

/// M3 Expressive loading indicator: a shape that spins and morphs into the next one.
#[derive(IntoElement)]
pub struct LoadingIndicator {
    size: Pixels,
    contained: bool,
}

impl LoadingIndicator {
    pub fn new() -> Self {
        Self {
            size: px(24.),
            contained: false,
        }
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// Inside a circle with the primary container.
    pub fn contained(mut self, contained: bool) -> Self {
        self.contained = contained;
        self
    }
}

impl Default for LoadingIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for LoadingIndicator {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let t = continuous(window, cx);
        // Each shape lasts 0.7 s and reaches the next one with a bouncy spring; the rotation is continuous.
        const SEGMENT: f32 = 0.7;
        let n = LOADING_SEQUENCE.len();
        let index = (t / SEGMENT) as usize;
        let local = t - index as f32 * SEGMENT;
        let progress = Spring {
            stiffness: 200.,
            damping_ratio: 0.6,
        }
        .step(local);
        let morph = Morph {
            from: LOADING_SEQUENCE[index % n],
            to: LOADING_SEQUENCE[(index + 1) % n],
            t: progress,
            rotation: (t / 1.4).fract() * TAU,
        };
        let (color, scale) = if self.contained {
            (s.on_primary_container, 0.62)
        } else {
            (s.primary, 0.78)
        };
        let size = f32::from(self.size);
        let inner = size * scale;

        let mut el = div().relative().flex_none().size(self.size);
        if self.contained {
            el = el.rounded(px(size / 2.)).bg(s.primary_container);
        }
        el.child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let offset = (size - inner) / 2.;
                    let shape_bounds = Bounds {
                        origin: bounds.origin + point(px(offset), px(offset)),
                        size: gpui::size(px(inner), px(inner)),
                    };
                    morph.paint(shape_bounds, color, window);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
    }
}

/// Wavy progress indicator (M3 Expressive). `value` between 0 and 1; `None` = indeterminate.
#[derive(IntoElement)]
pub struct WavyProgress {
    id: ElementId,
    value: Option<f32>,
}

impl WavyProgress {
    pub fn new(id: impl Into<ElementId>, value: Option<f32>) -> Self {
        Self {
            id: id.into(),
            value,
        }
    }
}

impl RenderOnce for WavyProgress {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let t = continuous(window, cx);
        let indeterminate = self.value.is_none();
        let value = animate_f32(
            key(&self.id, "value"),
            Spring::SPATIAL,
            self.value.unwrap_or(0.).clamp(0., 1.),
            window,
            cx,
        );
        let (active, track) = (s.primary, s.secondary_container);

        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let x0 = f32::from(bounds.origin.x);
                let w = f32::from(bounds.size.width);
                let cy = f32::from(bounds.origin.y) + f32::from(bounds.size.height) / 2.;
                const STROKE: f32 = 4.;
                const GAP: f32 = 4.;
                // Indeterminate: a segment of the wave travels along the track.
                let (start, end) = if indeterminate {
                    let p = (t / 1.8).fract();
                    let head = cubic_bezier(0.4, 0., 0.2, 1.)(p.min(1.)) * (w + w * 0.4);
                    let tail = cubic_bezier(0.4, 0., 0.2, 1.)((p - 0.2).max(0.) / 0.8)
                        * (w + w * 0.4)
                        - w * 0.15;
                    (tail.clamp(0., w), head.clamp(0., w))
                } else {
                    (0., (w - STROKE) * value.clamp(0., 1.))
                };

                // Track: from the end of the wave to the end point.
                let track_start = if end > 0.5 {
                    end + GAP + STROKE / 2.
                } else {
                    0.
                };
                let track_end = w - STROKE - GAP;
                if track_end > track_start {
                    paint_capsule(
                        point(px(x0 + track_start), px(cy)),
                        point(px(x0 + track_end), px(cy)),
                        STROKE,
                        track,
                        window,
                    );
                }
                if indeterminate && start > GAP {
                    paint_capsule(
                        point(px(x0), px(cy)),
                        point(px(x0 + start - GAP - STROKE / 2.), px(cy)),
                        STROKE,
                        track,
                        window,
                    );
                }
                // End point (always in the primary color).
                paint_dot(
                    point(px(x0 + w - STROKE / 2.), px(cy)),
                    STROKE / 2.,
                    active,
                    window,
                );

                // Active wave: 3 px amplitude, 20 px wavelength, advances one wavelength per second.
                if end - start > 0.5 {
                    let amplitude = 3.0f32.min((end - start) / 6.);
                    let phase = (t % 1.) * 20.;
                    let steps = ((end - start) / 1.5).ceil().max(2.) as usize;
                    let points: Vec<Point<Pixels>> = (0..=steps)
                        .map(|i| {
                            let x = start + (end - start) * i as f32 / steps as f32;
                            let y = cy - amplitude * ((x + phase) / 20. * TAU).sin();
                            point(px(x0 + x), px(y))
                        })
                        .collect();
                    paint_polyline(&points, STROKE, active, window);
                }
            },
        )
        .w_full()
        .h(px(12.))
    }
}

/// Progress ring (the reference design's context indicator).
#[derive(IntoElement)]
pub struct Ring {
    id: ElementId,
    value: f32,
    size: Pixels,
    stroke: f32,
}

impl Ring {
    pub fn new(id: impl Into<ElementId>, value: f32) -> Self {
        Self {
            id: id.into(),
            value,
            size: px(16.),
            stroke: 2.5,
        }
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    pub fn stroke(mut self, stroke: f32) -> Self {
        self.stroke = stroke;
        self
    }
}

impl RenderOnce for Ring {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let value = animate_f32(
            key(&self.id, "value"),
            Spring::SPATIAL,
            self.value.clamp(0., 1.),
            window,
            cx,
        );
        let stroke = self.stroke;
        let (track, active) = (s.secondary_container, s.primary);
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let size = f32::from(bounds.size.width);
                let r = (size - stroke) / 2.;
                let c = bounds.center();
                let arc = |from: f32, to: f32| -> Vec<Point<Pixels>> {
                    let steps = ((to - from).abs() / TAU * 64.).ceil().max(2.) as usize;
                    (0..=steps)
                        .map(|i| {
                            let a = from + (to - from) * i as f32 / steps as f32;
                            point(c.x + px(r * a.cos()), c.y + px(r * a.sin()))
                        })
                        .collect()
                };
                let mut track_points = arc(0., TAU);
                track_points.pop();
                let mut builder = PathBuilder::stroke(px(stroke));
                builder.add_polygon(&track_points, true);
                if let Ok(path) = builder.build() {
                    window.paint_path(path, track);
                }
                let v = value.clamp(0., 1.);
                if v > 0.001 {
                    paint_polyline(&arc(-PI / 2., -PI / 2. + TAU * v), stroke, active, window);
                }
            },
        )
        .flex_none()
        .size(self.size)
    }
}

/// Line with rounded ends.
fn paint_polyline(points: &[Point<Pixels>], width: f32, color: Hsla, window: &mut Window) {
    if points.len() < 2 {
        return;
    }
    let mut builder = PathBuilder::stroke(px(width));
    builder.move_to(points[0]);
    for p in &points[1..] {
        builder.line_to(*p);
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
    paint_dot(points[0], width / 2., color, window);
    paint_dot(points[points.len() - 1], width / 2., color, window);
}

fn paint_capsule(a: Point<Pixels>, b: Point<Pixels>, width: f32, color: Hsla, window: &mut Window) {
    paint_polyline(&[a, b], width, color, window);
}

fn paint_dot(center: Point<Pixels>, radius: f32, color: Hsla, window: &mut Window) {
    let points: Vec<Point<Pixels>> = (0..20)
        .map(|i| {
            let a = i as f32 / 20. * TAU;
            point(
                center.x + px(radius * a.cos()),
                center.y + px(radius * a.sin()),
            )
        })
        .collect();
    let mut builder = PathBuilder::fill();
    builder.add_polygon(&points, true);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// Tonal color container (background and content from one tone).
pub fn tone_colors(tone: Tone, cx: &App) -> (Hsla, Hsla) {
    tone.colors(Theme::of(cx))
}

/// Shape of a [`MorphDot`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DotShape {
    /// Square with 3 px corners.
    #[default]
    Square,
    Circle,
    /// The square turned 45°.
    Diamond,
}

/// Small status dot that morphs between a square, a circle and a diamond (the permission mode
/// dot in the reference design). Shape and rotation move with the fast spatial spring.
#[derive(IntoElement)]
pub struct MorphDot {
    id: ElementId,
    shape: DotShape,
    size: Pixels,
    color: Option<Hsla>,
}

impl MorphDot {
    pub fn new(id: impl Into<ElementId>, shape: DotShape) -> Self {
        Self {
            id: id.into(),
            shape,
            size: px(7.),
            color: None,
        }
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for MorphDot {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let size = f32::from(self.size);
        let (radius, rotation) = match self.shape {
            DotShape::Square => (3., 0.),
            DotShape::Circle => (size / 2., 0.),
            DotShape::Diamond => (3., PI / 4.),
        };
        let [radius, rotation] = animate(
            key(&self.id, "shape"),
            Spring::SPATIAL_FAST,
            [radius, rotation],
            window,
            cx,
        );
        let color = self.color.unwrap_or(Theme::of(cx).outline);
        let color = animate_color(key(&self.id, "color"), Spring::EFFECTS, color, window, cx);
        div().flex_none().size(self.size).child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let r = px(radius.clamp(0., size / 2.));
                    let corners = gpui::Corners {
                        top_left: r,
                        top_right: r,
                        bottom_right: r,
                        bottom_left: r,
                    };
                    let center = bounds.center();
                    let (sin, cos) = rotation.sin_cos();
                    let points: Vec<Point<Pixels>> = rounded_rect(bounds, corners)
                        .into_iter()
                        .map(|p| {
                            let (x, y) = (f32::from(p.x - center.x), f32::from(p.y - center.y));
                            point(
                                center.x + px(x * cos - y * sin),
                                center.y + px(x * sin + y * cos),
                            )
                        })
                        .collect();
                    let mut builder = PathBuilder::fill();
                    builder.add_polygon(&points, true);
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, color);
                    }
                },
            )
            .size_full(),
        )
    }
}
