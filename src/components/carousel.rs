//! Carousel (M3 Expressive): a strip of items that scroll, drag and snap.
//!
//! The sizes of the items are a pure function of the scroll offset ([`carousel_slots`]): in the
//! multi-browse and hero layouts an item shrinks toward the start edge as it leaves the focus
//! position, and the ones behind it grow into the larger sizes. The offset is counted in items:
//! `2.0` puts the third item at the focus position.

use std::{cell::Cell, rc::Rc, time::Instant};

use gpui::prelude::FluentBuilder as _;

use gpui::{
    AnyElement, App, Bounds, ElementId, FontWeight, Hsla, ImageSource, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ObjectFit, ParentElement, Pixels, RenderOnce, ScrollWheelEvent, SharedString,
    Styled, StyledImage, Window, canvas, div, img, px,
};

use super::{ChangeHandler, RoundedExt, all};
use crate::interaction::{focus_ring, focusable, use_focus};
use crate::motion::{MotionSettings, Spring, animate_f32, key, request_frame};
use crate::theme::{Theme, alpha, font_family};

/// Corner radius of an item.
const ITEM_RADIUS: f32 = 28.;

/// Space between two items.
const GAP: f32 = 8.;

/// Width of the small item of the multi-browse and hero layouts.
const SMALL: f32 = 56.;

/// Width of an uncontained item unless [`Carousel::item_width`] says otherwise.
const DEFAULT_ITEM_WIDTH: f32 = 186.;

/// A press that moves less than this (in px) is a click on an item, not a drag.
const CLICK_SLOP: f32 = 5.;

/// How long the wheel must rest before the strip snaps (seconds).
const WHEEL_IDLE: f32 = 0.12;

/// How far past the end a drag can pull, in items, and how much of the pull it follows.
const OVERSCROLL: f32 = 4.;
const RUBBER: f32 = 0.35;

/// A free-scrolling strip (uncontained) coasts to rest with this spring.
const FLING: Spring = Spring {
    stiffness: 140.,
    damping_ratio: 1.,
};

/// How the items of a [`Carousel`] are sized.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CarouselLayout {
    /// A large, a medium and a small item at once; they shrink as they leave the focus.
    #[default]
    MultiBrowse,
    /// Equal items that scroll freely (they snap only with [`Carousel::snap`]).
    Uncontained,
    /// One large item with a small one peeking.
    Hero,
    /// One large item in the middle with a small one on each side.
    CenterHero,
    /// One item at a time, as wide as the strip, taller by default.
    FullScreen,
}

/// Where and how big an item is, along the scrolling axis, relative to the strip's start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CarouselSlot {
    pub index: usize,
    pub start: f32,
    pub extent: f32,
}

/// Item sizes of the layouts that resize their items, by distance to the focus position, and
/// how many items come before the focus one. `None` for the layouts that scroll freely.
fn keylines(layout: CarouselLayout, container: f32) -> Option<(Vec<f32>, usize)> {
    let small = SMALL.min(container / 8.);
    match layout {
        CarouselLayout::MultiBrowse => {
            let large = (container - 2. * GAP - small).max(0.) / 1.6;
            Some((vec![large, large * 0.6, small], 0))
        }
        CarouselLayout::Hero => Some((vec![(container - GAP - small).max(0.), small], 0)),
        CarouselLayout::CenterHero => Some((
            vec![small, (container - 2. * GAP - 2. * small).max(0.), small],
            1,
        )),
        CarouselLayout::Uncontained | CarouselLayout::FullScreen => None,
    }
}

/// Size of an item `distance` items after the focus position (negative: before it).
fn extent_at(widths: &[f32], lead: usize, distance: f32) -> f32 {
    let q = distance + lead as f32;
    let last = widths.len() - 1;
    if q <= -1. {
        0.
    } else if q < 0. {
        widths[0] * (q + 1.)
    } else if q >= last as f32 {
        widths[last]
    } else {
        let i = q.floor() as usize;
        let f = q - i as f32;
        widths[i] + (widths[i + 1] - widths[i]) * f
    }
}

/// Size of the item at the focus position (what a card's content is laid out at).
fn natural_extent(layout: CarouselLayout, container: f32, item: f32) -> f32 {
    match keylines(layout, container) {
        Some((widths, lead)) => widths[lead],
        None => item,
    }
}

/// The visible items for a strip `container` px long, scrolled to `offset` (in items).
///
/// `item` is the size of an uncontained item; the other layouts size theirs from `container`.
/// Items that are not visible are left out.
pub(crate) fn carousel_slots(
    layout: CarouselLayout,
    container: f32,
    item: f32,
    count: usize,
    offset: f32,
) -> Vec<CarouselSlot> {
    let mut slots = Vec::new();
    if count == 0 || container <= 0. {
        return slots;
    }
    let Some((widths, lead)) = keylines(layout, container) else {
        for index in 0..count {
            let start = (index as f32 - offset) * (item + GAP);
            if start + item > 0. && start < container {
                slots.push(CarouselSlot {
                    index,
                    start,
                    extent: item,
                });
            }
        }
        return slots;
    };
    // Stack from the first item that has any size. Items before the first one are virtual:
    // they take room (so the first item can sit in the middle) but draw nothing.
    let mut i = offset.floor() as isize - lead as isize - 1;
    let mut x = 0.;
    while i < count as isize && x < container {
        let extent = extent_at(&widths, lead, i as f32 - offset);
        if i >= 0 && extent > 0.5 {
            slots.push(CarouselSlot {
                index: i as usize,
                start: x,
                extent,
            });
        }
        // The gap grows in with the item, so an item appearing from nothing does not push the others.
        x += extent + GAP * (extent / GAP).min(1.);
        i += 1;
    }
    slots
}

/// How far, in px, one item of scroll moves the strip.
fn step_px(layout: CarouselLayout, container: f32, item: f32) -> f32 {
    (natural_extent(layout, container, item) + GAP).max(1.)
}

/// The largest offset: the last item at the focus position, or (multi-browse) the last three
/// items showing, or (uncontained) the last item at the end of the strip.
fn max_offset(layout: CarouselLayout, container: f32, item: f32, count: usize) -> f32 {
    let last = count.saturating_sub(1) as f32;
    match layout {
        CarouselLayout::MultiBrowse => count.saturating_sub(3) as f32,
        CarouselLayout::Hero | CarouselLayout::CenterHero | CarouselLayout::FullScreen => last,
        CarouselLayout::Uncontained => {
            let step = item + GAP;
            ((count as f32 * step - GAP - container) / step).max(0.)
        }
    }
}

/// The offset a release settles on: the nearest item, thrown at most 1.5 items by the speed
/// (in items per second).
fn snap_target(offset: f32, velocity: f32, max: f32) -> f32 {
    let thrown = (velocity * 0.12).clamp(-1.5, 1.5);
    (offset + thrown).round().clamp(0., max.max(0.))
}

/// Where the wheel settles: the item in the direction it was turned, however little, so one
/// notch moves one item. `from` is where the wheel started.
fn wheel_target(from: f32, offset: f32, max: f32) -> f32 {
    let net = offset - from;
    let target = if net > 1e-3 {
        (offset - 1e-3).ceil()
    } else if net < -1e-3 {
        (offset + 1e-3).floor()
    } else {
        offset.round()
    };
    target.clamp(0., max.max(0.))
}

/// Where an arrow key goes from `from`: the next item that is not the current position.
fn step_target(from: f32, direction: f32, max: f32) -> f32 {
    let next = if direction > 0. {
        (from + 1e-3).floor() + 1.
    } else {
        (from - 1e-3).ceil() - 1.
    };
    next.clamp(0., max.max(0.))
}

/// A drag past either end follows the pointer at a fraction of its speed.
fn rubber_band(raw: f32, max: f32) -> f32 {
    let max = max.max(0.);
    if raw < 0. {
        raw * RUBBER
    } else if raw > max {
        max + (raw - max) * RUBBER
    } else {
        raw
    }
}

#[derive(Clone, Copy)]
struct Settle {
    spring: Spring,
    target: f32,
    from: f32,
    velocity: f32,
    start: Instant,
}

struct Drag {
    /// Pointer position along the axis and raw offset when the press started.
    origin: (f32, f32),
    /// Farthest the pointer got from where it started, in px.
    moved: f32,
    /// Recent offsets, to find the speed at release.
    samples: Vec<(Instant, f32)>,
}

/// Scroll state of a carousel, kept across frames.
#[derive(Default)]
struct Scroll {
    /// The offset on screen.
    pos: f32,
    settling: Option<Settle>,
    drag: Option<Drag>,
    last_wheel: Option<Instant>,
    /// Offset when the current wheel gesture started.
    wheel_from: f32,
    /// From the last frame, for the event handlers.
    slots: Vec<CarouselSlot>,
    step: f32,
    max: f32,
    snap: bool,
    /// Length of the strip the last layout used.
    used: f32,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl Scroll {
    fn settle_to(&mut self, target: f32, velocity: f32, spring: Spring, now: Instant) {
        if (target - self.pos).abs() < 1e-3 && velocity.abs() < 0.01 {
            self.pos = target;
            self.settling = None;
            return;
        }
        self.settling = Some(Settle {
            spring,
            target,
            from: self.pos - target,
            velocity,
            start: now,
        });
    }

    /// Where the strip is heading: the end of a settle, or where it is.
    fn heading(&self) -> f32 {
        self.settling.map_or(self.pos, |s| s.target)
    }

    fn busy(&self) -> bool {
        self.settling.is_some() || self.last_wheel.is_some()
    }

    /// Moves the offset to `now`: the wheel snaps once it rests and a settle runs its spring.
    fn advance(&mut self, now: Instant, motion: MotionSettings) {
        let scale = motion.time_scale.max(0.01);
        if self.drag.is_some() {
            return;
        }
        if let Some(wheel) = self.last_wheel {
            if now.duration_since(wheel).as_secs_f32() / scale >= WHEEL_IDLE {
                self.last_wheel = None;
                if self.snap {
                    let target = wheel_target(self.wheel_from, self.pos, self.max);
                    self.settle_to(target, 0., Spring::SPATIAL, now);
                }
            }
            return;
        }
        if let Some(settle) = self.settling {
            let t = now.duration_since(settle.start).as_secs_f32() / scale;
            let (x, v) = settle.spring.solve(settle.from, settle.velocity, t);
            let resting = t >= settle.spring.settle_time().as_secs_f32()
                || (x.abs() < 5e-4 && v.abs() < 0.01);
            if resting || motion.reduced {
                self.pos = settle.target;
                self.settling = None;
            } else {
                self.pos = settle.target + x;
            }
            return;
        }
        // Back in range after a resize or a change of items.
        let clamped = self.pos.clamp(0., self.max.max(0.));
        if (clamped - self.pos).abs() > 1e-3 {
            self.settle_to(clamped, 0., Spring::SPATIAL, now);
        }
    }

    fn begin_drag(&mut self, pointer: f32, now: Instant) {
        self.settling = None;
        self.last_wheel = None;
        self.drag = Some(Drag {
            origin: (pointer, self.pos),
            moved: 0.,
            samples: vec![(now, self.pos)],
        });
    }

    fn drag_to(&mut self, pointer: f32, now: Instant) {
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        let travelled = drag.origin.0 - pointer;
        drag.moved = drag.moved.max(travelled.abs());
        let raw = (drag.origin.1 + travelled / self.step.max(1.))
            .clamp(-OVERSCROLL, self.max.max(0.) + OVERSCROLL);
        self.pos = rubber_band(raw, self.max);
        drag.samples.push((now, self.pos));
        if drag.samples.len() > 8 {
            drag.samples.remove(0);
        }
    }

    /// Ends a drag. Returns whether it was a click, not a drag.
    fn end_drag(&mut self, now: Instant) -> bool {
        let Some(drag) = self.drag.take() else {
            return false;
        };
        let click = drag.moved < CLICK_SLOP;
        let velocity = drag
            .samples
            .iter()
            .find(|(t, _)| now.duration_since(*t).as_secs_f32() < 0.1)
            .filter(|(t, _)| *t < now)
            .map_or(0., |(t, p)| (self.pos - p) / now.duration_since(*t).as_secs_f32());
        let max = self.max.max(0.);
        if self.snap {
            let target = snap_target(self.pos, velocity, max);
            self.settle_to(target, velocity, Spring::SPATIAL, now);
        } else {
            let target = (self.pos + velocity * 0.3).clamp(0., max);
            self.settle_to(target, velocity, FLING, now);
        }
        click
    }

    /// The item under a point `along` the strip.
    fn item_at(&self, along: f32) -> Option<usize> {
        self.slots
            .iter()
            .find(|s| along >= s.start && along < s.start + s.extent)
            .map(|s| s.index)
    }
}

enum Content {
    Image(ImageSource),
    Element(AnyElement),
}

/// One item of a [`Carousel`]: an image or any element, with an optional caption.
pub struct CarouselItem {
    content: Content,
    caption: Option<SharedString>,
    background: Option<Hsla>,
}

impl CarouselItem {
    /// An image that covers the item, with rounded corners.
    pub fn image(source: impl Into<ImageSource>) -> Self {
        Self {
            content: Content::Image(source.into()),
            caption: None,
            background: None,
        }
    }

    /// Any element. It is laid out at the size of the focus item, from the item's start, and
    /// clipped as the item shrinks (to a rectangle: GPUI cannot round the corners of arbitrary
    /// content, so keep it inset from the edges).
    pub fn element(content: impl IntoElement) -> Self {
        Self {
            content: Content::Element(content.into_any_element()),
            caption: None,
            background: None,
        }
    }

    /// A label over the bottom of the item. It fades out as the item shrinks.
    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Color behind the content (`surface_container_highest` by default).
    pub fn background(mut self, color: Hsla) -> Self {
        self.background = Some(color);
        self
    }
}

/// A strip of items that scroll and snap: multi-browse, uncontained, hero, center-aligned hero
/// and full-screen layouts.
///
/// It scrolls with the wheel, by dragging and with the arrow keys while it has the focus
/// (Tab reaches it; a click gives it the focus too). `selected` and `on_select` are yours:
/// pressing an item calls `on_select` with its index and the carousel draws a ring around the
/// selected one.
///
/// The strip fills the width of its parent; give it a [`width`](Carousel::width) to size it
/// yourself. Items keep their layout size, so give the strip a width that suits the layout: the
/// multi-browse items are sized from it.
#[derive(IntoElement)]
pub struct Carousel {
    id: ElementId,
    layout: CarouselLayout,
    vertical: bool,
    width: Option<Pixels>,
    height: Option<Pixels>,
    item_width: Option<f32>,
    snap: Option<bool>,
    backdrop: Option<Hsla>,
    selected: Option<usize>,
    on_select: Option<ChangeHandler<usize>>,
    items: Vec<CarouselItem>,
}

impl Carousel {
    pub fn new(id: impl Into<ElementId>, layout: CarouselLayout) -> Self {
        Self {
            id: id.into(),
            layout,
            vertical: false,
            width: None,
            height: None,
            item_width: None,
            snap: None,
            backdrop: None,
            selected: None,
            on_select: None,
            items: Vec::new(),
        }
    }

    pub fn multi_browse(id: impl Into<ElementId>) -> Self {
        Self::new(id, CarouselLayout::MultiBrowse)
    }

    pub fn uncontained(id: impl Into<ElementId>) -> Self {
        Self::new(id, CarouselLayout::Uncontained)
    }

    pub fn hero(id: impl Into<ElementId>) -> Self {
        Self::new(id, CarouselLayout::Hero)
    }

    pub fn center_hero(id: impl Into<ElementId>) -> Self {
        Self::new(id, CarouselLayout::CenterHero)
    }

    pub fn full_screen(id: impl Into<ElementId>) -> Self {
        Self::new(id, CarouselLayout::FullScreen)
    }

    /// Scrolls from top to bottom. The strip is 220 px wide and 400 px tall unless sized with
    /// [`width`](Carousel::width) and [`height`](Carousel::height).
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    /// Width of the strip (it fills its parent by default; vertical: 220 px).
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = Some(width);
        self
    }

    /// Height of the strip (220 px; 400 px for full-screen and vertical ones).
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = Some(height);
        self
    }

    /// Size of an uncontained item along the strip (186 px by default).
    pub fn item_width(mut self, width: Pixels) -> Self {
        self.item_width = Some(f32::from(width));
        self
    }

    /// Whether the strip settles on an item when scrolling ends. On by default except for the
    /// uncontained layout, which scrolls freely.
    pub fn snap(mut self, snap: bool) -> Self {
        self.snap = Some(snap);
        self
    }

    /// The color the carousel sits on (`surface` by default), for the corners of image items.
    /// GPUI cannot round the corners of a cropped image, so they are covered in this color.
    pub fn backdrop(mut self, color: Hsla) -> Self {
        self.backdrop = Some(color);
        self
    }

    /// The item to ring as selected.
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    /// Called with the index of an item that was pressed.
    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    pub fn item(mut self, item: CarouselItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = CarouselItem>) -> Self {
        self.items.extend(items);
        self
    }
}

/// Id of the state of item `index`.
fn item_key(id: &ElementId, index: usize) -> ElementId {
    ElementId::NamedChild(Box::new(id.clone()), format!("item-{index}").into())
}

impl RenderOnce for Carousel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let layout = self.layout;
        let vertical = self.vertical;
        let snap = self.snap.unwrap_or(layout != CarouselLayout::Uncontained);
        let scroll = window.use_keyed_state(key(&self.id, "scroll"), cx, |_, _| Scroll::default());
        let bounds_cell = scroll.read(cx).bounds.clone();

        // Size of the strip: along the scrolling axis it is the one given, else the one measured
        // in the previous frame (a first guess until then).
        let tall_default = vertical || layout == CarouselLayout::FullScreen;
        let height = self
            .height
            .map_or(if tall_default { 400. } else { 220. }, f32::from);
        let cross_width = self.width.map_or(220., f32::from);
        let fixed = if vertical {
            Some(height)
        } else {
            self.width.map(f32::from)
        };
        let measured = {
            let b = bounds_cell.get();
            f32::from(if vertical { b.size.height } else { b.size.width })
        };
        let container = fixed.unwrap_or(measured);
        let container = if container > 0. {
            container
        } else {
            request_frame(window);
            480.
        };
        let cross = if vertical { cross_width } else { height };

        let item = match layout {
            CarouselLayout::Uncontained => self.item_width.unwrap_or(DEFAULT_ITEM_WIDTH).min(container),
            CarouselLayout::FullScreen => container,
            _ => 0.,
        };
        let count = self.items.len();
        let natural = natural_extent(layout, container, item);

        let now = Instant::now();
        let motion = MotionSettings::get(cx);
        let (pos, busy) = scroll.update(cx, |sc, _| {
            sc.snap = snap;
            sc.step = step_px(layout, container, item);
            sc.max = max_offset(layout, container, item, count);
            sc.used = container;
            sc.advance(now, motion);
            (sc.pos, sc.busy())
        });
        if busy {
            request_frame(window);
        }
        let slots = carousel_slots(layout, container, item, count, pos);
        scroll.update(cx, |sc, _| sc.slots = slots.clone());

        let on_select = self.on_select.clone();
        let mut items: Vec<Option<CarouselItem>> = self.items.into_iter().map(Some).collect();
        let mut strip = div().relative().size_full().overflow_hidden();
        for slot in slots {
            let Some(entry) = items.get_mut(slot.index).and_then(Option::take) else {
                continue;
            };
            let radius = ITEM_RADIUS.min(slot.extent / 2.).min(cross / 2.);
            let ring = animate_f32(
                key(&item_key(&self.id, slot.index), "ring"),
                Spring::EFFECTS,
                if self.selected == Some(slot.index) { 1. } else { 0. },
                window,
                cx,
            );
            let mut el = div()
                .absolute()
                .overflow_hidden()
                .corners(all(px(radius)))
                .bg(entry.background.unwrap_or(s.surface_container_highest));
            el = if vertical {
                el.top(px(slot.start)).left_0().h(px(slot.extent)).w(px(cross))
            } else {
                el.left(px(slot.start)).top_0().w(px(slot.extent)).h(px(cross))
            };
            el = match entry.content {
                Content::Image(source) => {
                    let backdrop = self.backdrop.unwrap_or(s.surface);
                    // A cover-fit image is cropped, and GPUI rounds the corners of the whole
                    // image, not of the visible part: a thick border in the backdrop color
                    // covers the corners instead. Its inner edge has the item's radius.
                    let cover = px(radius + 1.);
                    el.child(img(source).size_full().object_fit(ObjectFit::Cover))
                        .child(
                            div()
                                .absolute()
                                .top(-cover)
                                .left(-cover)
                                .right(-cover)
                                .bottom(-cover)
                                .border(cover)
                                .border_color(backdrop)
                                .rounded(cover + px(radius)),
                        )
                }
                Content::Element(content) => {
                    let inner = div().absolute().top_0().left_0().overflow_hidden().child(content);
                    el.child(if vertical {
                        inner.w(px(cross)).h(px(natural))
                    } else {
                        inner.h(px(cross)).w(px(natural))
                    })
                }
            };
            if let Some(caption) = entry.caption {
                let fade = ((slot.extent / natural.max(1.) - 0.6) / 0.4).clamp(0., 1.);
                if fade > 0. {
                    el = el.child(
                        div()
                            .absolute()
                            .left(px(12.))
                            .bottom(px(12.))
                            .max_w(px((natural - 24.).max(0.)))
                            .px(px(12.))
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .rounded(px(16.))
                            .bg(alpha(s.inverse_surface, 0.85 * fade))
                            .text_color(alpha(s.inverse_on_surface, fade))
                            .text_size(px(14.))
                            .font_weight(FontWeight(500.))
                            .whitespace_nowrap()
                            .child(caption),
                    );
                }
            }
            if ring > 0.01 {
                el = el.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .corners(all(px(radius)))
                        .border(px(3.))
                        .border_color(alpha(s.primary, ring.clamp(0., 1.))),
                );
            }
            strip = strip.child(el);
        }

        let (focus_state, handle, ring_visible) = use_focus(&self.id, window, cx);
        let listeners = {
            let scroll = scroll.clone();
            let handle = handle.clone();
            canvas(
                {
                    let scroll = scroll.clone();
                    move |bounds: Bounds<Pixels>, window, cx| {
                        bounds_cell.set(bounds);
                        let length = f32::from(if vertical { bounds.size.height } else { bounds.size.width });
                        if (length - scroll.read(cx).used).abs() > 0.5 {
                            window.request_animation_frame();
                        }
                    }
                },
                move |bounds, _, window, _| {
                    let along = move |p: gpui::Point<Pixels>| f32::from(if vertical { p.y } else { p.x });
                    let st = scroll.clone();
                    window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                        if !phase.bubble() || e.button != MouseButton::Left || !bounds.contains(&e.position) {
                            return;
                        }
                        window.focus(&handle);
                        st.update(cx, |sc, cx| {
                            sc.begin_drag(along(e.position), Instant::now());
                            cx.notify();
                        });
                    });
                    let st = scroll.clone();
                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                        if !phase.bubble() || st.read(cx).drag.is_none() {
                            return;
                        }
                        st.update(cx, |sc, cx| {
                            if e.pressed_button == Some(MouseButton::Left) {
                                sc.drag_to(along(e.position), Instant::now());
                            } else {
                                sc.end_drag(Instant::now());
                            }
                            cx.notify();
                        });
                    });
                    let st = scroll.clone();
                    let on_select = on_select.clone();
                    window.on_mouse_event(move |e: &MouseUpEvent, phase, window, cx| {
                        if !phase.bubble() || st.read(cx).drag.is_none() {
                            return;
                        }
                        let origin = along(bounds.origin);
                        let pressed = st.update(cx, |sc, cx| {
                            cx.notify();
                            sc.end_drag(Instant::now())
                                .then(|| sc.item_at(along(e.position) - origin))
                                .flatten()
                        });
                        if let (Some(index), Some(f)) = (pressed, &on_select) {
                            f(index, window, cx);
                        }
                    });
                },
            )
            .absolute()
            .size_full()
        };
        let wheel = scroll.clone();
        let keys = scroll.clone();
        let ring = focus_ring(&self.id, ring_visible, all(px(ITEM_RADIUS)), window, cx);
        let el = div()
            .id(self.id.clone())
            .relative()
            .flex_none()
            .font_family(font_family())
            .h(px(if vertical { height } else { cross }))
            .when_some(self.width, |d, w| d.w(w))
            .when(self.width.is_none() && !vertical, |d| d.w_full())
            .when(vertical && self.width.is_none(), |d| d.w(px(cross)))
            .when(self.on_select.is_some(), |d| d.cursor_pointer())
            .on_scroll_wheel(move |e: &ScrollWheelEvent, _, cx| {
                let delta = e.delta.pixel_delta(px(40.));
                let delta = f32::from(if vertical {
                    delta.y
                } else if delta.x.abs() >= delta.y.abs() {
                    delta.x
                } else {
                    delta.y
                });
                if delta == 0. {
                    return;
                }
                let moved = wheel.update(cx, |sc, cx| {
                    let next = (sc.pos - delta / sc.step.max(1.)).clamp(0., sc.max.max(0.));
                    let moved = (next - sc.pos).abs() > 1e-4;
                    if moved {
                        if sc.last_wheel.is_none() {
                            sc.wheel_from = sc.pos;
                        }
                        sc.settling = None;
                        sc.pos = next;
                        sc.last_wheel = Some(Instant::now());
                        cx.notify();
                    }
                    moved
                });
                // At either end the wheel goes on to scroll the page.
                if moved {
                    cx.stop_propagation();
                }
            })
            .on_key_down(move |e: &KeyDownEvent, _, cx| {
                let (back, forward) = if vertical { ("up", "down") } else { ("left", "right") };
                let key = e.keystroke.key.as_str();
                if ![back, forward, "home", "end"].contains(&key) {
                    return;
                }
                cx.stop_propagation();
                keys.update(cx, |sc, cx| {
                    let max = sc.max.max(0.);
                    let target = match key {
                        "home" => 0.,
                        "end" => max,
                        _ if key == forward => step_target(sc.heading(), 1., max),
                        _ => step_target(sc.heading(), -1., max),
                    };
                    let now = Instant::now();
                    // A running wheel or settle hands over at the position it is at.
                    sc.last_wheel = None;
                    sc.settle_to(target, 0., Spring::SPATIAL, now);
                    cx.notify();
                });
            })
            .child(strip.child(listeners))
            .children(ring);
        focusable(el, &focus_state, &handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f32 = 600.;

    fn slots(layout: CarouselLayout, offset: f32) -> Vec<CarouselSlot> {
        carousel_slots(layout, W, DEFAULT_ITEM_WIDTH, 10, offset)
    }

    fn extents(layout: CarouselLayout, offset: f32) -> Vec<f32> {
        slots(layout, offset).iter().map(|s| s.extent).collect()
    }

    #[test]
    fn multi_browse_shows_a_large_a_medium_and_a_small_item() {
        let at_rest = slots(CarouselLayout::MultiBrowse, 0.);
        assert_eq!(at_rest.len(), 3);
        let [large, medium, small] = [at_rest[0], at_rest[1], at_rest[2]];
        assert!(large.extent > medium.extent && medium.extent > small.extent);
        assert_eq!(small.extent, SMALL);
        // They fill the strip exactly, with the gaps between them.
        assert_eq!(large.start, 0.);
        assert!((medium.start - (large.extent + GAP)).abs() < 1e-3);
        assert!((small.start + small.extent - W).abs() < 1e-3);
    }

    #[test]
    fn multi_browse_items_shrink_toward_the_start_as_they_scroll() {
        let halfway = slots(CarouselLayout::MultiBrowse, 0.5);
        let at_rest = slots(CarouselLayout::MultiBrowse, 0.);
        // The first item is half way from large to nothing.
        assert!((halfway[0].extent - at_rest[0].extent / 2.).abs() < 1e-3);
        assert_eq!(halfway[0].index, 0);
        // The next one grew halfway from medium to large.
        let grown = (at_rest[0].extent + at_rest[1].extent) / 2.;
        assert!((halfway[1].extent - grown).abs() < 1e-3);
        // Once an item has left, the next one is the large one.
        let moved = slots(CarouselLayout::MultiBrowse, 1.);
        assert_eq!(moved[0].index, 1);
        assert!((moved[0].extent - at_rest[0].extent).abs() < 1e-3);
    }

    #[test]
    fn sizes_follow_the_offset_without_jumps() {
        for layout in [
            CarouselLayout::MultiBrowse,
            CarouselLayout::Hero,
            CarouselLayout::CenterHero,
        ] {
            let mut previous = slots(layout, 0.);
            let mut offset = 0.;
            while offset < 4. {
                offset += 0.01;
                let next = slots(layout, offset);
                for slot in &next {
                    if let Some(before) = previous.iter().find(|p| p.index == slot.index) {
                        // 1 % of an item of scroll moves a size by about 1 % of the strip. An item
                        // that is almost gone moves the next one twice as fast (its gap goes too).
                        assert!((slot.extent - before.extent).abs() < 6., "{layout:?} at {offset}");
                        assert!((slot.start - before.start).abs() < 12., "{layout:?} at {offset}");
                    }
                }
                previous = next;
            }
        }
    }

    #[test]
    fn hero_has_one_large_item_and_a_peeking_one() {
        let at_rest = slots(CarouselLayout::Hero, 0.);
        assert_eq!(at_rest.len(), 2);
        assert_eq!(at_rest[1].extent, SMALL);
        assert!((at_rest[0].extent - (W - GAP - SMALL)).abs() < 1e-3);
        assert!((at_rest[1].start + at_rest[1].extent - W).abs() < 1e-3);
    }

    #[test]
    fn center_hero_centers_the_large_item_with_a_small_one_each_side() {
        let middle = slots(CarouselLayout::CenterHero, 3.);
        assert_eq!(middle.iter().map(|s| s.index).collect::<Vec<_>>(), [2, 3, 4]);
        let large = middle[1];
        assert!((large.start + large.extent / 2. - W / 2.).abs() < 1e-3);
        assert_eq!(middle[0].extent, SMALL);
        assert_eq!(middle[2].extent, SMALL);
        // At the start there is nothing before it, but it stays in the middle.
        let first = slots(CarouselLayout::CenterHero, 0.);
        assert_eq!(first[0].index, 0);
        assert!((first[0].start + first[0].extent / 2. - W / 2.).abs() < 1e-3);
    }

    #[test]
    fn uncontained_items_are_equal_and_scroll_freely() {
        let at_rest = slots(CarouselLayout::Uncontained, 0.);
        assert!(at_rest.iter().all(|s| s.extent == DEFAULT_ITEM_WIDTH));
        assert_eq!(at_rest[1].start, DEFAULT_ITEM_WIDTH + GAP);
        // Scrolled by half an item, the first one is half out.
        let half = slots(CarouselLayout::Uncontained, 0.5);
        assert!((half[0].start + (DEFAULT_ITEM_WIDTH + GAP) / 2.).abs() < 1e-3);
        // Almost a whole item on, one more shows at the end.
        let almost = slots(CarouselLayout::Uncontained, 0.95);
        assert!(almost.last().unwrap().index > at_rest.last().unwrap().index);
        // Items entirely out of the strip are left out.
        let far = slots(CarouselLayout::Uncontained, 3.);
        assert_eq!(far[0].index, 3);
    }

    #[test]
    fn expressive_sizes_stay_inside_the_strip_and_never_negative() {
        for layout in [
            CarouselLayout::MultiBrowse,
            CarouselLayout::Hero,
            CarouselLayout::CenterHero,
            CarouselLayout::Uncontained,
        ] {
            for step in 0..60 {
                let offset = step as f32 * 0.15 - 1.;
                for slot in slots(layout, offset) {
                    assert!(slot.extent > 0. && slot.extent <= W, "{layout:?} at {offset}");
                    assert!(slot.start < W && slot.index < 10);
                }
            }
        }
    }

    #[test]
    fn there_is_nothing_to_lay_out_without_items_or_room() {
        assert!(carousel_slots(CarouselLayout::MultiBrowse, W, 0., 0, 0.).is_empty());
        assert!(carousel_slots(CarouselLayout::MultiBrowse, 0., 0., 5, 0.).is_empty());
        assert_eq!(extents(CarouselLayout::MultiBrowse, 0.).len(), 3);
        // Fewer items than slots.
        let two = carousel_slots(CarouselLayout::MultiBrowse, W, 0., 2, 0.);
        assert_eq!(two.len(), 2);
    }

    #[test]
    fn ranges_end_on_the_last_items() {
        assert_eq!(max_offset(CarouselLayout::MultiBrowse, W, 0., 10), 7.);
        assert_eq!(max_offset(CarouselLayout::MultiBrowse, W, 0., 2), 0.);
        assert_eq!(max_offset(CarouselLayout::Hero, W, 0., 10), 9.);
        assert_eq!(max_offset(CarouselLayout::CenterHero, W, 0., 1), 0.);
        // Uncontained: the last item ends where the strip ends.
        let max = max_offset(CarouselLayout::Uncontained, W, 186., 10);
        let slots = carousel_slots(CarouselLayout::Uncontained, W, 186., 10, max);
        let last = slots.last().unwrap();
        assert_eq!(last.index, 9);
        assert!((last.start + last.extent - W).abs() < 1e-2);
        // Fewer items than fit.
        assert_eq!(max_offset(CarouselLayout::Uncontained, W, 186., 2), 0.);
    }

    #[test]
    fn a_release_settles_on_the_nearest_item_thrown_by_the_speed() {
        assert_eq!(snap_target(1.4, 0., 7.), 1.);
        assert_eq!(snap_target(1.6, 0., 7.), 2.);
        // A flick forward carries it on, but never more than 1.5 items.
        assert_eq!(snap_target(1.2, 6., 7.), 2.);
        assert_eq!(snap_target(1.0, 100., 7.), 3.);
        assert_eq!(snap_target(1.0, -100., 7.), 0.);
        // Never past the ends.
        assert_eq!(snap_target(6.9, 100., 7.), 7.);
        assert_eq!(snap_target(-0.4, 0., 7.), 0.);
        assert_eq!(snap_target(0.5, 0., 0.), 0.);
    }

    #[test]
    fn arrow_keys_move_one_item_and_stop_at_the_ends() {
        assert_eq!(step_target(2., 1., 7.), 3.);
        assert_eq!(step_target(2., -1., 7.), 1.);
        assert_eq!(step_target(2.4, 1., 7.), 3.);
        assert_eq!(step_target(2.4, -1., 7.), 2.);
        assert_eq!(step_target(7., 1., 7.), 7.);
        assert_eq!(step_target(0., -1., 7.), 0.);
        // A fractional end (uncontained) can still be reached and left.
        assert_eq!(step_target(3., 1., 3.4), 3.4);
        assert_eq!(step_target(3.4, -1., 3.4), 3.);
    }

    #[test]
    fn pulling_past_the_end_follows_at_a_fraction_of_the_distance() {
        assert_eq!(rubber_band(2., 7.), 2.);
        assert!((rubber_band(-2., 7.) + 2. * RUBBER).abs() < 1e-6);
        assert!((rubber_band(9., 7.) - (7. + 2. * RUBBER)).abs() < 1e-6);
    }

    #[test]
    fn a_drag_moves_the_offset_and_settles_on_an_item() {
        let t0 = Instant::now();
        let mut scroll = Scroll {
            step: 100.,
            max: 7.,
            snap: true,
            ..Scroll::default()
        };
        scroll.begin_drag(300., t0);
        // 130 px to the left is 1.3 items forward.
        scroll.drag_to(170., t0);
        assert!((scroll.pos - 1.3).abs() < 1e-5);
        assert!(!scroll.end_drag(t0));
        assert_eq!(scroll.settling.map(|s| s.target), Some(1.));
        // A press that barely moved is a click, and the strip goes back to the item.
        let mut still = Scroll {
            step: 100.,
            max: 7.,
            snap: true,
            ..Scroll::default()
        };
        still.begin_drag(300., t0);
        still.drag_to(298., t0);
        assert!(still.end_drag(t0));
        assert_eq!(still.settling.map(|s| s.target), Some(0.));
    }

    #[test]
    fn a_settle_ends_exactly_on_its_target() {
        let t0 = Instant::now();
        let mut scroll = Scroll {
            max: 7.,
            snap: true,
            ..Scroll::default()
        };
        scroll.pos = 1.3;
        scroll.settle_to(1., 0., Spring::SPATIAL, t0);
        scroll.advance(t0 + std::time::Duration::from_secs(5), MotionSettings::default());
        assert_eq!(scroll.pos, 1.);
        assert!(!scroll.busy());
    }

    #[test]
    fn the_wheel_snaps_once_it_rests() {
        let t0 = Instant::now();
        let mut scroll = Scroll {
            max: 7.,
            snap: true,
            pos: 2.4,
            wheel_from: 2.,
            last_wheel: Some(t0),
            ..Scroll::default()
        };
        let motion = MotionSettings::default();
        scroll.advance(t0 + std::time::Duration::from_millis(50), motion);
        assert!(scroll.settling.is_none() && scroll.last_wheel.is_some());
        // Turned forward by less than half an item: it still goes on to the next one.
        scroll.advance(t0 + std::time::Duration::from_millis(200), motion);
        assert_eq!(scroll.settling.map(|s| s.target), Some(3.));
    }

    #[test]
    fn one_wheel_notch_moves_one_item_in_its_direction() {
        // Forward a little, backward a little, and not at all.
        assert_eq!(wheel_target(0., 0.24, 7.), 1.);
        assert_eq!(wheel_target(1., 0.76, 7.), 0.);
        assert_eq!(wheel_target(2., 2.0004, 7.), 2.);
        // Several notches from the middle of an item.
        assert_eq!(wheel_target(1., 2.3, 7.), 3.);
        // Never past the ends.
        assert_eq!(wheel_target(6.8, 7., 7.), 7.);
        assert_eq!(wheel_target(0.2, 0., 7.), 0.);
    }
}
