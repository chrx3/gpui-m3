//! Selection marks: radio buttons, checkboxes and choice rows.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
    div, px,
};

use super::{ChangeHandler, ClickHandler, Icon, RoundedExt, all};
use crate::interaction::{interactive, state_layer, use_interaction};
use crate::motion::{Spring, animate_color, animate_f32, key};
use crate::theme::{FONT_FAMILY, Theme, alpha};

/// Radio button (the reference design's model picker): a 2 px outline ring whose border
/// thickens to 5 px in the primary color when selected.
#[derive(IntoElement)]
pub struct Radio {
    id: ElementId,
    selected: bool,
    size: Pixels,
    disabled: bool,
    on_change: Option<ChangeHandler<bool>>,
}

impl Radio {
    pub fn new(id: impl Into<ElementId>, selected: bool) -> Self {
        Self {
            id: id.into(),
            selected,
            size: px(16.),
            disabled: false,
            on_change: None,
        }
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Makes it clickable on its own. Receives `true` (a radio never unselects itself).
    pub fn on_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Radio {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let border = animate_f32(
            key(&self.id, "border"),
            Spring::SPATIAL_FAST,
            if self.selected { 5. } else { 2. },
            window,
            cx,
        );
        let color = if self.disabled {
            alpha(s.on_surface, 0.38)
        } else if self.selected {
            s.primary
        } else {
            s.outline
        };
        let color = animate_color(key(&self.id, "color"), Spring::EFFECTS, color, window, cx);
        let size = f32::from(self.size);
        let ring = div()
            .id(self.id.clone())
            .flex_none()
            .size(self.size)
            .rounded(px(size / 2.))
            .border(px(border.clamp(1., size / 2.)))
            .border_color(color);
        let Some(f) = self.on_change.filter(|_| !self.disabled) else {
            return ring;
        };
        let state = use_interaction(&self.id, window, cx);
        interactive(ring, &state, true, false).on_click(move |_, w, cx| f(true, w, cx))
    }
}

/// M3 checkbox: an 18 px box that fills with the primary color and shows a check.
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    disabled: bool,
    on_change: Option<ChangeHandler<bool>>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            disabled: false,
            on_change: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Receives the new state.
    pub fn on_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let (bg, border, fg) = match (self.disabled, self.checked) {
            (true, true) => (
                alpha(s.on_surface, 0.38),
                alpha(s.on_surface, 0.),
                s.surface,
            ),
            (true, false) => (
                alpha(s.on_surface, 0.),
                alpha(s.on_surface, 0.38),
                s.surface,
            ),
            (false, true) => (s.primary, alpha(s.primary, 0.), s.on_primary),
            (false, false) => (alpha(s.primary, 0.), s.on_surface_variant, s.on_primary),
        };
        let bg = animate_color(key(&self.id, "bg"), Spring::EFFECTS, bg, window, cx);
        let border = animate_color(key(&self.id, "border"), Spring::EFFECTS, border, window, cx);
        let mark = animate_f32(
            key(&self.id, "mark"),
            Spring::SPATIAL_FAST,
            if self.checked { 1. } else { 0. },
            window,
            cx,
        );
        let state = use_interaction(&self.id, window, cx);
        let el = div()
            .id(self.id.clone())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(18.))
            .rounded(px(3.))
            .bg(bg)
            .border_2()
            .border_color(border)
            .child(
                Icon::new("check")
                    .size(px(14.))
                    .color(alpha(fg, mark.clamp(0., 1.)))
                    .scale(0.4 + 0.6 * mark.max(0.)),
            );
        let enabled = !self.disabled && self.on_change.is_some();
        let mut el = interactive(el, &state, enabled, false);
        if let (Some(f), true) = (self.on_change, enabled) {
            let checked = self.checked;
            el = el.on_click(move |_, w, cx| f(!checked, w, cx));
        }
        el
    }
}

/// Selection mark of a [`ChoiceRow`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChoiceMark {
    Radio,
    Checkbox,
}

/// Full-width selectable option (AskUserQuestion answers): a 14 px mark, a label and an optional
/// description. Selected rows fill with the primary color; corners tighten to 8 px on press.
#[derive(IntoElement)]
pub struct ChoiceRow {
    id: ElementId,
    label: SharedString,
    description: Option<SharedString>,
    mark: ChoiceMark,
    selected: bool,
    disabled: bool,
    trailing: Option<AnyElement>,
    on_click: Option<ClickHandler>,
}

impl ChoiceRow {
    /// Single-choice option.
    pub fn radio(id: impl Into<ElementId>, label: impl Into<SharedString>, selected: bool) -> Self {
        Self::new(id, label, selected, ChoiceMark::Radio)
    }

    /// Multiple-choice option.
    pub fn checkbox(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        checked: bool,
    ) -> Self {
        Self::new(id, label, checked, ChoiceMark::Checkbox)
    }

    fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        selected: bool,
        mark: ChoiceMark,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            mark,
            selected,
            disabled: false,
            trailing: None,
            on_click: None,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Content after the label, filling the row (e.g. the "Other…" [`super::TextField`]).
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing = Some(el.into_any_element());
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ChoiceRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let enabled = !self.disabled;
        let state = use_interaction(&self.id, window, cx);
        let pressed = state.read(cx).pressed && enabled;
        let radius = px(animate_f32(
            key(&self.id, "radius"),
            Spring::SPATIAL_FAST,
            if pressed { 8. } else { 16. },
            window,
            cx,
        ));
        let (bg, fg) = if self.selected {
            (s.primary, s.on_primary)
        } else {
            (s.surface, s.on_surface)
        };
        let bg = animate_color(key(&self.id, "bg"), Spring::EFFECTS, bg, window, cx);
        let fg = animate_color(key(&self.id, "fg"), Spring::EFFECTS, fg, window, cx);
        let layer = state_layer(&self.id, &state, all(radius), fg, window, cx);

        let mark_radius = match self.mark {
            ChoiceMark::Radio => px(7.),
            ChoiceMark::Checkbox => px(3.),
        };
        let mark = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(14.))
            .rounded(mark_radius);
        let mark = match (self.mark, self.selected) {
            (_, false) => mark.border(px(1.5)).border_color(s.on_surface_variant),
            // Filled with the content color and a 2.5 px inner ring of the row color.
            (ChoiceMark::Radio, true) => mark
                .bg(s.on_primary)
                .border(px(2.5))
                .border_color(s.primary),
            (ChoiceMark::Checkbox, true) => mark
                .bg(s.on_primary)
                .child(Icon::new("check").size(px(12.)).color(s.primary)),
        };
        let mut text = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(2.))
            .child(div().font_weight(FontWeight(500.)).child(self.label));
        if let Some(description) = self.description {
            text = text.child(
                div()
                    .text_size(px(12.))
                    .text_color(if self.selected {
                        alpha(fg, 0.8)
                    } else {
                        s.on_surface_variant
                    })
                    .child(description),
            );
        }
        let row = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(10.))
            .w_full()
            .px(px(14.))
            .py(px(10.))
            .corners(all(radius))
            .bg(bg)
            .text_color(fg)
            .font_family(FONT_FAMILY)
            .text_size(px(13.5))
            .when(!enabled, |d| d.opacity(0.5))
            .child(layer)
            .child(mark)
            .child(text)
            .children(self.trailing);
        let mut row = interactive(row, &state, enabled, true);
        if let (Some(f), true) = (self.on_click, enabled) {
            row = row.on_click(move |e, w, cx| f(e, w, cx));
        }
        row
    }
}
