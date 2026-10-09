//! Surfaces: menus, tonal cards, segmented lists and message bubbles.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, Div, ElementId, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Window, div, point, px,
};

use super::{ClickHandler, Icon, RoundedExt, all, child_id, corners};
use crate::interaction::{interactive, state_layer, use_interaction};
use crate::motion::{Spring, animate_color, animate_f32, entrance, key};
use crate::shapes::Tone;
use crate::theme::{FONT_FAMILY, Scheme, Theme, alpha, mix};

/// Shadow for menus and popovers (the reference design's `--shadow-pop`).
pub fn shadow_pop(s: &Scheme) -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: s.shadow,
            offset: point(px(0.), px(2.)),
            blur_radius: px(6.),
            spread_radius: px(0.),
        },
        BoxShadow {
            color: alpha(s.shadow, 0.85),
            offset: point(px(0.), px(10.)),
            blur_radius: px(28.),
            spread_radius: px(0.),
        },
    ]
}

/// Applies the M3 Expressive popover entrance: rises 12 px and fades in with the fast
/// spatial spring. `delay` is in seconds, used to stagger groups.
pub fn pop_in(el: Div, id: &ElementId, delay: f32, window: &mut Window, cx: &mut App) -> Div {
    let p = entrance(key(id, "pop"), Spring::SPATIAL_FAST, delay, window, cx);
    el.relative()
        .top(px((1. - p) * 12.))
        .opacity(p.clamp(0., 1.))
}

/// Menu: a surface with items, or several separate groups (M3 Expressive style).
#[derive(IntoElement)]
pub struct Menu {
    id: ElementId,
    groups: Vec<Vec<AnyElement>>,
    grouped: bool,
    width: f32,
    max_height: Option<Pixels>,
}

impl Menu {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            groups: vec![Vec::new()],
            grouped: false,
            width: 260.,
            max_height: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Limits the height; sections and items scroll inside the menu.
    pub fn max_h(mut self, max_height: Pixels) -> Self {
        self.max_height = Some(max_height);
        self
    }

    /// Each [`Menu::separator`] starts a group with its own card instead of a divider line.
    pub fn grouped(mut self, grouped: bool) -> Self {
        self.grouped = grouped;
        self
    }

    pub fn item(mut self, item: impl IntoElement) -> Self {
        self.groups
            .last_mut()
            .unwrap()
            .push(item.into_any_element());
        self
    }

    pub fn section(self, title: impl Into<SharedString>) -> Self {
        self.item(MenuSection(title.into()))
    }

    pub fn separator(mut self) -> Self {
        self.groups.push(Vec::new());
        self
    }
}

#[derive(IntoElement)]
struct MenuSection(SharedString);

impl RenderOnce for MenuSection {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .pt(px(6.))
            .pb(px(2.))
            .px(px(12.))
            .text_size(px(12.))
            .font_weight(FontWeight(700.))
            .text_color(Theme::of(cx).primary)
            .child(self.0)
    }
}

impl RenderOnce for Menu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let base = div()
            .flex()
            .flex_col()
            .w(px(self.width))
            .font_family(FONT_FAMILY)
            .text_size(px(13.))
            .text_color(s.on_surface);
        if self.grouped {
            let mut col = base.gap(px(4.));
            for (i, group) in self.groups.into_iter().enumerate() {
                let gid = ElementId::NamedInteger(format!("{}-group", self.id).into(), i as u64);
                let card = div()
                    .flex()
                    .flex_col()
                    .p(px(4.))
                    .rounded(px(18.))
                    .bg(s.surface_container)
                    .shadow(shadow_pop(&s))
                    .children(group);
                col = col.child(pop_in(card, &gid, i as f32 * 0.035, window, cx));
            }
            match self.max_height {
                Some(max_height) => div().child(
                    col.id(child_id(&self.id, "scroll"))
                        .max_h(max_height)
                        .overflow_y_scroll(),
                ),
                None => col,
            }
        } else {
            let mut content = div().flex().flex_col().p(px(6.));
            for (i, group) in self.groups.into_iter().enumerate() {
                if i > 0 {
                    content =
                        content.child(div().my(px(4.)).mx(px(10.)).h(px(1.)).bg(s.outline_variant));
                }
                content = content.children(group);
            }
            let card = base
                .rounded(px(24.))
                .bg(s.surface_container)
                .shadow(shadow_pop(&s));
            let card = match self.max_height {
                Some(max_height) => card.child(
                    content
                        .id(child_id(&self.id, "scroll"))
                        .max_h(max_height)
                        .overflow_y_scroll(),
                ),
                None => card.child(content),
            };
            pop_in(card, &self.id, 0., window, cx)
        }
    }
}

/// Menu item: rounds into a pill on hover.
#[derive(IntoElement)]
pub struct MenuItem {
    id: ElementId,
    label: SharedString,
    sublabel: Option<SharedString>,
    icon: Option<SharedString>,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
    selected: bool,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl MenuItem {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            sublabel: None,
            icon: None,
            leading: None,
            trailing: None,
            selected: false,
            disabled: false,
            on_click: None,
        }
    }

    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Leading element instead of the icon (a color dot, an [`super::Avatar`]…).
    pub fn leading(mut self, el: impl IntoElement) -> Self {
        self.leading = Some(el.into_any_element());
        self
    }

    /// Current option of a single-choice list: tonal background and a trailing check
    /// (unless a trailing element is set).
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn sublabel(mut self, sublabel: impl Into<SharedString>) -> Self {
        self.sublabel = Some(sublabel.into());
        self
    }

    /// Trailing element: a shortcut, a value, a [`super::Switch`]…
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing = Some(el.into_any_element());
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for MenuItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let state = use_interaction(&self.id, window, cx);
        let enabled = !self.disabled;
        let hovered = state.read(cx).hovered && enabled;
        let radius = px(animate_f32(
            key(&self.id, "radius"),
            Spring::SPATIAL_FAST,
            if hovered || self.selected { 18. } else { 10. },
            window,
            cx,
        ));
        let (bg, fg, muted) = if self.selected {
            (
                s.secondary_container,
                s.on_secondary_container,
                s.on_secondary_container,
            )
        } else if hovered {
            (
                s.surface_container_highest,
                s.on_surface,
                s.on_surface_variant,
            )
        } else {
            (
                alpha(s.surface_container_highest, 0.),
                s.on_surface,
                s.on_surface_variant,
            )
        };
        let bg = animate_color(key(&self.id, "bg"), Spring::EFFECTS, bg, window, cx);
        let fg = animate_color(key(&self.id, "fg"), Spring::EFFECTS, fg, window, cx);
        let layer = state_layer(&self.id, &state, all(radius), fg, window, cx);

        let mut row = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(10.))
            .min_h(px(36.))
            .px(px(10.))
            .py(px(4.))
            .corners(all(radius))
            .bg(bg)
            .text_color(fg)
            .when(self.selected, |d| d.font_weight(FontWeight(600.)))
            .when(!enabled, |d| d.opacity(0.38))
            .child(layer);
        if let Some(leading) = self.leading {
            row = row.child(leading);
        } else if let Some(icon) = self.icon {
            row = row.child(Icon::new(icon).size(px(17.)).color(muted));
        }
        let mut text = div().flex().flex_col().flex_1().child(self.label);
        if let Some(sub) = self.sublabel {
            text = text.child(
                div()
                    .text_size(px(11.5))
                    .font_weight(FontWeight(400.))
                    .text_color(muted)
                    .child(sub),
            );
        }
        row = row.child(text);
        let trailing = match (self.trailing, self.selected) {
            (Some(trailing), _) => Some(trailing),
            (None, true) => Some(
                Icon::new("check")
                    .size(px(17.))
                    .color(fg)
                    .into_any_element(),
            ),
            (None, false) => None,
        };
        if let Some(trailing) = trailing {
            row = row.child(div().text_size(px(11.5)).text_color(muted).child(trailing));
        }
        let mut row = interactive(row, &state, enabled, enabled);
        if let (Some(f), true) = (self.on_click, enabled) {
            row = row.on_click(move |e, w, cx| f(e, w, cx));
        }
        row
    }
}

/// Color card (the reference design's version card): 28 px corners and a tonal container.
#[derive(IntoElement)]
pub struct Card {
    id: Option<ElementId>,
    tone: Option<Tone>,
    padding: Pixels,
    on_click: Option<ClickHandler>,
    children: Vec<AnyElement>,
}

impl Card {
    /// Card with a tonal container.
    pub fn tonal(tone: Tone) -> Self {
        Self {
            id: None,
            tone: Some(tone),
            padding: px(22.),
            on_click: None,
            children: Vec::new(),
        }
    }

    /// Neutral card (surface container with a hint of the primary color).
    pub fn plain() -> Self {
        Self {
            id: None,
            tone: None,
            padding: px(22.),
            on_click: None,
            children: Vec::new(),
        }
    }

    /// Inner padding (22 px by default).
    pub fn padding(mut self, padding: Pixels) -> Self {
        self.padding = padding;
        self
    }

    /// Id unique within the view; required by [`Card::on_click`].
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Makes the card clickable, with hover and ripple. Set an [`Card::id`] too.
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl ParentElement for Card {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Card {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let (bg, fg) = match self.tone {
            Some(tone) => tone.colors(&s),
            None => (mix(s.primary, s.surface_container, 0.06), s.on_surface),
        };
        let card = div()
            .relative()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(self.padding)
            .rounded(px(28.))
            .bg(bg)
            .text_color(fg)
            .font_family(FONT_FAMILY);
        debug_assert!(
            self.on_click.is_none() || self.id.is_some(),
            "Card::on_click needs Card::id"
        );
        let (Some(id), Some(f)) = (self.id, self.on_click) else {
            return card.children(self.children).into_any_element();
        };
        let state = use_interaction(&id, window, cx);
        let layer = state_layer(&id, &state, all(px(28.)), fg, window, cx);
        let card = card.id(id).child(layer).children(self.children);
        interactive(card, &state, true, true)
            .on_click(move |e, w, cx| f(e, w, cx))
            .into_any_element()
    }
}

/// Segmented list: rows separated by 2 px, with large corners only at the ends.
#[derive(IntoElement)]
pub struct ListGroup {
    title: Option<SharedString>,
    rows: Vec<AnyElement>,
}

impl ListGroup {
    pub fn new() -> Self {
        Self {
            title: None,
            rows: Vec::new(),
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn row(mut self, label: impl Into<SharedString>, trailing: impl IntoElement) -> Self {
        self.rows.push(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .w_full()
                .child(div().font_weight(FontWeight(500.)).child(label.into()))
                .child(trailing)
                .into_any_element(),
        );
        self
    }

    /// Row with arbitrary content.
    pub fn custom_row(mut self, content: impl IntoElement) -> Self {
        self.rows.push(content.into_any_element());
        self
    }
}

impl Default for ListGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for ListGroup {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let bg = mix(s.primary, s.surface_container, 0.07);
        let n = self.rows.len();
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .font_family(FONT_FAMILY)
            .text_size(px(13.5))
            .text_color(s.on_surface);
        if let Some(title) = self.title {
            col = col.child(
                div()
                    .pt(px(4.))
                    .pb(px(6.))
                    .px(px(6.))
                    .text_size(px(14.))
                    .font_weight(FontWeight(700.))
                    .text_color(s.primary)
                    .child(title),
            );
        }
        for (i, row) in self.rows.into_iter().enumerate() {
            let top = if i == 0 { 22. } else { 6. };
            let bottom = if i + 1 == n { 22. } else { 6. };
            col = col.child(
                div()
                    .flex()
                    .items_center()
                    .min_h(px(52.))
                    .py(px(12.))
                    .px(px(18.))
                    .corners(corners([top, top, bottom, bottom]))
                    .bg(bg)
                    .child(row),
            );
        }
        col
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BubbleKind {
    /// Own message: squared bottom-right corner.
    User,
    /// Notice: squared top-left corner, tertiary container.
    Notice,
}

/// Message bubble with a squared "tail" corner.
#[derive(IntoElement)]
pub struct Bubble {
    kind: BubbleKind,
    max_width: Pixels,
    children: Vec<AnyElement>,
}

impl Bubble {
    pub fn new(kind: BubbleKind, text: impl Into<SharedString>) -> Self {
        Self::custom(kind).child(text.into())
    }

    /// Empty bubble for arbitrary content (markdown, attachments…).
    pub fn custom(kind: BubbleKind) -> Self {
        Self {
            kind,
            max_width: px(420.),
            children: Vec::new(),
        }
    }

    /// Maximum width (420 px by default).
    pub fn max_w(mut self, max_width: Pixels) -> Self {
        self.max_width = max_width;
        self
    }
}

impl ParentElement for Bubble {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Bubble {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let (bg, fg, r) = match self.kind {
            BubbleKind::User => (
                s.primary_container,
                s.on_primary_container,
                [20., 20., 4., 20.],
            ),
            BubbleKind::Notice => (
                s.tertiary_container,
                s.on_tertiary_container,
                [4., 20., 20., 20.],
            ),
        };
        div()
            .max_w(self.max_width)
            .px(px(14.))
            .py(px(10.))
            .corners(corners(r))
            .bg(bg)
            .text_color(fg)
            .font_family(FONT_FAMILY)
            .text_size(px(13.5))
            .children(self.children)
    }
}
