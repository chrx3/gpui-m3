//! M3 Expressive shape library (cookie, clover, sunny, burst, pentagon…) and morphing.
//!
//! Every shape is sampled with the same number of points and angles, so morphing one into
//! another is point-by-point interpolation, just like the reference design's `clip-path`.

use std::f32::consts::{PI, TAU};
use std::sync::OnceLock;

use gpui::{Bounds, Hsla, PathBuilder, Pixels, Point, Window, point, px};

const POINTS: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShapeName {
    Circle,
    Oval,
    Cookie4,
    Cookie6,
    Cookie9,
    Cookie12,
    Clover4,
    Clover8,
    Sunny,
    SoftBurst,
    Pentagon,
    Triangle,
    Gem,
    Square,
    /// Organic, asymmetric blobs with no corners or lobes: two or three slow waves.
    Blob,
    Blob2,
    /// A soft, slightly triangular pebble.
    Pebble,
    // The rest of the M3 Expressive library (Compose's `MaterialShapes`), traced from
    // rounded polygons.
    Slanted,
    Arch,
    Fan,
    Arrow,
    SemiCircle,
    Pill,
    Diamond,
    ClamShell,
    VerySunny,
    Cookie7,
    Ghostish,
    Burst,
    Boom,
    SoftBoom,
    Flower,
    Puffy,
    PuffyDiamond,
    PixelCircle,
    PixelTriangle,
    Bun,
    Heart,
}

impl ShapeName {
    /// Every shape, in declaration order (the point cache is indexed by it).
    pub const ALL: [ShapeName; 38] = [
        ShapeName::Circle,
        ShapeName::Oval,
        ShapeName::Cookie4,
        ShapeName::Cookie6,
        ShapeName::Cookie9,
        ShapeName::Cookie12,
        ShapeName::Clover4,
        ShapeName::Clover8,
        ShapeName::Sunny,
        ShapeName::SoftBurst,
        ShapeName::Pentagon,
        ShapeName::Triangle,
        ShapeName::Gem,
        ShapeName::Square,
        ShapeName::Blob,
        ShapeName::Blob2,
        ShapeName::Pebble,
        ShapeName::Slanted,
        ShapeName::Arch,
        ShapeName::Fan,
        ShapeName::Arrow,
        ShapeName::SemiCircle,
        ShapeName::Pill,
        ShapeName::Diamond,
        ShapeName::ClamShell,
        ShapeName::VerySunny,
        ShapeName::Cookie7,
        ShapeName::Ghostish,
        ShapeName::Burst,
        ShapeName::Boom,
        ShapeName::SoftBoom,
        ShapeName::Flower,
        ShapeName::Puffy,
        ShapeName::PuffyDiamond,
        ShapeName::PixelCircle,
        ShapeName::PixelTriangle,
        ShapeName::Bun,
        ShapeName::Heart,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ShapeName::Circle => "circle",
            ShapeName::Oval => "oval",
            ShapeName::Cookie4 => "cookie4",
            ShapeName::Cookie6 => "cookie6",
            ShapeName::Cookie9 => "cookie9",
            ShapeName::Cookie12 => "cookie12",
            ShapeName::Clover4 => "clover4",
            ShapeName::Clover8 => "clover8",
            ShapeName::Sunny => "sunny",
            ShapeName::SoftBurst => "softBurst",
            ShapeName::Pentagon => "pentagon",
            ShapeName::Triangle => "triangle",
            ShapeName::Gem => "gem",
            ShapeName::Square => "square",
            ShapeName::Blob => "blob",
            ShapeName::Blob2 => "blob2",
            ShapeName::Pebble => "pebble",
            ShapeName::Slanted => "slanted",
            ShapeName::Arch => "arch",
            ShapeName::Fan => "fan",
            ShapeName::Arrow => "arrow",
            ShapeName::SemiCircle => "semiCircle",
            ShapeName::Pill => "pill",
            ShapeName::Diamond => "diamond",
            ShapeName::ClamShell => "clamShell",
            ShapeName::VerySunny => "verySunny",
            ShapeName::Cookie7 => "cookie7",
            ShapeName::Ghostish => "ghostish",
            ShapeName::Burst => "burst",
            ShapeName::Boom => "boom",
            ShapeName::SoftBoom => "softBoom",
            ShapeName::Flower => "flower",
            ShapeName::Puffy => "puffy",
            ShapeName::PuffyDiamond => "puffyDiamond",
            ShapeName::PixelCircle => "pixelCircle",
            ShapeName::PixelTriangle => "pixelTriangle",
            ShapeName::Bun => "bun",
            ShapeName::Heart => "heart",
        }
    }

    /// Relative radius of the shape at angle `t`, for shapes without an outline
    /// (see [`ShapeName::outline`]).
    fn radius(self, t: f32) -> f32 {
        match self {
            ShapeName::Circle => 1.,
            ShapeName::Oval => 1. / ((t.cos()).powi(2) + (t.sin() / 0.78).powi(2)).sqrt(),
            ShapeName::Cookie4 => 1. + 0.11 * (4. * t).cos(),
            ShapeName::Cookie6 => 1. + 0.085 * (6. * t).cos(),
            ShapeName::Cookie9 => 1. + 0.065 * (9. * t).cos(),
            ShapeName::Cookie12 => 1. + 0.05 * (12. * t).cos(),
            ShapeName::Clover4 => 1. + 0.2 * (4. * t + PI / 4.).cos(),
            ShapeName::Clover8 => 1. + 0.13 * (8. * t).cos(),
            ShapeName::Sunny => 1. + 0.13 * (8. * t).cos().powi(3),
            ShapeName::SoftBurst => 1. + 0.1 * (10. * t).cos().powi(3),
            ShapeName::Pentagon => polygon(t, 5., 0.28, -PI / 2.),
            ShapeName::Triangle => polygon(t, 3., 0.3, -PI / 2.),
            ShapeName::Gem => polygon(t, 4., 0.22, 0.),
            ShapeName::Blob => 1. + 0.075 * (2. * t + 0.6).cos() + 0.045 * (3. * t - 1.1).cos(),
            ShapeName::Blob2 => 1. + 0.07 * (2. * t - 0.9).cos() + 0.05 * (3. * t + 2.0).cos(),
            ShapeName::Pebble => 1. + 0.06 * (3. * t).cos() + 0.03 * (2. * t + 1.3).cos(),
            // Traced from their outline instead.
            _ => 1.,
        }
    }

    /// Rounded-polygon outline (unit box, y down) and the point its rays are cast from, for
    /// the shapes that are not a simple radius function. Vertices and roundings follow
    /// Compose's `MaterialShapes`; corner smoothing is left out.
    fn outline(self) -> Option<(Vec<(f32, f32)>, (f32, f32))> {
        const CENTER: (f32, f32) = (0.5, 0.5);
        let corners = match self {
            ShapeName::Square => rectangle(1., 1., [0.3; 4]),
            ShapeName::Slanted => custom(&[(0.926, 0.970, 0.189), (-0.021, 0.967, 0.187)], 2, false),
            ShapeName::Arch => regular(4, &[1., 1., 0.2, 0.2], -135f32.to_radians()),
            ShapeName::Fan => custom(
                &[
                    (1.004, 1.000, 0.148),
                    (0.000, 1.000, 0.151),
                    (0.000, -0.003, 0.148),
                    (0.978, 0.020, 0.803),
                ],
                1,
                false,
            ),
            ShapeName::Arrow => custom(
                &[
                    (0.500, 0.892, 0.313),
                    (-0.216, 1.050, 0.207),
                    (0.499, -0.160, 0.215),
                    (1.225, 1.060, 0.211),
                ],
                1,
                false,
            ),
            // A 1.6 × 1 rectangle with soft bottom corners and a fully round top.
            ShapeName::SemiCircle => rectangle(1.6, 1., [0.2, 0.2, 1., 1.]),
            ShapeName::Pill => custom(
                &[(0.961, 0.039, 0.426), (1.001, 0.428, 0.), (1.000, 0.609, 1.)],
                2,
                true,
            ),
            ShapeName::Diamond => custom(&[(0.500, 1.096, 0.151), (0.040, 0.500, 0.159)], 2, false),
            ShapeName::ClamShell => custom(
                &[(0.171, 0.841, 0.159), (-0.020, 0.500, 0.140), (0.170, 0.159, 0.159)],
                2,
                false,
            ),
            ShapeName::VerySunny => star(8, 0.65, 0.15, 0.),
            ShapeName::Cookie7 => star(7, 0.75, 0.5, -PI / 2.),
            ShapeName::Ghostish => custom(
                &[
                    (0.500, 0.000, 1.),
                    (1.000, 0.000, 1.),
                    (1.000, 1.140, 0.254),
                    (0.575, 0.906, 0.253),
                ],
                1,
                true,
            ),
            ShapeName::Burst => custom(&[(0.500, -0.006, 0.006), (0.592, 0.158, 0.006)], 12, false),
            ShapeName::Boom => custom(&[(0.457, 0.296, 0.007), (0.500, -0.051, 0.007)], 15, false),
            ShapeName::SoftBoom => custom(
                &[
                    (0.733, 0.454, 0.),
                    (0.839, 0.437, 0.532),
                    (0.949, 0.449, 0.439),
                    (0.998, 0.478, 0.174),
                ],
                16,
                true,
            ),
            ShapeName::Flower => custom(
                &[(0.370, 0.187, 0.), (0.416, 0.049, 0.381), (0.479, 0.001, 0.095)],
                8,
                true,
            ),
            ShapeName::Puffy => custom(
                &[
                    (0.500, 0.053, 0.),
                    (0.545, -0.040, 0.405),
                    (0.670, -0.035, 0.426),
                    (0.717, 0.066, 0.574),
                    (0.722, 0.128, 0.),
                    (0.777, 0.002, 0.360),
                    (0.914, 0.149, 0.660),
                    (0.926, 0.289, 0.660),
                    (0.881, 0.346, 0.),
                    (0.940, 0.344, 0.126),
                    (1.003, 0.437, 0.255),
                ],
                2,
                true,
            )
            .into_iter()
            .map(|(x, y, r)| (x, (y - 0.5) * 0.742 + 0.5, r))
            .collect::<Vec<_>>(),
            ShapeName::PuffyDiamond => custom(
                &[(0.870, 0.130, 0.146), (0.818, 0.357, 0.), (1.000, 0.332, 0.853)],
                4,
                true,
            ),
            ShapeName::PixelCircle => custom(
                &[
                    (0.500, 0.000, 0.),
                    (0.704, 0.000, 0.),
                    (0.704, 0.065, 0.),
                    (0.843, 0.065, 0.),
                    (0.843, 0.148, 0.),
                    (0.926, 0.148, 0.),
                    (0.926, 0.296, 0.),
                    (1.000, 0.296, 0.),
                ],
                2,
                true,
            ),
            ShapeName::PixelTriangle => {
                let corners = custom(
                    &[
                        (0.110, 0.500, 0.),
                        (0.113, 0.000, 0.),
                        (0.287, 0.000, 0.),
                        (0.287, 0.087, 0.),
                        (0.421, 0.087, 0.),
                        (0.421, 0.170, 0.),
                        (0.560, 0.170, 0.),
                        (0.560, 0.265, 0.),
                        (0.674, 0.265, 0.),
                        (0.675, 0.344, 0.),
                        (0.789, 0.344, 0.),
                        (0.789, 0.439, 0.),
                        (0.888, 0.439, 0.),
                    ],
                    1,
                    true,
                );
                // Cast from left of the first step so that every step faces away from the origin.
                return Some((rounded_outline(&corners), (0.2, 0.5)));
            }
            ShapeName::Bun => custom(
                &[
                    (0.796, 0.500, 0.),
                    (0.853, 0.518, 1.),
                    (0.992, 0.631, 1.),
                    (0.968, 1.000, 1.),
                ],
                2,
                true,
            ),
            ShapeName::Heart => custom(
                &[
                    (0.500, 0.268, 0.016),
                    (0.792, -0.066, 0.958),
                    (1.064, 0.276, 1.),
                    (0.501, 0.946, 0.129),
                ],
                1,
                true,
            ),
            _ => return None,
        };
        Some((rounded_outline(&corners), CENTER))
    }

    /// Shape points fitted to the unit box (0–1), centered and undistorted.
    /// Computed only once per shape.
    pub fn unit_points(self) -> &'static [(f32, f32)] {
        static CACHE: OnceLock<Vec<Vec<(f32, f32)>>> = OnceLock::new();
        let all = CACHE.get_or_init(|| ShapeName::ALL.iter().map(|s| s.compute_points()).collect());
        &all[self as usize]
    }

    fn compute_points(self) -> Vec<(f32, f32)> {
        let outline = self.outline();
        let raw: Vec<(f32, f32)> = (0..POINTS)
            .map(|i| {
                let t = i as f32 / POINTS as f32 * TAU - PI / 2.;
                let k = match &outline {
                    Some((outline, origin)) => ray_distance(outline, *origin, t),
                    None => self.radius(t),
                };
                (t.cos() * k, t.sin() * k)
            })
            .collect();
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for &(x, y) in &raw {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        let scale = 1. / (max_x - min_x).max(max_y - min_y);
        let ox = (1. - (max_x - min_x) * scale) / 2.;
        let oy = (1. - (max_y - min_y) * scale) / 2.;
        raw.into_iter()
            .map(|(x, y)| ((x - min_x) * scale + ox, (y - min_y) * scale + oy))
            .collect()
    }
}

/// Regular polygon with smoothed corners (blended with a circle).
fn polygon(t: f32, sides: f32, roundness: f32, rotate: f32) -> f32 {
    let a = TAU / sides;
    let local = ((t - rotate) % a + a) % a - a / 2.;
    let sharp = (PI / sides).cos() / local.cos();
    sharp * (1. - roundness) + roundness
}

/// A rounded-polygon corner: position and rounding radius, in the unit box.
type Corner = (f32, f32, f32);

/// Port of Compose's `customPolygon`: repeats `points` `reps` times around the center of the
/// unit box, rotated, or (with `mirroring`) alternately mirrored.
fn custom(points: &[Corner], reps: usize, mirroring: bool) -> Vec<Corner> {
    let (cx, cy) = (0.5f32, 0.5f32);
    let polar: Vec<(f32, f32)> = points
        .iter()
        .map(|&(x, y, _)| ((y - cy).atan2(x - cx), (x - cx).hypot(y - cy)))
        .collect();
    let at = |angle: f32, distance: f32, r: f32| -> Corner {
        (cx + angle.cos() * distance, cy + angle.sin() * distance, r)
    };
    let mut out = Vec::new();
    if mirroring {
        let reps = reps * 2;
        let section = TAU / reps as f32;
        let first = polar[0].0;
        for rep in 0..reps {
            let even = rep % 2 == 0;
            for index in 0..points.len() {
                let i = if even { index } else { points.len() - 1 - index };
                // Odd sections skip the first point: it sits on the mirror axis.
                if i == 0 && !even {
                    continue;
                }
                let (angle, distance) = polar[i];
                let angle = if even {
                    section * rep as f32 + angle
                } else {
                    section * rep as f32 + section - angle + 2. * first
                };
                out.push(at(angle, distance, points[i].2));
            }
        }
    } else {
        for rep in 0..reps {
            let turn = rep as f32 * TAU / reps as f32;
            for (i, &(angle, distance)) in polar.iter().enumerate() {
                out.push(at(angle + turn, distance, points[i].2));
            }
        }
    }
    out
}

/// Compose's `RoundedPolygon(numVertices)`: a regular polygon of radius 1 (0.5 here) whose
/// first vertex is at angle `rotate`. Roundings are in Compose units.
fn regular(sides: usize, rounding: &[f32], rotate: f32) -> Vec<Corner> {
    (0..sides)
        .map(|i| {
            let a = rotate + i as f32 * TAU / sides as f32;
            let r = rounding[i % rounding.len()] / 2.;
            (0.5 + a.cos() * 0.5, 0.5 + a.sin() * 0.5, r)
        })
        .collect()
}

/// Compose's `RoundedPolygon.star`: outer vertices of radius 1 (0.5 here) alternating with
/// inner ones. The rounding is in Compose units.
fn star(points: usize, inner: f32, rounding: f32, rotate: f32) -> Vec<Corner> {
    let step = PI / points as f32;
    (0..points * 2)
        .map(|i| {
            let a = rotate + i as f32 * step;
            let radius = if i % 2 == 0 { 0.5 } else { 0.5 * inner };
            (0.5 + a.cos() * radius, 0.5 + a.sin() * radius, rounding / 2.)
        })
        .collect()
}

/// Compose's `RoundedPolygon.rectangle`, centered in the unit box. Roundings go bottom-right,
/// bottom-left, top-left, top-right.
fn rectangle(width: f32, height: f32, rounding: [f32; 4]) -> Vec<Corner> {
    let (left, right) = (0.5 - width / 2., 0.5 + width / 2.);
    let (top, bottom) = (0.5 - height / 2., 0.5 + height / 2.);
    vec![
        (right, bottom, rounding[0]),
        (left, bottom, rounding[1]),
        (left, top, rounding[2]),
        (right, top, rounding[3]),
    ]
}

/// Polyline of a closed polygon whose corners are rounded with a quadratic curve. When two
/// neighboring cuts don't fit on their shared edge, both shrink in proportion (as in Compose).
fn rounded_outline(corners: &[Corner]) -> Vec<(f32, f32)> {
    const CURVE_STEPS: usize = 8;
    let n = corners.len();
    let vertex = |i: usize| (corners[i % n].0, corners[i % n].1);
    let edge = |i: usize| {
        let (a, b) = (vertex(i), vertex(i + 1));
        (b.0 - a.0).hypot(b.1 - a.1)
    };
    // Distance from each vertex to where its rounding starts, along both edges.
    let wanted: Vec<f32> = (0..n)
        .map(|i| {
            let r = corners[i].2;
            if r <= 0. {
                return 0.;
            }
            let (v, p, q) = (vertex(i), vertex(i + n - 1), vertex(i + 1));
            let a = (p.0 - v.0, p.1 - v.1);
            let b = (q.0 - v.0, q.1 - v.1);
            let cos = (a.0 * b.0 + a.1 * b.1) / (a.0.hypot(a.1) * b.0.hypot(b.1)).max(1e-6);
            let half = cos.clamp(-1., 1.).acos() / 2.;
            if half < 1e-4 {
                f32::MAX
            } else {
                r / half.tan()
            }
        })
        .collect();
    let fit = |i: usize| {
        let (a, b) = (wanted[i % n], wanted[(i + 1) % n]);
        if a + b > edge(i) {
            edge(i) / (a + b)
        } else {
            1.
        }
    };
    let mut out = Vec::with_capacity(n * (CURVE_STEPS + 1));
    for i in 0..n {
        let d = wanted[i] * fit(i + n - 1).min(fit(i));
        let v = vertex(i);
        if d <= 1e-6 {
            out.push(v);
            continue;
        }
        let toward = |o: (f32, f32)| {
            let len = (o.0 - v.0).hypot(o.1 - v.1).max(1e-6);
            (v.0 + (o.0 - v.0) / len * d, v.1 + (o.1 - v.1) / len * d)
        };
        let (start, end) = (toward(vertex(i + n - 1)), toward(vertex(i + 1)));
        for step in 0..=CURVE_STEPS {
            let s = step as f32 / CURVE_STEPS as f32;
            let (a, b, c) = ((1. - s) * (1. - s), 2. * s * (1. - s), s * s);
            out.push((
                a * start.0 + b * v.0 + c * end.0,
                a * start.1 + b * v.1 + c * end.1,
            ));
        }
    }
    out
}

/// Distance from `origin` to a closed outline along the ray at angle `t`. Takes the farthest
/// crossing, so the sampled shape stays one closed curve even where the outline folds back.
fn ray_distance(outline: &[(f32, f32)], origin: (f32, f32), t: f32) -> f32 {
    let dir = (t.cos(), t.sin());
    let cross = |a: (f32, f32), b: (f32, f32)| a.0 * b.1 - a.1 * b.0;
    let mut best = 0f32;
    for i in 0..outline.len() {
        let p = outline[i];
        let q = outline[(i + 1) % outline.len()];
        let e = (q.0 - p.0, q.1 - p.1);
        let denom = cross(dir, e);
        if denom.abs() < 1e-9 {
            continue;
        }
        let w = (p.0 - origin.0, p.1 - origin.1);
        let s = cross(w, e) / denom;
        let u = cross(w, dir) / denom;
        if s > 0. && (-1e-5..=1. + 1e-5).contains(&u) {
            best = best.max(s);
        }
    }
    best
}

/// A shape at one point in its morph: `from` → `to` at progress `t`
/// (may go outside 0–1 for the spring's bounce), rotated by `rotation` radians.
#[derive(Clone, Copy, Debug)]
pub struct Morph {
    pub from: ShapeName,
    pub to: ShapeName,
    pub t: f32,
    pub rotation: f32,
    /// A third shape blended on top, with its weight (a hover that pulls the shape toward
    /// another while it keeps breathing). `None` for a plain two-shape morph.
    pub toward: Option<(ShapeName, f32)>,
}

impl Morph {
    pub fn still(shape: ShapeName) -> Self {
        Self {
            from: shape,
            to: shape,
            t: 0.,
            rotation: 0.,
            toward: None,
        }
    }

    /// Shape points within `bounds`.
    pub fn points(&self, bounds: Bounds<Pixels>) -> Vec<Point<Pixels>> {
        let a = self.from.unit_points();
        let b = if self.to == self.from {
            None
        } else {
            Some(self.to.unit_points())
        };
        let w = f32::from(bounds.size.width);
        let h = f32::from(bounds.size.height);
        let cx = f32::from(bounds.origin.x) + w / 2.;
        let cy = f32::from(bounds.origin.y) + h / 2.;
        let (sin, cos) = self.rotation.sin_cos();
        let toward = self
            .toward
            .filter(|(_, w)| *w != 0.)
            .map(|(shape, w)| (shape.unit_points(), w));
        a.iter()
            .enumerate()
            .map(|(i, &(ax, ay))| {
                let (x, y) = match b {
                    Some(b) => (ax + (b[i].0 - ax) * self.t, ay + (b[i].1 - ay) * self.t),
                    None => (ax, ay),
                };
                let (x, y) = match toward {
                    Some((h, w)) => (x + (h[i].0 - x) * w, y + (h[i].1 - y) * w),
                    None => (x, y),
                };
                let (dx, dy) = ((x - 0.5) * w, (y - 0.5) * h);
                point(px(cx + dx * cos - dy * sin), px(cy + dx * sin + dy * cos))
            })
            .collect()
    }

    /// Paints the filled shape. Only valid during the paint phase (e.g. inside a `canvas`).
    pub fn paint(
        &self,
        bounds: Bounds<Pixels>,
        color: impl Into<gpui::Background>,
        window: &mut Window,
    ) {
        let points = self.points(bounds);
        let mut builder = PathBuilder::fill();
        builder.add_polygon(&points, true);
        if let Ok(path) = builder.build() {
            window.paint_path(path, color);
        }
    }
}

/// M3 Expressive loading indicator sequence (Compose's `LoadingIndicatorDefaults`).
pub const LOADING_SEQUENCE: [ShapeName; 7] = [
    ShapeName::SoftBurst,
    ShapeName::Cookie9,
    ShapeName::Pentagon,
    ShapeName::Pill,
    ShapeName::Sunny,
    ShapeName::Cookie4,
    ShapeName::Oval,
];

const AVATAR_SHAPES: [ShapeName; 8] = [
    ShapeName::Cookie9,
    ShapeName::Clover4,
    ShapeName::Sunny,
    ShapeName::Pentagon,
    ShapeName::Cookie6,
    ShapeName::Gem,
    ShapeName::SoftBurst,
    ShapeName::Clover8,
];

/// Color tone of a container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Primary,
    Secondary,
    Tertiary,
}

impl Tone {
    /// Background and content colors of the tonal container.
    pub fn colors(self, scheme: &crate::Scheme) -> (Hsla, Hsla) {
        match self {
            Tone::Primary => (scheme.primary_container, scheme.on_primary_container),
            Tone::Secondary => (scheme.secondary_container, scheme.on_secondary_container),
            Tone::Tertiary => (scheme.tertiary_container, scheme.on_tertiary_container),
        }
    }
}

/// Stable shape and tone derived from a string (same hash as the reference design).
pub fn avatar_for(seed: &str) -> (ShapeName, Tone) {
    let mut h: u32 = 0;
    for ch in seed.chars() {
        let mut buf = [0u16; 2];
        let unit = ch.encode_utf16(&mut buf)[0];
        h = h.wrapping_mul(31).wrapping_add(unit as u32);
    }
    let tone = [Tone::Primary, Tone::Secondary, Tone::Tertiary][((h >> 4) % 3) as usize];
    (
        AVATAR_SHAPES[(h % AVATAR_SHAPES.len() as u32) as usize],
        tone,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_fit_the_unit_box() {
        for shape in ShapeName::ALL {
            let points = shape.unit_points();
            assert_eq!(points.len(), POINTS);
            assert!(
                points
                    .iter()
                    .all(|&(x, y)| (-1e-4..=1.0001).contains(&x) && (-1e-4..=1.0001).contains(&y))
            );
        }
    }

    #[test]
    fn all_lists_every_shape_in_order() {
        for (i, shape) in ShapeName::ALL.iter().enumerate() {
            assert_eq!(*shape as usize, i);
        }
    }

    #[test]
    fn traced_shapes_have_no_holes() {
        // Every ray from the origin must cross the outline.
        for shape in ShapeName::ALL {
            if let Some((outline, origin)) = shape.outline() {
                for i in 0..POINTS {
                    let t = i as f32 / POINTS as f32 * TAU;
                    assert!(ray_distance(&outline, origin, t) > 0.05, "{}", shape.label());
                }
            }
        }
    }

    #[test]
    fn rounded_square_keeps_flat_sides() {
        let outline = rounded_outline(&rectangle(1., 1., [0.3; 4]));
        assert!((ray_distance(&outline, (0.5, 0.5), 0.) - 0.5).abs() < 1e-4);
        // The corner is cut: the diagonal is shorter than the sharp square's.
        assert!(ray_distance(&outline, (0.5, 0.5), PI / 4.) < 0.5 * 2f32.sqrt() - 0.05);
    }

    #[test]
    fn cache_matches_enum_order() {
        for shape in ShapeName::ALL {
            assert_eq!(shape.unit_points(), shape.compute_points().as_slice());
        }
    }

    #[test]
    fn avatars_match_the_reference_hash() {
        // Expected values computed with the reference design's JavaScript implementation.
        assert_eq!(
            avatar_for("dashboard-php"),
            (ShapeName::Clover8, Tone::Primary)
        );
        assert_eq!(avatar_for("ñandú"), (ShapeName::Cookie6, Tone::Tertiary));
    }
}
