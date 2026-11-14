//! Button groups (M3 Expressive): standard and connected.
//!
//! A standard group keeps its buttons apart and, while one is pressed, widens it and
//! narrows its neighbours with the spatial spring. A connected group keeps them 2 px apart,
//! rounds the outer corners of the group and springs the selected button into a pill.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;

use gpui::{
    App, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use super::button::{ButtonVariant, IconButtonWidth, M3ButtonSize};
use super::{ChangeHandler, Icon, RoundedExt, corners};
use crate::interaction::{interactive, keyboard_focus, state_layer, use_interaction};
use crate::motion::{Spring, animate, animate_color, animate_f32, key};
use crate::theme::{Scheme, Theme, alpha, font_family};

/// Corner radius of the sides where two connected buttons meet.
const INNER_CORNER: f32 = 4.;

/// Gap between the buttons of a connected group.
const CONNECTED_GAP: f32 = 2.;

/// How much wider a pressed button of a standard group gets, as a share of its width.
const PRESS_GROW: f32 = 0.15;

/// The most a neighbour of a pressed button gives up, as a share of its width.
const MAX_SHRINK: f32 = 0.35;

/// A button of a [`ButtonGroup`]: a label, an icon or both.
#[derive(Clone)]
pub struct ButtonGroupItem {
    label: Option<SharedString>,
    icon: Option<SharedString>,
    tooltip: Option<SharedString>,
    disabled: bool,
    variant: Option<ButtonVariant>,
    autofocus: bool,
}

impl ButtonGroupItem {
    pub fn label(label: impl Into<SharedString>) -> Self {
        Self {
            label: Some(label.into()),
            icon: None,
            tooltip: None,
            disabled: false,
            variant: None,
            autofocus: false,
        }
    }

    pub fn icon(icon: impl Into<SharedString>) -> Self {
        Self {
            label: None,
            icon: Some(icon.into()),
            tooltip: None,
            disabled: false,
            variant: None,
            autofocus: false,
        }
    }

    /// Adds an icon before the label.
    pub fn with_icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Tooltip shown on hover (an icon-only button should have one).
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Overrides the group's [`ButtonGroup::variant`] for this button (a filled primary action
    /// among tonal ones).
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = Some(variant);
        self
    }

    /// Takes the keyboard focus the first time the group appears, never again after that.
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }
}

enum Selection {
    None,
    Single(usize),
    Multi(Vec<bool>),
}

/// Row of related buttons (M3 Expressive).
///
/// Standard by default; [`ButtonGroup::connected`] joins the buttons. Without a selection
/// the buttons are plain actions (see [`ButtonGroup::on_click`]); [`ButtonGroup::selected`]
/// and [`ButtonGroup::selected_many`] make them toggles.
#[derive(IntoElement)]
pub struct ButtonGroup {
    id: ElementId,
    items: Vec<ButtonGroupItem>,
    connected: bool,
    variant: ButtonVariant,
    size: M3ButtonSize,
    selection: Selection,
    on_click: Option<ChangeHandler<usize>>,
    on_change: Option<ChangeHandler<usize>>,
    on_change_many: Option<ChangeHandler<Vec<bool>>>,
}

/// What a standard group remembers between frames: each button's width at rest.
#[derive(Default)]
struct Slots {
    natural: RefCell<Vec<f32>>,
}

impl ButtonGroup {
    pub fn new(id: impl Into<ElementId>, items: impl IntoIterator<Item = ButtonGroupItem>) -> Self {
        Self {
            id: id.into(),
            items: items.into_iter().collect(),
            connected: false,
            variant: ButtonVariant::Tonal,
            size: M3ButtonSize::S,
            selection: Selection::None,
            on_click: None,
            on_change: None,
            on_change_many: None,
        }
    }

    /// Buttons that touch, with round outer corners and 4 px inner ones.
    pub fn connected(mut self) -> Self {
        self.connected = true;
        self
    }

    /// Filled, tonal, outlined or text (the other variants of [`ButtonVariant`] are drawn as tonal).
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn filled(self) -> Self {
        self.variant(ButtonVariant::Filled)
    }

    pub fn tonal(self) -> Self {
        self.variant(ButtonVariant::Tonal)
    }

    pub fn outlined(self) -> Self {
        self.variant(ButtonVariant::Outlined)
    }

    pub fn text(self) -> Self {
        self.variant(ButtonVariant::Text)
    }

    pub fn size(mut self, size: M3ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Single selection: the button at `index` is on.
    pub fn selected(mut self, index: usize) -> Self {
        self.selection = Selection::Single(index);
        self
    }

    /// Multiple selection: one flag per button.
    pub fn selected_many(mut self, selected: impl IntoIterator<Item = bool>) -> Self {
        self.selection = Selection::Multi(selected.into_iter().collect());
        self
    }

    /// Receives the index of a clicked button when the group has no selection.
    pub fn on_click(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }

    /// Receives the index to select (single selection).
    pub fn on_change(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    /// Receives the new flags with the clicked button toggled (multiple selection).
    pub fn on_change_many(
        mut self,
        f: impl Fn(Vec<bool>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change_many = Some(Rc::new(f));
        self
    }
}

/// Gap between the buttons: 2 px connected, 8 px standard and 12 px for the larger sizes.
fn group_gap(connected: bool, size: M3ButtonSize) -> f32 {
    match (connected, size) {
        (true, _) => CONNECTED_GAP,
        (false, M3ButtonSize::L | M3ButtonSize::Xl) => 12.,
        (false, _) => 8.,
    }
}

/// Corner radii [top left, top right, bottom right, bottom left] of button `index` of a
/// connected group: round at the group's ends, `INNER_CORNER` between buttons, a pill when
/// selected, and tighter outer corners while pressed.
fn connected_corners(
    index: usize,
    count: usize,
    selected: bool,
    pressed: bool,
    height: f32,
    pressed_radius: f32,
) -> [f32; 4] {
    let full = height / 2.;
    if selected && !pressed {
        return [full; 4];
    }
    let outer = if pressed {
        pressed_radius.min(full)
    } else {
        full
    };
    let start = if index == 0 { outer } else { INNER_CORNER };
    let end = if index + 1 == count {
        outer
    } else {
        INNER_CORNER
    };
    [start, end, end, start]
}

/// Corner radius of a button of a standard group: a pill, square when selected and tighter
/// while pressed.
fn standard_radius(
    selected: bool,
    pressed: bool,
    height: f32,
    square: f32,
    pressed_radius: f32,
) -> f32 {
    if pressed {
        pressed_radius
    } else if selected {
        square
    } else {
        height / 2.
    }
}

/// Change of width of each button of a standard group while `pressed` is held: it grows and
/// its neighbours give up the same amount between them, so the group keeps its width.
fn press_deltas(widths: &[f32], pressed: Option<usize>) -> Vec<f32> {
    let mut deltas = vec![0.; widths.len()];
    let Some(pressed) = pressed.filter(|p| *p < widths.len()) else {
        return deltas;
    };
    let neighbours: Vec<usize> = [pressed.checked_sub(1), pressed.checked_add(1)]
        .into_iter()
        .flatten()
        .filter(|n| *n < widths.len())
        .collect();
    if neighbours.is_empty() {
        return deltas;
    }
    let share = widths[pressed] * PRESS_GROW / neighbours.len() as f32;
    for n in neighbours {
        let shrink = share.min(widths[n] * MAX_SHRINK);
        deltas[n] = -shrink;
        deltas[pressed] += shrink;
    }
    deltas
}

/// The flags with button `index` flipped.
fn toggled(selected: &[bool], index: usize) -> Vec<bool> {
    let mut next = selected.to_vec();
    if let Some(flag) = next.get_mut(index) {
        *flag = !*flag;
    }
    next
}

/// Container, content and outline colors of a group button. `selected` is `None` for a plain
/// action button.
fn item_colors(
    s: &Scheme,
    variant: ButtonVariant,
    selected: Option<bool>,
    enabled: bool,
) -> (Hsla, Hsla, Option<Hsla>) {
    let outlined = variant == ButtonVariant::Outlined;
    if !enabled {
        let bg = match variant {
            ButtonVariant::Text => alpha(s.on_surface, 0.),
            ButtonVariant::Outlined => alpha(s.on_surface, 0.),
            _ => alpha(s.on_surface, 0.10),
        };
        return (
            bg,
            alpha(s.on_surface, 0.38),
            outlined.then(|| alpha(s.on_surface, 0.12)),
        );
    }
    let on = selected == Some(true);
    match variant {
        ButtonVariant::Filled if on || selected.is_none() => (s.primary, s.on_primary, None),
        ButtonVariant::Filled => (s.surface_container_high, s.on_surface_variant, None),
        ButtonVariant::Outlined if on => (
            s.inverse_surface,
            s.inverse_on_surface,
            Some(s.inverse_surface),
        ),
        ButtonVariant::Outlined => (
            alpha(s.inverse_surface, 0.),
            s.on_surface_variant,
            Some(s.outline_variant),
        ),
        ButtonVariant::Text if on => (s.secondary_container, s.on_secondary_container, None),
        ButtonVariant::Text => (alpha(s.secondary_container, 0.), s.primary, None),
        _ if on => (s.secondary, s.on_secondary, None),
        _ => (s.secondary_container, s.on_secondary_container, None),
    }
}

impl RenderOnce for ButtonGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let m = self.size.metrics();
        let count = self.items.len();
        let ids: Vec<ElementId> = (0..count)
            .map(|i| ElementId::NamedInteger(format!("{}-item", self.id).into(), i as u64))
            .collect();
        let states: Vec<_> = ids
            .iter()
            .map(|id| use_interaction(id, window, cx))
            .collect();
        let pressed: Vec<bool> = states
            .iter()
            .zip(&self.items)
            .map(|(state, item)| state.read(cx).pressed && !item.disabled)
            .collect();
        let selection: Vec<Option<bool>> = (0..count)
            .map(|i| match &self.selection {
                Selection::None => None,
                Selection::Single(selected) => Some(*selected == i),
                Selection::Multi(flags) => Some(flags.get(i).copied().unwrap_or(false)),
            })
            .collect();

        // Standard group: widths follow the pressed button. Each button's width at rest is
        // measured while nothing moves and used as the base of the spring.
        let slots = (!self.connected).then(|| {
            window
                .use_keyed_state(key(&self.id, "slots"), cx, |_, _| Rc::new(Slots::default()))
                .read(cx)
                .clone()
        });
        let natural: Vec<f32> = slots
            .as_ref()
            .map(|slots| slots.natural.borrow().clone())
            .filter(|natural| natural.len() == count)
            .unwrap_or_default();
        let targets = if natural.is_empty() {
            vec![0.; count]
        } else {
            press_deltas(&natural, pressed.iter().position(|p| *p))
        };
        let deltas: Vec<f32> = if slots.is_some() {
            ids.iter()
                .zip(&targets)
                .map(|(id, target)| {
                    animate_f32(key(id, "width"), Spring::SPATIAL, *target, window, cx)
                })
                .collect()
        } else {
            vec![0.; count]
        };
        // Springs return their exact target once settled.
        let at_rest = deltas.iter().all(|d| *d == 0.);

        let (icon, padding) = self.size.icon_button(IconButtonWidth::Default);
        let icon_only_width = icon + 2. * padding;
        let mut row = div()
            .flex()
            .items_center()
            .gap(px(group_gap(self.connected, self.size)))
            .font_family(font_family());
        if let Some(slots) = slots {
            row = row.on_children_prepainted(move |bounds, _, _| {
                if at_rest {
                    *slots.natural.borrow_mut() =
                        bounds.iter().map(|b| f32::from(b.size.width)).collect();
                }
            });
        }

        let selected_flags: Vec<bool> = selection.iter().map(|s| s == &Some(true)).collect();
        for (i, item) in self.items.into_iter().enumerate() {
            let id = ids[i].clone();
            let enabled = !item.disabled;
            let selected = selection[i];
            let is_pressed = pressed[i];
            let target = if self.connected {
                connected_corners(
                    i,
                    count,
                    selected == Some(true),
                    is_pressed,
                    m.height,
                    m.pressed,
                )
            } else {
                [standard_radius(
                    selected == Some(true),
                    is_pressed,
                    m.height,
                    m.square,
                    m.pressed,
                ); 4]
            };
            let radii = animate(key(&id, "radius"), Spring::SPATIAL_FAST, target, window, cx);
            let variant = item.variant.unwrap_or(self.variant);
            let (bg, fg, border) = item_colors(&s, variant, selected, enabled);
            let bg = animate_color(key(&id, "bg"), Spring::EFFECTS, bg, window, cx);
            let fg = animate_color(key(&id, "fg"), Spring::EFFECTS, fg, window, cx);
            let border =
                border.map(|c| animate_color(key(&id, "border"), Spring::EFFECTS, c, window, cx));
            let layer = state_layer(&id, &states[i], corners(radii), fg, window, cx);

            let icon_only = item.label.is_none();
            let width = if !at_rest && !natural.is_empty() {
                Some((natural[i] + deltas[i]).max(0.))
            } else if icon_only {
                Some(icon_only_width)
            } else {
                None
            };
            let mut el = div()
                .id(id.clone())
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .gap(px(m.gap))
                .h(px(m.height))
                .when_some(width, |d, w| d.w(px(w)))
                .when(!icon_only, |d| d.px(px(m.padding)))
                .corners(corners(radii))
                .bg(bg)
                .text_color(fg)
                .text_size(m.text.size)
                .font_weight(FontWeight(600.))
                .child(layer);
            if let Some(border) = border {
                el = el.border(px(m.outline)).border_color(border);
            }
            if let Some(icon) = item.icon {
                el = el.child(Icon::new(icon).size(px(m.icon)).color(fg));
            }
            if let Some(label) = item.label {
                el = el.child(div().whitespace_nowrap().child(label));
            }
            if let Some(tooltip) = item.tooltip {
                el = el.tooltip(super::Tooltip::text(tooltip));
            }
            let mut el = interactive(el, &states[i], enabled, true);
            if enabled {
                el = keyboard_focus(el, &id, corners(radii), item.autofocus, window, cx);
                let on_click = self.on_click.clone();
                let on_change = self.on_change.clone();
                let on_change_many = self.on_change_many.clone();
                let flags = selected_flags.clone();
                let single = matches!(self.selection, Selection::Single(_));
                let multi = matches!(self.selection, Selection::Multi(_));
                el = el.on_click(move |_, w, cx| {
                    if multi {
                        if let Some(f) = &on_change_many {
                            f(toggled(&flags, i), w, cx);
                        }
                    } else if single {
                        if let Some(f) = &on_change {
                            f(i, w, cx);
                        }
                    } else if let Some(f) = &on_click {
                        f(i, w, cx);
                    }
                });
            }
            row = row.child(el);
        }
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: &[f32], expected: &[f32]) {
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1e-4, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn gap_depends_on_kind_and_size() {
        assert_eq!(group_gap(true, M3ButtonSize::Xl), 2.);
        assert_eq!(group_gap(false, M3ButtonSize::Xs), 8.);
        assert_eq!(group_gap(false, M3ButtonSize::M), 8.);
        assert_eq!(group_gap(false, M3ButtonSize::L), 12.);
        assert_eq!(group_gap(false, M3ButtonSize::Xl), 12.);
    }

    #[test]
    fn connected_group_rounds_its_ends_and_the_selected_button() {
        let corners = |i, selected, pressed| connected_corners(i, 3, selected, pressed, 40., 8.);
        assert_eq!(corners(0, false, false), [20., 4., 4., 20.]);
        assert_eq!(corners(1, false, false), [4., 4., 4., 4.]);
        assert_eq!(corners(2, false, false), [4., 20., 20., 4.]);
        assert_eq!(corners(1, true, false), [20.; 4]);
        // Pressed: the selected one lets go of its pill and the outer corners tighten.
        assert_eq!(corners(1, true, true), [4.; 4]);
        assert_eq!(corners(0, false, true), [8., 4., 4., 8.]);
    }

    #[test]
    fn a_single_connected_button_is_a_pill() {
        assert_eq!(connected_corners(0, 1, false, false, 56., 12.), [28.; 4]);
    }

    #[test]
    fn standard_buttons_are_pills_and_square_when_selected() {
        assert_eq!(standard_radius(false, false, 40., 12., 8.), 20.);
        assert_eq!(standard_radius(true, false, 40., 12., 8.), 12.);
        assert_eq!(standard_radius(true, true, 40., 12., 8.), 8.);
    }

    #[test]
    fn pressing_widens_the_button_and_narrows_its_neighbours() {
        let widths = [100., 100., 100.];
        let deltas = press_deltas(&widths, Some(1));
        assert_close(&deltas, &[-7.5, 15., -7.5]);
        assert!(deltas.iter().sum::<f32>().abs() < 1e-4);
    }

    #[test]
    fn an_end_button_takes_everything_from_its_only_neighbour() {
        let deltas = press_deltas(&[100., 100., 100.], Some(0));
        assert_close(&deltas, &[15., -15., 0.]);
    }

    #[test]
    fn a_small_neighbour_gives_up_at_most_a_share_of_its_width() {
        let deltas = press_deltas(&[200., 20.], Some(0));
        assert_close(&deltas, &[7., -7.]);
    }

    #[test]
    fn nothing_moves_without_a_press_or_a_neighbour() {
        assert_eq!(press_deltas(&[100., 80.], None), vec![0., 0.]);
        assert_eq!(press_deltas(&[100.], Some(0)), vec![0.]);
        assert_eq!(press_deltas(&[100., 80.], Some(5)), vec![0., 0.]);
    }

    #[test]
    fn toggling_flips_only_that_flag() {
        assert_eq!(toggled(&[true, false, true], 1), vec![true, true, true]);
        assert_eq!(toggled(&[true, false], 0), vec![false, false]);
        assert_eq!(toggled(&[true], 4), vec![true]);
    }

    #[test]
    fn only_selected_buttons_use_the_selected_colors() {
        let s = Scheme::baseline_light();
        let colors = |variant, selected| item_colors(&s, variant, selected, true).0;
        assert_eq!(colors(ButtonVariant::Filled, Some(true)), s.primary);
        assert_eq!(colors(ButtonVariant::Filled, None), s.primary);
        assert_eq!(
            colors(ButtonVariant::Filled, Some(false)),
            s.surface_container_high
        );
        assert_eq!(colors(ButtonVariant::Tonal, Some(true)), s.secondary);
        assert_eq!(
            colors(ButtonVariant::Tonal, Some(false)),
            s.secondary_container
        );
        assert!(
            item_colors(&s, ButtonVariant::Outlined, Some(false), true)
                .2
                .is_some()
        );
        assert!(
            item_colors(&s, ButtonVariant::Filled, Some(false), true)
                .2
                .is_none()
        );
        // Disabled buttons ignore the selection.
        assert_eq!(
            item_colors(&s, ButtonVariant::Filled, Some(true), false).1,
            alpha(s.on_surface, 0.38)
        );
    }
}
