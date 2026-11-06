//! Rich tooltip: a card with an optional subhead, supporting text and text-button actions.

use std::rc::Rc;

use gpui::{
    AnyView, App, AppContext, ClickEvent, Context, ElementId, IntoElement, ParentElement, Render,
    RenderOnce, SharedString, Styled, Window, div, px,
};

use super::overlays::{ground, over};
use super::{Button, ClickHandler};
use crate::theme::type_scale::{BODY_MEDIUM, LABEL_LARGE, TypeScaleExt};
use crate::theme::{Elevation, Theme, font_family};

/// Widest a rich tooltip grows, in dp.
const MAX_WIDTH: f32 = 312.;

/// Rich tooltip (M3): `surface_container`, 12 dp corners, elevation level 2, an optional
/// subhead, supporting text and text-button actions.
///
/// Build it into a GPUI tooltip. A plain tooltip disappears when the pointer leaves its
/// element, so the actions could never be clicked; `hoverable_tooltip` keeps it open while the
/// pointer is over it, and appears at the pointer so it can be reached without crossing a gap:
///
/// ```ignore
/// div().id("help").hoverable_tooltip(
///     RichTooltip::new("help-tip", "Syncs your notes across devices.")
///         .title("Sync")
///         .action("Learn more", |_, _, _| {})
///         .build(),
/// )
/// ```
///
/// A tooltip without actions can use `.tooltip(...)` as well. The tooltip is also an element:
/// draw it in place to show how it looks. `id` must be unique in the view, and
/// [`RichTooltip::build`] keeps it for the actions.
#[derive(Clone, IntoElement)]
pub struct RichTooltip {
    id: ElementId,
    title: Option<SharedString>,
    text: SharedString,
    actions: Vec<(SharedString, ClickHandler)>,
}

impl RichTooltip {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: None,
            text: text.into(),
            actions: Vec::new(),
        }
    }

    /// The subhead above the supporting text.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// A text button below the text (usually one or two).
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.actions.push((label.into(), Rc::new(on_click)));
        self
    }

    /// The tooltip builder for GPUI's `.tooltip(...)` and `.hoverable_tooltip(...)`.
    pub fn build(self) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_, cx| cx.new(|_| RichTooltipView(self.clone())).into()
    }
}

struct RichTooltipView(RichTooltip);

impl Render for RichTooltipView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.clone()
    }
}

impl RenderOnce for RichTooltip {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let bg = if s.surface.a < 1. {
            over(s.surface_container, ground(&s))
        } else {
            s.surface_container
        };
        let has_actions = !self.actions.is_empty();
        let mut card = div()
            .flex()
            .flex_col()
            .max_w(px(MAX_WIDTH))
            .px(px(16.))
            .pt(px(12.))
            .pb(px(if has_actions { 8. } else { 12. }))
            .rounded(px(12.))
            .bg(bg)
            .shadow(Elevation::Level2.shadow(&s))
            .font_family(font_family())
            .text_color(s.on_surface_variant);
        if let Some(title) = self.title {
            card = card.child(
                div()
                    .pb(px(4.))
                    .type_emphasized(LABEL_LARGE)
                    .text_color(s.on_surface_variant)
                    .child(title),
            );
        }
        card = card.child(div().type_style(BODY_MEDIUM).child(self.text));
        if has_actions {
            let mut row = div().flex().gap(px(8.)).pt(px(12.)).ml(px(-12.));
            for (i, (label, on_click)) in self.actions.into_iter().enumerate() {
                let id = ElementId::NamedInteger(format!("{}-action", self.id).into(), i as u64);
                row = row.child(
                    Button::new(id, label)
                        .text()
                        .on_click(move |e, w, cx| on_click(e, w, cx)),
                );
            }
            card = card.child(row);
        }
        card
    }
}
