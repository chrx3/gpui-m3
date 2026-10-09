//! Surfaces: menus, tonal cards, segmented lists and message bubbles.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, Corner, Div, ElementId, FontWeight, Hsla,
    InteractiveElement, IntoElement, Length, ParentElement, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, anchored, deferred, div, point, px, relative,
};

use super::{ClickHandler, Icon, Radio, RoundedExt, all, child_id, corners};
use crate::interaction::{interactive, state_layer, use_interaction};
use crate::motion::{Spring, animate_color, animate_f32, entrance, key, replay};
use crate::shapes::Tone;
use crate::theme::{FONT_FAMILY, MONO_FONT_FAMILY, Scheme, Theme, alpha, mix};

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

/// Which way a popover opens from its anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PopDirection {
    /// Above the anchor: enters rising.
    #[default]
    Up,
    /// Below the anchor: enters dropping.
    Down,
}

/// Applies the M3 Expressive popover entrance: rises 12 px and fades in with the fast
/// spatial spring. `delay` is in seconds, used to stagger groups.
pub fn pop_in(el: Div, id: &ElementId, delay: f32, window: &mut Window, cx: &mut App) -> Div {
    pop_in_toward(el, id, delay, PopDirection::Up, window, cx)
}

/// [`pop_in`] for a popover that opens in `direction` (a dropdown uses [`PopDirection::Down`]).
pub fn pop_in_toward(
    el: Div,
    id: &ElementId,
    delay: f32,
    direction: PopDirection,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let p = entrance(key(id, "pop"), Spring::SPATIAL_FAST, delay, window, cx);
    let sign = match direction {
        PopDirection::Up => 1.,
        PopDirection::Down => -1.,
    };
    el.relative()
        .top(px((1. - p) * 12. * sign))
        .opacity(p.clamp(0., 1.))
}

/// Content that fades in and rises 6 px each time `generation` changes (a menu tab switch).
fn content_switch(
    el: Div,
    id: &ElementId,
    generation: u64,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    match replay(key(id, "switch"), generation, 0.25, window, cx) {
        Some(t) => {
            let p = Spring::EFFECTS.step(t).clamp(0., 1.);
            el.relative().top(px((1. - p) * 6.)).opacity(p)
        }
        None => el,
    }
}

/// Popover surface ("Account and usage", mentions…): surface container, pop shadow, the
/// pop entrance and an optional maximum height with scrolling.
///
/// Without [`Popover::anchor`] it is laid out in place (wrap it in an absolute `div`). With an
/// anchor it floats over the window from where it is placed, kept inside the window.
#[derive(IntoElement)]
pub struct Popover {
    id: ElementId,
    width: Option<Pixels>,
    padding: Pixels,
    radius: Pixels,
    gap: Pixels,
    max_height: Option<Pixels>,
    direction: PopDirection,
    anchor: Option<Corner>,
    on_dismiss: Option<ClickHandler>,
    children: Vec<AnyElement>,
}

impl Popover {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            width: None,
            padding: px(16.),
            radius: px(28.),
            gap: px(12.),
            max_height: None,
            direction: PopDirection::Up,
            anchor: None,
            on_dismiss: None,
            children: Vec::new(),
        }
    }

    /// Floats over the window: `corner` is the popover corner that sits on the anchor point
    /// (`BottomLeft` opens up and to the right). Sets the entrance direction to match.
    pub fn anchor(mut self, corner: Corner) -> Self {
        self.anchor = Some(corner);
        self.direction = match corner {
            Corner::TopLeft | Corner::TopRight => PopDirection::Down,
            Corner::BottomLeft | Corner::BottomRight => PopDirection::Up,
        };
        self
    }

    /// Called on a click outside the popover.
    pub fn on_dismiss(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }

    pub fn width(mut self, width: Pixels) -> Self {
        self.width = Some(width);
        self
    }

    /// Inner padding (16 px by default).
    pub fn padding(mut self, padding: Pixels) -> Self {
        self.padding = padding;
        self
    }

    /// Corner radius (28 px by default).
    pub fn radius(mut self, radius: Pixels) -> Self {
        self.radius = radius;
        self
    }

    /// Gap between children (12 px by default).
    pub fn gap(mut self, gap: Pixels) -> Self {
        self.gap = gap;
        self
    }

    pub fn max_h(mut self, max_height: Pixels) -> Self {
        self.max_height = Some(max_height);
        self
    }

    pub fn direction(mut self, direction: PopDirection) -> Self {
        self.direction = direction;
        self
    }
}

impl ParentElement for Popover {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Popover {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let content = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(self.gap)
            .p(self.padding)
            .children(self.children);
        let surface = div()
            .flex()
            .flex_col()
            .when_some(self.width, |d, w| d.w(w))
            .rounded(self.radius)
            .bg(s.surface_container)
            .shadow(shadow_pop(&s))
            .font_family(FONT_FAMILY)
            .text_size(px(13.))
            .text_color(s.on_surface);
        let surface = match self.max_height {
            Some(max_height) => surface.child(
                div()
                    .id(child_id(&self.id, "scroll"))
                    .max_h(max_height)
                    .overflow_y_scroll()
                    .child(content),
            ),
            None => surface.child(content),
        };
        let mut surface = pop_in_toward(surface, &self.id, 0., self.direction, window, cx)
            .id(child_id(&self.id, "surface"));
        if let Some(f) = self.on_dismiss {
            surface = surface.on_mouse_down_out(move |event, w, cx| {
                let click = ClickEvent::Mouse(gpui::MouseClickEvent {
                    down: event.clone(),
                    up: gpui::MouseUpEvent {
                        button: event.button,
                        position: event.position,
                        modifiers: event.modifiers,
                        click_count: event.click_count,
                    },
                });
                f(&click, w, cx)
            });
        }
        match self.anchor {
            Some(corner) => deferred(
                anchored()
                    .anchor(corner)
                    .snap_to_window_with_margin(px(8.))
                    .child(surface),
            )
            .with_priority(1)
            .into_any_element(),
            None => surface.into_any_element(),
        }
    }
}

/// Menu: a surface with items, or several separate groups (M3 Expressive style).
#[derive(IntoElement)]
pub struct Menu {
    id: ElementId,
    groups: Vec<Vec<AnyElement>>,
    grouped: bool,
    width: f32,
    max_height: Option<Pixels>,
    padding: Pixels,
    radius: Pixels,
    direction: PopDirection,
    header: Option<AnyElement>,
    back: Option<(SharedString, ClickHandler)>,
    content_key: Option<u64>,
}

impl Menu {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            groups: vec![Vec::new()],
            grouped: false,
            width: 260.,
            max_height: None,
            padding: px(6.),
            radius: px(24.),
            direction: PopDirection::Up,
            header: None,
            back: None,
            content_key: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Inner padding (6 px by default). Ignored by [`Menu::grouped`] menus.
    pub fn padding(mut self, padding: Pixels) -> Self {
        self.padding = padding;
        self
    }

    /// Corner radius (24 px by default). Ignored by [`Menu::grouped`] menus.
    pub fn radius(mut self, radius: Pixels) -> Self {
        self.radius = radius;
        self
    }

    /// Entrance direction: [`PopDirection::Down`] for menus that open below their anchor.
    pub fn direction(mut self, direction: PopDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Fixed content above the scrolling items (a filter field, [`super::IconTabs`]…).
    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }

    /// Submenu "back" row at the top: a reversed chevron and the submenu title.
    pub fn back(
        mut self,
        title: impl Into<SharedString>,
        on_back: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.back = Some((title.into(), Rc::new(on_back)));
        self
    }

    /// Key of the current content (a tab index): when it changes, the items fade in and rise 6 px.
    pub fn content_key(mut self, key: u64) -> Self {
        self.content_key = Some(key);
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
                    div()
                        .id(child_id(&self.id, "scroll"))
                        .max_h(max_height)
                        .overflow_y_scroll()
                        .child(col.flex_none()),
                ),
                None => col,
            }
        } else {
            let has_header = self.header.is_some() || self.back.is_some();
            let mut content = div().flex().flex_col().p(self.padding);
            if has_header {
                content = content.pt(px(2.));
            }
            for (i, group) in self.groups.into_iter().enumerate() {
                if i > 0 {
                    content =
                        content.child(div().my(px(4.)).mx(px(10.)).h(px(1.)).bg(s.outline_variant));
                }
                content = content.children(group);
            }
            if let Some(generation) = self.content_key {
                content = content_switch(content, &self.id, generation, window, cx);
            }
            let mut card = base
                .rounded(self.radius)
                .bg(s.surface_container)
                .shadow(shadow_pop(&s));
            if has_header {
                let mut header = div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .px(self.padding)
                    .pt(self.padding);
                if let Some((title, on_back)) = self.back {
                    header = header.child(BackRow {
                        id: child_id(&self.id, "back"),
                        title,
                        on_back,
                    });
                }
                card = card.child(header.children(self.header));
            }
            let card = match self.max_height {
                Some(max_height) => card.child(
                    div()
                        .id(child_id(&self.id, "scroll"))
                        .max_h(max_height)
                        .overflow_y_scroll()
                        .child(content.flex_none()),
                ),
                None => card.child(content),
            };
            pop_in_toward(card, &self.id, 0., self.direction, window, cx)
        }
    }
}

/// "Back" row of a submenu: tonal pill with a reversed chevron and the title.
#[derive(IntoElement)]
struct BackRow {
    id: ElementId,
    title: SharedString,
    on_back: ClickHandler,
}

impl RenderOnce for BackRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let state = use_interaction(&self.id, window, cx);
        let layer = state_layer(&self.id, &state, all(px(19.)), s.on_surface, window, cx);
        let row = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(9.))
            .min_h(px(34.))
            .px(px(9.))
            .mb(px(4.))
            .rounded(px(19.))
            .bg(s.surface_container_high)
            .font_weight(FontWeight(600.))
            .child(layer)
            .child(
                Icon::new("chevron-right")
                    .size(px(16.))
                    .color(s.on_surface_variant)
                    .rotate(std::f32::consts::PI),
            )
            .child(self.title);
        let on_back = self.on_back;
        interactive(row, &state, true, true).on_click(move |e, w, cx| on_back(e, w, cx))
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
    shortcut: Option<SharedString>,
    selected: bool,
    highlighted: bool,
    radio: Option<bool>,
    disabled: bool,
    dense: bool,
    static_row: bool,
    inline_sublabel: bool,
    danger: bool,
    confirm: Option<SharedString>,
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
            shortcut: None,
            selected: false,
            highlighted: false,
            radio: None,
            disabled: false,
            dense: false,
            static_row: false,
            inline_sublabel: false,
            danger: false,
            confirm: None,
            on_click: None,
        }
    }

    /// Destructive action: error color for text and icon, error container on hover.
    pub fn danger(mut self, danger: bool) -> Self {
        self.danger = danger;
        self
    }

    /// Two-step action: the first click arms the row and shows `label` ("Click again to
    /// delete"); the second click runs `on_click`. Moving the pointer away disarms it.
    pub fn confirm(mut self, label: impl Into<SharedString>) -> Self {
        self.confirm = Some(label.into());
        self
    }

    /// Compact row (30 px, 13 px text, 8 → 15 px corners), as in the agent menu.
    pub fn dense(mut self, dense: bool) -> Self {
        self.dense = dense;
        self
    }

    /// Keyboard highlight: the hover look, without the check of [`MenuItem::selected`].
    pub fn highlighted(mut self, highlighted: bool) -> Self {
        self.highlighted = highlighted;
        self
    }

    /// Leading radio mark instead of the icon; single-choice lists without a tonal background.
    pub fn radio(mut self, selected: bool) -> Self {
        self.radio = Some(selected);
        self
    }

    /// Row without hover or ripple, for rows that hold their own controls (a slider…).
    pub fn static_row(mut self, static_row: bool) -> Self {
        self.static_row = static_row;
        self
    }

    /// Keyboard shortcut shown at the end in a small monospace font.
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Shows the sublabel on the same line as the label, truncated.
    pub fn inline_sublabel(mut self, inline: bool) -> Self {
        self.inline_sublabel = inline;
        self
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
        let reactive = enabled && !self.static_row;
        let hovered = (state.read(cx).hovered && reactive) || self.highlighted;
        let armed_state = self
            .confirm
            .as_ref()
            .map(|_| window.use_keyed_state(key(&self.id, "armed"), cx, |_, _| false));
        let armed = match &armed_state {
            Some(armed) if *armed.read(cx) && !hovered => {
                armed.update(cx, |armed, _| *armed = false);
                false
            }
            Some(armed) => *armed.read(cx),
            None => false,
        };
        // (min height, horizontal padding, gap, radius at rest, radius on hover, icon, text)
        let (min_h, pad, gap, rest_r, hover_r, icon_size, text_size) = if self.dense {
            (30., 9., 9., 8., 15., 16., Some(13.))
        } else {
            (36., 10., 10., 10., 18., 17., None)
        };
        let radius = px(animate_f32(
            key(&self.id, "radius"),
            Spring::SPATIAL_FAST,
            if hovered || self.selected {
                hover_r
            } else {
                rest_r
            },
            window,
            cx,
        ));
        let (bg, fg, muted) = if self.selected && self.radio.is_none() {
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
        let (bg, fg, muted) = match (self.danger, armed, hovered) {
            (_, true, _) => (
                s.error_container,
                s.on_error_container,
                s.on_error_container,
            ),
            (true, false, true) => (s.error_container, s.error, s.error),
            (true, false, false) => (bg, s.error, s.error),
            _ => (bg, fg, muted),
        };
        let bg = animate_color(key(&self.id, "bg"), Spring::EFFECTS, bg, window, cx);
        let fg = animate_color(key(&self.id, "fg"), Spring::EFFECTS, fg, window, cx);
        let layer = state_layer(&self.id, &state, all(radius), fg, window, cx);
        let label = match (&self.confirm, armed) {
            (Some(confirm), true) => confirm.clone(),
            _ => self.label,
        };

        let mut row = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(gap))
            .min_h(px(min_h))
            .px(px(pad))
            .py(px(if self.dense { 0. } else { 4. }))
            .corners(all(radius))
            .bg(bg)
            .text_color(fg)
            .when_some(text_size, |d, size| d.text_size(px(size)))
            .when(self.selected, |d| d.font_weight(FontWeight(600.)))
            .when(!enabled, |d| d.opacity(0.38))
            .child(layer);
        if let Some(on) = self.radio {
            row = row.child(Radio::new(child_id(&self.id, "radio"), on));
        } else if let Some(leading) = self.leading {
            row = row.child(leading);
        } else if let Some(icon) = self.icon {
            row = row.child(Icon::new(icon).size(px(icon_size)).color(muted));
        }
        let sub = self.sublabel.map(|sub| {
            div()
                .text_size(px(11.5))
                .font_weight(FontWeight(400.))
                .text_color(muted)
                .child(sub)
        });
        let text = if self.inline_sublabel {
            div()
                .flex()
                .items_baseline()
                .gap(px(8.))
                .flex_1()
                .min_w_0()
                .child(div().flex_none().child(label.clone()))
                .children(sub.map(|sub| {
                    sub.flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                }))
        } else {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .py(px(if self.dense { 4. } else { 0. }))
                .line_height(gpui::relative(1.25))
                .child(label)
                .children(sub)
        };
        row = row.child(text);
        let trailing = match (self.trailing, self.selected && self.radio.is_none()) {
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
        if let Some(shortcut) = self.shortcut {
            row = row.child(
                div()
                    .flex_none()
                    .font_family(MONO_FONT_FAMILY)
                    .text_size(px(11.))
                    .font_weight(FontWeight(400.))
                    .text_color(s.outline)
                    .child(shortcut),
            );
        }
        let mut row = interactive(row, &state, reactive, reactive);
        if let (Some(f), true) = (self.on_click, enabled) {
            row = row.on_click(move |e, w, cx| {
                if let Some(armed) = &armed_state
                    && !armed.read(cx)
                {
                    armed.update(cx, |armed, cx| {
                        *armed = true;
                        cx.notify();
                    });
                    return;
                }
                if let Some(armed) = &armed_state {
                    armed.update(cx, |armed, _| *armed = false);
                }
                f(e, w, cx)
            });
        }
        row
    }
}

/// Color card (the reference design's version card): 28 px corners and a tonal container.
#[derive(IntoElement)]
pub struct Card {
    id: Option<ElementId>,
    tone: Option<Tone>,
    background: Option<Hsla>,
    padding: (Pixels, Pixels),
    radius: Pixels,
    gap: Pixels,
    entrance: bool,
    on_click: Option<ClickHandler>,
    children: Vec<AnyElement>,
}

impl Card {
    /// Card with a tonal container.
    pub fn tonal(tone: Tone) -> Self {
        Self {
            tone: Some(tone),
            ..Self::plain()
        }
    }

    /// Neutral card (surface container with a hint of the primary color).
    pub fn plain() -> Self {
        Self {
            id: None,
            tone: None,
            background: None,
            padding: (px(22.), px(22.)),
            radius: px(28.),
            gap: px(12.),
            entrance: false,
            on_click: None,
            children: Vec::new(),
        }
    }

    /// Inner padding on all sides (22 px by default).
    pub fn padding(mut self, padding: Pixels) -> Self {
        self.padding = (padding, padding);
        self
    }

    /// Horizontal and vertical padding.
    pub fn padding_xy(mut self, x: Pixels, y: Pixels) -> Self {
        self.padding = (x, y);
        self
    }

    /// Corner radius (28 px by default).
    pub fn radius(mut self, radius: Pixels) -> Self {
        self.radius = radius;
        self
    }

    /// Gap between children (12 px by default).
    pub fn gap(mut self, gap: Pixels) -> Self {
        self.gap = gap;
        self
    }

    /// Custom background (e.g. `surface_container_high` for tool cards); the text keeps the
    /// tone's or `on_surface` color.
    pub fn bg(mut self, background: Hsla) -> Self {
        self.background = Some(background);
        self
    }

    /// Shape entrance when the card appears: corners open from 48 px to the final radius while
    /// it rises 24 px and fades in (permission requests). Needs a [`Card::id`].
    pub fn entrance(mut self, entrance: bool) -> Self {
        self.entrance = entrance;
        self
    }

    /// Id unique within the view; required by [`Card::on_click`] and [`Card::entrance`].
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
        let bg = self.background.unwrap_or(bg);
        debug_assert!(
            (self.on_click.is_none() && !self.entrance) || self.id.is_some(),
            "Card::on_click and Card::entrance need Card::id"
        );
        let progress = match (&self.id, self.entrance) {
            (Some(id), true) => entrance(key(id, "enter"), Spring::SPATIAL_SLOW, 0., window, cx),
            _ => 1.,
        };
        let radius = self.radius + (px(48.) - self.radius) * (1. - progress).max(0.);
        let card = div()
            .relative()
            .flex()
            .flex_col()
            .gap(self.gap)
            .px(self.padding.0)
            .py(self.padding.1)
            .rounded(radius)
            .bg(bg)
            .text_color(fg)
            .font_family(FONT_FAMILY)
            .when(progress < 1., |d| {
                d.top(px((1. - progress) * 24.))
                    .opacity(progress.clamp(0., 1.))
            });
        let (Some(id), Some(f)) = (self.id, self.on_click) else {
            return card.children(self.children).into_any_element();
        };
        let state = use_interaction(&id, window, cx);
        let layer = state_layer(&id, &state, all(radius), fg, window, cx);
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
    /// Error notice: like [`BubbleKind::Notice`] with the text in the error color.
    Error,
}

/// Message bubble with a squared "tail" corner.
#[derive(IntoElement)]
pub struct Bubble {
    kind: BubbleKind,
    max_width: Length,
    padding: (Pixels, Pixels),
    entrance: Option<(ElementId, f32)>,
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
            max_width: px(420.).into(),
            padding: (px(14.), px(10.)),
            entrance: None,
            children: Vec::new(),
        }
    }

    /// Maximum width (420 px by default).
    pub fn max_w(mut self, max_width: Pixels) -> Self {
        self.max_width = max_width.into();
        self
    }

    /// Maximum width as a fraction of the parent (e.g. 0.8).
    pub fn max_w_fraction(mut self, fraction: f32) -> Self {
        self.max_width = relative(fraction).into();
        self
    }

    /// Horizontal and vertical padding (14 × 10 by default).
    pub fn padding(mut self, x: Pixels, y: Pixels) -> Self {
        self.padding = (x, y);
        self
    }

    /// Entrance when it first appears: grows out of its tail corner with the fast spatial
    /// spring, after `delay` seconds (the reference design delays notices by 0.12 s).
    pub fn entrance(mut self, id: impl Into<ElementId>, delay: f32) -> Self {
        self.entrance = Some((id.into(), delay));
        self
    }
}

impl ParentElement for Bubble {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Bubble {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
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
            BubbleKind::Error => (s.tertiary_container, s.error, [4., 20., 20., 20.]),
        };
        // GPUI cannot scale an element, so the entrance rounds every corner fully, fades in and
        // slides 8 px from the tail side while the spring settles.
        let p = match &self.entrance {
            Some((id, delay)) => {
                entrance(key(id, "bubble"), Spring::SPATIAL_FAST, *delay, window, cx)
            }
            None => 1.,
        };
        let r = r.map(|corner| corner + (20. - corner) * (1. - p).clamp(0., 1.));
        let slide = (1. - p) * 8.;
        div()
            .relative()
            .when(p < 1., |d| match self.kind {
                BubbleKind::User => d.left(px(slide)).top(px(slide)).opacity(p.clamp(0., 1.)),
                _ => d.left(px(-slide)).top(px(-slide)).opacity(p.clamp(0., 1.)),
            })
            .max_w(self.max_width)
            .px(self.padding.0)
            .py(self.padding.1)
            .corners(corners(r))
            .bg(bg)
            .text_color(fg)
            .font_family(FONT_FAMILY)
            .text_size(px(13.5))
            .children(self.children)
    }
}
