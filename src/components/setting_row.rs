//! A settings row: title and description on the left, the control on the right.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use super::{ClickHandler, Icon, all};
use crate::interaction::{interactive, state_layer, use_interaction};
use crate::theme::{FONT_FAMILY, Theme};

/// A row of a settings list: an optional icon, a title, a description under it and a control
/// (a [`super::Switch`], a [`super::KeyCombo`], a button…) aligned to the right.
///
/// It has no background of its own: put it in [`super::ListGroup::custom_row`]. With `on_click`
/// the whole row is a button (hover layer and ripple that bleed 10 px past the text).
#[derive(IntoElement)]
pub struct SettingRow {
    id: ElementId,
    title: SharedString,
    description: Option<SharedString>,
    icon: Option<SharedString>,
    control: Option<AnyElement>,
    muted: bool,
    on_click: Option<ClickHandler>,
}

impl SettingRow {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            icon: None,
            control: None,
            muted: false,
            on_click: None,
        }
    }

    /// Smaller text under the title (wraps onto several lines).
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// The control at the end of the row.
    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.control = Some(control.into_any_element());
        self
    }

    /// Quieter title: a setting that cannot be changed.
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for SettingRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let clickable = self.on_click.is_some();
        let state = clickable.then(|| use_interaction(&self.id, window, cx));
        let layer = state
            .as_ref()
            .map(|state| state_layer(&self.id, state, all(px(14.)), s.on_surface, window, cx));
        let mut text = div().flex().flex_col().flex_1().min_w_0().gap(px(1.)).child(
            div()
                .font_weight(FontWeight(500.))
                .text_color(if self.muted {
                    s.on_surface_variant
                } else {
                    s.on_surface
                })
                .child(self.title),
        );
        if let Some(description) = self.description {
            text = text.child(
                div()
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .text_color(s.outline)
                    .child(description),
            );
        }
        let mut row = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(14.))
            .w_full()
            .font_family(FONT_FAMILY)
            .text_size(px(13.5))
            .children(layer);
        if clickable {
            row = row.mx(px(-10.)).px(px(10.)).py(px(4.)).rounded(px(14.));
        }
        if let Some(icon) = self.icon {
            row = row.child(
                div()
                    .flex_none()
                    .child(Icon::new(icon).size(px(18.)).color(s.on_surface_variant)),
            );
        }
        row = row.child(text);
        if let Some(control) = self.control {
            row = row.child(div().flex().flex_none().items_center().gap(px(6.)).child(control));
        }
        match (state, self.on_click) {
            (Some(state), Some(f)) => interactive(row, &state, true, true)
                .on_click(move |e, w, cx| f(e, w, cx))
                .into_any_element(),
            _ => row.into_any_element(),
        }
    }
}
