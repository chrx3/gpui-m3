//! Overlays and feedback: dialog, toast, context menu, tooltip and linear progress.

use std::rc::Rc;

use gpui::{
    AnyElement, AnyView, App, AppContext, ClickEvent, Context, ElementId, FontWeight,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point, Render, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, anchored, deferred, div, px,
};

use gpui::prelude::FluentBuilder as _;

use super::{ClickHandler, RoundedExt, all, pop_in};
use crate::motion::{Spring, animate_f32, entrance, key};
use crate::theme::{FONT_FAMILY, Theme, alpha};

/// Modal dialog: a scrim over the whole window and a 28 px card that rises in with the
/// spatial spring. Render it anywhere while it is open: it covers the whole window.
#[derive(IntoElement)]
pub struct Dialog {
    id: ElementId,
    title: Option<SharedString>,
    icon: Option<SharedString>,
    width: Pixels,
    children: Vec<AnyElement>,
    actions: Vec<AnyElement>,
    on_dismiss: Option<ClickHandler>,
}

impl Dialog {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            title: None,
            icon: None,
            width: px(400.),
            children: Vec::new(),
            actions: Vec::new(),
            on_dismiss: None,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Icon above the title (M3 "hero icon").
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    /// A button in the actions row (end-aligned).
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// Called when the scrim is clicked.
    pub fn on_dismiss(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }
}

impl ParentElement for Dialog {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Dialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let fade = entrance(key(&self.id, "scrim"), Spring::EFFECTS, 0., window, cx).clamp(0., 1.);
        let mut card = div()
            .id(key(&self.id, "card"))
            .flex()
            .flex_col()
            .gap(px(16.))
            .w(self.width)
            .p(px(24.))
            .rounded(px(28.))
            .bg(s.surface_container_high)
            .text_color(s.on_surface_variant)
            .text_size(px(13.5))
            .shadow(super::shadow_pop(&s))
            // Clicks inside the card must not reach the scrim.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation());
        let centered = self.icon.is_some();
        if let Some(icon) = self.icon {
            card = card.child(
                div()
                    .flex()
                    .justify_center()
                    .child(super::Icon::new(icon).size(px(24.)).color(s.primary)),
            );
        }
        if let Some(title) = self.title {
            card = card.child(
                div()
                    .when(centered, |d| d.text_center())
                    .text_size(px(22.))
                    .font_weight(FontWeight(600.))
                    .text_color(s.on_surface)
                    .child(title),
            );
        }
        card = card.children(self.children);
        if !self.actions.is_empty() {
            card = card.child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .pt(px(8.))
                    .children(self.actions),
            );
        }
        let card = pop_in(div().child(card), &self.id, 0., window, cx);

        // Covers the whole window even when the dialog is drawn inside a panel.
        let viewport = window.viewport_size();
        let mut scrim = div()
            .id(self.id.clone())
            .w(viewport.width)
            .h(viewport.height)
            .flex()
            .items_center()
            .justify_center()
            .bg(alpha(gpui::black(), 0.32 * fade))
            .font_family(FONT_FAMILY)
            .occlude()
            .child(card);
        if let Some(f) = self.on_dismiss {
            scrim = scrim.on_click(move |e, w, cx| f(e, w, cx));
        }
        deferred(
            anchored()
                .position(gpui::point(px(0.), px(0.)))
                .child(scrim),
        )
        .with_priority(1)
    }
}

/// Snackbar-style toast on an inverse surface, with an optional text action.
/// Place it where it should appear (e.g. absolutely at the bottom of the window).
#[derive(IntoElement)]
pub struct Toast {
    id: ElementId,
    text: SharedString,
    action: Option<(SharedString, ClickHandler)>,
}

impl Toast {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            action: None,
        }
    }

    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(f)));
        self
    }
}

impl RenderOnce for Toast {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        // Inverse surface: the content color of the scheme used as background.
        let mut el = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .min_h(px(48.))
            .pl(px(16.))
            .pr(px(if self.action.is_some() { 8. } else { 16. }))
            .rounded(px(14.))
            .bg(s.on_surface)
            .text_color(s.surface)
            .font_family(FONT_FAMILY)
            .text_size(px(13.5))
            .shadow(super::shadow_pop(&s))
            .child(div().flex_1().child(self.text));
        if let Some((label, f)) = self.action {
            el = el.child(
                div()
                    .id(key(&self.id, "action"))
                    .px(px(12.))
                    .h(px(36.))
                    .flex()
                    .items_center()
                    .rounded(px(18.))
                    .cursor_pointer()
                    .font_weight(FontWeight(600.))
                    .text_color(s.primary_container)
                    .hover(|d| d.bg(alpha(s.surface, 0.1)))
                    .on_click(move |e, w, cx| f(e, w, cx))
                    .child(label),
            );
        }
        pop_in(el, &self.id, 0., window, cx)
    }
}

/// Places `content` (usually a [`super::Menu`]) at a window position, on top of everything
/// and kept inside the window. Use it for right-click menus.
pub fn context_menu(position: Point<Pixels>, content: impl IntoElement) -> impl IntoElement {
    deferred(
        anchored()
            .position(position)
            .snap_to_window_with_margin(px(8.))
            .child(content),
    )
    .with_priority(1)
}

/// Plain tooltip (M3): inverse surface, small corners. Use with GPUI's `.tooltip(...)`:
///
/// ```ignore
/// div().id("copy").tooltip(Tooltip::text("Copy"))
/// ```
pub struct Tooltip {
    text: SharedString,
}

impl Tooltip {
    pub fn text(
        text: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let text = text.into();
        move |_, cx| cx.new(|_| Tooltip { text: text.clone() }).into()
    }
}

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(4.))
            .bg(s.on_surface)
            .text_color(s.surface)
            .font_family(FONT_FAMILY)
            .text_size(px(12.))
            .child(self.text.clone())
    }
}

/// Linear progress meter (the reference design's usage meter): a rounded track with a spring-animated fill.
#[derive(IntoElement)]
pub struct LinearProgress {
    id: ElementId,
    value: f32,
    height: Pixels,
}

impl LinearProgress {
    pub fn new(id: impl Into<ElementId>, value: f32) -> Self {
        Self {
            id: id.into(),
            value,
            height: px(8.),
        }
    }

    pub fn height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }
}

impl RenderOnce for LinearProgress {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let value = animate_f32(
            key(&self.id, "value"),
            Spring::SPATIAL,
            self.value.clamp(0., 1.),
            window,
            cx,
        );
        let radius = self.height / 2.;
        div()
            .w_full()
            .h(self.height)
            .corners(all(radius))
            .bg(s.secondary_container)
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .w(gpui::relative(value.clamp(0., 1.)))
                    .corners(all(radius))
                    .bg(s.primary),
            )
    }
}
