//! Code editor: a multi-line, monospace editing surface with line numbers and a virtualized
//! list of lines, for files of any size.
//!
//! The editing rules live in [`CodeBuffer`] (tested without GPUI). This file wires them to keys,
//! the mouse, the clipboard and the platform input handler (IME), the same way
//! [`super::TextArea`] does (after GPUI's `examples/input.rs`), and paints only the visible lines
//! with `uniform_list`. Syntax colors come from a [`LineHighlighter`].

use std::ops::{Range, RangeInclusive};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, ClipboardItem, ContentMask, Context, CursorStyle, ElementInputHandler,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, HighlightStyle, KeyBinding,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Render,
    ScrollStrategy, ScrollWheelEvent, ShapedLine, SharedString, TextRun, UTF16Selection,
    UnderlineStyle, UniformListScrollHandle, Window, canvas, div, fill, font, point, prelude::*,
    px, size, uniform_list,
};

use super::code_buffer::{BufferPos, BufferSelection, CodeBuffer};
use crate::motion::continuous_at;
use crate::theme::{MONO_FONT_FAMILY, Theme, alpha};

mod keys {
    gpui::actions!(
        m3_code_editor,
        [
            Backspace,
            Delete,
            DeleteWordLeft,
            DeleteWordRight,
            Left,
            Right,
            Up,
            Down,
            SelectLeft,
            SelectRight,
            SelectUp,
            SelectDown,
            WordLeft,
            WordRight,
            SelectWordLeft,
            SelectWordRight,
            Home,
            End,
            SelectHome,
            SelectEnd,
            DocStart,
            DocEnd,
            SelectDocStart,
            SelectDocEnd,
            PageUp,
            PageDown,
            SelectPageUp,
            SelectPageDown,
            SelectAll,
            Copy,
            Cut,
            Paste,
            Undo,
            Redo,
            Newline,
            Indent,
            Outdent,
            Save,
            Cancel
        ]
    );
}
use keys::*;

const KEY_CONTEXT: &str = "M3CodeEditor";
/// Space between the line numbers and the text, and before the text.
const TEXT_PAD: f32 = 8.;

/// Registers the code editor key bindings. Called by [`crate::init`].
pub fn bind_code_editor_keys(cx: &mut App) {
    cx.bind_keys(code_editor_bindings());
}

pub(crate) fn code_editor_bindings() -> Vec<KeyBinding> {
    let ctx = Some(KEY_CONTEXT);
    // Word jumps use Alt on macOS and Ctrl elsewhere.
    #[cfg(target_os = "macos")]
    let word = "alt";
    #[cfg(not(target_os = "macos"))]
    let word = "ctrl";
    let w = |key: &str| format!("{word}-{key}");
    vec![
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("shift-backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new(&w("backspace"), DeleteWordLeft, ctx),
        KeyBinding::new(&w("delete"), DeleteWordRight, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
        KeyBinding::new(&w("left"), WordLeft, ctx),
        KeyBinding::new(&w("right"), WordRight, ctx),
        KeyBinding::new(&w("shift-left"), SelectWordLeft, ctx),
        KeyBinding::new(&w("shift-right"), SelectWordRight, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-home", SelectHome, ctx),
        KeyBinding::new("shift-end", SelectEnd, ctx),
        KeyBinding::new("secondary-home", DocStart, ctx),
        KeyBinding::new("secondary-end", DocEnd, ctx),
        KeyBinding::new("secondary-up", DocStart, ctx),
        KeyBinding::new("secondary-down", DocEnd, ctx),
        KeyBinding::new("secondary-shift-home", SelectDocStart, ctx),
        KeyBinding::new("secondary-shift-end", SelectDocEnd, ctx),
        KeyBinding::new("pageup", PageUp, ctx),
        KeyBinding::new("pagedown", PageDown, ctx),
        KeyBinding::new("shift-pageup", SelectPageUp, ctx),
        KeyBinding::new("shift-pagedown", SelectPageDown, ctx),
        KeyBinding::new("secondary-a", SelectAll, ctx),
        KeyBinding::new("secondary-c", Copy, ctx),
        KeyBinding::new("secondary-x", Cut, ctx),
        KeyBinding::new("secondary-v", Paste, ctx),
        KeyBinding::new("secondary-z", Undo, ctx),
        KeyBinding::new("secondary-shift-z", Redo, ctx),
        KeyBinding::new("secondary-y", Redo, ctx),
        KeyBinding::new("enter", Newline, ctx),
        KeyBinding::new("shift-enter", Newline, ctx),
        KeyBinding::new("tab", Indent, ctx),
        KeyBinding::new("shift-tab", Outdent, ctx),
        KeyBinding::new("secondary-s", Save, ctx),
        KeyBinding::new("escape", Cancel, ctx),
    ]
}

/// Colored ranges of one line: byte ranges within the line (without its line break).
pub type LineHighlights = Vec<(Range<usize>, HighlightStyle)>;

/// Gives a [`CodeEditor`] its syntax colors, one line at a time.
///
/// The editor asks only for the lines it draws and caches the answer until the text changes;
/// then it calls [`LineHighlighter::invalidate`] with the first changed line. A stateful
/// highlighter (one that parses from the top of the file, such as syntect) can keep its parse
/// state per line and resume from the closest line before the one requested.
///
/// Any `FnMut(usize, &str) -> LineHighlights` closure is a highlighter, for simple cases.
pub trait LineHighlighter: 'static {
    /// Highlights of line `line` of `buffer`. Ranges are clipped to character boundaries.
    fn highlight_line(&mut self, buffer: &CodeBuffer, line: usize, cx: &App) -> LineHighlights;

    /// The text changed from `from_line` on: forget what was computed for it and later lines.
    fn invalidate(&mut self, from_line: usize) {
        let _ = from_line;
    }
}

impl<F> LineHighlighter for F
where
    F: FnMut(usize, &str) -> LineHighlights + 'static,
{
    fn highlight_line(&mut self, buffer: &CodeBuffer, line: usize, _: &App) -> LineHighlights {
        self(line, buffer.line(line))
    }
}

/// Events emitted by a [`CodeEditor`]. Subscribe with `cx.subscribe(&editor, ...)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodeEditorEvent {
    /// The text changed. Read it with [`CodeEditor::text`] (it is not copied into the event,
    /// since files can be large).
    Changed,
    /// Ctrl+S (Cmd+S on macOS). Write the file, then call [`CodeEditor::mark_saved`].
    Save,
    /// The caret moved or the selection changed.
    SelectionChanged,
    /// Escape was pressed with nothing selected.
    Cancelled,
}

/// The current selection as context for an agent: file, 1-based lines and the selected text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeSelection {
    pub path: Option<SharedString>,
    /// First and last line, starting at 1. A caret without selection gives its own line.
    pub lines: RangeInclusive<usize>,
    /// The selected text; empty when nothing is selected.
    pub text: String,
}

impl CodeSelection {
    /// `path:12` or `path:12-20`, the usual way to cite lines.
    pub fn label(&self) -> String {
        let path: &str = self.path.as_ref().map_or("untitled", |p| p.as_ref());
        if self.lines.start() == self.lines.end() {
            format!("{path}:{}", self.lines.start())
        } else {
            format!("{path}:{}-{}", self.lines.start(), self.lines.end())
        }
    }
}

/// A code editor. Create it once and keep the entity in your view:
///
/// ```ignore
/// let editor = cx.new(|cx| CodeEditor::new(cx).path("src/main.rs").with_text(&source));
/// cx.subscribe(&editor, |this, editor, event, cx| {
///     if *event == CodeEditorEvent::Save {
///         std::fs::write(path, editor.read(cx).text()).ok();
///         editor.update(cx, |e, cx| e.mark_saved(cx));
///     }
/// });
/// ```
///
/// It fills its parent (give it a height) and draws no background, like [`super::TextArea`].
///
/// Not included yet: code folding, multiple cursors, soft wrap, find and replace, and tab stops
/// for hard tabs (a `'\t'` is drawn one column wide).
pub struct CodeEditor {
    focus_handle: FocusHandle,
    buffer: CodeBuffer,
    path: Option<SharedString>,
    highlighter: Option<Box<dyn LineHighlighter>>,
    highlights: Vec<Option<Rc<LineHighlights>>>,
    scroll: UniformListScrollHandle,
    scroll_x: Pixels,
    reveal_x: bool,
    font_size: Pixels,
    line_numbers: bool,
    read_only: bool,
    /// Lines drawn in the last frame, to map the pointer to a column.
    rows: Vec<(usize, ShapedLine)>,
    viewport: Option<Bounds<Pixels>>,
    char_width: Pixels,
    marked: Option<Range<BufferPos>>,
    selecting: bool,
    last_input: Instant,
}

impl EventEmitter<CodeEditorEvent> for CodeEditor {}

impl Focusable for CodeEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl CodeEditor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            buffer: CodeBuffer::new(""),
            path: None,
            highlighter: None,
            highlights: Vec::new(),
            scroll: UniformListScrollHandle::new(),
            scroll_x: px(0.),
            reveal_x: false,
            font_size: px(13.),
            line_numbers: true,
            read_only: false,
            rows: Vec::new(),
            viewport: None,
            char_width: px(7.),
            marked: None,
            selecting: false,
            last_input: Instant::now(),
        }
    }

    /// Initial text (marked as saved).
    pub fn with_text(mut self, text: &str) -> Self {
        self.buffer.set_text(text);
        self
    }

    /// File path, reported by [`CodeEditor::selection_context`].
    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Text size (13 px by default); lines are 1.5 times as tall.
    pub fn font_size(mut self, size: Pixels) -> Self {
        self.font_size = size;
        self
    }

    /// Shows the line number gutter (on by default).
    pub fn line_numbers(mut self, show: bool) -> Self {
        self.line_numbers = show;
        self
    }

    /// Allows selecting and copying but not editing.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Spaces per indentation level (4 by default).
    pub fn tab_size(mut self, size: usize) -> Self {
        self.buffer.tab_size = size.max(1);
        self
    }

    /// Indents with tab characters instead of spaces.
    pub fn hard_tabs(mut self, hard: bool) -> Self {
        self.buffer.hard_tabs = hard;
        self
    }

    /// Inserts and skips over closing brackets and quotes (on by default).
    pub fn auto_pair(mut self, on: bool) -> Self {
        self.buffer.auto_pair = on;
        self
    }

    pub fn highlighter(mut self, highlighter: impl LineHighlighter) -> Self {
        self.highlighter = Some(Box::new(highlighter));
        self
    }

    /// The text, with the line endings it was loaded with.
    pub fn text(&self) -> String {
        self.buffer.text()
    }

    pub fn buffer(&self) -> &CodeBuffer {
        &self.buffer
    }

    /// Replaces the whole text (loading a file): clears the history and marks it saved.
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.buffer.set_text(text);
        self.scroll_x = px(0.);
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.after_edit(cx);
    }

    pub fn set_path(&mut self, path: Option<SharedString>, cx: &mut Context<Self>) {
        self.path = path;
        cx.notify();
    }

    pub fn file_path(&self) -> Option<&SharedString> {
        self.path.as_ref()
    }

    /// Swaps the highlighter (or removes it with `None`).
    pub fn set_highlighter(
        &mut self,
        highlighter: Option<Box<dyn LineHighlighter>>,
        cx: &mut Context<Self>,
    ) {
        self.highlighter = highlighter;
        self.highlights.clear();
        cx.notify();
    }

    /// Forgets the cached colors, e.g. after the theme changed (the highlighter is asked again).
    pub fn refresh_highlights(&mut self, cx: &mut Context<Self>) {
        if let Some(h) = self.highlighter.as_mut() {
            h.invalidate(0);
        }
        self.highlights.clear();
        cx.notify();
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        cx.notify();
    }

    /// Whether the text changed since it was loaded or last saved.
    pub fn is_dirty(&self) -> bool {
        self.buffer.is_dirty()
    }

    pub fn mark_saved(&mut self, cx: &mut Context<Self>) {
        self.buffer.mark_saved();
        cx.notify();
    }

    /// The selection (or the caret's line) with the file path, to send as context.
    pub fn selection_context(&self) -> CodeSelection {
        let sel = self.buffer.selection();
        let lines = sel.lines();
        CodeSelection {
            path: self.path.clone(),
            lines: lines.start + 1..=lines.end.max(lines.start + 1),
            text: self.buffer.selected_text(),
        }
    }

    pub fn selection(&self) -> BufferSelection {
        self.buffer.selection()
    }

    /// Moves the caret to a zero-based line and byte column and scrolls to it.
    pub fn go_to(&mut self, line: usize, col: usize, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.set_cursor(BufferPos::new(line, col), false));
        self.scroll
            .scroll_to_item(self.buffer.cursor().line, ScrollStrategy::Center);
    }

    /// Selects a range and scrolls to it.
    pub fn select(&mut self, selection: BufferSelection, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.set_selection(selection));
    }

    pub fn focus(&self, window: &mut Window) {
        window.focus(&self.focus_handle);
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        self.edit(cx, |b| {
            b.undo();
        });
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        self.edit(cx, |b| {
            b.redo();
        });
    }

    // ---- Plumbing ----

    fn line_height(&self) -> Pixels {
        (self.font_size * 1.5).round()
    }

    fn reveal(&mut self) {
        self.scroll
            .scroll_to_item(self.buffer.cursor().line, ScrollStrategy::Top);
        self.reveal_x = true;
    }

    /// Runs a caret move: reveals the caret and reports a selection change.
    fn motion(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut CodeBuffer)) {
        let before = self.buffer.selection();
        f(&mut self.buffer);
        self.marked = None;
        self.last_input = Instant::now();
        self.reveal();
        if self.buffer.selection() != before {
            cx.emit(CodeEditorEvent::SelectionChanged);
        }
        cx.notify();
    }

    /// Runs an edit (ignored when read-only). Typing after a pause of one second starts a new
    /// undo step.
    fn edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut CodeBuffer)) {
        if self.read_only {
            return;
        }
        if self.last_input.elapsed() > Duration::from_secs(1) {
            self.buffer.break_undo_group();
        }
        let before = self.buffer.selection();
        f(&mut self.buffer);
        self.last_input = Instant::now();
        self.reveal();
        self.after_edit(cx);
        if self.buffer.selection() != before {
            cx.emit(CodeEditorEvent::SelectionChanged);
        }
    }

    fn after_edit(&mut self, cx: &mut Context<Self>) {
        if let Some(from) = self.buffer.take_edited_from() {
            self.highlights.truncate(from);
            if let Some(h) = self.highlighter.as_mut() {
                h.invalidate(from);
            }
            cx.emit(CodeEditorEvent::Changed);
        }
        cx.notify();
    }

    fn page_lines(&self) -> isize {
        let height = self.viewport.map_or(px(400.), |b| b.size.height);
        ((height / self.line_height()).floor() as isize - 1).max(1)
    }

    fn gutter_width(&self) -> Pixels {
        if !self.line_numbers {
            return px(0.);
        }
        let digits = self.buffer.line_count().to_string().len().max(2);
        self.char_width * digits as f32 + px(28.)
    }

    /// Buffer position under a window position (clamped to the text).
    fn pos_for_point(&self, position: Point<Pixels>) -> BufferPos {
        let Some(viewport) = self.viewport else {
            return BufferPos::default();
        };
        let lh = self.line_height();
        let offset = self.scroll.0.borrow().base_handle.offset();
        let y = position.y - viewport.top() - offset.y;
        if y < px(0.) {
            return BufferPos::default();
        }
        let line = (y / lh).floor() as usize;
        if line >= self.buffer.line_count() {
            let last = self.buffer.line_count() - 1;
            return BufferPos::new(last, self.buffer.line(last).len());
        }
        let x = position.x - viewport.left() - self.gutter_width() - px(TEXT_PAD) + self.scroll_x;
        let col = match self.rows.iter().find(|(l, _)| *l == line) {
            Some((_, shaped)) => shaped.closest_index_for_x(x.max(px(0.))),
            None => {
                let chars = (x / self.char_width).round().max(0.) as usize;
                let text = self.buffer.line(line);
                text.char_indices()
                    .nth(chars)
                    .map_or(text.len(), |(i, _)| i)
            }
        };
        self.buffer.clip(BufferPos::new(line, col))
    }

    fn highlights_for(&mut self, line: usize, cx: &App) -> Option<Rc<LineHighlights>> {
        let highlighter = self.highlighter.as_mut()?;
        if self.highlights.len() < self.buffer.line_count() {
            self.highlights.resize(self.buffer.line_count(), None);
        }
        if let Some(Some(cached)) = self.highlights.get(line) {
            return Some(cached.clone());
        }
        let computed = Rc::new(highlighter.highlight_line(&self.buffer, line, cx));
        if let Some(slot) = self.highlights.get_mut(line) {
            *slot = Some(computed.clone());
        }
        Some(computed)
    }

    // ---- Actions ----

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, CodeBuffer::backspace);
    }
    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, CodeBuffer::delete);
    }
    fn delete_word_left(&mut self, _: &DeleteWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, CodeBuffer::delete_word_left);
    }
    fn delete_word_right(&mut self, _: &DeleteWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, CodeBuffer::delete_word_right);
    }
    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_left(false));
    }
    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_right(false));
    }
    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_vertically(-1, false));
    }
    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_vertically(1, false));
    }
    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_left(true));
    }
    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_right(true));
    }
    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_vertically(-1, true));
    }
    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_vertically(1, true));
    }
    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_word_left(false));
    }
    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_word_right(false));
    }
    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_word_left(true));
    }
    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_word_right(true));
    }
    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_home(false));
    }
    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_end(false));
    }
    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_home(true));
    }
    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_end(true));
    }
    fn doc_start(&mut self, _: &DocStart, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_to_start(false));
    }
    fn doc_end(&mut self, _: &DocEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_to_end(false));
    }
    fn select_doc_start(&mut self, _: &SelectDocStart, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_to_start(true));
    }
    fn select_doc_end(&mut self, _: &SelectDocEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, |b| b.move_to_end(true));
    }
    fn page_up(&mut self, _: &PageUp, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.page_lines();
        self.motion(cx, |b| b.move_vertically(-n, false));
    }
    fn page_down(&mut self, _: &PageDown, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.page_lines();
        self.motion(cx, |b| b.move_vertically(n, false));
    }
    fn select_page_up(&mut self, _: &SelectPageUp, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.page_lines();
        self.motion(cx, |b| b.move_vertically(-n, true));
    }
    fn select_page_down(&mut self, _: &SelectPageDown, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.page_lines();
        self.motion(cx, |b| b.move_vertically(n, true));
    }
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(cx, CodeBuffer::select_all);
    }

    /// Copies the selection, or the whole line when nothing is selected.
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let text = if self.buffer.selection().is_empty() {
            format!("{}\n", self.buffer.line(self.buffer.cursor().line))
        } else {
            self.buffer.selected_text()
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    /// Cuts the selection, or the whole line when nothing is selected.
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return self.copy(&Copy, window, cx);
        }
        if self.buffer.selection().is_empty() {
            let line = self.buffer.cursor().line;
            self.buffer.select_line(line);
        }
        cx.write_to_clipboard(ClipboardItem::new_string(self.buffer.selected_text()));
        self.edit(cx, |b| b.insert(""));
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        self.edit(cx, |b| b.insert(&text));
    }

    fn undo_action(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.undo(cx);
    }
    fn redo_action(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        self.redo(cx);
    }
    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, CodeBuffer::newline);
    }
    fn indent(&mut self, _: &Indent, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, CodeBuffer::indent);
    }
    fn outdent(&mut self, _: &Outdent, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, CodeBuffer::outdent);
    }
    fn save(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(CodeEditorEvent::Save);
    }
    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.buffer.selection().is_empty() {
            cx.emit(CodeEditorEvent::Cancelled);
        } else {
            let head = self.buffer.cursor();
            self.motion(cx, |b| b.set_cursor(head, false));
        }
    }

    // ---- Pointer ----

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        let pos = self.pos_for_point(event.position);
        self.selecting = true;
        match event.click_count {
            2 => self.motion(cx, |b| b.select_word_at(pos)),
            n if n >= 3 => self.motion(cx, |b| b.select_line(pos.line)),
            _ => {
                let shift = event.modifiers.shift;
                self.motion(cx, |b| b.set_cursor(pos, shift));
            }
        }
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selecting || event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let pos = self.pos_for_point(event.position);
        if pos != self.buffer.cursor() {
            self.motion(cx, |b| b.set_cursor(pos, true));
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = event.delta.pixel_delta(self.line_height());
        // The line list already scrolled; keep the wheel from also scrolling the page around it.
        if delta.y != px(0.) && self.scroll.is_scrollable() {
            cx.stop_propagation();
        }
        let dx = delta.x;
        if dx == px(0.) {
            return;
        }
        cx.stop_propagation();
        let max = self.max_scroll_x();
        let next = (self.scroll_x - dx).clamp(px(0.), max);
        if next != self.scroll_x {
            self.scroll_x = next;
            cx.notify();
        }
    }

    fn text_width(&self) -> Pixels {
        self.viewport.map_or(px(0.), |b| b.size.width) - self.gutter_width() - px(TEXT_PAD * 2.)
    }

    fn max_scroll_x(&self) -> Pixels {
        let content = self.char_width * (self.buffer.longest_line_len() + 2) as f32;
        (content - self.text_width()).max(px(0.))
    }

    /// Keeps the caret inside the text area horizontally (an estimate from the monospace
    /// character width).
    fn reveal_caret_x(&mut self) {
        if !std::mem::take(&mut self.reveal_x) || self.viewport.is_none() {
            return;
        }
        let head = self.buffer.cursor();
        let chars = self.buffer.line(head.line)[..head.col].chars().count();
        let x = self.char_width * chars as f32;
        let width = self.text_width();
        let margin = self.char_width * 4.;
        if x < self.scroll_x + margin {
            self.scroll_x = (x - margin).max(px(0.));
        } else if x > self.scroll_x + width - margin {
            self.scroll_x = x - width + margin;
        }
        self.scroll_x = self.scroll_x.clamp(px(0.), self.max_scroll_x());
    }

    // ---- Rows ----

    fn render_rows(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let s = *Theme::of(cx);
        let lh = self.line_height();
        let gutter = self.gutter_width();
        let sel = self.buffer.selection();
        let sel_range = sel.range();
        let focused = self.focus_handle.is_focused(window);
        let since = self.last_input.elapsed().as_secs_f32();
        let caret_on = focused && (since < 0.5 || ((since / 0.53) as u32).is_multiple_of(2));
        let scroll_x = self.scroll_x;
        let base = TextRun {
            len: 0,
            font: font(MONO_FONT_FAMILY),
            color: s.on_surface,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let selection_color = alpha(s.primary, if focused { 0.28 } else { 0.16 });
        self.rows.retain(|(line, _)| range.contains(line));
        let mut out = Vec::with_capacity(range.len());
        for ix in range {
            let highlights = self.highlights_for(ix, cx);
            let text = self.buffer.line(ix);
            // A tab is drawn as one space so byte columns keep matching the shaped line.
            let display: SharedString = if text.contains('\t') {
                text.replace('\t', " ").into()
            } else {
                SharedString::from(text.to_owned())
            };
            let marked = self.marked.as_ref().and_then(|m| {
                (m.start.line <= ix && ix <= m.end.line).then(|| {
                    let start = if m.start.line == ix { m.start.col } else { 0 };
                    let end = if m.end.line == ix {
                        m.end.col
                    } else {
                        text.len()
                    };
                    start..end
                })
            });
            let runs = line_runs(
                &display,
                &base,
                highlights.as_deref().map_or(&[][..], |h| &h[..]),
                marked,
            );
            let shaped = window
                .text_system()
                .shape_line(display, self.font_size, &runs, None);
            let selected = (!sel.is_empty()
                && sel_range.start.line <= ix
                && ix <= sel_range.end.line)
                .then(|| {
                    let start = if sel_range.start.line == ix {
                        shaped.x_for_index(sel_range.start.col)
                    } else {
                        px(0.)
                    };
                    let end = if sel_range.end.line == ix {
                        shaped.x_for_index(sel_range.end.col)
                    } else {
                        // Past the end of the line: show the selected line break.
                        shaped.width + self.char_width * 0.6
                    };
                    (start, end)
                });
            let caret = (caret_on && sel.head.line == ix).then(|| shaped.x_for_index(sel.head.col));
            let current = sel.head.line == ix;
            self.rows.retain(|(line, _)| *line != ix);
            self.rows.push((ix, shaped.clone()));
            let primary = s.primary;
            let paint = canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    let origin = point(bounds.left() + px(TEXT_PAD) - scroll_x, bounds.top());
                    window.with_content_mask(Some(ContentMask { bounds }), |window| {
                        if let Some((a, b)) = selected {
                            window.paint_quad(fill(
                                Bounds::from_corners(
                                    point(origin.x + a, bounds.top()),
                                    point(origin.x + b, bounds.bottom()),
                                ),
                                selection_color,
                            ));
                        }
                        shaped.paint(origin, lh, window, cx).ok();
                        if let Some(x) = caret {
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(origin.x + x - px(1.), bounds.top()),
                                    size(px(2.), lh),
                                ),
                                primary,
                            ));
                        }
                    });
                },
            )
            .flex_1()
            .h_full();
            let mut row = div()
                .id(("code-line", ix))
                .flex()
                .w_full()
                .h(lh)
                .when(current && sel.is_empty(), |d| {
                    d.bg(alpha(s.on_surface, 0.045))
                });
            if self.line_numbers {
                row = row.child(
                    div()
                        .flex_none()
                        .w(gutter)
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_end()
                        .pr(px(12.))
                        .text_size(self.font_size * 0.92)
                        .text_color(if current {
                            s.on_surface_variant
                        } else {
                            alpha(s.outline, 0.8)
                        })
                        .child(SharedString::from((ix + 1).to_string())),
                );
            }
            out.push(row.child(paint).into_any_element());
        }
        out
    }
}

/// Text runs for one line: the base style with the highlights layered on top, clipped to
/// character boundaries, and the IME composition underlined.
pub(crate) fn line_runs(
    text: &str,
    base: &TextRun,
    highlights: &[(Range<usize>, HighlightStyle)],
    marked: Option<Range<usize>>,
) -> Vec<TextRun> {
    let len = text.len();
    let snap = |mut i: usize| {
        i = i.min(len);
        while !text.is_char_boundary(i) {
            i -= 1;
        }
        i
    };
    let mut cuts = vec![0, len];
    for (range, _) in highlights {
        cuts.push(snap(range.start));
        cuts.push(snap(range.end));
    }
    if let Some(m) = &marked {
        cuts.push(snap(m.start));
        cuts.push(snap(m.end));
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut runs: Vec<TextRun> = Vec::with_capacity(cuts.len());
    for pair in cuts.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        if start == end {
            continue;
        }
        let mut run = TextRun {
            len: end - start,
            ..base.clone()
        };
        for (range, style) in highlights {
            if snap(range.start) <= start && end <= snap(range.end) {
                apply_highlight(&mut run, style);
            }
        }
        if let Some(m) = &marked
            && snap(m.start) <= start
            && end <= snap(m.end)
        {
            run.underline = Some(UnderlineStyle {
                color: Some(run.color),
                thickness: px(1.),
                wavy: false,
            });
        }
        // Merge with the previous run when nothing differs.
        if let Some(prev) = runs.last_mut()
            && prev.font == run.font
            && prev.color == run.color
            && prev.background_color == run.background_color
            && prev.underline == run.underline
            && prev.strikethrough == run.strikethrough
        {
            prev.len += run.len;
            continue;
        }
        runs.push(run);
    }
    if runs.is_empty() {
        runs.push(TextRun {
            len: 0,
            ..base.clone()
        });
    }
    runs
}

fn apply_highlight(run: &mut TextRun, style: &HighlightStyle) {
    if let Some(color) = style.color {
        run.color = color;
    }
    if let Some(fade) = style.fade_out {
        run.color.a *= 1. - fade.clamp(0., 1.);
    }
    if let Some(weight) = style.font_weight {
        run.font.weight = weight;
    }
    if let Some(font_style) = style.font_style {
        run.font.style = font_style;
    }
    if let Some(bg) = style.background_color {
        run.background_color = Some(bg);
    }
    if let Some(underline) = style.underline {
        run.underline = Some(underline);
    }
    if let Some(strike) = style.strikethrough {
        run.strikethrough = Some(strike);
    }
}

impl EntityInputHandler for CodeEditor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let start = self.buffer.pos_of_utf16(range_utf16.start);
        let end = self.buffer.pos_of_utf16(range_utf16.end);
        actual_range.replace(self.buffer.utf16_of(start)..self.buffer.utf16_of(end));
        Some(self.buffer.text_in(start..end))
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let sel = self.buffer.selection();
        let r = sel.range();
        Some(UTF16Selection {
            range: self.buffer.utf16_of(r.start)..self.buffer.utf16_of(r.end),
            reversed: sel.reversed(),
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked
            .as_ref()
            .map(|m| self.buffer.utf16_of(m.start)..self.buffer.utf16_of(m.end))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .map(|r| self.buffer.pos_of_utf16(r.start)..self.buffer.pos_of_utf16(r.end))
            .or(self.marked.take());
        self.marked = None;
        match range {
            Some(range) => self.edit(cx, |b| b.replace(range, text)),
            // Plain typing: brackets pair, quotes skip, typing groups into one undo step.
            None => self.edit(cx, |b| b.type_text(text)),
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = range_utf16
            .map(|r| self.buffer.pos_of_utf16(r.start)..self.buffer.pos_of_utf16(r.end))
            .or(self.marked.clone())
            .unwrap_or_else(|| self.buffer.selection().range());
        let start = range.start;
        self.edit(cx, |b| b.replace(range, text));
        let start_utf16 = self.buffer.utf16_of(start);
        let end = self
            .buffer
            .pos_of_utf16(start_utf16 + text.encode_utf16().count());
        self.marked = (!text.is_empty()).then_some(start..end);
        if let Some(sel) = new_selected_range_utf16 {
            let a = self.buffer.pos_of_utf16(start_utf16 + sel.start);
            let b = self.buffer.pos_of_utf16(start_utf16 + sel.end);
            self.buffer
                .set_selection(BufferSelection { anchor: a, head: b });
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let pos = self.buffer.pos_of_utf16(range_utf16.start);
        let lh = self.line_height();
        let offset = self.scroll.0.borrow().base_handle.offset();
        let x = match self.rows.iter().find(|(l, _)| *l == pos.line) {
            Some((_, shaped)) => shaped.x_for_index(pos.col),
            None => self.char_width * self.buffer.line(pos.line)[..pos.col].chars().count() as f32,
        };
        let origin = point(
            bounds.left() + self.gutter_width() + px(TEXT_PAD) - self.scroll_x + x,
            bounds.top() + offset.y + lh * pos.line as f32,
        );
        Some(Bounds::new(origin, size(px(2.), lh)))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let pos = self.pos_for_point(point);
        Some(self.buffer.utf16_of(pos))
    }
}

impl Render for CodeEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let mono = font(MONO_FONT_FAMILY);
        let font_id = window.text_system().resolve_font(&mono);
        if let Ok(advance) = window.text_system().advance(font_id, self.font_size, 'm') {
            self.char_width = advance.width;
        }
        if self.focus_handle.is_focused(window) {
            continuous_at(4., window, cx);
        }
        self.reveal_caret_x();
        let entity = cx.entity();
        let focus = self.focus_handle.clone();
        let mut list = uniform_list(
            ("m3-code-lines", cx.entity_id().as_u64()),
            self.buffer.line_count(),
            cx.processor(|this, range, window, cx| this.render_rows(range, window, cx)),
        )
        .track_scroll(self.scroll.clone())
        .size_full();
        // Sideways wheel deltas scroll the text horizontally (see `on_scroll_wheel`), not the list.
        list.style().restrict_scroll_to_axis = Some(true);
        div()
            .id(("m3-code-editor", cx.entity_id().as_u64()))
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .relative()
            .size_full()
            .overflow_hidden()
            .font_family(MONO_FONT_FAMILY)
            .text_size(self.font_size)
            .text_color(s.on_surface)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_word_right))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::doc_start))
            .on_action(cx.listener(Self::doc_end))
            .on_action(cx.listener(Self::select_doc_start))
            .on_action(cx.listener(Self::select_doc_end))
            .on_action(cx.listener(Self::page_up))
            .on_action(cx.listener(Self::page_down))
            .on_action(cx.listener(Self::select_page_up))
            .on_action(cx.listener(Self::select_page_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::undo_action))
            .on_action(cx.listener(Self::redo_action))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::indent))
            .on_action(cx.listener(Self::outdent))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::cancel))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .child(list)
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        entity.update(cx, |editor, _| editor.viewport = Some(bounds));
                        window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{FontWeight, Hsla};

    fn base() -> TextRun {
        TextRun {
            len: 0,
            font: font("Mono"),
            color: Hsla::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    #[test]
    fn runs_cover_the_line_and_layer_highlights() {
        let red = gpui::red();
        let bold = HighlightStyle {
            font_weight: Some(FontWeight::BOLD),
            ..Default::default()
        };
        let color = HighlightStyle {
            color: Some(red),
            ..Default::default()
        };
        let runs = line_runs("let x = 1;", &base(), &[(0..3, color), (0..5, bold)], None);
        let lens: Vec<_> = runs.iter().map(|r| r.len).collect();
        assert_eq!(lens, vec![3, 2, 5]);
        assert_eq!(runs[0].color, red);
        assert_eq!(runs[0].font.weight, FontWeight::BOLD);
        assert_eq!(runs[1].font.weight, FontWeight::BOLD);
        assert_eq!(runs[2].font.weight, base().font.weight);
    }

    #[test]
    fn runs_snap_to_char_boundaries_and_merge_equal_neighbours() {
        let style = HighlightStyle {
            color: Some(gpui::red()),
            ..Default::default()
        };
        // "ñ" is two bytes: a range ending inside it snaps back to its start.
        let runs = line_runs("añb", &base(), &[(0..2, style), (40..50, style)], None);
        assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), 4);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].len, 1);
        let plain = line_runs("abc", &base(), &[], None);
        assert_eq!(plain.len(), 1);
        assert_eq!(plain[0].len, 3);
    }

    #[test]
    fn marked_text_is_underlined() {
        let runs = line_runs("abcd", &base(), &[], Some(1..3));
        assert_eq!(runs.len(), 3);
        assert!(runs[1].underline.is_some());
        assert!(runs[0].underline.is_none());
    }

    #[test]
    fn selection_label_cites_lines() {
        let one = CodeSelection {
            path: Some("src/a.rs".into()),
            lines: 3..=3,
            text: String::new(),
        };
        assert_eq!(one.label(), "src/a.rs:3");
        let many = CodeSelection {
            path: None,
            lines: 3..=9,
            text: "x".into(),
        };
        assert_eq!(many.label(), "untitled:3-9");
    }
}
