//! Multi-line text area that grows with its content.
//!
//! Shares the editing model of [`super::TextField`] (GPUI's `examples/input.rs`: keyboard
//! editing, mouse selection, clipboard and IME) and adds soft wrapping, vertical caret
//! movement, autosize between `min_rows` and `max_height`, and scrolling beyond that.
//! It draws no background: the host paints the surrounding box (e.g. a composer).

use std::ops::Range;
use std::time::Instant;

use gpui::{
    App, AvailableSpace, Bounds, ClipboardItem, ContentMask, Context, CursorStyle, Element,
    ElementId, ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle,
    Focusable, FontWeight, GlobalElementId, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, Render, ScrollWheelEvent, SharedString, Style,
    TextAlign, TextRun, UTF16Selection, UnderlineStyle, Window, WrappedLine, div, fill, point,
    prelude::*, px, relative, size,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::motion::continuous_at;
use crate::theme::{FONT_FAMILY, Theme, alpha};

/// Kept private so they do not clash with the [`super::TextField`] actions.
mod keys {
    gpui::actions!(
        m3_text_area,
        [
            Backspace,
            Delete,
            Left,
            Right,
            Up,
            Down,
            SelectLeft,
            SelectRight,
            SelectUp,
            SelectDown,
            SelectAll,
            Home,
            End,
            SelectHome,
            SelectEnd,
            Paste,
            Cut,
            Copy,
            Submit,
            Newline,
            Cancel
        ]
    );
}
use keys::*;

const KEY_CONTEXT: &str = "M3TextArea";

/// Registers the text area key bindings. Called by [`crate::init`].
pub fn bind_text_area_keys(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
        KeyBinding::new("secondary-a", SelectAll, ctx),
        KeyBinding::new("secondary-v", Paste, ctx),
        KeyBinding::new("secondary-c", Copy, ctx),
        KeyBinding::new("secondary-x", Cut, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-home", SelectHome, ctx),
        KeyBinding::new("shift-end", SelectEnd, ctx),
        KeyBinding::new("enter", Submit, ctx),
        KeyBinding::new("shift-enter", Newline, ctx),
        KeyBinding::new("escape", Cancel, ctx),
    ]);
}

/// Events emitted by a [`TextArea`]. Subscribe with `cx.subscribe(&area, ...)`.
#[derive(Clone, Debug)]
pub enum TextAreaEvent {
    /// The text changed (typing, paste, cut, [`TextArea::set_text`], [`TextArea::replace_range`]).
    Changed(SharedString),
    /// Enter was pressed.
    Submit(SharedString),
    /// Shift+Enter inserted a line break.
    Newline,
    /// The user pasted. Emitted before the text is inserted; items without text (such as
    /// images) are not inserted, so the host can turn them into attachments.
    Paste(ClipboardItem),
    /// Escape was pressed.
    Cancelled,
}

/// A multi-line text area. Create it once and keep the entity in your view:
///
/// ```ignore
/// let draft = cx.new(|cx| TextArea::new(cx).placeholder("Ask anything").min_rows(2));
/// ```
pub struct TextArea {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    min_rows: usize,
    max_height: Pixels,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    layout: Option<Layout>,
    last_bounds: Option<Bounds<Pixels>>,
    scroll_y: Pixels,
    scroll_to_cursor: bool,
    is_selecting: bool,
    last_edit: Instant,
}

impl EventEmitter<TextAreaEvent> for TextArea {}

impl Focusable for TextArea {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TextArea {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: SharedString::default(),
            placeholder: SharedString::default(),
            min_rows: 1,
            max_height: px(240.),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            layout: None,
            last_bounds: None,
            scroll_y: px(0.),
            scroll_to_cursor: false,
            is_selecting: false,
            last_edit: Instant::now(),
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Minimum height in text rows (1 by default).
    pub fn min_rows(mut self, rows: usize) -> Self {
        self.min_rows = rows.max(1);
        self
    }

    /// Height at which it stops growing and scrolls (240 px by default).
    pub fn max_height(mut self, max_height: Pixels) -> Self {
        self.max_height = max_height;
        self
    }

    pub fn text(&self) -> &SharedString {
        &self.content
    }

    /// Replaces the whole text and puts the caret at the end.
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = text.into();
        self.selected_range = self.content.len()..self.content.len();
        self.marked_range = None;
        self.changed(cx);
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_text("", cx);
    }

    /// Caret position, in UTF-8 bytes.
    pub fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    /// Replaces a byte range (e.g. an `@query` with the chosen mention) and puts the caret after
    /// the inserted text.
    pub fn replace_range(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
        let end = range.end.min(self.content.len());
        let start = range.start.min(end);
        self.content = (self.content[..start].to_owned() + text + &self.content[end..]).into();
        self.selected_range = start + text.len()..start + text.len();
        self.selection_reversed = false;
        self.marked_range = None;
        self.changed(cx);
    }

    pub fn focus(&self, window: &mut Window) {
        window.focus(&self.focus_handle);
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.last_edit = Instant::now();
        self.scroll_to_cursor = true;
        cx.emit(TextAreaEvent::Changed(self.content.clone()));
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.vertical_offset(-1.);
        self.move_to(offset, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.vertical_offset(1.);
        self.move_to(offset, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.vertical_offset(-1.), cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.vertical_offset(1.), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.row_edge(false), cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.row_edge(true), cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.row_edge(false), cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.row_edge(true), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextAreaEvent::Submit(self.content.clone()));
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_text_in_range(None, "\n", window, cx);
        cx.emit(TextAreaEvent::Newline);
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextAreaEvent::Cancelled);
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.is_selecting = true;
        let offset = self.index_for_mouse_position(event.position);
        if event.modifiers.shift {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(layout) = self.layout.as_ref() else {
            return;
        };
        let overflow = (layout.height() - self.max_height).max(px(0.));
        if overflow <= px(0.) {
            return;
        }
        let delta = event.delta.pixel_delta(window.line_height()).y;
        self.scroll_y = (self.scroll_y - delta).clamp(px(0.), overflow);
        self.scroll_to_cursor = false;
        cx.stop_propagation();
        cx.notify();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        cx.emit(TextAreaEvent::Paste(item.clone()));
        if let Some(text) = item.text() {
            self.replace_text_in_range(None, &text.replace("\r\n", "\n"), window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.last_edit = Instant::now();
        self.scroll_to_cursor = true;
        cx.notify()
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.scroll_to_cursor = true;
        cx.notify()
    }

    /// Offset one visual row above (`-1.`) or below (`1.`) the caret, keeping its x.
    fn vertical_offset(&self, direction: f32) -> usize {
        let cursor = self.cursor_offset();
        let Some(layout) = self.layout.as_ref() else {
            return cursor;
        };
        let position = layout.position_for_offset(cursor);
        let y = position.y + layout.line_height * (direction + 0.5);
        if y < px(0.) {
            0
        } else if y >= layout.height() {
            self.content.len()
        } else {
            layout.offset_for_position(point(position.x, y))
        }
    }

    /// Start or end of the caret's visual row.
    fn row_edge(&self, end: bool) -> usize {
        let cursor = self.cursor_offset();
        let Some(layout) = self.layout.as_ref() else {
            return if end { self.content.len() } else { 0 };
        };
        let y = layout.position_for_offset(cursor).y + layout.line_height * 0.5;
        let x = if end { Pixels::MAX / 2. } else { px(0.) };
        layout.offset_for_position(point(x, y))
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(layout)) = (self.last_bounds.as_ref(), self.layout.as_ref()) else {
            return 0;
        };
        if self.content.is_empty() {
            return 0;
        }
        let local = position - bounds.origin + point(px(0.), self.scroll_y);
        if local.y < px(0.) {
            return 0;
        }
        if local.y >= layout.height() {
            return self.content.len();
        }
        layout.offset_for_position(local)
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }
}

impl EntityInputHandler for TextArea {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.selection_reversed = false;
        self.marked_range.take();
        self.changed(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.marked_range =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        self.changed(cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let origin = bounds.origin - point(px(0.), self.scroll_y);
        let start = layout.position_for_offset(range.start);
        let end = layout.position_for_offset(range.end);
        Some(Bounds::from_corners(
            origin + start,
            origin + point(end.x, end.y + layout.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let layout = self.layout.as_ref()?;
        let local = point - bounds.origin + gpui::point(px(0.), self.scroll_y);
        Some(self.offset_to_utf16(layout.offset_for_position(local)))
    }
}

/// The wrapped lines of the text (or the placeholder), one per hard line.
#[derive(Clone)]
struct Layout {
    /// Byte offset where each hard line starts, and its wrapped layout.
    lines: Vec<(usize, WrappedLine)>,
    line_height: Pixels,
}

impl Layout {
    fn shape(
        text: SharedString,
        runs: &[TextRun],
        font_size: Pixels,
        line_height: Pixels,
        wrap_width: Option<Pixels>,
        window: &Window,
    ) -> Self {
        let wrapped = window
            .text_system()
            .shape_text(text, font_size, runs, wrap_width, None)
            .unwrap_or_default();
        let mut start = 0;
        let lines = wrapped
            .into_iter()
            .map(|line| {
                let line_start = start;
                start += line.len() + 1;
                (line_start, line)
            })
            .collect();
        Self { lines, line_height }
    }

    fn rows(line: &WrappedLine) -> usize {
        line.wrap_boundaries().len() + 1
    }

    fn height(&self) -> Pixels {
        let rows: usize = self.lines.iter().map(|(_, line)| Self::rows(line)).sum();
        self.line_height * rows.max(1) as f32
    }

    /// Top-left of the caret for `offset`, relative to the text origin.
    fn position_for_offset(&self, offset: usize) -> Point<Pixels> {
        let mut y = px(0.);
        for (start, line) in &self.lines {
            if offset <= start + line.len() {
                let local = offset.saturating_sub(*start);
                let position = line
                    .position_for_index(local, self.line_height)
                    .unwrap_or_default();
                return point(position.x, y + position.y);
            }
            y += self.line_height * Self::rows(line) as f32;
        }
        point(px(0.), y)
    }

    /// Closest offset to a position relative to the text origin.
    fn offset_for_position(&self, position: Point<Pixels>) -> usize {
        let mut y = px(0.);
        let last = self.lines.len().saturating_sub(1);
        for (i, (start, line)) in self.lines.iter().enumerate() {
            let height = self.line_height * Self::rows(line) as f32;
            if position.y < y + height || i == last {
                let local = point(
                    position.x.max(px(0.)),
                    (position.y - y).clamp(px(0.), height - px(1.)),
                );
                let index = match line.closest_index_for_position(local, self.line_height) {
                    Ok(index) | Err(index) => index,
                };
                return start + index.min(line.len());
            }
            y += height;
        }
        0
    }
}

/// Text runs of the shaped text, underlining the IME composition.
fn text_runs(len: usize, run: TextRun, marked: Option<&Range<usize>>) -> Vec<TextRun> {
    let Some(marked) = marked else {
        return vec![run];
    };
    vec![
        TextRun {
            len: marked.start,
            ..run.clone()
        },
        TextRun {
            len: marked.end - marked.start,
            underline: Some(UnderlineStyle {
                color: Some(run.color),
                thickness: px(1.),
                wavy: false,
            }),
            ..run.clone()
        },
        TextRun {
            len: len - marked.end,
            ..run
        },
    ]
    .into_iter()
    .filter(|run| run.len > 0)
    .collect()
}

/// Shapes, sizes and paints the text, selection and caret.
struct TextBody {
    area: Entity<TextArea>,
}

struct BodyPaint {
    layout: Layout,
    selection: Vec<Bounds<Pixels>>,
    cursor: Option<Bounds<Pixels>>,
}

impl IntoElement for TextBody {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl TextBody {
    /// Text to shape (the placeholder when empty) and its runs.
    fn source(&self, window: &Window, cx: &App) -> (SharedString, Vec<TextRun>) {
        let s = Theme::of(cx);
        let area = self.area.read(cx);
        let style = window.text_style();
        let (text, color) = if area.content.is_empty() {
            (area.placeholder.clone(), s.outline)
        } else {
            (area.content.clone(), style.color)
        };
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let marked = if area.content.is_empty() {
            None
        } else {
            area.marked_range.as_ref()
        };
        let runs = text_runs(text.len(), run, marked);
        (text, runs)
    }
}

impl Element for TextBody {
    type RequestLayoutState = ();
    type PrepaintState = BodyPaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let (text, runs) = self.source(window, cx);
        let (min_rows, max_height) = {
            let area = self.area.read(cx);
            (area.min_rows, area.max_height)
        };
        let font_size = window.text_style().font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let layout_id =
            window.request_measured_layout(style, move |known, available, window, _| {
                let width = known.width.or(match available.width {
                    AvailableSpace::Definite(width) => Some(width),
                    _ => None,
                });
                let layout =
                    Layout::shape(text.clone(), &runs, font_size, line_height, width, window);
                let height = layout
                    .height()
                    .max(line_height * min_rows as f32)
                    .min(max_height);
                size(width.unwrap_or(px(0.)), height)
            });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let (text, runs) = self.source(window, cx);
        let font_size = window.text_style().font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let layout = Layout::shape(
            text,
            &runs,
            font_size,
            line_height,
            Some(bounds.size.width),
            window,
        );

        let (selected_range, cursor, scroll_to_cursor, mut scroll_y, empty) = {
            let area = self.area.read(cx);
            (
                area.selected_range.clone(),
                area.cursor_offset(),
                area.scroll_to_cursor,
                area.scroll_y,
                area.content.is_empty(),
            )
        };
        // Keep the caret visible after edits and keyboard moves; otherwise only clamp.
        let overflow = (layout.height() - bounds.size.height).max(px(0.));
        let caret = layout.position_for_offset(if empty { 0 } else { cursor });
        if scroll_to_cursor {
            if caret.y < scroll_y {
                scroll_y = caret.y;
            } else if caret.y + line_height > scroll_y + bounds.size.height {
                scroll_y = caret.y + line_height - bounds.size.height;
            }
        }
        let scroll_y = scroll_y.clamp(px(0.), overflow);
        self.area.update(cx, |area, _| {
            area.scroll_y = scroll_y;
            area.scroll_to_cursor = false;
        });
        let origin = bounds.origin - point(px(0.), scroll_y);

        let mut selection = Vec::new();
        let cursor = if selected_range.is_empty() || empty {
            Some(Bounds::new(origin + caret, size(px(2.), line_height)))
        } else {
            let start = layout.position_for_offset(selected_range.start);
            let end = layout.position_for_offset(selected_range.end);
            let right = bounds.size.width;
            if start.y == end.y {
                selection.push(Bounds::from_corners(
                    origin + start,
                    origin + point(end.x, end.y + line_height),
                ));
            } else {
                selection.push(Bounds::from_corners(
                    origin + start,
                    origin + point(right, start.y + line_height),
                ));
                if end.y > start.y + line_height {
                    selection.push(Bounds::from_corners(
                        origin + point(px(0.), start.y + line_height),
                        origin + point(right, end.y),
                    ));
                }
                selection.push(Bounds::from_corners(
                    origin + point(px(0.), end.y),
                    origin + point(end.x, end.y + line_height),
                ));
            }
            None
        };
        BodyPaint {
            layout,
            selection,
            cursor,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let s = *Theme::of(cx);
        let (focus_handle, last_edit, scroll_y) = {
            let area = self.area.read(cx);
            (area.focus_handle.clone(), area.last_edit, area.scroll_y)
        };
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.area.clone()),
            cx,
        );
        let line_height = prepaint.layout.line_height;
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for selection in prepaint.selection.drain(..) {
                window.paint_quad(fill(selection, alpha(s.primary, 0.3)));
            }
            let mut y = bounds.top() - scroll_y;
            for (_, line) in &prepaint.layout.lines {
                let height = line_height * Layout::rows(line) as f32;
                if y + height >= bounds.top() && y <= bounds.bottom() {
                    line.paint(
                        point(bounds.left(), y),
                        line_height,
                        TextAlign::Left,
                        Some(bounds),
                        window,
                        cx,
                    )
                    .ok();
                }
                y += height;
            }
            // The caret stays solid while typing and blinks every 0.53 s after that.
            if focus_handle.is_focused(window)
                && let Some(cursor) = prepaint.cursor.take()
            {
                let since_edit = last_edit.elapsed().as_secs_f32();
                if since_edit < 0.5 || ((since_edit / 0.53) as u32).is_multiple_of(2) {
                    window.paint_quad(fill(cursor, s.primary));
                }
            }
        });
        let layout = prepaint.layout.clone();
        self.area.update(cx, |area, _| {
            area.layout = Some(layout);
            area.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        if self.focus_handle.is_focused(window) {
            // Caret blink: a few frames per second is enough.
            continuous_at(4., window, cx);
        }
        div()
            .id(("m3-text-area", cx.entity_id().as_u64()))
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::cancel))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .w_full()
            .text_color(s.on_surface)
            .font_family(FONT_FAMILY)
            .text_size(px(13.5))
            .font_weight(FontWeight(400.))
            .line_height(relative(1.5))
            .child(TextBody { area: cx.entity() })
    }
}
