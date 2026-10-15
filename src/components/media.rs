//! Attachments: a drop target for files and image thumbnails.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, ClickEvent, ElementId, ExternalPaths, FileDropEvent, FontWeight, ImageSource,
    InteractiveElement, IntoElement, ObjectFit, ParentElement, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, StyledImage, Window, canvas, div, img, px,
};

use super::{ClickHandler, Icon};
use crate::motion::{Spring, animate_f32, animate_from, key, presence};
use crate::theme::{FONT_FAMILY, Theme, alpha, mix};

type DropHandler = Rc<dyn Fn(&ExternalPaths, &mut Window, &mut App)>;

/// An area that accepts files dragged from the system (`ExternalPaths`). While files are over
/// it, a "Drop to attach" layer covers it: primary border, tinted background, icon and label.
/// The layer springs in and fades out.
#[derive(IntoElement)]
pub struct DropZone {
    id: ElementId,
    label: SharedString,
    icon: SharedString,
    radius: Pixels,
    on_drop: Option<DropHandler>,
    children: Vec<AnyElement>,
}

impl DropZone {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: "Drop to attach".into(),
            icon: "clip".into(),
            radius: px(26.),
            on_drop: None,
            children: Vec::new(),
        }
    }

    /// Text of the drop layer ("Drop to attach" by default).
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Icon of the drop layer ("clip" by default).
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = icon.into();
        self
    }

    /// Corner radius of the layer; match the area it covers (26 px, the composer, by default).
    pub fn radius(mut self, radius: Pixels) -> Self {
        self.radius = radius;
        self
    }

    /// Receives the dropped paths (`paths.paths()`).
    pub fn on_drop(mut self, f: impl Fn(&ExternalPaths, &mut Window, &mut App) + 'static) -> Self {
        self.on_drop = Some(Rc::new(f));
        self
    }
}

impl ParentElement for DropZone {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for DropZone {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let over_state = window.use_keyed_state(key(&self.id, "over"), cx, |_, _| false);
        // A drag that left the window or ended elsewhere no longer counts.
        let over = *over_state.read(cx) && cx.has_active_drag();
        let set_over = {
            let state = over_state.clone();
            move |value: bool, cx: &mut App| {
                state.update(cx, |over, cx| {
                    if *over != value {
                        *over = value;
                        cx.notify();
                    }
                })
            }
        };

        let layer = presence(key(&self.id, "layer"), over, window, cx).map(|fade| {
            // m3-drop: grows from 96 % (an inset that closes) with the spatial spring.
            let [grow] = animate_from(
                key(&self.id, "grow"),
                Spring::SPATIAL,
                [0.6],
                [1.],
                window,
                cx,
            );
            let inset = px((1. - grow) * 8.);
            div()
                .absolute()
                .top(inset)
                .left(inset)
                .right(inset)
                .bottom(inset)
                .flex()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .rounded(self.radius)
                .border_2()
                .border_color(s.primary)
                .bg(mix(s.primary, s.surface_container, 0.12))
                .text_color(s.primary)
                .font_family(FONT_FAMILY)
                .font_weight(FontWeight(600.))
                .opacity(fade.min(grow.clamp(0., 1.) * 1.6).clamp(0., 1.))
                .child(Icon::new(self.icon.clone()).size(px(18.)).color(s.primary))
                .child(self.label.clone())
        });

        let moved = set_over.clone();
        let hovered = set_over.clone();
        let dropped = set_over.clone();
        let exited = set_over;
        let on_drop = self.on_drop;
        div()
            .id(self.id.clone())
            .relative()
            .on_drag_move::<ExternalPaths>(move |event, _, cx| {
                moved(event.bounds.contains(&event.event.position), cx)
            })
            .on_hover(move |hovering, _, cx| {
                if !*hovering || !cx.has_active_drag() {
                    hovered(false, cx)
                }
            })
            .on_drop::<ExternalPaths>(move |paths, window, cx| {
                dropped(false, cx);
                if let Some(f) = &on_drop {
                    f(paths, window, cx);
                }
            })
            .children(self.children)
            .children(layer)
            .child(
                // Files dragged out of the window: GPUI sends `FileDropEvent::Exited`.
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        let exited = exited.clone();
                        window.on_mouse_event(move |event: &FileDropEvent, _, _, cx| {
                            if matches!(event, FileDropEvent::Exited) {
                                exited(false, cx);
                            }
                        });
                    },
                )
                .absolute()
                .size_0(),
            )
    }
}

/// A rounded image thumbnail: the composer's attachments and the images of a user
/// message. Keeps the image's own aspect ratio (`size` caps the longest side). With
/// [`ImageThumb::on_remove`] a small round "×" appears on hover.
#[derive(IntoElement)]
pub struct ImageThumb {
    id: ElementId,
    source: ImageSource,
    size: Pixels,
    radius: Pixels,
    aspect: Option<(f32, f32)>,
    on_remove: Option<ClickHandler>,
    remove_label: SharedString,
    on_click: Option<ClickHandler>,
}

impl ImageThumb {
    /// `source` is anything GPUI's `img` accepts: a path, a URL, an `Arc<RenderImage>` or an
    /// `Arc<Image>` (for pasted or base64 images).
    pub fn new(id: impl Into<ElementId>, source: impl Into<ImageSource>) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            size: px(56.),
            radius: px(12.),
            aspect: None,
            on_remove: None,
            remove_label: "Remove".into(),
            on_click: None,
        }
    }

    /// Longest side in pixels (56 by default): the other side follows the image.
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// Corner radius (12 px, M3's medium corner, by default).
    pub fn radius(mut self, radius: Pixels) -> Self {
        self.radius = radius;
        self
    }

    /// The image's real size (width, height): the thumbnail keeps its shape — a wide
    /// screenshot comes out landscape, whole. Without it the thumbnail is square.
    pub fn aspect(mut self, width: f32, height: f32) -> Self {
        if width > 0. && height > 0. {
            self.aspect = Some((width, height));
        }
        self
    }

    /// Shows a remove button on hover that calls `f`.
    pub fn on_remove(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_remove = Some(Rc::new(f));
        self
    }

    /// Tooltip of the remove button ("Remove" by default).
    pub fn remove_label(mut self, label: impl Into<SharedString>) -> Self {
        self.remove_label = label.into();
        self
    }

    /// Makes the thumbnail clickable (e.g. to preview the image).
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ImageThumb {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let hovered = window.use_keyed_state(key(&self.id, "hover"), cx, |_, _| false);
        let show_x = animate_f32(
            key(&self.id, "x"),
            Spring::EFFECTS,
            if *hovered.read(cx) { 1. } else { 0. },
            window,
            cx,
        );
        let radius = self.radius;
        let remove = self.on_remove.map(|f| {
            let label = self.remove_label.clone();
            div()
                .id(key(&self.id, "remove"))
                .absolute()
                .top(px(3.))
                .right(px(3.))
                .size(px(18.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(alpha(gpui::black(), 0.6))
                .opacity(show_x.clamp(0., 1.))
                .cursor_pointer()
                .tooltip(super::Tooltip::text(label))
                .child(Icon::new("x").size(px(11.)).color(gpui::white()))
                .on_click(move |e, w, cx| {
                    cx.stop_propagation();
                    f(e, w, cx)
                })
        });
        let hover = hovered.clone();
        // With the real size the box follows the image (`size` is the longest side);
        // without it the box stays square and the image covers it, as before.
        let (box_w, box_h) = match self.aspect {
            Some((w, h)) if h <= w => (self.size, self.size * (h / w)),
            Some((w, h)) => (self.size * (w / h), self.size),
            None => (self.size, self.size),
        };
        div()
            .id(self.id.clone())
            .relative()
            .flex_none()
            .w(box_w)
            .h(box_h)
            .rounded(radius)
            .bg(s.surface_container_highest)
            .on_hover(move |on, _, cx| {
                hover.update(cx, |h, cx| {
                    *h = *on;
                    cx.notify();
                })
            })
            .when_some(self.on_click, |d, f| {
                d.cursor_pointer().on_click(move |e, w, cx| f(e, w, cx))
            })
            .child(
                img(self.source)
                    .size_full()
                    .rounded(radius)
                    .object_fit(if self.aspect.is_some() { ObjectFit::Contain } else { ObjectFit::Cover }),
            )
            .children(remove)
    }
}
