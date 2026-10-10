//! Photos clipped to M3 shapes. GPUI clips only to rectangles, so the photo is resampled once into
//! a square raster whose alpha follows the shape (anti-aliased), in the background, and cached
//! through GPUI's asset system. Morphing shapes use a few cached steps.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{App, Asset, Bounds, ImgResourceLoader, RenderImage, Resource, Window, point, px, size};

use crate::shapes::{Morph, ShapeName};

/// Number of cached steps of a photo's shape morph (plus the rest shape).
pub(crate) const MORPH_STEPS: u8 = 8;

/// Where a shaped photo comes from.
#[derive(Clone, Debug)]
pub enum PhotoSource {
    /// A file, loaded and decoded in the background by GPUI.
    Path(Arc<Path>),
    /// An image you already decoded (a cropped profile picture, a pasted image).
    Image(Arc<RenderImage>),
}

impl From<PathBuf> for PhotoSource {
    fn from(path: PathBuf) -> Self {
        PhotoSource::Path(path.into())
    }
}

impl From<&Path> for PhotoSource {
    fn from(path: &Path) -> Self {
        PhotoSource::Path(path.into())
    }
}

impl From<&str> for PhotoSource {
    fn from(path: &str) -> Self {
        PhotoSource::Path(Path::new(path).into())
    }
}

impl From<Arc<Path>> for PhotoSource {
    fn from(path: Arc<Path>) -> Self {
        PhotoSource::Path(path)
    }
}

impl From<Arc<RenderImage>> for PhotoSource {
    fn from(image: Arc<RenderImage>) -> Self {
        PhotoSource::Image(image)
    }
}

impl PhotoSource {
    /// The decoded image, once it is ready (requests a redraw of the current view when it is).
    pub(crate) fn load(&self, window: &mut Window, cx: &mut App) -> Option<Arc<RenderImage>> {
        match self {
            PhotoSource::Image(image) => Some(image.clone()),
            PhotoSource::Path(path) => window
                .use_asset::<ImgResourceLoader>(&Resource::Path(path.clone()), cx)?
                .ok(),
        }
    }
}

/// A photo masked to one step of a shape morph, at a size in device pixels.
#[derive(Clone)]
pub(crate) struct MaskKey {
    pub image: Arc<RenderImage>,
    pub from: ShapeName,
    pub to: ShapeName,
    pub step: u8,
    pub size: u32,
}

impl Hash for MaskKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.image.id.hash(state);
        self.from.hash(state);
        self.to.hash(state);
        self.step.hash(state);
        self.size.hash(state);
    }
}

pub(crate) enum MaskLoader {}

impl Asset for MaskLoader {
    type Source = MaskKey;
    type Output = Option<Arc<RenderImage>>;

    // `async fn` would borrow the `App` argument; the future must be `'static`.
    #[allow(clippy::manual_async_fn)]
    fn load(
        source: Self::Source,
        _: &mut App,
    ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
        async move { masked(&source).map(Arc::new) }
    }
}

/// The shaped photo for `t` (0–1) of the morph `from → to`, or the closest step already made.
pub(crate) fn shaped_photo(
    image: Arc<RenderImage>,
    from: ShapeName,
    to: ShapeName,
    t: f32,
    size: u32,
    window: &mut Window,
    cx: &mut App,
) -> Option<Arc<RenderImage>> {
    let step = if from == to {
        0
    } else {
        (t.clamp(0., 1.) * MORPH_STEPS as f32).round() as u8
    };
    let request = |step: u8| MaskKey {
        image: image.clone(),
        from,
        to,
        step,
        size,
    };
    if let Some(Some(done)) = window.use_asset::<MaskLoader>(&request(step), cx) {
        return Some(done);
    }
    // Not ready yet: fall back to the nearest step toward the rest shape.
    (0..step)
        .rev()
        .find_map(|s| window.get_asset::<MaskLoader>(&request(s), cx).flatten())
}

/// Starts making every step of a morph so hovering finds them ready.
pub(crate) fn prewarm(
    image: &Arc<RenderImage>,
    from: ShapeName,
    to: ShapeName,
    size: u32,
    window: &mut Window,
    cx: &mut App,
) {
    for step in 0..=MORPH_STEPS {
        let key = MaskKey {
            image: image.clone(),
            from,
            to,
            step,
            size,
        };
        window.use_asset::<MaskLoader>(&key, cx);
    }
}

/// Resamples the photo to cover a `size`² square and masks it to the shape.
fn masked(key: &MaskKey) -> Option<RenderImage> {
    let n = key.size.max(1) as usize;
    let src = key.image.as_bytes(0)?;
    let src_size = key.image.size(0);
    let (sw, sh) = (src_size.width.0 as usize, src_size.height.0 as usize);
    if sw == 0 || sh == 0 || src.len() < sw * sh * 4 {
        return None;
    }
    let t = key.step as f32 / MORPH_STEPS as f32;
    let morph = Morph {
        from: key.from,
        to: key.to,
        t,
        rotation: 0.,
    };
    let bounds = Bounds {
        origin: point(px(0.), px(0.)),
        size: size(px(n as f32), px(n as f32)),
    };
    let polygon: Vec<(f32, f32)> = morph
        .points(bounds)
        .into_iter()
        .map(|p| (f32::from(p.x), f32::from(p.y)))
        .collect();
    let mask = coverage(&polygon, n);
    // Cover: scale the shorter side to the square and center the crop.
    let scale = (sw as f32 / n as f32).min(sh as f32 / n as f32);
    let ox = (sw as f32 - n as f32 * scale) / 2.;
    let oy = (sh as f32 - n as f32 * scale) / 2.;
    let mut out = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let a = mask[y * n + x];
            if a == 0 {
                continue;
            }
            let fx = ox + (x as f32 + 0.5) * scale - 0.5;
            let fy = oy + (y as f32 + 0.5) * scale - 0.5;
            let px = bilinear(src, sw, sh, fx, fy);
            let o = (y * n + x) * 4;
            out[o..o + 3].copy_from_slice(&px[..3]);
            out[o + 3] = ((px[3] as u32 * a as u32) / 255) as u8;
        }
    }
    let buffer = image::RgbaImage::from_raw(n as u32, n as u32, out)?;
    Some(RenderImage::new(vec![image::Frame::new(buffer)]))
}

/// Bilinear sample of a 4-channel image (channel order is kept: GPUI stores BGRA).
fn bilinear(src: &[u8], w: usize, h: usize, x: f32, y: f32) -> [u8; 4] {
    let x = x.clamp(0., (w - 1) as f32);
    let y = y.clamp(0., (h - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (dx, dy) = (x - x0 as f32, y - y0 as f32);
    let at = |x: usize, y: usize, c: usize| src[(y * w + x) * 4 + c] as f32;
    let mut px = [0u8; 4];
    for (c, out) in px.iter_mut().enumerate() {
        let top = at(x0, y0, c) * (1. - dx) + at(x1, y0, c) * dx;
        let bottom = at(x0, y1, c) * (1. - dx) + at(x1, y1, c) * dx;
        *out = (top * (1. - dy) + bottom * dy).round().clamp(0., 255.) as u8;
    }
    px
}

/// Anti-aliased coverage (0–255) of a polygon over an `n`² grid, with 4×4 samples per pixel.
pub(crate) fn coverage(polygon: &[(f32, f32)], n: usize) -> Vec<u8> {
    const SAMPLES: usize = 4;
    let mut out = vec![0u8; n * n];
    let mut crossings: Vec<f32> = Vec::with_capacity(16);
    for y in 0..n {
        let mut row = vec![0u16; n];
        for sy in 0..SAMPLES {
            let yy = y as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
            // Scanline: x positions where the polygon's edges cross this sample row.
            crossings.clear();
            for i in 0..polygon.len() {
                let (ax, ay) = polygon[i];
                let (bx, by) = polygon[(i + 1) % polygon.len()];
                if (ay <= yy) != (by <= yy) {
                    crossings.push(ax + (yy - ay) / (by - ay) * (bx - ax));
                }
            }
            crossings.sort_by(|a, b| a.total_cmp(b));
            for pair in crossings.chunks_exact(2) {
                let (start, end) = (pair[0], pair[1]);
                for (x, cell) in row.iter_mut().enumerate() {
                    for sx in 0..SAMPLES {
                        let xx = x as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                        if xx >= start && xx < end {
                            *cell += 1;
                        }
                    }
                }
            }
        }
        for (x, cell) in row.into_iter().enumerate() {
            out[y * n + x] = (cell as u32 * 255 / (SAMPLES * SAMPLES) as u32) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_covers_inside_and_antialiases_edges() {
        let square = [(1., 1.), (3., 1.), (3., 3.), (1., 3.)];
        let c = coverage(&square, 4);
        assert_eq!(c[5], 255); // (1,1) fully inside
        assert_eq!(c[0], 0); // (0,0) outside
        let half = [(0., 0.), (2., 0.), (2., 2.), (0., 2.)].map(|(x, y): (f32, f32)| (x + 0.5, y));
        let c = coverage(&half, 3);
        assert!(
            (100..=155).contains(&c[0]),
            "half-covered pixel got {}",
            c[0]
        );
    }

    #[test]
    fn photo_is_masked_to_the_circle() {
        // A 2×2 opaque white image becomes a 16 px circle: opaque center, transparent corners.
        let buffer = image::RgbaImage::from_raw(2, 2, vec![255; 16]).unwrap();
        let image = Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]));
        let key = MaskKey {
            image,
            from: ShapeName::Circle,
            to: ShapeName::Circle,
            step: 0,
            size: 16,
        };
        let out = masked(&key).unwrap();
        let bytes = out.as_bytes(0).unwrap();
        let alpha = |x: usize, y: usize| bytes[(y * 16 + x) * 4 + 3];
        assert_eq!(alpha(8, 8), 255);
        assert_eq!(alpha(0, 0), 0);
        assert_eq!(alpha(15, 15), 0);
    }
}
