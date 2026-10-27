//! Text the reader can select and copy, like in a document: drag to select (across the
//! paragraphs of a [`Markdown`](super::Markdown), and across messages inside a
//! [`SelectionScope`]; past the edge of a scrolling list it scrolls), double-click for a word,
//! triple-click for the paragraph, Shift+click to extend, and Cmd/Ctrl+C to copy. Cmd/Ctrl+A
//! selects everything and Escape clears it. A right click opens a menu with Copy, Copy all and
//! Select all ([`set_text_menu`] lets the app use its own, such as the native one on macOS).
//!
//! A [`TextSelection`] is the state of one group of text blocks. Each block is a
//! [`StyledText`] wrapped by [`selectable_block`]. The group's root ([`selection_root`])
//! follows the drag and takes focus once something is selected, so the copy key reaches it.
//!
//! On its own, a `Markdown`, `CodeBlock`, `CodeOutput` or [`SelectableText`] is a group of its
//! own. Inside a [`SelectionScope`] (one per message of a chat, all sharing the chat's
//! selection), its blocks join that group under the scope's `owner`: the selection then runs
//! from one message to the next, and remembers the text of messages that scrolled away.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Bounds, ClipboardItem, Context, CursorStyle, DispatchPhase, Div, Element,
    ElementId, Entity, FocusHandle, Global, GlobalElementId, HitboxBehavior, Hsla,
    InspectorElementId, InteractiveElement, IntoElement, KeyDownEvent, LayoutId, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    PlatformInput, Point, RenderOnce, ScrollDelta, ScrollWheelEvent, SharedString, Styled,
    StyledText, TextLayout, TouchPhase, Window, anchored, canvas, deferred, div, fill, point, px,
};

use super::{Menu, MenuItem};
use crate::theme::{Theme, alpha};

/// A block: (owner, number within the owner), in reading order.
type Key = (u64, u32);
/// A position: (block, byte offset in its text).
type Spot = (Key, usize);

/// How often the list scrolls while a drag sits past its edge.
const EDGE_TICK: Duration = Duration::from_millis(16);

struct Block {
    layout: TextLayout,
    /// The text's own bounds (they may scroll out of view).
    bounds: Bounds<Pixels>,
    /// The part of it that shows.
    visible: Bounds<Pixels>,
    text: SharedString,
    /// What goes before this block's text when the copy also takes the one before it in the
    /// same owner (between owners, a blank line).
    joint: SharedString,
    /// What "Copy all" copies on a right click over it (its Markdown source).
    whole: Option<SharedString>,
    /// The frame it was last painted in: blocks of messages that scrolled away keep their
    /// text for a copy, but are not under the pointer.
    frame: u64,
}

/// The selection of one group of text blocks.
pub struct TextSelection {
    focus: FocusHandle,
    blocks: BTreeMap<Key, Block>,
    /// Blocks painted so far in this frame, per owner.
    counts: HashMap<u64, u32>,
    frame: u64,
    anchor: Option<Spot>,
    head: Option<Spot>,
    dragging: bool,
    /// Where the pointer is during a drag.
    pointer: Point<Pixels>,
    /// The area the group shows in (its scrolling list's), to scroll on a drag past it.
    viewport: Option<Bounds<Pixels>>,
    edge_scrolling: bool,
    /// When the root was last painted: one that scrolled out of view stops getting the
    /// mouse, so the edge scroll must not go on for it.
    painted: Option<Instant>,
    /// Shared by the messages of a chat (made with [`TextSelection::shared`]).
    shared: bool,
    /// Where the drawn right-click menu is open, and the block it was opened on.
    menu: Option<(Point<Pixels>, Key)>,
}

impl TextSelection {
    fn new(shared: bool, cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            blocks: BTreeMap::new(),
            counts: HashMap::new(),
            frame: 0,
            anchor: None,
            head: None,
            dragging: false,
            pointer: Point::default(),
            viewport: None,
            edge_scrolling: false,
            painted: None,
            shared,
            menu: None,
        }
    }

    /// The state of the group `id`, kept while it keeps being painted.
    pub fn of(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> Entity<Self> {
        window.use_keyed_state(id, cx, |_, cx| Self::new(false, cx))
    }

    /// A selection for a whole chat: keep it, wrap the chat in [`selection_root`] and each
    /// message in a [`SelectionScope`].
    pub fn shared(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(true, cx))
    }

    /// Start and end, in order; `None` when nothing is selected.
    fn range(&self) -> Option<(Spot, Spot)> {
        let (a, h) = (self.anchor?, self.head?);
        (a != h).then(|| if a <= h { (a, h) } else { (h, a) })
    }

    /// The part of block `key` that is selected.
    fn block_range(&self, key: Key) -> Option<Range<usize>> {
        let ((sk, so), (ek, eo)) = self.range()?;
        let text = &self.blocks.get(&key)?.text;
        if key < sk || key > ek {
            return None;
        }
        let start = if key == sk { floor_char(text, so) } else { 0 };
        let end = if key == ek { floor_char(text, eo) } else { text.len() };
        (start < end).then_some(start..end)
    }

    /// The selected text, the blocks joined by their joints (a blank line between
    /// paragraphs and messages, a line break and the marker between list items, a tab
    /// between cells).
    pub fn selected_text(&self) -> String {
        let Some(((sk, _), (ek, _))) = self.range() else {
            return String::new();
        };
        let mut out = String::new();
        let mut last_owner = None;
        for (&key, block) in self.blocks.range(sk..=ek) {
            if let Some(range) = self.block_range(key) {
                if !out.is_empty() {
                    out.push_str(if last_owner == Some(key.0) { &block.joint } else { "\n\n" });
                }
                out.push_str(&block.text[range]);
                last_owner = Some(key.0);
            }
        }
        out
    }

    /// What "Copy all" copies for block `key`: its source, or its owner's text.
    fn whole_text(&self, key: Key) -> String {
        if let Some(whole) = self.blocks.get(&key).and_then(|b| b.whole.clone()) {
            return whole.to_string();
        }
        let mut out = String::new();
        for (k, block) in self.blocks.range((key.0, 0)..=(key.0, u32::MAX)) {
            if k.1 > 0 {
                out.push_str(&block.joint);
            }
            out.push_str(&block.text);
        }
        out
    }

    fn select_all(&mut self) -> bool {
        let (Some((&first, _)), Some((&last, block))) =
            (self.blocks.first_key_value(), self.blocks.last_key_value())
        else {
            return false;
        };
        self.anchor = Some((first, 0));
        self.head = Some((last, block.text.len()));
        true
    }

    /// The blocks under the pointer this frame.
    fn shown(&self) -> impl Iterator<Item = (&Key, &Block)> {
        let frame = self.frame;
        self.blocks.iter().filter(move |(_, b)| b.frame == frame)
    }

    /// Where the pointer at `pos` falls: inside a block, or the start of the next one.
    fn spot_at(&self, pos: Point<Pixels>) -> Option<Spot> {
        let mut last = None;
        for (&key, block) in self.shown() {
            if pos.y < block.bounds.top() {
                return Some((key, 0));
            }
            if pos.y <= block.bounds.bottom() {
                return Some((key, index_at(block, pos)));
            }
            last = Some((key, block.text.len()));
        }
        last
    }

    fn clear(&mut self) -> bool {
        let had = self.range().is_some();
        self.anchor = None;
        self.head = None;
        self.dragging = false;
        had
    }

    /// Owner `owner` is about to paint its blocks again.
    fn restart(&mut self, owner: u64) {
        self.counts.remove(&owner);
        self.blocks.retain(|k, _| k.0 != owner);
    }
}

fn index_at(block: &Block, pos: Point<Pixels>) -> usize {
    let ix = match block.layout.index_for_position(pos) {
        Ok(ix) | Err(ix) => ix,
    };
    floor_char(&block.text, ix)
}

fn floor_char(text: &str, mut ix: usize) -> usize {
    ix = ix.min(text.len());
    while !text.is_char_boundary(ix) {
        ix -= 1;
    }
    ix
}

/// The word around `ix` (letters, digits and `_`); off a word, the single character.
pub fn word_at(text: &str, ix: usize) -> Range<usize> {
    let ix = floor_char(text, ix);
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let at = text[ix..].chars().next();
    let on_word = at.is_some_and(is_word)
        || (at.is_none() && text[..ix].chars().next_back().is_some_and(is_word));
    if !on_word {
        return ix..at.map_or(ix, |c| ix + c.len_utf8());
    }
    let start = text[..ix]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map_or(ix, |(i, _)| i);
    let end = text[ix..]
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(text.len(), |(i, _)| ix + i);
    start..end
}

/* ─────────── Scope ─────────── */

/// Where the selectable text being built goes: the group and its owner.
#[derive(Clone)]
pub struct Selection {
    pub state: Entity<TextSelection>,
    pub owner: u64,
}

thread_local! {
    static SCOPE: RefCell<Vec<Selection>> = const { RefCell::new(Vec::new()) };
}

/// The scope the element being laid out is in, if any.
pub fn current_selection() -> Option<Selection> {
    SCOPE.with(|s| s.borrow().last().cloned())
}

/// One message of a chat whose selection is shared: the selectable text inside joins
/// `state` as `owner` (the message's place in the chat, so the order is the reading order).
pub struct SelectionScope {
    scope: Selection,
    child: Option<AnyElement>,
}

impl SelectionScope {
    pub fn new(state: Entity<TextSelection>, owner: u64, child: impl IntoElement) -> Self {
        Self {
            scope: Selection { state, owner },
            child: Some(child.into_any_element()),
        }
    }
}

impl IntoElement for SelectionScope {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for SelectionScope {
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
        SCOPE.with(|s| s.borrow_mut().push(self.scope.clone()));
        let id = child.request_layout(window, cx);
        SCOPE.with(|s| s.borrow_mut().pop());
        (id, child)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let owner = self.scope.owner;
        self.scope.state.update(cx, |s, _| s.restart(owner));
        child.paint(window, cx);
    }
}

/* ─────────── Right-click menu ─────────── */

/// What a right click on selectable text offers.
pub struct TextMenuRequest {
    /// Where the click was, in window coordinates.
    pub at: Point<Pixels>,
    /// The selection, if there is one.
    pub selected: Option<String>,
    /// The whole text under the click ("Copy all").
    pub all: String,
    /// Selects everything in the group.
    pub select_all: Rc<dyn Fn(&mut App)>,
}

type TextMenuHandler = Rc<dyn Fn(TextMenuRequest, &mut Window, &mut App)>;

struct TextMenu {
    labels: Rc<dyn Fn() -> [SharedString; 3]>,
    native: Option<TextMenuHandler>,
}

impl Global for TextMenu {}

/// The labels of the drawn right-click menu (Copy, Copy all, Select all; asked each time,
/// so they follow the app's language), and optionally a handler that shows the app's own
/// menu instead.
pub fn set_text_menu(
    cx: &mut App,
    labels: impl Fn() -> [SharedString; 3] + 'static,
    native: Option<impl Fn(TextMenuRequest, &mut Window, &mut App) + 'static>,
) {
    cx.set_global(TextMenu {
        labels: Rc::new(labels),
        native: native.map(|f| Rc::new(f) as TextMenuHandler),
    });
}

fn menu_labels(cx: &App) -> [SharedString; 3] {
    cx.try_global::<TextMenu>()
        .map(|m| (m.labels)())
        .unwrap_or_else(|| ["Copy".into(), "Copy all".into(), "Select all".into()])
}

fn open_menu(
    state: &Entity<TextSelection>,
    key: Key,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let native = cx.try_global::<TextMenu>().and_then(|m| m.native.clone());
    let Some(native) = native else {
        state.update(cx, |s, cx| {
            s.menu = Some((at, key));
            cx.notify();
        });
        return;
    };
    let sel = state.read(cx);
    let selected = Some(sel.selected_text()).filter(|t| !t.is_empty());
    let all = sel.whole_text(key);
    let target = state.clone();
    let select_all: Rc<dyn Fn(&mut App)> = Rc::new(move |cx: &mut App| {
        target.update(cx, |s, cx| {
            if s.select_all() {
                cx.notify();
            }
        })
    });
    native(TextMenuRequest { at, selected, all, select_all }, window, cx);
}

fn drawn_menu(state: &Entity<TextSelection>, at: Point<Pixels>, key: Key, cx: &App) -> AnyElement {
    let [copy, copy_all, select_all] = menu_labels(cx);
    let sel = state.read(cx);
    let selected = sel.selected_text();
    let all = sel.whole_text(key);
    let close = |state: &Entity<TextSelection>| {
        let state = state.clone();
        move |cx: &mut App| {
            state.update(cx, |s, cx| {
                s.menu = None;
                cx.notify();
            })
        }
    };
    let mut menu = Menu::new("text-menu").width(200.).stagger(false);
    if !selected.is_empty() {
        let done = close(state);
        menu = menu.item(MenuItem::new("text-copy", copy).icon("copy").on_click(move |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(selected.clone()));
            done(cx);
        }));
    }
    let done = close(state);
    menu = menu.item(MenuItem::new("text-copy-all", copy_all).icon("copy").on_click(move |_, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(all.clone()));
        done(cx);
    }));
    let target = state.clone();
    menu = menu.item(MenuItem::new("text-select-all", select_all).on_click(move |_, _, cx| {
        target.update(cx, |s, cx| {
            s.select_all();
            s.menu = None;
            cx.notify();
        })
    }));
    let done = close(state);
    deferred(
        anchored()
            .position(at)
            .snap_to_window_with_margin(px(8.))
            .child(div().occlude().on_mouse_down_out(move |_, _, cx| done(cx)).child(menu)),
    )
    .with_priority(1)
    .into_any_element()
}

/* ─────────── Root and blocks ─────────── */

/// The root of a group: follows the drag across its blocks, takes the copy and select-all
/// keys once it has focus, and shows the drawn right-click menu. It adds an absolutely
/// placed layer, so it makes `root` `relative()`.
pub fn selection_root(state: &Entity<TextSelection>, root: Div, cx: &App) -> Div {
    let focus = state.read(cx).focus.clone();
    let menu = state.read(cx).menu;
    let keys_state = state.clone();
    let paint_state = state.clone();
    root.relative()
        .track_focus(&focus)
        .on_key_down(move |event: &KeyDownEvent, _, cx| on_key(&keys_state, event, cx))
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    let mask = window.content_mask().bounds;
                    paint_state.update(cx, |s, _| {
                        s.frame += 1;
                        // A chat's root is its list: scroll at its edges. A message on its
                        // own scrolls the list it is in, at the clip around it.
                        s.viewport = Some(if s.shared { bounds.intersect(&mask) } else { mask });
                        s.painted = Some(Instant::now());
                        if !s.shared {
                            s.restart(0);
                        }
                    });
                    follow(&paint_state, window);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .when_some(menu, |root, (at, key)| root.child(drawn_menu(state, at, key, cx)))
}

fn on_key(state: &Entity<TextSelection>, event: &KeyDownEvent, cx: &mut App) {
    let keys = &event.keystroke;
    let key = keys.key.as_str();
    if keys.modifiers.secondary() && key == "c" {
        let text = state.read(cx).selected_text();
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            cx.stop_propagation();
        }
    } else if keys.modifiers.secondary() && key == "a" {
        state.update(cx, |s, cx| {
            if s.select_all() {
                cx.notify();
            }
        });
        cx.stop_propagation();
    } else if key == "escape" {
        let had = state.update(cx, |s, cx| {
            let had = s.clear() || s.menu.take().is_some();
            if had {
                cx.notify();
            }
            had
        });
        if had {
            cx.stop_propagation();
        }
    }
}

/// The drag over the whole group: moves the end of the selection, scrolls the list when
/// the pointer goes past its edge, and clears the selection on a press outside the group.
/// All in the capture phase: an element painted above the text (the list, the composer) may
/// stop these events on their way up.
fn follow(state: &Entity<TextSelection>, window: &mut Window) {
    let s = state.clone();
    window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
        if phase != DispatchPhase::Capture {
            return;
        }
        let inside = s.read(cx).shown().any(|(_, b)| b.visible.contains(&event.position));
        if !inside {
            s.update(cx, |sel, cx| {
                if sel.clear() {
                    cx.notify();
                }
            });
        }
    });
    let s = state.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture || !s.read(cx).dragging {
            return;
        }
        if event.pressed_button != Some(MouseButton::Left) {
            s.update(cx, |sel, _| sel.dragging = false);
            return;
        }
        s.update(cx, |sel, cx| {
            sel.pointer = event.position;
            let head = sel.spot_at(event.position);
            if head.is_some() && head != sel.head {
                sel.head = head;
                cx.notify();
            }
        });
        if edge_step(&s, cx).is_some() && !s.read(cx).edge_scrolling {
            edge_scroll(&s, window, cx);
        }
    });
    let s = state.clone();
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture || event.button != MouseButton::Left || !s.read(cx).dragging {
            return;
        }
        s.update(cx, |sel, _| sel.dragging = false);
        if s.read(cx).range().is_some() {
            let focus = s.read(cx).focus.clone();
            window.focus(&focus);
        }
    });
}

/// How far to scroll this tick while the drag is past the edge of the viewport (positive:
/// toward the start), and the point inside the viewport to scroll at.
fn edge_step(state: &Entity<TextSelection>, cx: &App) -> Option<(Pixels, Point<Pixels>)> {
    let s = state.read(cx);
    let view = s.viewport?;
    if !s.dragging {
        return None;
    }
    // A message on its own scrolls only while it has text past that edge: scrolled out of
    // view, it would no longer hear the release. A chat's root is always there.
    let (mut top, mut bottom) = (view.top(), view.bottom());
    if !s.shared {
        for (_, b) in s.shown() {
            top = if b.bounds.top() < top { b.bounds.top() } else { top };
            bottom = if b.bounds.bottom() > bottom { b.bounds.bottom() } else { bottom };
        }
    }
    let pos = s.pointer;
    let x = pos.x.clamp(view.left() + px(1.), view.right() - px(1.));
    let speed = |past: Pixels| (past / px(4.)).clamp(2., 40.);
    if pos.y < view.top() && (s.shared || top < view.top()) {
        Some((px(speed(view.top() - pos.y)), point(x, view.top() + px(1.))))
    } else if pos.y > view.bottom() && (s.shared || bottom > view.bottom()) {
        Some((px(-speed(pos.y - view.bottom())), point(x, view.bottom() - px(1.))))
    } else {
        None
    }
}

/// Scrolls the list under the group while the drag stays past its edge, with the same
/// wheel events the list already takes.
fn edge_scroll(state: &Entity<TextSelection>, window: &mut Window, cx: &mut App) {
    state.update(cx, |s, _| s.edge_scrolling = true);
    let handle = window.window_handle();
    let state = state.clone();
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(EDGE_TICK).await;
            let more = handle.update(cx, |_, window, cx| {
                // Not painted lately: it left the view and no longer hears the release.
                if state.read(cx).painted.is_none_or(|t| t.elapsed() > EDGE_TICK * 8) {
                    state.update(cx, |s, _| s.dragging = false);
                    return false;
                }
                let Some((dy, at)) = edge_step(&state, cx) else {
                    return false;
                };
                window.dispatch_event(
                    PlatformInput::ScrollWheel(ScrollWheelEvent {
                        position: at,
                        delta: ScrollDelta::Pixels(point(px(0.), dy)),
                        modifiers: Modifiers::default(),
                        touch_phase: TouchPhase::Moved,
                    }),
                    cx,
                );
                state.update(cx, |s, cx| {
                    let head = s.spot_at(s.pointer);
                    if head.is_some() && head != s.head {
                        s.head = head;
                        cx.notify();
                    }
                });
                true
            });
            if !more.unwrap_or(false) {
                break;
            }
        }
        let _ = state.update(cx, |s, _| s.edge_scrolling = false);
    })
    .detach();
}

/// A selectable block in `group` (numbered in paint order within its owner): the text of
/// `layout` (a `StyledText`'s) drawn by `inner` (that `StyledText`, or an `InteractiveText`
/// around it), with the selection painted behind. `joint` goes before its text when a copy
/// also takes the block before it; `whole` is what "Copy all" copies over it.
pub fn selectable_block(
    group: &Selection,
    layout: TextLayout,
    source: SharedString,
    joint: SharedString,
    whole: Option<SharedString>,
    inner: AnyElement,
    color: Hsla,
) -> AnyElement {
    let state = group.state.clone();
    let owner = group.owner;
    div()
        .relative()
        .child(
            canvas(
                |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                move |bounds, hitbox, window, cx| {
                    window.set_cursor_style(CursorStyle::IBeam, &hitbox);
                    let visible = window.content_mask().bounds.intersect(&bounds);
                    let (key, selected) = state.update(cx, |s, _| {
                        let n = s.counts.entry(owner).or_default();
                        let key = (owner, *n);
                        *n += 1;
                        let frame = s.frame;
                        s.blocks.insert(
                            key,
                            Block {
                                layout: layout.clone(),
                                bounds,
                                visible,
                                text: source.clone(),
                                joint: joint.clone(),
                                whole: whole.clone(),
                                frame,
                            },
                        );
                        (key, s.block_range(key))
                    });
                    if let Some(range) = selected {
                        paint_range(&layout, bounds, range, color, window);
                    }
                    press(&state, key, hitbox, window);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .child(inner)
        .into_any_element()
}

/// The group for a selectable component: the scope it is in, or its own (`own` is then
/// true and the component wraps itself in [`selection_root`]).
pub fn group_for(id: ElementId, window: &mut Window, cx: &mut App) -> (Selection, bool) {
    match current_selection() {
        Some(scope) => (scope, false),
        None => (Selection { state: TextSelection::of(id, window, cx), owner: 0 }, true),
    }
}

fn paint_range(
    layout: &TextLayout,
    bounds: Bounds<Pixels>,
    range: Range<usize>,
    color: Hsla,
    window: &mut Window,
) {
    let (Some(a), Some(b)) = (
        layout.position_for_index(range.start),
        layout.position_for_index(range.end),
    ) else {
        return;
    };
    let lh = layout.line_height();
    let rect = |x0: Pixels, y0: Pixels, x1: Pixels, y1: Pixels| {
        fill(Bounds::from_corners(point(x0, y0), point(x1, y1)), color)
    };
    if a.y == b.y {
        window.paint_quad(rect(a.x, a.y, b.x, a.y + lh));
        return;
    }
    window.paint_quad(rect(a.x, a.y, bounds.right(), a.y + lh));
    if b.y > a.y + lh {
        window.paint_quad(rect(bounds.left(), a.y + lh, bounds.right(), b.y));
    }
    window.paint_quad(rect(bounds.left(), b.y, b.x, b.y + lh));
}

/// A press on block `key`: starts a selection there (a word on a double click, the block
/// on a triple click, up to there with Shift); a right click opens the menu. In the capture
/// phase too, like the drag.
fn press(state: &Entity<TextSelection>, key: Key, hitbox: gpui::Hitbox, window: &mut Window) {
    let s = state.clone();
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture || !hitbox.is_hovered(window) {
            return;
        }
        if event.button == MouseButton::Right {
            open_menu(&s, key, event.position, window, cx);
            return;
        }
        if event.button != MouseButton::Left {
            return;
        }
        s.update(cx, |sel, cx| {
            let Some(block) = sel.blocks.get(&key) else {
                return;
            };
            let ix = index_at(block, event.position);
            let len = block.text.len();
            sel.pointer = event.position;
            if event.modifiers.shift && sel.anchor.is_some() {
                sel.head = Some((key, ix));
                sel.dragging = true;
            } else {
                let range = match event.click_count {
                    0 | 1 => ix..ix,
                    2 => word_at(&block.text, ix),
                    _ => 0..len,
                };
                sel.anchor = Some((key, range.start));
                sel.head = Some((key, range.end));
                sel.dragging = event.click_count <= 1;
            }
            sel.menu = None;
            cx.notify();
        });
        if s.read(cx).range().is_some() {
            let focus = s.read(cx).focus.clone();
            window.focus(&focus);
        }
    });
}

/// Plain text that can be selected and copied, in the style around it.
#[derive(IntoElement)]
pub struct SelectableText {
    id: ElementId,
    text: SharedString,
}

impl SelectableText {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
        }
    }
}

impl RenderOnce for SelectableText {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = alpha(Theme::of(cx).primary, 0.3);
        let (group, own) = group_for(self.id, window, cx);
        let styled = StyledText::new(self.text.clone());
        let layout = styled.layout().clone();
        let block = selectable_block(
            &group,
            layout,
            self.text,
            SharedString::default(),
            None,
            styled.into_any_element(),
            color,
        );
        let root = if own { selection_root(&group.state, div(), cx) } else { div() };
        root.child(block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_click_takes_the_word() {
        let text = "hola, señor_x mundo";
        assert_eq!(&text[word_at(text, 1)], "hola");
        assert_eq!(&text[word_at(text, 8)], "señor_x");
        assert_eq!(&text[word_at(text, 4)], ",");
        assert_eq!(&text[word_at(text, text.len())], "mundo");
    }
}
