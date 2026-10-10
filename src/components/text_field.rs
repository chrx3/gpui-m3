//! Single-line text field.
//!
//! Based on GPUI's official `examples/input.rs` (Apache-2.0): keyboard editing, mouse
//! selection, clipboard and IME. Styled as M3 Expressive: a tonal pill that morphs its
//! corners and deepens its color when focused, with a primary-colored caret.

use std::ops::Range;
use std::time::Instant;

use gpui::{
    AnyElement, App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    FontWeight, GlobalElementId, Hsla, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, Render, ShapedLine, SharedString,
    Style, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div, fill, point, prelude::*,
    px, relative, size,
};
use unicode_segmentation::UnicodeSegmentation;

use super::{Icon, RoundedExt, all, corners};
use crate::motion::{Spring, animate_color, animate_f32, continuous_at, key};
use crate::theme::{FONT_FAMILY, Theme, alpha};

actions!(
    m3_text_field,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        Submit,
        Cancel
    ]
);

const KEY_CONTEXT: &str = "M3TextField";

/// Registers the text field key bindings. Called by [`crate::init`].
pub fn bind_text_field_keys(cx: &mut App) {
    cx.bind_keys(text_field_bindings());
}

pub(crate) fn text_field_bindings() -> Vec<KeyBinding> {
    let ctx = Some(KEY_CONTEXT);
    vec![
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("secondary-a", SelectAll, ctx),
        KeyBinding::new("secondary-v", Paste, ctx),
        KeyBinding::new("secondary-c", Copy, ctx),
        KeyBinding::new("secondary-x", Cut, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("enter", Submit, ctx),
        KeyBinding::new("escape", Cancel, ctx),
    ]
}

/// Events emitted by a [`TextField`]. Subscribe with `cx.subscribe(&field, ...)`.
#[derive(Clone, Debug)]
pub enum TextFieldEvent {
    /// The text changed (typing, paste, cut, or [`TextField::set_text`]).
    Changed(SharedString),
    /// Enter was pressed.
    Submitted(SharedString),
    /// Escape was pressed.
    Cancelled,
}

/// A single-line text field. Create it once and keep the entity in your view:
///
/// ```ignore
/// let search = cx.new(|cx| TextField::new(cx).placeholder("Search").icon("search"));
/// ```
/// Look of a [`TextField`].
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum TextFieldStyle {
    /// 44 px tonal pill that deepens and opens its corners on focus (search, forms).
    #[default]
    Pill,
    /// 38 px tonal pill with fixed corners (a menu's filter).
    Compact,
    /// In-place editing (renaming a row): 34 px pill with a 1.5 px primary border.
    Inline,
    /// Text only: no background, border or padding ("Other…" answers, tab renames).
    Bare,
    /// M3 filled field: 40 px, top corners rounded, an underline that thickens on focus.
    Filled,
    /// Form field: 36 px, 14 px corners and a 1 px border that turns into a 2 px primary one.
    Outlined,
}

/// Builds the trailing content of a [`TextField`] on each render (e.g. a clear button).
type TrailingBuilder = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

pub struct TextField {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    icon: Option<SharedString>,
    style: TextFieldStyle,
    height: Option<Pixels>,
    font_size: Option<Pixels>,
    background: Option<Hsla>,
    focus_ring: Option<Pixels>,
    label: Option<SharedString>,
    trailing: Option<TrailingBuilder>,
    select_all_on_focus: bool,
    restore_on_cancel: bool,
    was_focused: bool,
    text_at_focus: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    last_edit: Instant,
    suggesting: bool,
}

impl EventEmitter<TextFieldEvent> for TextField {}

impl Focusable for TextField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TextField {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: SharedString::default(),
            placeholder: SharedString::default(),
            icon: None,
            style: TextFieldStyle::Pill,
            height: None,
            font_size: None,
            background: None,
            focus_ring: None,
            label: None,
            trailing: None,
            select_all_on_focus: false,
            restore_on_cancel: false,
            was_focused: false,
            text_at_focus: SharedString::default(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            last_edit: Instant::now(),
            suggesting: false,
        }
    }

    /// Marks that a suggestion list is open for this input: while on, ↑ ↓ Enter Tab Esc dispatch
    /// the [`super::SuggestionList`] actions instead of their usual editing behavior.
    pub fn set_suggesting(&mut self, suggesting: bool, cx: &mut Context<Self>) {
        if self.suggesting != suggesting {
            self.suggesting = suggesting;
            cx.notify();
        }
    }

    pub fn is_suggesting(&self) -> bool {
        self.suggesting
    }

    fn key_context(&self) -> gpui::KeyContext {
        let mut context = gpui::KeyContext::default();
        context.add(KEY_CONTEXT);
        if self.suggesting {
            context.add(super::SUGGESTING_CONTEXT);
        }
        context
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    /// Leading icon (see [`crate::Assets::icon_names`]).
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn style(mut self, style: TextFieldStyle) -> Self {
        self.style = style;
        self
    }

    /// Shorthand for [`TextFieldStyle::Compact`].
    pub fn compact(self) -> Self {
        self.style(TextFieldStyle::Compact)
    }

    /// Shorthand for [`TextFieldStyle::Inline`]; usually with `select_all_on_focus` and
    /// `restore_on_cancel`.
    pub fn inline(self) -> Self {
        self.style(TextFieldStyle::Inline)
    }

    /// Shorthand for [`TextFieldStyle::Filled`].
    pub fn filled(self) -> Self {
        self.style(TextFieldStyle::Filled)
    }

    /// Shorthand for [`TextFieldStyle::Outlined`].
    pub fn outlined(self) -> Self {
        self.style(TextFieldStyle::Outlined)
    }

    /// Label above the field (primary for filled fields, muted otherwise).
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Background instead of the style's.
    pub fn bg(mut self, background: Hsla) -> Self {
        self.background = Some(background);
        self
    }

    /// Inner primary ring of this width while focused (pill and compact styles).
    pub fn focus_ring(mut self, width: Pixels) -> Self {
        self.focus_ring = Some(width);
        self
    }

    /// Content at the end of the field, built on each render (e.g. a clear button).
    pub fn trailing(
        mut self,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.trailing = Some(Box::new(build));
        self
    }

    /// Shorthand for [`TextFieldStyle::Bare`].
    pub fn bare(self) -> Self {
        self.style(TextFieldStyle::Bare)
    }

    /// Overrides the style's height (44 / 38 / 34 / 26 px).
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = Some(height);
        self
    }

    /// Overrides the style's text size.
    pub fn text_size(mut self, size: Pixels) -> Self {
        self.font_size = Some(size);
        self
    }

    /// Selects the whole text each time the field gains focus.
    pub fn select_all_on_focus(mut self, select: bool) -> Self {
        self.select_all_on_focus = select;
        self
    }

    /// Escape puts back the text the field had when it gained focus (before emitting
    /// [`TextFieldEvent::Cancelled`]).
    pub fn restore_on_cancel(mut self, restore: bool) -> Self {
        self.restore_on_cancel = restore;
        self
    }

    /// Moves keyboard focus to the field.
    pub fn focus(&self, window: &mut Window) {
        window.focus(&self.focus_handle);
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    pub fn text(&self) -> &SharedString {
        &self.content
    }

    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = text.into();
        self.selected_range = self.content.len()..self.content.len();
        self.marked_range = None;
        self.changed(cx);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.last_edit = Instant::now();
        cx.emit(TextFieldEvent::Changed(self.content.clone()));
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

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
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
        cx.emit(TextFieldEvent::Submitted(self.content.clone()));
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.restore_on_cancel && self.content != self.text_at_focus {
            self.set_text(self.text_at_focus.clone(), cx);
        }
        cx.emit(TextFieldEvent::Cancelled);
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.is_selecting = true;
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
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

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace('\n', " "), window, cx);
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
        self.last_edit = Instant::now();
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        line.closest_index_for_x(position.x - bounds.left())
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
        cx.notify()
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

impl EntityInputHandler for TextField {
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
        let last_layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        let utf8_index = last_layout.index_for_x(point.x - line_point.x)?;
        Some(self.offset_to_utf16(utf8_index))
    }
}

/// The shaped line, caret and selection.
struct TextLine {
    field: Entity<TextField>,
}

struct LinePaint {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for TextLine {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextLine {
    type RequestLayoutState = ();
    type PrepaintState = LinePaint;

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
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
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
        let s = *Theme::of(cx);
        let field = self.field.read(cx);
        let content = field.content.clone();
        let selected_range = field.selected_range.clone();
        let cursor = field.cursor_offset();
        let style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (field.placeholder.clone(), s.on_surface_variant)
        } else {
            (content, style.color)
        };
        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked) = field.marked_range.as_ref() {
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
                    len: display_text.len() - marked.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);
        let height = bounds.bottom() - bounds.top();
        let (selection, cursor) = if selected_range.is_empty() {
            let x = line.x_for_index(cursor);
            (
                None,
                Some(fill(
                    Bounds::new(point(bounds.left() + x, bounds.top()), size(px(2.), height)),
                    s.primary,
                )),
            )
        } else {
            let selection = fill(
                Bounds::from_corners(
                    point(
                        bounds.left() + line.x_for_index(selected_range.start),
                        bounds.top(),
                    ),
                    point(
                        bounds.left() + line.x_for_index(selected_range.end),
                        bounds.bottom(),
                    ),
                ),
                alpha(s.primary, 0.3),
            );
            (Some(selection), None)
        };
        LinePaint {
            line: Some(line),
            cursor,
            selection,
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
        let (focus_handle, last_edit) = {
            let field = self.field.read(cx);
            (field.focus_handle.clone(), field.last_edit)
        };
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.field.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let line = prepaint.line.take().unwrap();
        line.paint(bounds.origin, window.line_height(), window, cx)
            .ok();

        // The caret stays solid while typing and blinks every 0.53 s after that.
        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            let since_edit = last_edit.elapsed().as_secs_f32();
            if since_edit < 0.5 || ((since_edit / 0.53) as u32).is_multiple_of(2) {
                window.paint_quad(cursor);
            }
        }
        self.field.update(cx, |field, _| {
            field.last_layout = Some(line);
            field.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let id = ElementId::from(("m3-text-field", cx.entity_id().as_u64()));
        let focused = self.focus_handle.is_focused(window);
        if focused && !self.was_focused {
            self.text_at_focus = self.content.clone();
            if self.select_all_on_focus {
                self.selected_range = 0..self.content.len();
                self.selection_reversed = false;
            }
        }
        self.was_focused = focused;
        if focused {
            // Caret blink: a few frames per second is enough.
            continuous_at(4., window, cx);
        }
        // (height, horizontal padding, gap, icon size, text size, radius at rest / focused)
        let (height, pad, gap, icon_size, text_size, rest_r, focus_r) = match self.style {
            TextFieldStyle::Pill => (44., 18., 10., 18., 13.5, 22., 16.),
            TextFieldStyle::Compact => (38., 12., 8., 14., 13., 19., 19.),
            TextFieldStyle::Inline => (34., 10., 8., 16., 13.5, 17., 17.),
            TextFieldStyle::Bare => (26., 0., 6., 16., 13.5, 0., 0.),
            TextFieldStyle::Filled => (40., 12., 8., 16., 13.5, 8., 8.),
            TextFieldStyle::Outlined => (36., 12., 8., 16., 13.5, 14., 14.),
        };
        let height = self.height.map_or(height, f32::from);
        let text_size = self.font_size.map_or(text_size, f32::from);
        // Like the reference design's composer and search fields: the pill deepens and its corners open up on focus.
        let radius = px(animate_f32(
            key(&id, "radius"),
            Spring::SPATIAL_FAST,
            if focused { focus_r } else { rest_r },
            window,
            cx,
        ));
        let (rest_bg, focus_bg) = match self.style {
            TextFieldStyle::Pill | TextFieldStyle::Compact => {
                (s.surface_container_high, s.surface_container_highest)
            }
            TextFieldStyle::Inline => (s.surface_container, s.surface_container),
            TextFieldStyle::Filled => (s.surface_container_high, s.surface_container_high),
            TextFieldStyle::Outlined => (s.surface, s.surface),
            TextFieldStyle::Bare => (alpha(s.surface, 0.), alpha(s.surface, 0.)),
        };
        let (rest_bg, focus_bg) = match self.background {
            Some(bg) => (bg, bg),
            None => (rest_bg, focus_bg),
        };
        let bg = animate_color(
            key(&id, "bg"),
            Spring::EFFECTS,
            if focused { focus_bg } else { rest_bg },
            window,
            cx,
        );
        let mut field = div()
            .id(id)
            .key_context(self.key_context())
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::cancel))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .flex()
            .items_center()
            .gap(px(gap))
            .w_full()
            .h(px(height))
            .px(px(pad))
            .bg(bg)
            .text_color(s.on_surface)
            .font_family(FONT_FAMILY)
            .text_size(px(text_size))
            .font_weight(FontWeight(400.))
            .line_height(px(20.));
        field = match self.style {
            TextFieldStyle::Filled => field
                .corners(corners([f32::from(radius), f32::from(radius), 0., 0.]))
                .border_b(px(if focused { 2. } else { 1. }))
                .border_color(if focused { s.primary } else { s.outline }),
            TextFieldStyle::Outlined => field
                .corners(all(radius))
                .border(px(if focused { 2. } else { 1. }))
                .border_color(if focused {
                    s.primary
                } else {
                    s.outline_variant
                }),
            TextFieldStyle::Inline => field
                .corners(all(radius))
                .border(px(1.5))
                .border_color(s.primary),
            _ => {
                let field = field.corners(all(radius));
                match self.focus_ring {
                    Some(width) if focused => field.border(width).border_color(s.primary),
                    _ => field,
                }
            }
        };
        if let Some(icon) = self.icon.clone() {
            field = field.child(Icon::new(icon).size(px(icon_size)).color(if focused {
                s.primary
            } else {
                s.on_surface_variant
            }));
        }
        let trailing = self.trailing.as_ref().map(|build| build(window, cx));
        let field = field
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(TextLine { field: cx.entity() }),
            )
            .children(trailing);
        match self.label.clone() {
            Some(label) => {
                let filled = self.style == TextFieldStyle::Filled;
                div()
                    .flex()
                    .flex_col()
                    .gap(px(if filled { 4. } else { 8. }))
                    .w_full()
                    .font_family(FONT_FAMILY)
                    .child(
                        div()
                            .text_size(px(if filled { 11.5 } else { 12. }))
                            .font_weight(FontWeight(600.))
                            .text_color(if filled {
                                s.primary
                            } else {
                                s.on_surface_variant
                            })
                            .child(label),
                    )
                    .child(field)
                    .into_any_element()
            }
            None => field.into_any_element(),
        }
    }
}
