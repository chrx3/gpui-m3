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
}

impl ShapeName {
    pub const ALL: [ShapeName; 17] = [
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
        }
    }

    /// Relative radius of the shape at angle `t`.
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
            ShapeName::Square => polygon(t, 4., 0.3, PI / 4.),
            ShapeName::Blob => 1. + 0.075 * (2. * t + 0.6).cos() + 0.045 * (3. * t - 1.1).cos(),
            ShapeName::Blob2 => 1. + 0.07 * (2. * t - 0.9).cos() + 0.05 * (3. * t + 2.0).cos(),
            ShapeName::Pebble => 1. + 0.06 * (3. * t).cos() + 0.03 * (2. * t + 1.3).cos(),
        }
    }

    /// Shape points fitted to the unit box (0–1), centered and undistorted.
    /// Computed only once per shape.
    pub fn unit_points(self) -> &'static [(f32, f32)] {
        static CACHE: OnceLock<Vec<Vec<(f32, f32)>>> = OnceLock::new();
        let all = CACHE.get_or_init(|| ShapeName::ALL.iter().map(|s| s.compute_points()).collect());
        &all[self as usize]
    }

    fn compute_points(self) -> Vec<(f32, f32)> {
        let raw: Vec<(f32, f32)> = (0..POINTS)
            .map(|i| {
                let t = i as f32 / POINTS as f32 * TAU - PI / 2.;
                let k = self.radius(t);
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

/// M3 Expressive loading indicator sequence.
pub const LOADING_SEQUENCE: [ShapeName; 7] = [
    ShapeName::SoftBurst,
    ShapeName::Cookie9,
    ShapeName::Pentagon,
    ShapeName::Oval,
    ShapeName::Sunny,
    ShapeName::Cookie4,
    ShapeName::Clover4,
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
