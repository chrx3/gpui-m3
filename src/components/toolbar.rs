//! Toolbars (M3 Expressive): a floating pill and a docked bar with icon buttons and an
//! optional FAB.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;

use gpui::{
    App, ClickEvent, ElementId, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use super::{ClickHandler, Fab, Icon, RoundedExt, all};
use crate::interaction::{interactive, keyboard_focus, state_layer, use_interaction};
use crate::motion::{Spring, animate_color, animate_f32, key};
use crate::theme::{Elevation, Scheme, Theme, alpha};

/// Height of a toolbar, or the thickness of a vertical one.
const THICKNESS: f32 = 64.;

/// Side of an icon button of a toolbar.
const ITEM_SIZE: f32 = 40.;

/// Space around the content of a floating toolbar: the thickness minus the buttons.
const FLOATING_PADDING: f32 = (THICKNESS - ITEM_SIZE) / 2.;

/// Corner radius of a toolbar button while pressed.
const PRESSED_RADIUS: f32 = 12.;

/// An icon button of a [`Toolbar`].
#[derive(IntoElement)]
pub struct ToolbarItem {
    id: ElementId,
    icon: SharedString,
    selected: bool,
    disabled: bool,
    tooltip: Option<SharedString>,
    on_click: Option<ClickHandler>,
    /// Set by the toolbar that holds it.
    vibrant: bool,
}

impl ToolbarItem {
    pub fn new(id: impl Into<ElementId>, icon: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            selected: false,
            disabled: false,
            tooltip: None,
            on_click: None,
            vibrant: false,
        }
    }

    /// Shows the selected state (tonal in a standard toolbar, primary in a vibrant one).
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Tooltip shown on hover.
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

/// Container color of a toolbar.
fn container_color(s: &Scheme, vibrant: bool) -> Hsla {
    if vibrant {
        s.primary_container
    } else {
        s.surface_container
    }
}

/// Container and content colors of the FAB of a toolbar: it stands out from the container.
fn fab_colors(s: &Scheme, vibrant: bool) -> (Hsla, Hsla) {
    if vibrant {
        (s.primary, s.on_primary)
    } else {
        (s.primary_container, s.on_primary_container)
    }
}

/// Container and content colors of a toolbar button.
fn item_colors(s: &Scheme, vibrant: bool, selected: bool, enabled: bool) -> (Hsla, Hsla) {
    match (vibrant, selected, enabled) {
        (_, _, false) => (alpha(s.on_surface, 0.), alpha(s.on_surface, 0.38)),
        (false, true, _) => (s.secondary_container, s.on_secondary_container),
        (false, false, _) => (alpha(s.secondary_container, 0.), s.on_surface_variant),
        (true, true, _) => (s.primary, s.on_primary),
        (true, false, _) => (alpha(s.primary, 0.), s.on_primary_container),
    }
}

impl RenderOnce for ToolbarItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let enabled = !self.disabled;
        let state = use_interaction(&self.id, window, cx);
        let pressed = state.read(cx).pressed && enabled;
        let radius = px(animate_f32(
            key(&self.id, "radius"),
            Spring::SPATIAL_FAST,
            if pressed { PRESSED_RADIUS } else { ITEM_SIZE / 2. },
            window,
            cx,
        ));
        let (bg, fg) = item_colors(&s, self.vibrant, self.selected, enabled);
        let bg = animate_color(key(&self.id, "bg"), Spring::EFFECTS, bg, window, cx);
        let fg = animate_color(key(&self.id, "fg"), Spring::EFFECTS, fg, window, cx);
        let layer = state_layer(&self.id, &state, all(radius), fg, window, cx);

        let mut el = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(ITEM_SIZE))
            .corners(all(radius))
            .bg(bg)
            .child(layer)
            .child(Icon::new(self.icon).size(px(24.)).color(fg));
        if let Some(text) = self.tooltip {
            el = el.tooltip(super::Tooltip::text(text));
        }
        let mut el = interactive(el, &state, enabled, true);
        if enabled {
            el = keyboard_focus(el, &self.id, all(radius), false, window, cx);
        }
        if let (Some(f), true) = (self.on_click, enabled) {
            el = el.on_click(move |e, w, cx| f(e, w, cx));
        }
        el
    }
}

/// Toolbar: icon buttons and an optional FAB.
///
/// [`Toolbar::floating`] is a pill with a level 3 shadow, horizontal or
/// [`vertical`](Toolbar::vertical). [`Toolbar::docked`] is a full-width 64 px bar.
#[derive(IntoElement)]
pub struct Toolbar {
    docked: bool,
    vertical: bool,
    vibrant: bool,
    items: Vec<ToolbarItem>,
    fab: Option<Fab>,
}

impl Toolbar {
    /// A pill that floats over the content (standard colors: `surface_container`).
    pub fn floating() -> Self {
        Self {
            docked: false,
            vertical: false,
            vibrant: false,
            items: Vec::new(),
            fab: None,
        }
    }

    /// A full-width bar, 64 px tall, with the buttons spread along it and the FAB at the end.
    pub fn docked() -> Self {
        Self {
            docked: true,
            ..Self::floating()
        }
    }

    /// Stacks a floating toolbar from top to bottom, 64 px wide.
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    /// Colors it with `primary_container` instead of `surface_container`.
    pub fn vibrant(mut self) -> Self {
        self.vibrant = true;
        self
    }

    pub fn item(mut self, item: ToolbarItem) -> Self {
        self.items.push(item);
        self
    }

    /// The FAB at the end. The toolbar sizes it, colors it and drops its shadow.
    pub fn fab(mut self, fab: Fab) -> Self {
        self.fab = Some(fab);
        self
    }
}

impl RenderOnce for Toolbar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let vibrant = self.vibrant;
        let (fab_container, fab_content) = fab_colors(&s, vibrant);
        let fab = self.fab.map(|fab| {
            let fab = fab.in_toolbar(fab_container, fab_content);
            if self.docked { fab } else { fab.small() }
        });
        let items = self.items.into_iter().map(|mut item| {
            item.vibrant = vibrant;
            item
        });
        let bar = div().flex().flex_none().items_center().bg(container_color(&s, vibrant));
        if self.docked {
            return bar
                .w_full()
                .h(px(THICKNESS))
                .px(px(16.))
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .items_center()
                        .justify_around()
                        .children(items),
                )
                .children(fab);
        }
        bar.when(self.vertical, |d| d.flex_col())
            .gap(px(4.))
            .p(px(FLOATING_PADDING))
            .rounded(px(THICKNESS / 2.))
            .shadow(Elevation::Level3.shadow(&s))
            .children(items)
            .children(fab)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floating_padding_makes_a_64_px_toolbar() {
        assert_eq!(FLOATING_PADDING * 2. + ITEM_SIZE, THICKNESS);
    }

    #[test]
    fn container_and_fab_always_differ() {
        let s = Scheme::baseline_light();
        for vibrant in [false, true] {
            assert_ne!(container_color(&s, vibrant), fab_colors(&s, vibrant).0);
        }
        assert_eq!(container_color(&s, false), s.surface_container);
        assert_eq!(container_color(&s, true), s.primary_container);
    }

    #[test]
    fn selected_buttons_are_tonal_or_primary() {
        let s = Scheme::baseline_dark();
        assert_eq!(item_colors(&s, false, true, true).0, s.secondary_container);
        assert_eq!(item_colors(&s, true, true, true), (s.primary, s.on_primary));
        // Unselected ones are transparent over the container.
        assert_eq!(item_colors(&s, false, false, true).0.a, 0.);
        assert_eq!(item_colors(&s, true, false, true).0.a, 0.);
        assert_eq!(item_colors(&s, true, false, true).1, s.on_primary_container);
    }

    #[test]
    fn disabled_buttons_ignore_the_selection() {
        let s = Scheme::baseline_light();
        for vibrant in [false, true] {
            let (bg, fg) = item_colors(&s, vibrant, true, false);
            assert_eq!(bg.a, 0.);
            assert_eq!(fg, alpha(s.on_surface, 0.38));
        }
    }
}
