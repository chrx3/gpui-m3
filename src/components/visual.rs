//! Shapes, loading indicators and progress indicators.

use std::f32::consts::{PI, TAU};

use gpui::{
    AnyElement, App, Bounds, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, PathBuilder, Pixels, Point, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, div, point, px,
};

use super::photo::{self, PhotoSource};

use crate::interaction::rounded_rect;
use crate::motion::{
    Spring, animate, animate_color, animate_f32, continuous_at, cubic_bezier, key,
};
use crate::shapes::{LOADING_SEQUENCE, Morph, ShapeName, Tone, avatar_for};
use crate::theme::{FONT_FAMILY, Theme};

/// A shape from the M3 shape library. `morph` makes it breathe between two shapes and `spin` rotates it.
///
/// [`Shape::breathe_on_hover`] morphs it toward another shape while the pointer is over it and
/// back when it leaves, with a spring and on top of the ambient breath; [`Shape::image`] fills it
/// with a photo.
#[derive(IntoElement)]
pub struct Shape {
    shape: ShapeName,
    morph: Option<(ShapeName, f32)>,
    spin: Option<f32>,
    hover: Option<(ElementId, ShapeName)>,
    active: Option<bool>,
    photo: Option<PhotoSource>,
    size: Pixels,
    color: Option<Hsla>,
    child: Option<AnyElement>,
    turn: f32,
    grow: f32,
}

impl Shape {
    pub fn new(shape: ShapeName) -> Self {
        Self {
            shape,
            morph: None,
            spin: None,
            hover: None,
            active: None,
            photo: None,
            size: px(48.),
            color: None,
            child: None,
            turn: 0.,
            grow: 1.,
        }
    }

    /// Rotates the shape (not its child) by `radians`. Photos are not rotated.
    pub fn rotate(mut self, radians: f32) -> Self {
        self.turn = radians;
        self
    }

    /// Draws the shape `factor` times larger around its center without changing the space it
    /// takes up (the child keeps its size).
    pub fn scale(mut self, factor: f32) -> Self {
        self.grow = factor;
        self
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// Breathes toward `other` and back, in a cycle of `period` seconds (7 s feels calm).
    pub fn breathe(mut self, other: ShapeName, period: f32) -> Self {
        self.morph = Some((other, period));
        self
    }

    /// While the pointer is over it, morphs toward `other` (and back on leave). `id` must be
    /// unique within the view. Use [`Shape::hovered`] to follow the hover of a larger parent
    /// (a whole profile button) instead of the shape's own.
    pub fn breathe_on_hover(mut self, id: impl Into<ElementId>, other: ShapeName) -> Self {
        self.hover = Some((id.into(), other));
        self
    }

    /// Drives [`Shape::breathe_on_hover`] from your own hover state.
    pub fn hovered(mut self, hovered: bool) -> Self {
        self.active = Some(hovered);
        self
    }

    /// Fills the shape with a photo (covering it, centered), clipped to the shape with smooth
    /// edges. While the photo loads, the shape shows its color and child.
    pub fn image(mut self, source: impl Into<PhotoSource>) -> Self {
        self.photo = Some(source.into());
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

/// 0 → 1 → 0 over a phase from 0 to 1.
fn triangle(phase: f32) -> f32 {
    if phase < 0.5 {
        phase * 2.
    } else {
        2. - phase * 2.
    }
}

impl RenderOnce for Shape {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = self.color.unwrap_or(Theme::of(cx).primary_container);
        // Ambient shapes hold still while the window is in the background: no reason to
        // redraw it at 30 fps nobody is looking at (CPU and battery).
        let animated = (self.morph.is_some() || self.spin.is_some()) && window.is_window_active();
        // Ambient motion: it is slow, so 30 fps is enough.
        let t = if animated {
            continuous_at(30., window, cx)
        } else {
            0.
        };
        let ease = cubic_bezier(0.42, 0., 0.58, 1.);
        let mut morph = match self.morph {
            Some((other, period)) => Morph {
                from: self.shape,
                to: other,
                t: ease(triangle((t / period).fract())),
                rotation: 0.,
                toward: None,
            },
            None => Morph::still(self.shape),
        };

        // Hover breath: own hover (kept in element state) unless the caller drives it.
        let mut hover_state = None;
        if let Some((id, other)) = &self.hover {
            let hovered = match self.active {
                Some(active) => active,
                None => {
                    let state = window.use_keyed_state(key(id, "hovered"), cx, |_, _| false);
                    let on = *state.read(cx);
                    hover_state = Some(state);
                    on
                }
            };
            // Held while hovered and released on leave, both with a spring, and blended over
            // the ambient breath so neither cuts the other.
            let weight = animate_f32(
                key(id, "hover"),
                Spring::SPATIAL,
                if hovered { 1. } else { 0. },
                window,
                cx,
            );
            if self.photo.is_some() {
                // The photo mask morphs between two shapes only (cached steps).
                morph = Morph {
                    from: self.shape,
                    to: *other,
                    t: weight.clamp(0., 1.),
                    rotation: 0.,
                    toward: None,
                };
            } else {
                morph.toward = Some((*other, weight));
            }
        }
        let rotation = self.spin.map_or(0., |p| (t / p).fract() * TAU) + self.turn;
        let morph = Morph { rotation, ..morph };
        let grow = self.grow;

        // Photo: a raster masked to the shape (cached steps of the hover morph).
        let device = (f32::from(self.size) * window.scale_factor())
            .round()
            .max(1.) as u32;
        let image = self
            .photo
            .as_ref()
            .and_then(|source| source.load(window, cx));
        let photo = image.and_then(|image| {
            if let Some((_, other)) = &self.hover {
                photo::prewarm(&image, self.shape, *other, device, window, cx);
            }
            photo::shaped_photo(image, morph.from, morph.to, morph.t, device, window, cx)
        });
        let show_child = photo.is_none();

        let base = div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(self.size)
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let bounds = if grow != 1. {
                            let size =
                                gpui::size(bounds.size.width * grow, bounds.size.height * grow);
                            let half = point(size.width / 2., size.height / 2.);
                            Bounds::new(bounds.center() - half, size)
                        } else {
                            bounds
                        };
                        match photo {
                            Some(photo) => {
                                window
                                    .paint_image(bounds, gpui::Corners::default(), photo, 0, false)
                                    .ok();
                            }
                            None => morph.paint(bounds, color, window),
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .children(self.child.filter(|_| show_child));
        match (self.hover, hover_state) {
            (Some((id, _)), Some(state)) => base
                .id(key(&id, "shape"))
                .on_hover(move |on, _, cx| {
                    state.update(cx, |hovered, cx| {
                        *hovered = *on;
                        cx.notify();
                    })
                })
                .into_any_element(),
            _ => base.into_any_element(),
        }
    }
}

/// Avatar with the initial inside an M3 shape; shape and tone are stable for each name.
///
/// [`Avatar::image`] shows a photo clipped to the shape instead (the user's profile picture),
/// with the initials as a fallback while it loads.
#[derive(IntoElement)]
pub struct Avatar {
    name: SharedString,
    size: Pixels,
    photo: Option<PhotoSource>,
    shape: Option<ShapeName>,
    hover: Option<(ElementId, ShapeName)>,
    hovered: Option<bool>,
    spin: Option<ElementId>,
}

impl Avatar {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            size: px(22.),
            photo: None,
            shape: None,
            hover: None,
            hovered: None,
            spin: None,
        }
    }

    /// Turns 40° and grows 8 % while hovered, with the fast spatial spring (for project
    /// avatars); the initial stays upright. `id` must be unique within the view. Use
    /// [`Avatar::hovered`] to follow a whole row's hover instead of the avatar's own.
    pub fn hover_spin(mut self, id: impl Into<ElementId>) -> Self {
        self.spin = Some(id.into());
        self
    }

    /// A photo avatar: the image (a path or an `Arc<RenderImage>`) clipped to a Cookie9 shape.
    /// Add [`Avatar::name`] for the initials shown while it loads.
    pub fn image(source: impl Into<PhotoSource>) -> Self {
        Self {
            shape: Some(ShapeName::Cookie9),
            ..Self::new("").photo(source)
        }
    }

    /// Name for the initials and the stable tone (and shape, unless [`Avatar::shape`] is set).
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = name.into();
        self
    }

    /// Adds a photo to a name avatar.
    pub fn photo(mut self, source: impl Into<PhotoSource>) -> Self {
        self.photo = Some(source.into());
        self
    }

    /// Overrides the shape derived from the name.
    pub fn shape(mut self, shape: ShapeName) -> Self {
        self.shape = Some(shape);
        self
    }

    /// Morphs toward `other` and back when hovered (Clover8 suits a profile avatar).
    /// See [`Shape::breathe_on_hover`].
    pub fn breathe_on_hover(mut self, id: impl Into<ElementId>, other: ShapeName) -> Self {
        self.hover = Some((id.into(), other));
        self
    }

    /// Drives [`Avatar::breathe_on_hover`] from a parent's hover.
    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = Some(hovered);
        self
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Avatar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (shape, tone) = avatar_for(&self.name);
        let s = Theme::of(cx);
        let has_photo = self.photo.is_some();
        // Photo avatars sit on the tertiary container.
        let (bg, fg) = if has_photo {
            (s.tertiary_container, s.on_tertiary_container)
        } else {
            tone.colors(s)
        };
        // Name avatars show one initial; photo avatars fall back to the user's initials.
        let initial: String = if has_photo {
            self.name
                .split_whitespace()
                .take(2)
                .filter_map(|word| word.chars().next())
                .flat_map(|c| c.to_uppercase())
                .collect()
        } else {
            self.name
                .trim()
                .chars()
                .next()
                .map(|c| c.to_uppercase().collect())
                .unwrap_or_default()
        };
        let size = f32::from(self.size);
        let text_scale = if has_photo { 0.38 } else { 0.5 };
        let mut el = Shape::new(self.shape.unwrap_or(shape))
            .size(self.size)
            .color(bg)
            .child(
                div()
                    .text_color(fg)
                    .font_family(FONT_FAMILY)
                    .text_size(px((size * text_scale).max(11.)))
                    .font_weight(FontWeight(800.))
                    .child(initial),
            );
        if let Some(photo) = self.photo {
            el = el.image(photo);
        }
        if let Some((id, other)) = self.hover {
            el = el.breathe_on_hover(id, other);
        }
        if let Some(hovered) = self.hovered {
            el = el.hovered(hovered);
        }
        let Some(id) = self.spin else {
            return el.into_any_element();
        };
        let own = self
            .hovered
            .is_none()
            .then(|| window.use_keyed_state(key(&id, "spin-hover"), cx, |_, _| false));
        let hovered = match (&own, self.hovered) {
            (_, Some(h)) => h,
            (Some(state), None) => *state.read(cx),
            (None, None) => false,
        };
        let [turn, grow] = animate(
            key(&id, "spin"),
            Spring::SPATIAL_FAST,
            if hovered {
                [40f32.to_radians(), 1.08]
            } else {
                [0., 1.]
            },
            window,
            cx,
        );
        let el = el.rotate(turn).scale(grow);
        match own {
            Some(state) => div()
                .id(key(&id, "spin-target"))
                .flex_none()
                .on_hover(move |on, _, cx| {
                    state.update(cx, |h, cx| {
                        *h = *on;
                        cx.notify();
                    })
                })
                .child(el)
                .into_any_element(),
            None => el.into_any_element(),
        }
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
        // 30 fps: it looks just as smooth and each frame redraws the view that holds it.
        let t = continuous_at(30., window, cx);
        // As in Compose: a new morph every 650 ms driven by a bouncy spring, each one adding a
        // quarter turn, on top of a constant full turn every 4666 ms.
        const SEGMENT: f32 = 0.65;
        const SPIN_PERIOD: f32 = 4.666;
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
            rotation: ((index % 4) as f32 + progress) * (PI / 2.) + (t / SPIN_PERIOD).fract() * TAU,
            toward: None,
        };
        let color = if self.contained {
            s.on_primary_container
        } else {
            s.primary
        };
        // The shape takes 38 of the 48 px container.
        let scale = 38. / 48.;
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
        // 30 fps: it looks just as smooth and each frame redraws the view that holds it.
        let t = continuous_at(30., window, cx);
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

/// Progress ring (a context indicator).
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

/// Wavy circular progress indicator (M3 Expressive). `value` between 0 and 1; `None` =
/// indeterminate. 48 px with a 4 px stroke by default; the wave has a 1.6 px amplitude and a
/// 15 px wavelength, and the track stays 4 px clear of the indicator.
#[derive(IntoElement)]
pub struct WavyCircularProgress {
    id: ElementId,
    value: Option<f32>,
    size: Pixels,
    stroke: f32,
}

impl WavyCircularProgress {
    pub fn new(id: impl Into<ElementId>, value: Option<f32>) -> Self {
        Self {
            id: id.into(),
            value,
            size: px(48.),
            stroke: 4.,
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

impl RenderOnce for WavyCircularProgress {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        // 30 fps: it looks just as smooth and each frame redraws the view that holds it.
        let t = continuous_at(30., window, cx);
        let indeterminate = self.value.is_none();
        let value = animate_f32(
            key(&self.id, "value"),
            Spring::SPATIAL,
            self.value.unwrap_or(0.).clamp(0., 1.),
            window,
            cx,
        );
        let stroke = self.stroke;
        let (active, track) = (s.primary, s.secondary_container);
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                const AMPLITUDE: f32 = 1.6;
                const WAVELENGTH: f32 = 15.;
                const GAP: f32 = 4.;
                let size = f32::from(bounds.size.width).min(f32::from(bounds.size.height));
                let r = (size - stroke) / 2. - AMPLITUDE;
                if r <= 0. {
                    return;
                }
                let c = bounds.center();
                let at = |a: f32, radius: f32| {
                    point(c.x + px(radius * a.cos()), c.y + px(radius * a.sin()))
                };

                // Where the indicator starts and how far it sweeps, in radians.
                let (start, sweep) = if indeterminate {
                    // Each 1.333 s cycle the head runs ahead (the arc grows from 0.1 to 0.75
                    // of a turn), then the tail catches up; the whole thing also turns slowly.
                    const CYCLE: f32 = 1.333;
                    let cycle = t / CYCLE;
                    // 20 cycles advance a whole number of turns (13), so wrapping is seamless.
                    let base = (cycle.floor() as u64 % 20) as f32 * 0.65;
                    let p = cycle.fract();
                    let ease = cubic_bezier(0.4, 0., 0.2, 1.);
                    let (tail, head) = if p < 0.5 {
                        (base, base + 0.1 + 0.65 * ease(p * 2.))
                    } else {
                        (base + 0.65 * ease((p - 0.5) * 2.), base + 0.75)
                    };
                    let spin = (t / 2.).fract();
                    ((tail + spin) * TAU - PI / 2., (head - tail) * TAU)
                } else {
                    (-PI / 2., TAU * value.clamp(0., 1.))
                };

                // Track: the rest of the circle, clear of both ends of the indicator (the
                // round caps add half a stroke on each side).
                if sweep <= 0.001 {
                    let ring: Vec<Point<Pixels>> =
                        (0..64).map(|i| at(i as f32 / 64. * TAU, r)).collect();
                    let mut builder = PathBuilder::stroke(px(stroke));
                    builder.add_polygon(&ring, true);
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, track);
                    }
                    return;
                }
                let gap = (GAP + stroke) / r;
                let track_sweep = TAU - sweep - 2. * gap;
                if track_sweep > 0.01 {
                    let from = start + sweep + gap;
                    let steps = (track_sweep / TAU * 64.).ceil().max(2.) as usize;
                    let points: Vec<Point<Pixels>> = (0..=steps)
                        .map(|i| at(from + track_sweep * i as f32 / steps as f32, r))
                        .collect();
                    paint_polyline(&points, stroke, track, window);
                }

                // Indicator: a whole number of waves around the circle, travelling one
                // wavelength per second. When determinate, the wave flattens near empty and full.
                let amplitude = if indeterminate {
                    AMPLITUDE
                } else {
                    AMPLITUDE * (value / 0.1).min((1. - value) / 0.05).clamp(0., 1.)
                };
                let waves = (TAU * r / WAVELENGTH).round().max(1.);
                let phase = (t % 1.) * TAU;
                let steps = (sweep * r / 1.5).ceil().max(2.) as usize;
                let points: Vec<Point<Pixels>> = (0..=steps)
                    .map(|i| {
                        let a = start + sweep * i as f32 / steps as f32;
                        at(a, r + amplitude * (waves * a - phase).sin())
                    })
                    .collect();
                paint_polyline(&points, stroke, active, window);
            },
        )
        .flex_none()
        .size(self.size)
    }
}

/// Line with rounded ends.
pub(crate) fn paint_polyline(
    points: &[Point<Pixels>],
    width: f32,
    color: Hsla,
    window: &mut Window,
) {
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
