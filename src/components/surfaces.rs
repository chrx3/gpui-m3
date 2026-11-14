//! Surfaces: menus, tonal cards, segmented lists and message bubbles.

use std::rc::Rc;
use std::time::Instant;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Bounds, BoxShadow, ClickEvent, ContentMask, Corner, Div, Element, ElementId,
    FontWeight, GlobalElementId, Hsla, InspectorElementId, InteractiveElement, IntoElement,
    LayoutId, Length, ParentElement, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, anchored, deferred, div, point, px, relative,
};

use super::{ClickHandler, Icon, Radio, RoundedExt, all, child_id, corners};
use crate::interaction::{interactive, state_layer, use_interaction};
use crate::motion::{
    MotionSettings, Spring, animate_color, animate_f32, elapsed, element_state, entrance, key,
    replay, request_frame,
};
use crate::shapes::Tone;
use crate::theme::{MONO_FONT_FAMILY, Scheme, Theme, alpha, font_family, mix};

/// Shadow for menus and popovers .
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

/// Where a [`reveal`] grows from: the side of the surface next to what opened it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RevealOrigin {
    /// The whole edge on the anchor side, at full width: [`PopDirection::Up`] grows upward
    /// from the bottom edge, [`PopDirection::Down`] downward from the top edge.
    Edge(PopDirection),
    /// The surface corner that sits on the anchor (as in [`Popover::anchor`]): grows outward
    /// from it, down or up and sideways (`BottomLeft` grows up and to the right).
    Corner(Corner),
}

impl From<PopDirection> for RevealOrigin {
    fn from(direction: PopDirection) -> Self {
        RevealOrigin::Edge(direction)
    }
}

impl From<Corner> for RevealOrigin {
    fn from(corner: Corner) -> Self {
        RevealOrigin::Corner(corner)
    }
}

/// Room left around a revealed surface for its pop shadow (blur 28 + offset 10).
const REVEAL_SHADOW: f32 = 40.;
/// Width a corner reveal starts at, as a fraction of the surface.
const REVEAL_WIDTH_FROM: f32 = 0.5;

/// The visible part of a surface laid out at `bounds`, `progress` (0 → 1) into its reveal from
/// `origin`. At 1 it covers the surface and its shadow.
fn reveal_mask(bounds: Bounds<Pixels>, origin: RevealOrigin, progress: f32) -> Bounds<Pixels> {
    let p = progress.clamp(0., 1.);
    let m = REVEAL_SHADOW;
    let (left, top) = (f32::from(bounds.left()), f32::from(bounds.top()));
    let (right, bottom) = (f32::from(bounds.right()), f32::from(bounds.bottom()));
    let (w, h) = (right - left, bottom - top);
    let (from_top, from_left) = match origin {
        RevealOrigin::Edge(direction) => (direction == PopDirection::Down, None),
        RevealOrigin::Corner(corner) => (
            matches!(corner, Corner::TopLeft | Corner::TopRight),
            Some(matches!(corner, Corner::TopLeft | Corner::BottomLeft)),
        ),
    };
    let reach_y = p * (h + m);
    let (y0, y1) = if from_top {
        (top - m, top + reach_y)
    } else {
        (bottom - reach_y, bottom + m)
    };
    let reach_x = (REVEAL_WIDTH_FROM + (1. - REVEAL_WIDTH_FROM) * p) * (w + m);
    let (x0, x1) = match from_left {
        None => (left - m, right + m),
        Some(true) => (left - m, left + reach_x),
        Some(false) => (right - reach_x, right + m),
    };
    Bounds::from_corners(point(px(x0), px(y0)), point(px(x1), px(y1.max(y0))))
}

struct RevealState {
    start: Instant,
    /// The size spring already reached the full surface (its bounce past it is not visible).
    opened: bool,
}

/// M3 Expressive menu and popover entrance: the surface grows out of what opened it.
///
/// GPUI cannot scale elements, so the surface is laid out at its final size and clipped to a
/// rectangle that grows from `origin` with the fast spatial spring while it fades in with the
/// effects spring: the content never reflows. It settles in about 0.25 s and then requests no
/// more frames; with reduced motion it appears at once. `id` keeps the entrance across frames
/// (unique within the view).
#[track_caller]
pub fn reveal<E: Styled + IntoElement>(
    el: E,
    id: &ElementId,
    origin: impl Into<RevealOrigin>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (size, fade) = reveal_progress(id, window, cx);
    reveal_with(el, origin.into(), size, fade)
}

/// [`reveal`] at a given `progress` (1 = shown, 0 = gone), for exits driven by
/// [`crate::motion::presence`]: the surface shrinks back toward `origin` while it fades out.
pub fn reveal_at<E: Styled + IntoElement>(
    el: E,
    origin: impl Into<RevealOrigin>,
    progress: f32,
) -> AnyElement {
    let progress = progress.clamp(0., 1.);
    reveal_with(el, origin.into(), progress, progress)
}

fn reveal_with<E: Styled + IntoElement>(
    el: E,
    origin: RevealOrigin,
    size: f32,
    fade: f32,
) -> AnyElement {
    if size >= 1. && fade >= 1. {
        return el.into_any_element();
    }
    RevealMask {
        origin,
        progress: size,
        child: Some(el.opacity(fade.clamp(0., 1.)).into_any_element()),
    }
    .into_any_element()
}

/// Size and opacity progress of a [`reveal`] entrance this frame.
#[track_caller]
fn reveal_progress(id: &ElementId, window: &mut Window, cx: &mut App) -> (f32, f32) {
    if MotionSettings::get(cx).reduced {
        return (1., 1.);
    }
    let (t, opened) = element_state(key(id, "reveal"), window, |state: Option<RevealState>, _| {
        let mut s = state.unwrap_or(RevealState { start: Instant::now(), opened: false });
        let t = elapsed(s.start, cx);
        s.opened |= Spring::SPATIAL_FAST.step(t) >= 1.;
        ((t, s.opened), s)
    });
    let size = if opened { 1. } else { Spring::SPATIAL_FAST.step(t) };
    let faded = t >= Spring::EFFECTS.settle_time().as_secs_f32();
    let fade = if faded { 1. } else { Spring::EFFECTS.step(t) };
    if !(opened && faded) {
        request_frame(window);
    }
    (size, fade)
}

/// Lays out its child as is and draws it clipped to [`reveal_mask`].
struct RevealMask {
    origin: RevealOrigin,
    progress: f32,
    child: Option<AnyElement>,
}

impl IntoElement for RevealMask {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for RevealMask {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let mut child = self.child.take().expect("laid out once");
        let id = child.request_layout(window, cx);
        (id, child)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        // The hidden part takes no hover or clicks either.
        let mask = ContentMask { bounds: reveal_mask(bounds, self.origin, self.progress) };
        window.with_content_mask(Some(mask), |window| child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let mask = ContentMask { bounds: reveal_mask(bounds, self.origin, self.progress) };
        window.with_content_mask(Some(mask), |window| child.paint(window, cx));
    }
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
/// [`reveal`] entrance from its anchor and an optional maximum height with scrolling.
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
    origin: Option<Corner>,
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
            origin: None,
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

    /// For a popover placed by the app: the corner next to what opened it, which the entrance
    /// grows from (an anchored popover already grows from its [`Popover::anchor`] corner).
    /// Also sets the direction to match.
    pub fn origin(mut self, corner: Corner) -> Self {
        self.origin = Some(corner);
        self.direction = match corner {
            Corner::TopLeft | Corner::TopRight => PopDirection::Down,
            Corner::BottomLeft | Corner::BottomRight => PopDirection::Up,
        };
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
            .min_w_0()
            .gap(self.gap)
            .p(self.padding)
            .children(self.children);
        let surface = div()
            .flex()
            .flex_col()
            .when_some(self.width, |d, w| d.w(w).max_w_full())
            .rounded(self.radius)
            .bg(s.surface_container)
            .shadow(shadow_pop(&s))
            .font_family(font_family())
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
        let mut surface = surface.id(child_id(&self.id, "surface"));
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
        let origin = match self.origin.or(self.anchor) {
            Some(corner) => RevealOrigin::Corner(corner),
            None => RevealOrigin::Edge(self.direction),
        };
        let surface = reveal(surface, &self.id, origin, window, cx);
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
    origin: Option<Corner>,
    header: Option<AnyElement>,
    footer: Option<AnyElement>,
    back: Option<(SharedString, ClickHandler)>,
    content_key: Option<u64>,
    stagger: bool,
}

/// Delay between the entrances of consecutive menu items (25 ms, up to the fifth).
const ITEM_STAGGER: f32 = 0.025;

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
            origin: None,
            header: None,
            footer: None,
            back: None,
            content_key: None,
            stagger: true,
        }
    }

    /// Items fade in one by one, 25 ms apart, while the surface grows (on by default; the
    /// fifth and later items share the last delay). Ignored by [`Menu::grouped`] menus, whose
    /// groups stagger instead.
    pub fn stagger(mut self, stagger: bool) -> Self {
        self.stagger = stagger;
        self
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
    /// The menu grows out of its edge on the anchor side, at full width.
    pub fn direction(mut self, direction: PopDirection) -> Self {
        self.direction = direction;
        self
    }

    /// The menu corner that sits next to what opened it (the corner given to `anchored`):
    /// the menu grows out of that corner, sideways as well (see [`reveal`]). Also sets the
    /// direction to match.
    pub fn origin(mut self, corner: Corner) -> Self {
        self.origin = Some(corner);
        self.direction = match corner {
            Corner::TopLeft | Corner::TopRight => PopDirection::Down,
            Corner::BottomLeft | Corner::BottomRight => PopDirection::Up,
        };
        self
    }

    /// Fixed content above the scrolling items (a filter field, [`super::IconTabs`]…).
    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }

    /// Fixed content below the scrolling items. In a menu that opens upward its edge next to
    /// the anchor stays put, so a filter field or tabs here don't move while the items
    /// above them grow or shrink.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
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
        let origin = match self.origin {
            Some(corner) => RevealOrigin::Corner(corner),
            None => RevealOrigin::Edge(self.direction),
        };
        let base = div()
            .flex()
            .flex_col()
            .w(px(self.width))
            .max_w_full()
            .font_family(font_family())
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
                let p = entrance(key(&gid, "fade"), Spring::EFFECTS, i as f32 * 0.035, window, cx);
                col = col.child(card.opacity(p.clamp(0., 1.)));
            }
            let menu = match self.max_height {
                Some(max_height) => div().child(
                    div()
                        .id(child_id(&self.id, "scroll"))
                        .max_h(max_height)
                        .overflow_y_scroll()
                        .child(col.flex_none()),
                ),
                None => col,
            };
            reveal(menu, &self.id, origin, window, cx)
        } else {
            let has_header = self.header.is_some() || self.back.is_some();
            let mut content = div().flex().flex_col().p(self.padding);
            if has_header {
                content = content.pt(px(2.));
            }
            if self.footer.is_some() {
                content = content.pb(px(2.));
            }
            let mut n = 0usize;
            for (i, group) in self.groups.into_iter().enumerate() {
                if i > 0 {
                    content =
                        content.child(div().my(px(4.)).mx(px(10.)).h(px(1.)).bg(s.outline_variant));
                }
                if !self.stagger {
                    content = content.children(group);
                    continue;
                }
                for item in group {
                    let id = ElementId::NamedChild(
                        Box::new(self.id.clone()),
                        format!("item-{n}").into(),
                    );
                    let delay = n.min(4) as f32 * ITEM_STAGGER;
                    let p = entrance(id, Spring::EFFECTS, delay, window, cx);
                    n += 1;
                    content = content.child(div().opacity(p.clamp(0., 1.)).child(item));
                }
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
            let card = match self.footer {
                Some(footer) => card.child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_none()
                        .px(self.padding)
                        .pb(self.padding)
                        .child(footer),
                ),
                None => card,
            };
            reveal(card, &self.id, origin, window, cx)
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
    hover_fill: bool,
    highlights: Vec<std::ops::Range<usize>>,
    confirm: Option<SharedString>,
    actions: Vec<AnyElement>,
    line_clamp: Option<usize>,
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
            hover_fill: true,
            highlights: Vec::new(),
            confirm: None,
            actions: Vec::new(),
            line_clamp: None,
            on_click: None,
        }
    }

    /// Adds an action under the text (a small [`super::Chip`] or [`super::Button`]), for
    /// two-line rows such as the Rewind list: "Code", "Conversation", "Both". Rows with actions
    /// align their content to the top; combine with [`MenuItem::static_row`] when the actions are
    /// the only way to act on the row.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// Adds several actions under the text. See [`MenuItem::action`].
    pub fn actions(mut self, actions: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.actions
            .extend(actions.into_iter().map(IntoElement::into_any_element));
        self
    }

    /// Shows at most `lines` lines of the label, cutting the rest with an ellipsis (a long
    /// message preview).
    pub fn line_clamp(mut self, lines: usize) -> Self {
        self.line_clamp = Some(lines.max(1));
        self
    }

    /// Whether hovering paints the row's highlight (on by default). Lists where the pointer moves
    /// the keyboard highlight ([`super::CommandPalette`], [`super::SuggestionList`]) turn it off,
    /// so only the highlighted row is lit after using the arrows.
    pub fn hover_fill(mut self, fill: bool) -> Self {
        self.hover_fill = fill;
        self
    }

    /// Byte ranges of the label to emphasize (the part that matches a query), in bold and the
    /// primary color. See [`super::match_ranges`].
    pub fn label_highlights(mut self, ranges: Vec<std::ops::Range<usize>>) -> Self {
        self.highlights = ranges;
        self
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
        let hovered = (state.read(cx).hovered && reactive && self.hover_fill) || self.highlighted;
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
        let highlighted_label = |label: SharedString| -> AnyElement {
            if self.highlights.is_empty() {
                return label.into_any_element();
            }
            let style = gpui::HighlightStyle {
                color: Some(s.primary),
                font_weight: Some(FontWeight(700.)),
                ..Default::default()
            };
            let ranges: Vec<_> = self
                .highlights
                .iter()
                .filter(|r| {
                    r.end <= label.len()
                        && label.is_char_boundary(r.start)
                        && label.is_char_boundary(r.end)
                })
                .map(|r| (r.clone(), style))
                .collect();
            gpui::StyledText::new(label)
                .with_highlights(ranges)
                .into_any_element()
        };
        let label = match (&self.confirm, armed) {
            (Some(confirm), true) => confirm.clone().into_any_element(),
            _ => highlighted_label(self.label),
        };

        let has_actions = !self.actions.is_empty();
        let label = match self.line_clamp {
            Some(lines) => div()
                .overflow_hidden()
                .text_ellipsis()
                .line_clamp(lines)
                .child(label)
                .into_any_element(),
            None => label,
        };
        let mut row = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .when(!has_actions, |d| d.items_center())
            .when(has_actions, |d| d.items_start())
            .gap(px(gap))
            .min_h(px(min_h))
            .px(px(pad))
            .py(px(if has_actions {
                6.
            } else if self.dense {
                0.
            } else {
                4.
            }))
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
                .child(div().flex_none().child(label))
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
                .when(has_actions, |d| {
                    d.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(4.))
                            .mt(px(6.))
                            .children(self.actions),
                    )
                })
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
            row = row.child(
                div()
                    .flex_none()
                    .text_size(px(11.5))
                    .text_color(muted)
                    .child(trailing),
            );
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

/// Color card (a version card): 28 px corners and a tonal container.
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
            .font_family(font_family())
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
            .font_family(font_family())
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
                    .child(div().flex_1().min_w_0().child(row)),
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
            .font_family(font_family())
            .text_size(px(13.5))
            .children(self.children)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    fn card() -> Bounds<Pixels> {
        Bounds::new(point(px(100.), px(200.)), size(px(200.), px(300.)))
    }

    #[test]
    fn an_upward_reveal_grows_from_the_bottom_edge() {
        let half = reveal_mask(card(), PopDirection::Up.into(), 0.5);
        assert_eq!(half.bottom(), px(500. + REVEAL_SHADOW));
        assert_eq!(half.top(), px(500. - 0.5 * (300. + REVEAL_SHADOW)));
        assert_eq!(half.left(), px(100. - REVEAL_SHADOW));
        assert_eq!(half.right(), px(300. + REVEAL_SHADOW));
        let closed = reveal_mask(card(), PopDirection::Up.into(), 0.);
        assert_eq!(closed.size.height, px(REVEAL_SHADOW));
        assert!(closed.top() >= card().bottom());
    }

    #[test]
    fn a_corner_reveal_grows_away_from_its_corner() {
        let start = reveal_mask(card(), Corner::TopRight.into(), 0.);
        assert_eq!(start.top(), px(200. - REVEAL_SHADOW));
        assert_eq!(start.right(), px(300. + REVEAL_SHADOW));
        assert!(start.left() > card().left());
        let open = reveal_mask(card(), Corner::TopRight.into(), 1.);
        assert!(open.left() <= card().left() - px(REVEAL_SHADOW) + px(0.01));
        assert!(open.bottom() >= card().bottom() + px(REVEAL_SHADOW) - px(0.01));
    }

    #[test]
    fn the_bounce_past_the_end_is_not_drawn() {
        let open = reveal_mask(card(), Corner::BottomLeft.into(), 1.);
        assert_eq!(reveal_mask(card(), Corner::BottomLeft.into(), 1.1), open);
    }
}
