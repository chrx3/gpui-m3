//! Framing a photo before saving it (a profile picture): drag to move, zoom with the wheel or a
//! slider, a shaped mask that shows exactly what is kept, Enter to crop and Escape to cancel.

use std::sync::Arc;

use gpui::{
    App, Bounds, Context, Corners, EventEmitter, FocusHandle, Focusable, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder, Pixels, Point, Render, RenderImage,
    ScrollWheelEvent, Window, canvas, div, point, prelude::*, px, size,
};

use super::Slider;
use super::photo::bilinear;
use crate::shapes::{Morph, ShapeName};
use crate::theme::{FONT_FAMILY, Theme, alpha};

mod keys {
    gpui::actions!(m3_image_cropper, [Confirm, Cancel]);
}
use keys::*;

const KEY_CONTEXT: &str = "M3ImageCropper";

/// Registers Enter and Escape for the cropper. Called by [`crate::init`].
pub fn bind_image_cropper_keys(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("enter", Confirm, ctx),
        KeyBinding::new("escape", Cancel, ctx),
    ]);
}

/// Events emitted by an [`ImageCropper`].
#[derive(Clone)]
pub enum ImageCropperEvent {
    /// The framed square, [`ImageCropper::output_size`] pixels wide (not masked: the shape is
    /// applied wherever it is shown, e.g. by [`super::Avatar::image`]).
    Cropped(Arc<RenderImage>),
    Cancelled,
}

/// The framing state, in view pixels: the image's top-left corner inside the square view and the
/// zoom (1 = the image's short side fills the view).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framing {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

/// Image size in view pixels at `zoom` (it always covers the view).
fn scaled(image: (f32, f32), view: f32, zoom: f32) -> (f32, f32) {
    let k = view / image.0.min(image.1).max(1.) * zoom;
    (image.0 * k, image.1 * k)
}

/// Keeps the image covering the whole view.
pub(crate) fn clamp_framing(f: Framing, image: (f32, f32), view: f32) -> Framing {
    let (w, h) = scaled(image, view, f.zoom);
    Framing {
        x: f.x.clamp(view - w, 0.),
        y: f.y.clamp(view - h, 0.),
        zoom: f.zoom,
    }
}

/// Zooms to `zoom` keeping the view point `(cx, cy)` still.
pub(crate) fn zoom_framing(
    f: Framing,
    zoom: f32,
    at: (f32, f32),
    image: (f32, f32),
    view: f32,
    max_zoom: f32,
) -> Framing {
    let next = zoom.clamp(1., max_zoom.max(1.));
    let r = next / f.zoom;
    clamp_framing(
        Framing {
            x: at.0 - (at.0 - f.x) * r,
            y: at.1 - (at.1 - f.y) * r,
            zoom: next,
        },
        image,
        view,
    )
}

/// The source rectangle (x, y, side) in image pixels that the view shows.
pub(crate) fn source_rect(f: Framing, image: (f32, f32), view: f32) -> (f32, f32, f32) {
    let k = view / image.0.min(image.1).max(1.) * f.zoom;
    (-f.x / k, -f.y / k, view / k)
}

/// Resamples the square `rect` of `image` into an `out`² image (GPUI's BGRA order is kept).
fn crop(image: &RenderImage, rect: (f32, f32, f32), out: u32) -> Option<RenderImage> {
    let src = image.as_bytes(0)?;
    let s = image.size(0);
    let (sw, sh) = (s.width.0 as usize, s.height.0 as usize);
    if sw == 0 || sh == 0 || src.len() < sw * sh * 4 {
        return None;
    }
    let n = out.max(1) as usize;
    let step = rect.2 / n as f32;
    let mut pixels = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let fx = rect.0 + (x as f32 + 0.5) * step - 0.5;
            let fy = rect.1 + (y as f32 + 0.5) * step - 0.5;
            let o = (y * n + x) * 4;
            pixels[o..o + 4].copy_from_slice(&bilinear(src, sw, sh, fx, fy));
        }
    }
    let buffer = image::RgbaImage::from_raw(n as u32, n as u32, pixels)?;
    Some(RenderImage::new(vec![image::Frame::new(buffer)]))
}

/// Photo framing for a profile picture. Create it with the decoded image
/// and keep the entity; put it in a [`super::Dialog`] with your own Cancel / Save buttons that
/// call [`ImageCropper::cancel`] and [`ImageCropper::confirm`]. Focus it so Enter and Escape work.
///
/// Outside the shape the photo is dimmed; inside it is sharp, so you see exactly what is kept.
pub struct ImageCropper {
    focus_handle: FocusHandle,
    image: Arc<RenderImage>,
    framing: Framing,
    view: f32,
    max_zoom: f32,
    output: u32,
    shape: ShapeName,
    drag: Option<(Point<Pixels>, Framing)>,
    origin: Point<Pixels>,
}

impl EventEmitter<ImageCropperEvent> for ImageCropper {}

impl Focusable for ImageCropper {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ImageCropper {
    pub fn new(image: Arc<RenderImage>, cx: &mut Context<Self>) -> Self {
        let mut cropper = Self {
            focus_handle: cx.focus_handle(),
            image,
            framing: Framing {
                x: 0.,
                y: 0.,
                zoom: 1.,
            },
            view: 280.,
            max_zoom: 4.,
            output: 512,
            shape: ShapeName::Cookie9,
            drag: None,
            origin: point(px(0.), px(0.)),
        };
        cropper.center();
        cropper
    }

    /// Side of the square view (280 px by default).
    pub fn view_size(mut self, side: Pixels) -> Self {
        self.view = f32::from(side).max(40.);
        self.center();
        self
    }

    /// Side of the cropped image in pixels (512 by default).
    pub fn output_size(mut self, side: u32) -> Self {
        self.output = side.max(1);
        self
    }

    pub fn max_zoom(mut self, zoom: f32) -> Self {
        self.max_zoom = zoom.max(1.);
        self
    }

    /// Mask shape (Cookie9 by default, like the M3 profile avatar). `Circle` for a round crop.
    pub fn shape(mut self, shape: ShapeName) -> Self {
        self.shape = shape;
        self
    }

    pub fn framing(&self) -> Framing {
        self.framing
    }

    pub fn focus(&self, window: &mut Window) {
        window.focus(&self.focus_handle);
    }

    fn image_size(&self) -> (f32, f32) {
        let s = self.image.size(0);
        (s.width.0 as f32, s.height.0 as f32)
    }

    fn center(&mut self) {
        let (w, h) = scaled(self.image_size(), self.view, 1.);
        self.framing = Framing {
            x: (self.view - w) / 2.,
            y: (self.view - h) / 2.,
            zoom: 1.,
        };
    }

    /// Zooms around the center of the view.
    pub fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let c = self.view / 2.;
        self.framing = zoom_framing(
            self.framing,
            zoom,
            (c, c),
            self.image_size(),
            self.view,
            self.max_zoom,
        );
        cx.notify();
    }

    /// The framed square as a new image.
    pub fn cropped(&self) -> Option<Arc<RenderImage>> {
        let rect = source_rect(self.framing, self.image_size(), self.view);
        crop(&self.image, rect, self.output).map(Arc::new)
    }

    /// Crops and emits [`ImageCropperEvent::Cropped`] (Enter does the same).
    pub fn confirm(&mut self, cx: &mut Context<Self>) {
        if let Some(image) = self.cropped() {
            cx.emit(ImageCropperEvent::Cropped(image));
        }
    }

    /// Emits [`ImageCropperEvent::Cancelled`] (Escape does the same).
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        cx.emit(ImageCropperEvent::Cancelled);
    }

    fn on_confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        self.confirm(cx);
    }

    fn on_cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        self.cancel(cx);
    }

    fn on_mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle);
        self.drag = Some((e.position, self.framing));
        cx.notify();
    }

    fn on_mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some((start, framing)) = self.drag else {
            return;
        };
        if e.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            return;
        }
        let d = e.position - start;
        self.framing = clamp_framing(
            Framing {
                x: framing.x + f32::from(d.x),
                y: framing.y + f32::from(d.y),
                zoom: framing.zoom,
            },
            self.image_size(),
            self.view,
        );
        cx.notify();
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.drag = None;
        cx.notify();
    }

    fn on_scroll(&mut self, e: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let dy = f32::from(e.delta.pixel_delta(px(20.)).y);
        if dy == 0. {
            return;
        }
        let at = e.position - self.origin;
        // Wheel up zooms in (positive y delta in GPUI means scrolling up).
        let zoom = self.framing.zoom * (dy * 0.0025).exp();
        self.framing = zoom_framing(
            self.framing,
            zoom,
            (f32::from(at.x), f32::from(at.y)),
            self.image_size(),
            self.view,
            self.max_zoom,
        );
        cx.stop_propagation();
        cx.notify();
    }
}

impl Render for ImageCropper {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let view = self.view;
        let f = self.framing;
        let (w, h) = scaled(self.image_size(), view, f.zoom);
        let image = self.image.clone();
        let shape = self.shape;
        let entity = cx.entity();
        let scrim = alpha(s.surface_container_high, 0.62);
        let outline = alpha(s.on_surface, 0.85);
        let frame = canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, cx| {
                entity.update(cx, |c, _| c.origin = bounds.origin);
                let image_bounds =
                    Bounds::new(bounds.origin + point(px(f.x), px(f.y)), size(px(w), px(h)));
                window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                    window
                        .paint_image(image_bounds, Corners::default(), image, 0, false)
                        .ok();
                    // Dim everything outside the shape: the view rectangle with the shape cut out.
                    let points = Morph::still(shape).points(bounds);
                    let mut scrim_path = PathBuilder::fill();
                    scrim_path.add_polygon(
                        &[
                            bounds.origin,
                            bounds.top_right(),
                            bounds.bottom_right(),
                            bounds.bottom_left(),
                        ],
                        true,
                    );
                    scrim_path.add_polygon(&points, true);
                    if let Ok(path) = scrim_path.build() {
                        window.paint_path(path, scrim);
                    }
                    let mut edge = PathBuilder::stroke(px(1.5));
                    edge.add_polygon(&points, true);
                    if let Ok(path) = edge.build() {
                        window.paint_path(path, outline);
                    }
                });
            },
        )
        .size_full();
        let weak = cx.entity().downgrade();
        let zoom_slider = Slider::new(
            ("m3-crop-zoom", cx.entity_id().as_u64()),
            f.zoom,
            1.0..=self.max_zoom,
        )
        .step(0.01)
        .width(px(view - 30.))
        .on_change(move |z, _, cx| {
            weak.update(cx, |c, cx| c.set_zoom(z, cx)).ok();
        });
        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_confirm))
            .on_action(cx.listener(Self::on_cancel))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(14.))
            .font_family(FONT_FAMILY)
            .child(
                div()
                    .id(("m3-crop-view", cx.entity_id().as_u64()))
                    .size(px(view))
                    .rounded(px(20.))
                    .overflow_hidden()
                    .bg(s.surface_container_highest)
                    .when(self.drag.is_some(), |d| d.cursor_grabbing())
                    .when(self.drag.is_none(), |d| d.cursor_grab())
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_scroll_wheel(cx.listener(Self::on_scroll))
                    .child(frame),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        super::Icon::new("search")
                            .size(px(16.))
                            .color(s.on_surface_variant),
                    )
                    .child(zoom_slider),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_always_covers_the_view() {
        // A 400×200 image in a 100 px view: at zoom 1 it is 200×100.
        let f = clamp_framing(
            Framing {
                x: 10.,
                y: -5.,
                zoom: 1.,
            },
            (400., 200.),
            100.,
        );
        assert_eq!(
            f,
            Framing {
                x: 0.,
                y: 0.,
                zoom: 1.
            }
        );
        let f = clamp_framing(
            Framing {
                x: -500.,
                y: 0.,
                zoom: 1.,
            },
            (400., 200.),
            100.,
        );
        assert_eq!(f.x, -100.);
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let image = (200., 200.);
        let f = Framing {
            x: 0.,
            y: 0.,
            zoom: 1.,
        };
        let z = zoom_framing(f, 2., (50., 50.), image, 100., 4.);
        assert_eq!(
            z,
            Framing {
                x: -50.,
                y: -50.,
                zoom: 2.
            }
        );
        // The point (50, 50) of the view still maps to the same image pixel.
        let (sx, sy, side) = source_rect(z, image, 100.);
        assert_eq!((sx + side / 2., sy + side / 2.), (100., 100.));
        // Zoom is clamped to 1…max.
        assert_eq!(zoom_framing(f, 9., (0., 0.), image, 100., 4.).zoom, 4.);
        assert_eq!(zoom_framing(f, 0.2, (0., 0.), image, 100., 4.).zoom, 1.);
    }

    #[test]
    fn crop_resamples_the_framed_square() {
        // 4×2 image: left half black, right half white (BGRA). Framing the right half gives white.
        let mut px = Vec::new();
        for _y in 0..2 {
            for x in 0..4 {
                let v = if x < 2 { 0 } else { 255 };
                px.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let image = RenderImage::new(vec![image::Frame::new(
            image::RgbaImage::from_raw(4, 2, px).unwrap(),
        )]);
        let f = clamp_framing(
            Framing {
                x: -100.,
                y: 0.,
                zoom: 1.,
            },
            (4., 2.),
            50.,
        );
        let out = crop(&image, source_rect(f, (4., 2.), 50.), 8).unwrap();
        let bytes = out.as_bytes(0).unwrap();
        assert_eq!(out.size(0).width.0, 8);
        // Away from the left edge (bilinear blends with the black half there) it is white.
        for y in 0..8 {
            for x in 2..8 {
                assert!(bytes[(y * 8 + x) * 4] > 200, "pixel {x},{y}");
            }
        }
    }
}
