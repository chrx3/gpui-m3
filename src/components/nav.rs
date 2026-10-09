//! Navigation: drawer rows, navigation rail items and stretching icon tabs.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
    relative,
};

use super::{ChangeHandler, ClickHandler, Icon, RoundedExt, all};
use crate::interaction::{interactive, state_layer, use_interaction};
use crate::motion::{Spring, animate_color, animate_f32, animate_on_activate, key};
use crate::shapes::Tone;
use crate::theme::{FONT_FAMILY, Theme, alpha};

/// Navigation row (the sidebar in the reference design). The selected indicator starts at the
/// center and stretches with the spring.
#[derive(IntoElement)]
pub struct NavItem {
    id: ElementId,
    label: SharedString,
    icon: Option<SharedString>,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
    selected: bool,
    dense: bool,
    muted: bool,
    emphasized: bool,
    large: bool,
    tone: Option<Tone>,
    meta: Option<SharedString>,
    group: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl NavItem {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            leading: None,
            trailing: None,
            selected: false,
            dense: false,
            muted: false,
            emphasized: false,
            large: false,
            tone: None,
            meta: None,
            group: None,
            on_click: None,
        }
    }

    /// Unread row: heavier label in the full text color.
    pub fn emphasized(mut self, emphasized: bool) -> Self {
        self.emphasized = emphasized;
        self
    }

    /// Short trailing text in the outline color (a time, a count). It gives way to
    /// [`NavItem::trailing`] while the row is hovered.
    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// Large row (52 px, 26 px corners), like settings tabs; the icon pops when selected.
    pub fn large(mut self, large: bool) -> Self {
        self.large = large;
        self
    }

    /// Selected indicator in a tonal container instead of the secondary container.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Leading element instead of the icon (e.g. an [`super::Avatar`]).
    pub fn leading(mut self, el: impl IntoElement) -> Self {
        self.leading = Some(el.into_any_element());
        self
    }

    /// Trailing element (e.g. a [`super::Badge`]).
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing = Some(el.into_any_element());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Dimmed label (archived or inactive rows). Ignored while selected.
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// Names the row as a GPUI hover group, so trailing elements can use
    /// `group_hover(name, ..)` to appear on hover.
    pub fn group(mut self, name: impl Into<SharedString>) -> Self {
        self.group = Some(name.into());
        self
    }

    /// Conversation row (30 px, 13 px text).
    pub fn dense(mut self, dense: bool) -> Self {
        self.dense = dense;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for NavItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let state = use_interaction(&self.id, window, cx);
        let hovered = state.read(cx).hovered;
        let (height, font) = if self.large {
            (52., 14.)
        } else if self.dense {
            (30., 13.)
        } else {
            (34., 13.5)
        };
        let (indicator_bg, selected_fg) = match self.tone {
            Some(tone) => tone.colors(&s),
            None => (s.secondary_container, s.on_secondary_container),
        };
        let radius = px(height / 2.);
        let [grow, opacity] = animate_on_activate(
            key(&self.id, "indicator"),
            Spring::SPATIAL_FAST,
            self.selected,
            [0.35, 0.],
            [1., 1.],
            window,
            cx,
        );
        let fg = if self.selected {
            selected_fg
        } else if self.emphasized {
            s.on_surface
        } else if self.muted {
            s.on_surface_variant
        } else {
            s.on_surface
        };
        let fg = animate_color(key(&self.id, "fg"), Spring::EFFECTS, fg, window, cx);
        let icon_color = if self.selected {
            selected_fg
        } else {
            s.on_surface_variant
        };
        let layer = state_layer(&self.id, &state, all(radius), s.on_surface, window, cx);

        let mut row = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(10.))
            .w_full()
            .h(px(height))
            .px(px(12.))
            .rounded(radius)
            .text_color(fg)
            .font_family(FONT_FAMILY)
            .text_size(px(font))
            .font_weight(FontWeight(if self.selected {
                700.
            } else if self.emphasized {
                650.
            } else {
                500.
            }));
        if let Some(group) = self.group {
            row = row.group(group);
        }
        if self.selected {
            row = row.child(
                div()
                    .absolute()
                    .top_0()
                    .h_full()
                    .left(relative((1. - grow) / 2.))
                    .w(relative(grow.max(0.)))
                    .rounded(radius)
                    .bg(alpha(indicator_bg, opacity.clamp(0., 1.))),
            );
        }
        row = row.child(layer);
        if let Some(leading) = self.leading {
            row = row.child(leading);
        } else if let Some(icon) = self.icon {
            let mut icon = Icon::new(icon)
                .size(px(if self.large { 20. } else { 17. }))
                .color(icon_color);
            if self.large {
                let [scale, rotation] = animate_on_activate(
                    key(&self.id, "icon-pop"),
                    Spring::SPATIAL,
                    self.selected,
                    [1.25, -8f32.to_radians()],
                    [1., 0.],
                    window,
                    cx,
                );
                icon = icon.scale(scale).rotate(rotation);
            }
            row = row.child(icon);
        }
        row = row.child(
            div()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(self.label),
        );
        let show_trailing = self.meta.is_none() || hovered;
        if let Some(meta) = self.meta.filter(|_| !(hovered && self.trailing.is_some())) {
            row = row.child(
                div()
                    .flex_none()
                    .text_size(px(11.5))
                    .font_weight(FontWeight(400.))
                    .text_color(s.outline)
                    .child(meta),
            );
        }
        if let Some(trailing) = self.trailing.filter(|_| show_trailing) {
            row = row.child(trailing);
        }
        let mut row = interactive(row, &state, true, true);
        if let Some(f) = self.on_click {
            row = row.on_click(move |e, w, cx| f(e, w, cx));
        }
        row
    }
}

/// Navigation rail item: a 56 × 32 pill with the icon and the label below.
#[derive(IntoElement)]
pub struct RailItem {
    id: ElementId,
    label: SharedString,
    icon: SharedString,
    selected: bool,
    on_click: Option<ClickHandler>,
}

impl RailItem {
    pub fn new(
        id: impl Into<ElementId>,
        icon: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: icon.into(),
            selected: false,
            on_click: None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for RailItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let state = use_interaction(&self.id, window, cx);
        let pressed = state.read(cx).pressed;
        let radius = px(animate_f32(
            key(&self.id, "radius"),
            Spring::SPATIAL_FAST,
            if pressed { 8. } else { 16. },
            window,
            cx,
        ));
        let [width, opacity] = animate_on_activate(
            key(&self.id, "pill"),
            Spring::SPATIAL,
            self.selected,
            [24., 0.],
            [56., 1.],
            window,
            cx,
        );
        let fg = if self.selected {
            s.on_secondary_container
        } else {
            s.on_surface_variant
        };
        let fg = animate_color(key(&self.id, "fg"), Spring::EFFECTS, fg, window, cx);
        let label_color = if self.selected {
            s.on_surface
        } else {
            s.on_surface_variant
        };
        let layer = state_layer(&self.id, &state, all(radius), s.on_surface, window, cx);

        let mut pill = div()
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .w(px(56.))
            .h(px(32.))
            .rounded(radius);
        if self.selected {
            pill = pill.child(
                div()
                    .absolute()
                    .top_0()
                    .h_full()
                    .left(px((56. - width) / 2.))
                    .w(px(width.max(0.)))
                    .corners(all(radius))
                    .bg(alpha(s.secondary_container, opacity.clamp(0., 1.))),
            );
        }
        pill = pill
            .child(layer)
            .child(Icon::new(self.icon).size(px(20.)).color(fg));

        let el = div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.))
            .w(px(72.))
            .pt(px(4.))
            .pb(px(6.))
            .font_family(FONT_FAMILY)
            .text_size(px(12.))
            .text_color(label_color)
            .font_weight(FontWeight(if self.selected { 700. } else { 500. }))
            .child(pill)
            .child(self.label);
        let mut el = interactive(el, &state, true, true);
        if let Some(f) = self.on_click {
            el = el.on_click(move |e, w, cx| f(e, w, cx));
        }
        el
    }
}

/// A tab in [`IconTabs`].
#[derive(Clone)]
pub struct IconTab {
    pub icon: SharedString,
    pub label: SharedString,
}

impl IconTab {
    pub fn new(icon: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            icon: icon.into(),
            label: label.into(),
        }
    }
}

/// Round icon tabs: the active one stretches into a pill showing its name
/// (the action-menu tabs in the reference design).
#[derive(IntoElement)]
pub struct IconTabs {
    id: ElementId,
    tabs: Vec<IconTab>,
    selected: usize,
    on_change: Option<ChangeHandler<usize>>,
}

impl IconTabs {
    pub fn new(
        id: impl Into<ElementId>,
        tabs: impl IntoIterator<Item = IconTab>,
        selected: usize,
    ) -> Self {
        Self {
            id: id.into(),
            tabs: tabs.into_iter().collect(),
            selected,
            on_change: None,
        }
    }

    pub fn on_change(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for IconTabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let mut row = div().flex().w_full().gap(px(4.)).font_family(FONT_FAMILY);
        for (i, tab) in self.tabs.into_iter().enumerate() {
            let id = ElementId::NamedInteger(format!("{}-tab", self.id).into(), i as u64);
            let state = use_interaction(&id, window, cx);
            let hovered = state.read(cx).hovered;
            let on = i == self.selected;
            // The active one takes the free space (flex-grow 1); the rest stay at 34 px.
            let grow = animate_f32(
                key(&id, "grow"),
                Spring::SPATIAL,
                if on { 1. } else { 0. },
                window,
                cx,
            );
            let radius = if hovered && !on { 12. } else { 17. };
            let radius = px(animate_f32(
                key(&id, "radius"),
                Spring::SPATIAL_FAST,
                radius,
                window,
                cx,
            ));
            let (bg, fg) = match (on, hovered) {
                (true, _) => (s.secondary_container, s.on_secondary_container),
                (false, true) => (s.surface_container_highest, s.on_surface_variant),
                (false, false) => (s.surface_container_high, s.on_surface_variant),
            };
            let bg = animate_color(key(&id, "bg"), Spring::EFFECTS, bg, window, cx);
            let fg = animate_color(key(&id, "fg"), Spring::EFFECTS, fg, window, cx);
            let [scale, rot] = animate_on_activate(
                key(&id, "pop"),
                Spring::SPATIAL,
                on,
                [0.6, -0.52],
                [1., 0.],
                window,
                cx,
            );

            let mut el = div()
                .id(id.clone())
                .flex()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .h(px(34.))
                .min_w(px(34.))
                .flex_basis(px(34.))
                .overflow_hidden()
                .whitespace_nowrap()
                .corners(all(radius))
                .bg(bg)
                .text_color(fg)
                .text_size(px(12.5))
                .font_weight(FontWeight(600.))
                .child(
                    Icon::new(tab.icon)
                        .size(px(17.))
                        .color(fg)
                        .scale(scale)
                        .rotate(rot),
                );
            el.style().flex_grow = Some(grow.max(0.) * 10.);
            if grow > 0.35 {
                el = el.child(
                    div()
                        .opacity(((grow - 0.35) / 0.65).clamp(0., 1.))
                        .child(tab.label),
                );
            }
            let mut el = interactive(el, &state, true, false);
            if let Some(f) = self.on_change.clone() {
                el = el.on_click(move |_, w, cx| f(i, w, cx));
            }
            row = row.child(el);
        }
        row
    }
}
