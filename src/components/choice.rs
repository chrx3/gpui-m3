//! Selection marks: radio buttons, checkboxes and choice rows.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
    div, px,
};

use super::{ChangeHandler, ClickHandler, Icon, RoundedExt, all};
use crate::interaction::{interactive, keyboard_focus, overlay, state_layer, use_interaction};
use crate::motion::{Spring, animate_color, animate_f32, key};
use crate::theme::{Scheme, Theme, alpha, font_family};

/// Radio button (the reference design's model picker): a 2 px outline ring whose border
/// thickens to 5 px in the primary color when selected.
#[derive(IntoElement)]
pub struct Radio {
    id: ElementId,
    selected: bool,
    size: Pixels,
    disabled: bool,
    error: bool,
    on_change: Option<ChangeHandler<bool>>,
}

impl Radio {
    pub fn new(id: impl Into<ElementId>, selected: bool) -> Self {
        Self {
            id: id.into(),
            selected,
            size: px(16.),
            disabled: false,
            error: false,
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

    /// Validation error: the ring takes the `error` color.
    pub fn error(mut self, error: bool) -> Self {
        self.error = error;
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
        } else if self.error {
            s.error
        } else if self.selected {
            s.primary
        } else {
            s.outline
        };
        let color = animate_color(key(&self.id, "color"), Spring::EFFECTS, color, window, cx);
        let size = f32::from(self.size);
        // The border-less wrapper is what the focus ring is measured from.
        let ring = div().id(self.id.clone()).flex_none().size(self.size).child(
            div()
                .size_full()
                .rounded(px(size / 2.))
                .border(px(border.clamp(1., size / 2.)))
                .border_color(color),
        );
        let Some(f) = self.on_change.filter(|_| !self.disabled) else {
            return ring;
        };
        let state = use_interaction(&self.id, window, cx);
        let ring = interactive(ring, &state, true, false);
        keyboard_focus(ring, &self.id, all(px(size / 2.)), false, window, cx)
            .on_click(move |_, w, cx| f(true, w, cx))
    }
}

/// State of a [`Checkbox`]: besides checked and not, the "some of the children" state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckState {
    #[default]
    Unchecked,
    Checked,
    /// Drawn as a dash: what a parent box shows when only some of its children are checked.
    Indeterminate,
}

impl CheckState {
    /// The state after a click: a checked box clears, any other one (also the indeterminate
    /// one) becomes checked.
    pub fn toggled(self) -> Self {
        match self {
            Self::Checked => Self::Unchecked,
            Self::Unchecked | Self::Indeterminate => Self::Checked,
        }
    }

    pub fn is_checked(self) -> bool {
        self == Self::Checked
    }

    /// State of a parent box given how many of its `total` children are checked.
    pub fn from_count(checked: usize, total: usize) -> Self {
        match checked {
            0 => Self::Unchecked,
            n if n >= total => Self::Checked,
            _ => Self::Indeterminate,
        }
    }
}

impl From<bool> for CheckState {
    fn from(checked: bool) -> Self {
        if checked {
            Self::Checked
        } else {
            Self::Unchecked
        }
    }
}

/// M3 checkbox: an 18 px box that fills with the primary color and shows a check, or a dash
/// when it is [indeterminate](Checkbox::indeterminate).
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    state: CheckState,
    disabled: bool,
    error: bool,
    on_change: Option<ChangeHandler<bool>>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            id: id.into(),
            state: checked.into(),
            disabled: false,
            error: false,
            on_change: None,
        }
    }

    /// Sets the full state (checked, not or indeterminate).
    pub fn state(mut self, state: CheckState) -> Self {
        self.state = state;
        self
    }

    /// Shows a dash instead of the check. A click turns it into a checked box
    /// (`on_change` receives `true`).
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        if indeterminate {
            self.state = CheckState::Indeterminate;
        } else if self.state == CheckState::Indeterminate {
            self.state = CheckState::Unchecked;
        }
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Validation error: the border (and the fill when marked) take the `error` color.
    pub fn error(mut self, error: bool) -> Self {
        self.error = error;
        self
    }

    /// Receives the new state: `true` when the click checks it, `false` when it clears it.
    pub fn on_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

/// Container, outline and mark colors of a checkbox.
fn checkbox_colors(
    s: &Scheme,
    marked: bool,
    error: bool,
    disabled: bool,
) -> (gpui::Hsla, gpui::Hsla, gpui::Hsla) {
    match (disabled, marked, error) {
        (true, true, _) => (
            alpha(s.on_surface, 0.38),
            alpha(s.on_surface, 0.),
            s.surface,
        ),
        (true, false, _) => (
            alpha(s.on_surface, 0.),
            alpha(s.on_surface, 0.38),
            s.surface,
        ),
        (false, true, false) => (s.primary, alpha(s.primary, 0.), s.on_primary),
        (false, false, false) => (alpha(s.primary, 0.), s.on_surface_variant, s.on_primary),
        (false, true, true) => (s.error, alpha(s.error, 0.), s.on_error),
        (false, false, true) => (alpha(s.error, 0.), s.error, s.on_error),
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let (bg, border, fg) = checkbox_colors(
            &s,
            self.state != CheckState::Unchecked,
            self.error,
            self.disabled,
        );
        let bg = animate_color(key(&self.id, "bg"), Spring::EFFECTS, bg, window, cx);
        let border = animate_color(key(&self.id, "border"), Spring::EFFECTS, border, window, cx);
        let mark = animate_f32(
            key(&self.id, "mark"),
            Spring::SPATIAL_FAST,
            if self.state == CheckState::Checked {
                1.
            } else {
                0.
            },
            window,
            cx,
        );
        let dash = animate_f32(
            key(&self.id, "dash"),
            Spring::SPATIAL_FAST,
            if self.state == CheckState::Indeterminate {
                1.
            } else {
                0.
            },
            window,
            cx,
        );
        let state = use_interaction(&self.id, window, cx);
        // The border-less wrapper is what the focus ring is measured from.
        let el = div().id(self.id.clone()).flex_none().size(px(18.)).child(
            div()
                .size_full()
                .rounded(px(3.))
                .bg(bg)
                .border_2()
                .border_color(border)
                .child(
                    overlay().flex().items_center().justify_center().child(
                        Icon::new("check")
                            .size(px(14.))
                            .color(alpha(fg, mark.clamp(0., 1.)))
                            .scale(0.4 + 0.6 * mark.max(0.)),
                    ),
                )
                .child(
                    overlay().flex().items_center().justify_center().child(
                        div()
                            .w(px(10. * (0.4 + 0.6 * dash.max(0.))))
                            .h(px(2.))
                            .rounded(px(1.))
                            .bg(alpha(fg, dash.clamp(0., 1.))),
                    ),
                ),
        );
        let enabled = !self.disabled && self.on_change.is_some();
        let mut el = interactive(el, &state, enabled, false);
        if let (Some(f), true) = (self.on_change, enabled) {
            let next = self.state.toggled().is_checked();
            el = keyboard_focus(el, &self.id, all(px(3.)), false, window, cx);
            el = el.on_click(move |_, w, cx| f(next, w, cx));
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
    indeterminate: bool,
    error: bool,
    disabled: bool,
    autofocus: bool,
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
            indeterminate: false,
            error: false,
            disabled: false,
            autofocus: false,
            trailing: None,
            on_click: None,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// A checkbox row shows a dash and the filled look, like a checked one (a parent whose
    /// children are partly checked). Radio rows ignore it.
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.indeterminate = indeterminate;
        self
    }

    /// Validation error: the mark's outline (and the fill when selected) take the `error` color.
    pub fn error(mut self, error: bool) -> Self {
        self.error = error;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Takes the keyboard focus the first time it appears (the first option of a question).
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
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
        let dash = self.indeterminate && self.mark == ChoiceMark::Checkbox;
        let filled = self.selected || dash;
        let (accent, on_accent) = if self.error {
            (s.error, s.on_error)
        } else {
            (s.primary, s.on_primary)
        };
        let (bg, fg) = if filled {
            (accent, on_accent)
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
            (ChoiceMark::Checkbox, _) if dash => mark
                .bg(on_accent)
                .child(div().w(px(8.)).h(px(2.)).rounded(px(1.)).bg(accent)),
            (_, false) => mark.border(px(1.5)).border_color(if self.error {
                s.error
            } else {
                s.on_surface_variant
            }),
            // Filled with the content color and a 2.5 px inner ring of the row color.
            (ChoiceMark::Radio, true) => mark.bg(on_accent).border(px(2.5)).border_color(accent),
            (ChoiceMark::Checkbox, true) => mark
                .bg(on_accent)
                .child(Icon::new("check").size(px(12.)).color(accent)),
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
                    .text_color(if filled {
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
            .font_family(font_family())
            .text_size(px(13.5))
            .when(!enabled, |d| d.opacity(0.5))
            .child(layer)
            .child(mark)
            .child(text)
            .children(self.trailing);
        let mut row = interactive(row, &state, enabled, true);
        if enabled {
            row = keyboard_focus(row, &self.id, all(radius), self.autofocus, window, cx);
        }
        if let (Some(f), true) = (self.on_click, enabled) {
            row = row.on_click(move |e, w, cx| f(e, w, cx));
        }
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_checks_unchecked_and_indeterminate_and_clears_checked() {
        assert_eq!(CheckState::Unchecked.toggled(), CheckState::Checked);
        assert_eq!(CheckState::Indeterminate.toggled(), CheckState::Checked);
        assert_eq!(CheckState::Checked.toggled(), CheckState::Unchecked);
    }

    #[test]
    fn indeterminate_builder_overrides_and_restores() {
        let boxed = Checkbox::new("c", true).indeterminate(true);
        assert_eq!(boxed.state, CheckState::Indeterminate);
        assert_eq!(boxed.indeterminate(false).state, CheckState::Unchecked);
        let boxed = Checkbox::new("c", true).indeterminate(false);
        assert_eq!(boxed.state, CheckState::Checked);
    }

    #[test]
    fn parent_state_follows_its_children() {
        assert_eq!(CheckState::from_count(0, 3), CheckState::Unchecked);
        assert_eq!(CheckState::from_count(2, 3), CheckState::Indeterminate);
        assert_eq!(CheckState::from_count(3, 3), CheckState::Checked);
        assert_eq!(CheckState::from_count(0, 0), CheckState::Unchecked);
    }
}
