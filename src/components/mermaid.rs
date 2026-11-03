//! Mermaid diagrams for [`super::Markdown`] (feature `mermaid`): a ```` ```mermaid ```` block is
//! drawn as an image instead of code.
//!
//! [mermaid-rs-renderer](https://crates.io/crates/mermaid-rs-renderer) (pure Rust: no browser, no
//! Node) turns the diagram into SVG and `resvg` (the one GPUI already uses) rasterizes it, both
//! off the UI thread and once per content: the result is cached by the block's code and the colors
//! of the theme, so a theme change redraws it and a new frame does not. While it is being made, or
//! if the diagram cannot be read, the caller shows the normal code block.

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, LazyLock};

use gpui::{
    AnyElement, App, ClipboardItem, ElementId, Hsla, ImageSource, InteractiveElement, IntoElement,
    ParentElement, RenderImage, Rgba, StatefulInteractiveElement, Styled, Window, div, hsla, img,
    prelude::FluentBuilder, px,
};
use mermaid_rs_renderer as mmdr;

use crate::cache::Lru;
use crate::theme::Scheme;
use super::{IconButton, IconButtonVariant};

/// Diagrams bigger than this (in logical pixels, per side) are not drawn.
const MAX_SIDE: f32 = 12_000.;
/// Largest raster per side, in device pixels.
const MAX_RASTER: f32 = 4096.;
/// A wide diagram shrinks to fit its container down to this width, and scrolls beyond it.
const MIN_WIDTH: f32 = 480.;

/// A drawn diagram, as [`super::Markdown::on_diagram`] hands it over (to show it bigger, say).
#[derive(Clone)]
pub struct MermaidDiagram {
    /// The raster, at twice the screen's scale so it stays sharp when zoomed in.
    pub image: Arc<RenderImage>,
    /// The same pixels as a PNG file (to copy or save).
    pub png: Arc<Vec<u8>>,
    /// Natural size in logical pixels.
    pub size: (f32, f32),
}

impl MermaidDiagram {
    /// Puts the diagram on the clipboard as an image.
    pub fn copy(&self, cx: &mut App) {
        let image = gpui::Image::from_bytes(gpui::ImageFormat::Png, self.png.as_ref().clone());
        cx.write_to_clipboard(ClipboardItem::new_image(&image));
    }
}

pub(super) type DiagramHandler = std::rc::Rc<dyn Fn(&MermaidDiagram, &mut Window, &mut App)>;

#[derive(Clone)]
enum State {
    Pending,
    Failed,
    Ready(MermaidDiagram),
}

thread_local! {
    static DIAGRAMS: RefCell<Lru<State>> = const { RefCell::new(Lru::new(64)) };
}

/// Raw result of a render: straight-alpha BGRA pixels, their size and the logical size.
struct Raster {
    bgra: Vec<u8>,
    png: Vec<u8>,
    pixels: (u32, u32),
    size: (f32, f32),
}

fn css(color: Hsla) -> String {
    let c = Rgba::from(color);
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(c.r), byte(c.g), byte(c.b))
}

/// `color` painted over an opaque `base`: an SVG fill has no window behind it, so a
/// translucent color (Liquid Glass's surfaces are) would lose its alpha and turn solid white.
fn over(color: Hsla, base: Hsla) -> Hsla {
    let (c, b) = (Rgba::from(color), Rgba::from(base));
    let a = c.a.clamp(0., 1.);
    let mix = |x: f32, y: f32| x * a + y * (1. - a);
    Hsla::from(Rgba { r: mix(c.r, b.r), g: mix(c.g, b.g), b: mix(c.b, b.b), a: 1. })
}

/// The opaque ground the diagram is painted on: the theme's container surface, laid over a
/// near-black or near-white one when the surfaces themselves are translucent.
fn ground(s: &Scheme) -> Hsla {
    let dark = s.on_surface.l > 0.5;
    let fallback = if dark { hsla(0., 0., 0.12, 1.) } else { hsla(0., 0., 0.97, 1.) };
    over(s.surface_container, over(s.surface, fallback))
}

/// The diagram theme made from the app's colors (light or dark follows them).
fn diagram_theme(s: &Scheme) -> mmdr::Theme {
    let base = ground(s);
    let css = |color: Hsla| css(over(color, base));
    let mut t = mmdr::Theme::modern();
    t.font_family = "'Segoe UI', 'Helvetica Neue', Roboto, Arial, sans-serif".into();
    t.font_size = 14.;
    t.primary_color = css(s.surface_container_high);
    t.primary_text_color = css(s.on_surface);
    t.primary_border_color = css(s.outline);
    t.line_color = css(s.on_surface_variant);
    t.secondary_color = css(s.secondary_container);
    t.tertiary_color = css(s.tertiary_container);
    t.edge_label_background = css(s.surface_container);
    t.cluster_background = css(s.surface_container_highest);
    t.cluster_border = css(s.outline_variant);
    t.background = css(s.surface_container);
    t.text_color = css(s.on_surface);
    t.sequence_actor_fill = css(s.surface_container_high);
    t.sequence_actor_border = css(s.outline);
    t.sequence_actor_line = css(s.outline);
    t.sequence_note_fill = css(s.tertiary_container);
    t.sequence_note_border = css(s.outline);
    t.sequence_activation_fill = css(s.surface_container_highest);
    t.sequence_activation_border = css(s.outline);
    let palette = [
        s.primary,
        s.tertiary,
        s.success,
        s.warning,
        s.error,
        s.secondary_container,
    ];
    for (i, slot) in t.pie_colors.iter_mut().enumerate() {
        *slot = css(palette[i % palette.len()]);
    }
    t.pie_title_text_color = css(s.on_surface);
    t.pie_section_text_color = css(s.on_primary);
    t.pie_legend_text_color = css(s.on_surface);
    t.pie_stroke_color = css(s.surface_container);
    t.pie_outer_stroke_color = css(s.outline_variant);
    t
}

/// System fonts, loaded once for the whole process (the first diagram pays for it).
static FONTS: LazyLock<Arc<usvg::fontdb::Database>> = LazyLock::new(|| {
    let mut db = usvg::fontdb::Database::new();
    db.load_system_fonts();
    Arc::new(db)
});

fn rasterize(code: &str, theme: mmdr::Theme, scale: f32) -> Option<Raster> {
    let options = mmdr::RenderOptions {
        theme,
        layout: mmdr::LayoutConfig::default(),
    };
    let svg = mmdr::render_strict(code, options).ok()?;
    let usvg_options = usvg::Options {
        fontdb: FONTS.clone(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&svg, &usvg_options).ok()?;
    let (w, h) = (tree.size().width(), tree.size().height());
    if !(w >= 1. && h >= 1. && w <= MAX_SIDE && h <= MAX_SIDE) {
        return None;
    }
    let scale = scale.min(MAX_RASTER / w).min(MAX_RASTER / h);
    let (pw, ph) = ((w * scale).ceil() as u32, (h * scale).ceil() as u32);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pw, ph)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(pw as f32 / w, ph as f32 / h),
        &mut pixmap.as_mut(),
    );
    let mut rgba = pixmap.take();
    for px in rgba.chunks_exact_mut(4) {
        // tiny-skia gives premultiplied RGBA; PNG and GPUI want straight alpha.
        if px[3] > 0 && px[3] < 255 {
            let a = px[3] as f32 / 255.;
            for c in &mut px[..3] {
                *c = (*c as f32 / a).min(255.) as u8;
            }
        }
    }
    let mut png = Vec::new();
    image::RgbaImage::from_raw(pw, ph, rgba.clone())?
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    // GPUI wants BGRA.
    let mut bgra = rgba;
    for px in bgra.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    Some(Raster {
        bgra,
        png,
        pixels: (pw, ph),
        size: (w, h),
    })
}

/// The diagram of a ```` ```mermaid ```` block, or `None` while it is being drawn or when it
/// cannot be (show the code then).
pub(super) fn diagram(
    id: ElementId,
    code: &str,
    s: &Scheme,
    on_open: Option<DiagramHandler>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let theme = diagram_theme(s);
    // Twice the screen's scale: it stays sharp in a zoomed-in viewer.
    let scale = window.scale_factor().max(1.) * 2.;
    let mut hasher = DefaultHasher::new();
    code.hash(&mut hasher);
    format!("{theme:?}").hash(&mut hasher);
    scale.to_bits().hash(&mut hasher);
    let key = hasher.finish();

    let state = DIAGRAMS.with(|c| c.borrow_mut().get(key));
    match state {
        Some(State::Ready(diagram)) => Some(view(id, diagram, s, on_open)),
        Some(State::Pending | State::Failed) => None,
        None => {
            DIAGRAMS.with(|c| c.borrow_mut().insert(key, State::Pending));
            let code = code.to_string();
            let executor = cx.background_executor().clone();
            window
                .spawn(cx, async move |cx| {
                    let raster = executor
                        .spawn(async move {
                            catch_unwind(AssertUnwindSafe(|| rasterize(&code, theme, scale)))
                                .ok()
                                .flatten()
                        })
                        .await;
                    let state = match raster.and_then(|r| {
                        let frame = image::RgbaImage::from_raw(r.pixels.0, r.pixels.1, r.bgra)?;
                        Some((RenderImage::new(vec![image::Frame::new(frame)]), r.png, r.size))
                    }) {
                        Some((image, png, size)) => State::Ready(MermaidDiagram {
                            image: Arc::new(image),
                            png: Arc::new(png),
                            size,
                        }),
                        None => State::Failed,
                    };
                    DIAGRAMS.with(|c| c.borrow_mut().insert(key, state));
                    cx.update(|window, _| window.refresh()).ok();
                })
                .detach();
            None
        }
    }
}

fn view(id: ElementId, diagram: MermaidDiagram, s: &Scheme, on_open: Option<DiagramHandler>) -> AnyElement {
    let (w, _) = diagram.size;
    let group = gpui::SharedString::from(format!("mermaid-{id:?}"));
    let base = ground(s);
    let image = diagram.image.clone();
    let open = on_open.clone().map(|f| {
        let diagram = diagram.clone();
        move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| f(&diagram, window, cx)
    });
    let copy = diagram.clone();
    // Ampliar y copiar: arriba a la derecha, al pasar el puntero.
    let tools = div()
        .absolute()
        .top(px(6.))
        .right(px(6.))
        .flex()
        .gap(px(2.))
        .p(px(2.))
        .rounded(px(10.))
        .bg(base)
        .invisible()
        .group_hover(group.clone(), |d| d.visible())
        .when_some(on_open, |d, f| {
            let diagram = diagram.clone();
            d.child(
                IconButton::new("mermaid-open", "maximize")
                    .variant(IconButtonVariant::Standard)
                    .size(px(28.))
                    .tooltip("Ampliar")
                    .on_click(move |_, window, cx| f(&diagram, window, cx)),
            )
        })
        .child(
            IconButton::new("mermaid-copy", "copy")
                .variant(IconButtonVariant::Standard)
                .size(px(28.))
                .tooltip("Copiar imagen")
                .on_click(move |_, _, cx| copy.copy(cx)),
        );
    div()
        .id(id)
        .group(group)
        .relative()
        .max_w_full()
        .rounded(px(12.))
        .bg(base)
        .child(
            div()
                .id("mermaid-scroll")
                .max_w_full()
                .overflow_x_scroll()
                .p(px(12.))
                .when_some(open, |d, f| d.cursor_pointer().on_click(f))
                // The height follows from the image's proportions.
                .child(
                    img(ImageSource::Render(image))
                        .w(px(w))
                        .max_w_full()
                        .min_w(px(w.min(MIN_WIDTH))),
                ),
        )
        .child(tools)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flowchart_and_sequence_become_pixels() {
        let s = Scheme::baseline_light();
        for code in [
            "flowchart LR\n  A[Inicio] --> B{Listo?}\n  B -->|Sí| C[Fin]\n  B -->|No| A",
            "sequenceDiagram\n  Ana->>Beto: Hola\n  Beto-->>Ana: Chao",
        ] {
            let r = rasterize(code, diagram_theme(&s), 2.).expect(code);
            assert_eq!(r.bgra.len(), (r.pixels.0 * r.pixels.1 * 4) as usize);
            // Not blank: something was drawn.
            assert!(r.bgra.chunks_exact(4).any(|p| p[3] > 0 && p[..3] != r.bgra[..3]));
        }
    }

    #[test]
    fn translucent_surfaces_do_not_turn_white() {
        let mut s = Scheme::baseline_dark();
        // Like Liquid Glass: the surfaces are a white wash over the window.
        s.surface = hsla(0., 0., 1., 0.06);
        s.surface_container = hsla(0., 0., 1., 0.08);
        s.surface_container_high = hsla(0., 0., 1., 0.12);
        let fill = over(s.surface_container_high, ground(&s));
        assert!(fill.a > 0.99);
        assert!(fill.l < 0.4, "{fill:?}");
    }

    #[test]
    fn unreadable_diagrams_fall_back() {
        let s = Scheme::baseline_light();
        assert!(rasterize("esto no es mermaid", diagram_theme(&s), 1.).is_none());
        assert!(rasterize("", diagram_theme(&s), 1.).is_none());
    }
}
