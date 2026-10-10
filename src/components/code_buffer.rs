//! Text model of [`super::CodeEditor`]: lines, one selection, editing commands and undo history.
//!
//! It knows nothing about GPUI, so the editing rules (indentation, bracket pairs, undo grouping)
//! are tested on their own. Positions are `(line, byte column)`; lines never contain `'\n'`.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

/// A position in a [`CodeBuffer`]: zero-based line and byte column within that line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BufferPos {
    pub line: usize,
    pub col: usize,
}

impl BufferPos {
    pub const fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

/// The selection: `anchor` stays put while `head` (the caret) moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BufferSelection {
    pub anchor: BufferPos,
    pub head: BufferPos,
}

impl BufferSelection {
    pub fn caret(pos: BufferPos) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The selected range, start before end.
    pub fn range(&self) -> Range<BufferPos> {
        if self.anchor <= self.head {
            self.anchor..self.head
        } else {
            self.head..self.anchor
        }
    }

    /// The caret sits at the start of the selection.
    pub fn reversed(&self) -> bool {
        self.head < self.anchor
    }

    /// Lines the selection touches. A selection that ends at column 0 of a later line does not
    /// count that line (selecting whole lines with Shift+Down).
    pub fn lines(&self) -> Range<usize> {
        let r = self.range();
        let last = if r.end.col == 0 && r.end.line > r.start.line {
            r.end.line - 1
        } else {
            r.end.line
        };
        r.start.line..last + 1
    }
}

/// Kind of edit, used to group consecutive typing into one undo step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditKind {
    Insert,
    Delete,
    Other,
}

#[derive(Clone, Debug)]
struct Edit {
    start: BufferPos,
    old: String,
    new: String,
}

#[derive(Clone, Debug)]
struct Transaction {
    id: u64,
    kind: EditKind,
    edits: Vec<Edit>,
    before: BufferSelection,
    after: BufferSelection,
}

const PAIRS: [(char, char); 6] = [
    ('(', ')'),
    ('[', ']'),
    ('{', '}'),
    ('"', '"'),
    ('\'', '\''),
    ('`', '`'),
];

fn closer_for(open: char) -> Option<char> {
    PAIRS.iter().find(|(o, _)| *o == open).map(|(_, c)| *c)
}

fn is_closer(c: char) -> bool {
    PAIRS.iter().any(|(_, close)| *close == c)
}

fn is_quote(c: char) -> bool {
    matches!(c, '"' | '\'' | '`')
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum CharClass {
    Space,
    Word,
    Punct,
}

fn class(c: char) -> CharClass {
    if c.is_whitespace() {
        CharClass::Space
    } else if is_word(c) {
        CharClass::Word
    } else {
        CharClass::Punct
    }
}

/// End position of `text` inserted at `start`.
fn end_of(start: BufferPos, text: &str) -> BufferPos {
    match text.rfind('\n') {
        Some(i) => BufferPos::new(start.line + text.matches('\n').count(), text.len() - i - 1),
        None => BufferPos::new(start.line, start.col + text.len()),
    }
}

/// Lines, one selection and an undo history, with code-editing commands.
#[derive(Clone, Debug)]
pub struct CodeBuffer {
    lines: Vec<String>,
    selection: BufferSelection,
    /// Column (in characters) that vertical moves try to keep.
    goal: Option<usize>,
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    next_id: u64,
    saved: u64,
    break_group: bool,
    edited_from: Option<usize>,
    crlf: bool,
    /// Spaces per indentation level (4 by default).
    pub tab_size: usize,
    /// Indent with `'\t'` instead of spaces.
    pub hard_tabs: bool,
    /// Insert and skip over closing brackets and quotes.
    pub auto_pair: bool,
}

impl Default for CodeBuffer {
    fn default() -> Self {
        Self::new("")
    }
}

impl CodeBuffer {
    pub fn new(text: &str) -> Self {
        let mut buffer = Self {
            lines: vec![String::new()],
            selection: BufferSelection::default(),
            goal: None,
            undo: Vec::new(),
            redo: Vec::new(),
            next_id: 1,
            saved: 0,
            break_group: false,
            edited_from: None,
            crlf: false,
            tab_size: 4,
            hard_tabs: false,
            auto_pair: true,
        };
        buffer.set_text(text);
        buffer
    }

    /// Replaces the whole text, clears the history and marks it saved (loading a file).
    /// `"\r\n"` line endings are remembered and restored by [`CodeBuffer::text`].
    pub fn set_text(&mut self, text: &str) {
        self.crlf = text.contains("\r\n");
        let normalized = text.replace("\r\n", "\n");
        self.lines = normalized.split('\n').map(str::to_owned).collect();
        self.selection = BufferSelection::default();
        self.goal = None;
        self.undo.clear();
        self.redo.clear();
        self.saved = 0;
        self.edited_from = Some(0);
    }

    /// The whole text, with the line endings it was loaded with.
    pub fn text(&self) -> String {
        self.lines.join(if self.crlf { "\r\n" } else { "\n" })
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn line(&self, ix: usize) -> &str {
        self.lines.get(ix).map(String::as_str).unwrap_or("")
    }

    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(String::as_str)
    }

    /// Length in bytes of the longest line.
    pub fn longest_line_len(&self) -> usize {
        self.lines.iter().map(String::len).max().unwrap_or(0)
    }

    pub fn selection(&self) -> BufferSelection {
        self.selection
    }

    pub fn cursor(&self) -> BufferPos {
        self.selection.head
    }

    /// Text between two positions (joined with `'\n'`).
    pub fn text_in(&self, range: Range<BufferPos>) -> String {
        let (start, end) = (self.clip(range.start), self.clip(range.end));
        if start.line == end.line {
            return self.lines[start.line][start.col..end.col.max(start.col)].to_owned();
        }
        let mut out = self.lines[start.line][start.col..].to_owned();
        for line in &self.lines[start.line + 1..end.line] {
            out.push('\n');
            out.push_str(line);
        }
        out.push('\n');
        out.push_str(&self.lines[end.line][..end.col]);
        out
    }

    pub fn selected_text(&self) -> String {
        self.text_in(self.selection.range())
    }

    /// Whether the text differs from the last [`CodeBuffer::mark_saved`] (or load).
    pub fn is_dirty(&self) -> bool {
        self.version() != self.saved
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.version();
        self.break_group = true;
    }

    fn version(&self) -> u64 {
        self.undo.last().map_or(0, |t| t.id)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// First line changed since the last call (for highlighter caches), if any.
    pub fn take_edited_from(&mut self) -> Option<usize> {
        self.edited_from.take()
    }

    /// The next edit starts a new undo step even if it continues the current typing.
    pub fn break_undo_group(&mut self) {
        self.break_group = true;
    }

    /// The same position kept inside the text and on a character boundary.
    pub fn clip(&self, pos: BufferPos) -> BufferPos {
        let line = pos.line.min(self.lines.len() - 1);
        let text = &self.lines[line];
        let mut col = pos.col.min(text.len());
        while !text.is_char_boundary(col) {
            col -= 1;
        }
        BufferPos::new(line, col)
    }

    fn end(&self) -> BufferPos {
        let last = self.lines.len() - 1;
        BufferPos::new(last, self.lines[last].len())
    }

    // ---- Offsets (for the platform input handler) ----

    /// Byte offset of a position in the `'\n'`-joined text.
    pub fn offset_of(&self, pos: BufferPos) -> usize {
        let pos = self.clip(pos);
        self.lines[..pos.line]
            .iter()
            .map(|l| l.len() + 1)
            .sum::<usize>()
            + pos.col
    }

    /// Position of a byte offset in the `'\n'`-joined text.
    pub fn pos_of(&self, mut offset: usize) -> BufferPos {
        for (i, line) in self.lines.iter().enumerate() {
            if offset <= line.len() {
                return self.clip(BufferPos::new(i, offset));
            }
            offset -= line.len() + 1;
        }
        self.end()
    }

    /// UTF-16 offset of a position in the `'\n'`-joined text.
    pub fn utf16_of(&self, pos: BufferPos) -> usize {
        let pos = self.clip(pos);
        self.lines[..pos.line]
            .iter()
            .map(|l| l.encode_utf16().count() + 1)
            .sum::<usize>()
            + self.lines[pos.line][..pos.col].encode_utf16().count()
    }

    /// Position of a UTF-16 offset in the `'\n'`-joined text.
    pub fn pos_of_utf16(&self, mut offset: usize) -> BufferPos {
        for (i, line) in self.lines.iter().enumerate() {
            let len = line.encode_utf16().count();
            if offset <= len {
                let mut units = 0;
                for (col, ch) in line.char_indices() {
                    if units >= offset {
                        return BufferPos::new(i, col);
                    }
                    units += ch.len_utf16();
                }
                return BufferPos::new(i, line.len());
            }
            offset -= len + 1;
        }
        self.end()
    }

    // ---- Characters around a position ----

    fn char_before(&self, pos: BufferPos) -> Option<char> {
        self.lines[pos.line][..pos.col].chars().next_back()
    }

    fn char_after(&self, pos: BufferPos) -> Option<char> {
        self.lines[pos.line][pos.col..].chars().next()
    }

    fn prev_pos(&self, pos: BufferPos) -> BufferPos {
        if pos.col == 0 {
            if pos.line == 0 {
                return pos;
            }
            return BufferPos::new(pos.line - 1, self.lines[pos.line - 1].len());
        }
        let line = &self.lines[pos.line];
        let col = line[..pos.col]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i);
        BufferPos::new(pos.line, col)
    }

    fn next_pos(&self, pos: BufferPos) -> BufferPos {
        let line = &self.lines[pos.line];
        if pos.col >= line.len() {
            if pos.line + 1 >= self.lines.len() {
                return pos;
            }
            return BufferPos::new(pos.line + 1, 0);
        }
        let col = line[pos.col..]
            .graphemes(true)
            .next()
            .map_or(line.len(), |g| pos.col + g.len());
        BufferPos::new(pos.line, col)
    }

    fn word_start(&self, pos: BufferPos) -> BufferPos {
        if pos.col == 0 {
            return self.prev_pos(pos);
        }
        let line = &self.lines[pos.line];
        let mut chars = line[..pos.col].char_indices().rev().peekable();
        let mut col = pos.col;
        while let Some(&(i, c)) = chars.peek() {
            if !c.is_whitespace() {
                break;
            }
            col = i;
            chars.next();
        }
        if let Some(&(_, c)) = chars.peek() {
            let k = class(c);
            while let Some(&(i, c)) = chars.peek() {
                if class(c) != k {
                    break;
                }
                col = i;
                chars.next();
            }
        }
        BufferPos::new(pos.line, col)
    }

    fn word_end(&self, pos: BufferPos) -> BufferPos {
        let line = &self.lines[pos.line];
        if pos.col >= line.len() {
            return self.next_pos(pos);
        }
        let mut chars = line[pos.col..].char_indices().peekable();
        let mut col = pos.col;
        while let Some(&(i, c)) = chars.peek() {
            if !c.is_whitespace() {
                break;
            }
            col = pos.col + i + c.len_utf8();
            chars.next();
        }
        if let Some(&(_, c)) = chars.peek() {
            let k = class(c);
            while let Some(&(i, c)) = chars.peek() {
                if class(c) != k {
                    break;
                }
                col = pos.col + i + c.len_utf8();
                chars.next();
            }
        }
        BufferPos::new(pos.line, col)
    }

    /// Characters before a column (the column vertical moves keep).
    fn char_col(&self, pos: BufferPos) -> usize {
        self.lines[pos.line][..pos.col].chars().count()
    }

    fn col_for_chars(&self, line: usize, chars: usize) -> usize {
        let text = &self.lines[line];
        text.char_indices()
            .nth(chars)
            .map_or(text.len(), |(i, _)| i)
    }

    fn indent_of(&self, line: usize) -> &str {
        let text = &self.lines[line];
        let n = text.len() - text.trim_start_matches([' ', '\t']).len();
        &text[..n]
    }

    fn indent_unit(&self) -> String {
        if self.hard_tabs {
            "\t".into()
        } else {
            " ".repeat(self.tab_size.max(1))
        }
    }

    // ---- BufferSelection and movement ----

    fn place(&mut self, head: BufferPos, select: bool) {
        let head = self.clip(head);
        self.selection.head = head;
        if !select {
            self.selection.anchor = head;
        }
    }

    /// Moves the caret (extending the selection with `select`). Mouse clicks use this.
    pub fn set_cursor(&mut self, pos: BufferPos, select: bool) {
        self.goal = None;
        self.place(pos, select);
    }

    pub fn set_selection(&mut self, selection: BufferSelection) {
        self.goal = None;
        self.selection = BufferSelection {
            anchor: self.clip(selection.anchor),
            head: self.clip(selection.head),
        };
    }

    pub fn select_all(&mut self) {
        self.goal = None;
        self.selection = BufferSelection {
            anchor: BufferPos::default(),
            head: self.end(),
        };
    }

    /// Selects the word (or run of spaces or punctuation) at `pos` (double click).
    pub fn select_word_at(&mut self, pos: BufferPos) {
        let pos = self.clip(pos);
        let line = &self.lines[pos.line];
        let Some(c) = line[pos.col..]
            .chars()
            .next()
            .or_else(|| line[..pos.col].chars().next_back())
        else {
            self.set_cursor(pos, false);
            return;
        };
        let k = class(c);
        let start = line[..pos.col]
            .char_indices()
            .rev()
            .take_while(|(_, c)| class(*c) == k)
            .last()
            .map_or(pos.col, |(i, _)| i);
        let end = line[pos.col..]
            .char_indices()
            .take_while(|(_, c)| class(*c) == k)
            .last()
            .map_or(pos.col, |(i, c)| pos.col + i + c.len_utf8());
        self.goal = None;
        self.selection = BufferSelection {
            anchor: BufferPos::new(pos.line, start),
            head: BufferPos::new(pos.line, end),
        };
    }

    /// Selects a whole line, including its line break (triple click).
    pub fn select_line(&mut self, line: usize) {
        let line = line.min(self.lines.len() - 1);
        let head = if line + 1 < self.lines.len() {
            BufferPos::new(line + 1, 0)
        } else {
            BufferPos::new(line, self.lines[line].len())
        };
        self.goal = None;
        self.selection = BufferSelection {
            anchor: BufferPos::new(line, 0),
            head,
        };
    }

    pub fn move_left(&mut self, select: bool) {
        self.goal = None;
        if !select && !self.selection.is_empty() {
            let start = self.selection.range().start;
            self.place(start, false);
        } else {
            let to = self.prev_pos(self.selection.head);
            self.place(to, select);
        }
    }

    pub fn move_right(&mut self, select: bool) {
        self.goal = None;
        if !select && !self.selection.is_empty() {
            let end = self.selection.range().end;
            self.place(end, false);
        } else {
            let to = self.next_pos(self.selection.head);
            self.place(to, select);
        }
    }

    /// Moves `lines` up (negative) or down, keeping the column. Past the first or last line the
    /// caret goes to the start or end of the text.
    pub fn move_vertically(&mut self, lines: isize, select: bool) {
        let head = self.selection.head;
        let goal = match self.goal {
            Some(goal) => goal,
            None => self.char_col(head),
        };
        self.goal = Some(goal);
        let target = head.line as isize + lines;
        let to = if target < 0 {
            BufferPos::default()
        } else if target as usize >= self.lines.len() {
            self.end()
        } else {
            let line = target as usize;
            BufferPos::new(line, self.col_for_chars(line, goal))
        };
        self.place(to, select);
    }

    pub fn move_word_left(&mut self, select: bool) {
        self.goal = None;
        let to = self.word_start(self.selection.head);
        self.place(to, select);
    }

    pub fn move_word_right(&mut self, select: bool) {
        self.goal = None;
        let to = self.word_end(self.selection.head);
        self.place(to, select);
    }

    /// Smart Home: to the first non-blank character, or to column 0 if already there.
    pub fn move_home(&mut self, select: bool) {
        self.goal = None;
        let head = self.selection.head;
        let indent = self.indent_of(head.line).len();
        let col = if head.col == indent { 0 } else { indent };
        self.place(BufferPos::new(head.line, col), select);
    }

    pub fn move_end(&mut self, select: bool) {
        self.goal = None;
        let line = self.selection.head.line;
        self.place(BufferPos::new(line, self.lines[line].len()), select);
    }

    pub fn move_to_start(&mut self, select: bool) {
        self.goal = None;
        self.place(BufferPos::default(), select);
    }

    pub fn move_to_end(&mut self, select: bool) {
        self.goal = None;
        let end = self.end();
        self.place(end, select);
    }

    // ---- Editing ----

    /// Replaces `range` with `text` without recording history; returns the end of the new text.
    fn splice(&mut self, range: Range<BufferPos>, text: &str) -> BufferPos {
        let (start, end) = (self.clip(range.start), self.clip(range.end));
        let tail = self.lines[end.line][end.col..].to_owned();
        let head = self.lines[start.line][..start.col].to_owned();
        let mut new_lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
        let last = new_lines.len() - 1;
        let end_pos = end_of(start, text);
        new_lines[0].insert_str(0, &head);
        new_lines[last].push_str(&tail);
        self.lines.splice(start.line..=end.line, new_lines);
        self.edited_from = Some(self.edited_from.map_or(start.line, |l| l.min(start.line)));
        end_pos
    }

    /// Records one replacement inside a transaction.
    fn record(&mut self, edits: &mut Vec<Edit>, range: Range<BufferPos>, text: &str) -> BufferPos {
        let (start, end) = (self.clip(range.start), self.clip(range.end));
        let old = self.text_in(start..end);
        let end = self.splice(start..end, text);
        edits.push(Edit {
            start,
            old,
            new: text.to_owned(),
        });
        end
    }

    fn transact(&mut self, kind: EditKind, f: impl FnOnce(&mut Self, &mut Vec<Edit>)) {
        let before = self.selection;
        let mut edits = Vec::new();
        f(self, &mut edits);
        self.goal = None;
        if edits.is_empty() {
            return;
        }
        self.redo.clear();
        let after = self.selection;
        let merge = !self.break_group
            && kind != EditKind::Other
            && self.undo.last().is_some_and(|last| {
                last.kind == kind && last.after == before && last.id != self.saved
            });
        self.break_group = false;
        if merge {
            let last = self.undo.last_mut().unwrap();
            last.edits.extend(edits);
            last.after = after;
        } else {
            let id = self.next_id;
            self.next_id += 1;
            self.undo.push(Transaction {
                id,
                kind,
                edits,
                before,
                after,
            });
        }
    }

    fn replace_selection(&mut self, text: &str, kind: EditKind) {
        self.transact(kind, |b, edits| {
            let end = b.record(edits, b.selection.range(), text);
            b.selection = BufferSelection::caret(end);
        });
    }

    /// Replaces the selection with `text` as one undo step (paste, completion, drop).
    pub fn insert(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n");
        self.replace_selection(&text, EditKind::Other);
    }

    /// Replaces a range (an IME composition, a find-and-replace) and puts the caret after it.
    pub fn replace(&mut self, range: Range<BufferPos>, text: &str) {
        self.transact(EditKind::Other, |b, edits| {
            let end = b.record(edits, range, text);
            b.selection = BufferSelection::caret(end);
        });
    }

    /// Typed text: single characters pair brackets and quotes, skip over a closing one that is
    /// already there, and wrap a selection; consecutive typing undoes as one step.
    pub fn type_text(&mut self, text: &str) {
        let mut chars = text.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            self.replace_selection(&text.replace("\r\n", "\n"), EditKind::Insert);
            return;
        };
        if c == '\n' {
            self.newline();
            return;
        }
        if !self.auto_pair {
            self.replace_selection(text, EditKind::Insert);
            return;
        }
        let sel = self.selection;
        let head = sel.head;
        // A closing character that is already next to the caret: step over it.
        if sel.is_empty() && is_closer(c) && self.char_after(head) == Some(c) {
            let to = self.next_pos(head);
            self.set_cursor(to, false);
            self.break_group = true;
            return;
        }
        if let Some(close) = closer_for(c) {
            if !sel.is_empty() {
                // Wrap the selection, keeping it selected inside the pair.
                let range = sel.range();
                self.transact(EditKind::Other, |b, edits| {
                    let mut close_buf = [0; 4];
                    b.record(
                        edits,
                        range.end..range.end,
                        close.encode_utf8(&mut close_buf),
                    );
                    let mut open_buf = [0; 4];
                    b.record(
                        edits,
                        range.start..range.start,
                        c.encode_utf8(&mut open_buf),
                    );
                    let shift = |p: BufferPos| {
                        if p.line == range.start.line {
                            BufferPos::new(p.line, p.col + c.len_utf8())
                        } else {
                            p
                        }
                    };
                    b.selection = BufferSelection {
                        anchor: shift(sel.anchor),
                        head: shift(sel.head),
                    };
                });
                return;
            }
            let next_ok = self
                .char_after(head)
                .is_none_or(|n| n.is_whitespace() || is_closer(n) || n == ',' || n == ';');
            let prev = self.char_before(head);
            let prev_ok = !is_quote(c) || prev.is_none_or(|p| !is_word(p) && p != c);
            if next_ok && prev_ok {
                let pair = format!("{c}{close}");
                self.transact(EditKind::Insert, |b, edits| {
                    b.record(edits, head..head, &pair);
                    b.selection =
                        BufferSelection::caret(BufferPos::new(head.line, head.col + c.len_utf8()));
                });
                return;
            }
        }
        self.replace_selection(text, EditKind::Insert);
    }

    /// Enter: keeps the line's indentation, adds a level after an opening bracket and moves the
    /// matching closing bracket to its own line.
    pub fn newline(&mut self) {
        let range = self.selection.range();
        let start = range.start;
        let indent = self.indent_of(start.line);
        let indent = indent[..indent.len().min(start.col)].to_owned();
        let before = self.lines[start.line][..start.col]
            .trim_end()
            .chars()
            .next_back();
        let after = self.lines[range.end.line][range.end.col..].chars().next();
        let opens = before.filter(|c| matches!(c, '(' | '[' | '{'));
        let unit = self.indent_unit();
        self.transact(EditKind::Other, |b, edits| match opens {
            Some(open) if after == closer_for(open) => {
                let text = format!("\n{indent}{unit}\n{indent}");
                b.record(edits, range.clone(), &text);
                b.selection = BufferSelection::caret(BufferPos::new(
                    start.line + 1,
                    indent.len() + unit.len(),
                ));
            }
            Some(_) => {
                let end = b.record(edits, range.clone(), &format!("\n{indent}{unit}"));
                b.selection = BufferSelection::caret(end);
            }
            None => {
                let end = b.record(edits, range.clone(), &format!("\n{indent}"));
                b.selection = BufferSelection::caret(end);
            }
        });
    }

    /// Backspace: deletes the selection, an empty bracket pair, one indentation level inside
    /// leading spaces, or the previous character.
    pub fn backspace(&mut self) {
        let sel = self.selection;
        if !sel.is_empty() {
            self.replace_selection("", EditKind::Delete);
            return;
        }
        let head = sel.head;
        if head == BufferPos::default() {
            return;
        }
        let mut start = self.prev_pos(head);
        let mut end = head;
        let before = self.char_before(head);
        let after = self.char_after(head);
        if self.auto_pair
            && let (Some(b), Some(a)) = (before, after)
            && closer_for(b) == Some(a)
        {
            end = self.next_pos(head);
        } else if !self.hard_tabs && head.col > 0 {
            let leading = &self.lines[head.line][..head.col];
            if leading.chars().all(|c| c == ' ') {
                let size = self.tab_size.max(1);
                let back = match head.col % size {
                    0 => size,
                    r => r,
                };
                start = BufferPos::new(head.line, head.col - back);
            }
        }
        self.transact(EditKind::Delete, |b, edits| {
            b.record(edits, start..end, "");
            b.selection = BufferSelection::caret(start);
        });
    }

    /// Delete: deletes the selection or the next character.
    pub fn delete(&mut self) {
        if !self.selection.is_empty() {
            self.replace_selection("", EditKind::Delete);
            return;
        }
        let head = self.selection.head;
        let end = self.next_pos(head);
        if end == head {
            return;
        }
        self.transact(EditKind::Delete, |b, edits| {
            b.record(edits, head..end, "");
            b.selection = BufferSelection::caret(head);
        });
    }

    /// Deletes to the start of the word (Ctrl+Backspace).
    pub fn delete_word_left(&mut self) {
        if self.selection.is_empty() {
            let to = self.word_start(self.selection.head);
            self.place(to, true);
        }
        self.replace_selection("", EditKind::Other);
    }

    /// Deletes to the end of the word (Ctrl+Delete).
    pub fn delete_word_right(&mut self) {
        if self.selection.is_empty() {
            let to = self.word_end(self.selection.head);
            self.place(to, true);
        }
        self.replace_selection("", EditKind::Other);
    }

    /// Tab: with a selection over several lines, indents them all; otherwise inserts spaces up
    /// to the next tab stop (or a tab character with [`CodeBuffer::hard_tabs`]).
    pub fn indent(&mut self) {
        let sel = self.selection;
        if sel.is_empty() {
            let text = if self.hard_tabs {
                "\t".to_owned()
            } else {
                let size = self.tab_size.max(1);
                " ".repeat(size - self.char_col(sel.head) % size)
            };
            self.replace_selection(&text, EditKind::Other);
            return;
        }
        let lines = sel.lines();
        let unit = self.indent_unit();
        self.shift_lines(lines, |_| (0, unit.clone()));
    }

    /// Shift+Tab: removes one indentation level from every line the selection touches.
    pub fn outdent(&mut self) {
        let lines = self.selection.lines();
        let size = self.tab_size.max(1);
        let snapshot: Vec<(usize, usize)> = lines
            .clone()
            .map(|l| {
                let text = &self.lines[l];
                let n = if text.starts_with('\t') {
                    1
                } else {
                    text.chars().take(size).take_while(|c| *c == ' ').count()
                };
                (l, n)
            })
            .collect();
        self.shift_lines(lines, |l| {
            let n = snapshot
                .iter()
                .find(|(line, _)| *line == l)
                .map_or(0, |s| s.1);
            (n, String::new())
        });
    }

    /// Replaces the first `remove` bytes of each line with `insert`, keeping the selection on the
    /// same text.
    fn shift_lines(&mut self, lines: Range<usize>, f: impl Fn(usize) -> (usize, String)) {
        let sel = self.selection;
        let changes: Vec<(usize, usize, String)> = lines
            .map(|l| {
                let (remove, insert) = f(l);
                (l, remove, insert)
            })
            .filter(|(_, remove, insert)| *remove > 0 || !insert.is_empty())
            .collect();
        if changes.is_empty() {
            return;
        }
        let adjust = |p: BufferPos| -> BufferPos {
            match changes.iter().find(|(l, _, _)| *l == p.line) {
                Some((_, remove, insert)) => {
                    BufferPos::new(p.line, p.col.saturating_sub(*remove) + insert.len())
                }
                None => p,
            }
        };
        let after = BufferSelection {
            anchor: adjust(sel.anchor),
            head: adjust(sel.head),
        };
        self.transact(EditKind::Other, |b, edits| {
            for (line, remove, insert) in &changes {
                b.record(
                    edits,
                    BufferPos::new(*line, 0)..BufferPos::new(*line, *remove),
                    insert,
                );
            }
            b.selection = after;
        });
    }

    pub fn undo(&mut self) -> bool {
        let Some(t) = self.undo.pop() else {
            return false;
        };
        for edit in t.edits.iter().rev() {
            let end = end_of(edit.start, &edit.new);
            self.splice(edit.start..end, &edit.old);
        }
        self.selection = t.before;
        self.goal = None;
        self.break_group = true;
        self.redo.push(t);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(t) = self.redo.pop() else {
            return false;
        };
        for edit in &t.edits {
            let end = end_of(edit.start, &edit.old);
            self.splice(edit.start..end, &edit.new);
        }
        self.selection = t.after;
        self.goal = None;
        self.break_group = true;
        self.undo.push(t);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: usize, col: usize) -> BufferPos {
        BufferPos::new(line, col)
    }

    fn typed(b: &mut CodeBuffer, text: &str) {
        for c in text.chars() {
            b.type_text(&c.to_string());
        }
    }

    #[test]
    fn splits_lines_and_keeps_crlf() {
        let b = CodeBuffer::new("a\r\nbc\r\n");
        assert_eq!(b.line_count(), 3);
        assert_eq!(b.line(1), "bc");
        assert_eq!(b.text(), "a\r\nbc\r\n");
        assert!(!b.is_dirty());
    }

    #[test]
    fn inserts_multiline_text_and_reads_ranges() {
        let mut b = CodeBuffer::new("hello world");
        b.set_cursor(at(0, 5), false);
        b.insert(",\nnew\nline");
        assert_eq!(b.text(), "hello,\nnew\nline world");
        assert_eq!(b.cursor(), at(2, 4));
        assert_eq!(b.text_in(at(0, 3)..at(2, 2)), "lo,\nnew\nli");
        assert_eq!(b.offset_of(at(1, 1)), 8);
        assert_eq!(b.pos_of(8), at(1, 1));
    }

    #[test]
    fn utf16_offsets_round_trip() {
        let b = CodeBuffer::new("añ😀\nx");
        let pos = at(1, 1);
        let u = b.utf16_of(pos);
        assert_eq!(u, 6); // a ñ 😀(2) \n x
        assert_eq!(b.pos_of_utf16(u), pos);
        assert_eq!(b.pos_of_utf16(2), at(0, 3));
    }

    #[test]
    fn typing_undoes_as_one_step_and_redo_restores() {
        let mut b = CodeBuffer::new("");
        typed(&mut b, "let x");
        assert_eq!(b.text(), "let x");
        assert!(b.is_dirty());
        assert!(b.undo());
        assert_eq!(b.text(), "");
        assert!(!b.is_dirty());
        assert!(b.redo());
        assert_eq!(b.text(), "let x");
        assert_eq!(b.cursor(), at(0, 5));
    }

    #[test]
    fn moving_the_caret_starts_a_new_undo_step() {
        let mut b = CodeBuffer::new("");
        typed(&mut b, "ab");
        b.move_left(false);
        typed(&mut b, "X");
        assert_eq!(b.text(), "aXb");
        b.undo();
        assert_eq!(b.text(), "ab");
        b.undo();
        assert_eq!(b.text(), "");
    }

    #[test]
    fn saving_marks_clean_and_does_not_merge_into_the_saved_step() {
        let mut b = CodeBuffer::new("");
        typed(&mut b, "ab");
        b.mark_saved();
        assert!(!b.is_dirty());
        typed(&mut b, "c");
        assert!(b.is_dirty());
        b.undo();
        assert_eq!(b.text(), "ab");
        assert!(!b.is_dirty());
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let mut b = CodeBuffer::new("");
        typed(&mut b, "a");
        b.undo();
        typed(&mut b, "b");
        assert!(!b.redo());
        assert_eq!(b.text(), "b");
    }

    #[test]
    fn newline_keeps_indentation() {
        let mut b = CodeBuffer::new("    let x = 1;");
        b.move_end(false);
        b.newline();
        assert_eq!(b.text(), "    let x = 1;\n    ");
        assert_eq!(b.cursor(), at(1, 4));
    }

    #[test]
    fn newline_after_brace_indents_and_splits_the_pair() {
        let mut b = CodeBuffer::new("  fn f() {}");
        b.set_cursor(at(0, 10), false);
        b.newline();
        assert_eq!(b.text(), "  fn f() {\n      \n  }");
        assert_eq!(b.cursor(), at(1, 6));
        // Without a closing brace next to the caret, it only adds a level.
        let mut b = CodeBuffer::new("if x {");
        b.move_end(false);
        b.newline();
        assert_eq!(b.text(), "if x {\n    ");
    }

    #[test]
    fn tab_inserts_to_the_next_stop_and_shift_tab_outdents() {
        let mut b = CodeBuffer::new("ab");
        b.set_cursor(at(0, 1), false);
        b.indent();
        assert_eq!(b.text(), "a   b");
        assert_eq!(b.cursor(), at(0, 4));
        b.outdent();
        assert_eq!(b.text(), "a   b"); // the line had no leading indentation
        let mut b = CodeBuffer::new("      x");
        b.move_end(false);
        b.outdent();
        assert_eq!(b.text(), "  x");
        assert_eq!(b.cursor(), at(0, 3));
    }

    #[test]
    fn tab_indents_every_selected_line_and_keeps_the_selection() {
        let mut b = CodeBuffer::new("a\nb\nc");
        b.set_selection(BufferSelection {
            anchor: at(0, 1),
            head: at(2, 0),
        });
        b.indent();
        // The last line is not touched: the selection ends at its column 0.
        assert_eq!(b.text(), "    a\n    b\nc");
        assert_eq!(b.selection().anchor, at(0, 5));
        assert_eq!(b.selection().head, at(2, 0));
        b.outdent();
        assert_eq!(b.text(), "a\nb\nc");
        b.undo();
        assert_eq!(b.text(), "    a\n    b\nc");
        b.undo();
        assert_eq!(b.text(), "a\nb\nc");
    }

    #[test]
    fn hard_tabs_indent_with_tab_characters() {
        let mut b = CodeBuffer::new("x\ny");
        b.hard_tabs = true;
        b.select_all();
        b.indent();
        assert_eq!(b.text(), "\tx\n\ty");
        b.outdent();
        assert_eq!(b.text(), "x\ny");
    }

    #[test]
    fn backspace_in_leading_spaces_removes_one_level() {
        let mut b = CodeBuffer::new("      x");
        b.set_cursor(at(0, 6), false);
        b.backspace();
        assert_eq!(b.text(), "    x");
        b.backspace();
        assert_eq!(b.text(), "x");
    }

    #[test]
    fn brackets_pair_skip_and_delete_together() {
        let mut b = CodeBuffer::new("");
        typed(&mut b, "f(");
        assert_eq!(b.text(), "f()");
        assert_eq!(b.cursor(), at(0, 2));
        typed(&mut b, "a)");
        assert_eq!(b.text(), "f(a)");
        assert_eq!(b.cursor(), at(0, 4));
        let mut b = CodeBuffer::new("");
        typed(&mut b, "[");
        b.backspace();
        assert_eq!(b.text(), "");
    }

    #[test]
    fn brackets_do_not_pair_before_a_word_and_quotes_not_after_one() {
        let mut b = CodeBuffer::new("x");
        b.set_cursor(at(0, 0), false);
        typed(&mut b, "(");
        assert_eq!(b.text(), "(x");
        let mut b = CodeBuffer::new("don");
        b.move_end(false);
        typed(&mut b, "'");
        assert_eq!(b.text(), "don'");
        let mut b = CodeBuffer::new("");
        typed(&mut b, "\"hi\"");
        assert_eq!(b.text(), "\"hi\"");
    }

    #[test]
    fn typing_a_bracket_wraps_the_selection() {
        let mut b = CodeBuffer::new("call x now");
        b.set_selection(BufferSelection {
            anchor: at(0, 5),
            head: at(0, 6),
        });
        typed(&mut b, "(");
        assert_eq!(b.text(), "call (x) now");
        assert_eq!(b.selected_text(), "x");
    }

    #[test]
    fn deleting_across_lines_joins_them() {
        let mut b = CodeBuffer::new("ab\ncd");
        b.set_cursor(at(1, 0), false);
        b.backspace();
        assert_eq!(b.text(), "abcd");
        assert_eq!(b.cursor(), at(0, 2));
        b.delete();
        assert_eq!(b.text(), "abd");
    }

    #[test]
    fn vertical_moves_keep_the_goal_column() {
        let mut b = CodeBuffer::new("abcdef\nab\nabcdef");
        b.set_cursor(at(0, 5), false);
        b.move_vertically(1, false);
        assert_eq!(b.cursor(), at(1, 2));
        b.move_vertically(1, false);
        assert_eq!(b.cursor(), at(2, 5));
        b.move_vertically(5, false);
        assert_eq!(b.cursor(), at(2, 6));
    }

    #[test]
    fn word_moves_and_double_click_selection() {
        let mut b = CodeBuffer::new("let foo_bar = 1;");
        b.set_cursor(at(0, 0), false);
        b.move_word_right(false);
        assert_eq!(b.cursor(), at(0, 3));
        b.move_word_right(false);
        assert_eq!(b.cursor(), at(0, 11));
        b.move_word_left(false);
        assert_eq!(b.cursor(), at(0, 4));
        b.select_word_at(at(0, 6));
        assert_eq!(b.selected_text(), "foo_bar");
    }

    #[test]
    fn smart_home_toggles_between_indent_and_column_zero() {
        let mut b = CodeBuffer::new("    x");
        b.move_end(false);
        b.move_home(false);
        assert_eq!(b.cursor(), at(0, 4));
        b.move_home(false);
        assert_eq!(b.cursor(), at(0, 0));
    }

    #[test]
    fn edited_from_reports_the_first_changed_line() {
        let mut b = CodeBuffer::new("a\nb\nc");
        b.take_edited_from();
        b.set_cursor(at(2, 1), false);
        typed(&mut b, "x");
        assert_eq!(b.take_edited_from(), Some(2));
        assert_eq!(b.take_edited_from(), None);
    }

    #[test]
    fn selection_lines_skip_a_trailing_column_zero() {
        let s = BufferSelection {
            anchor: at(3, 2),
            head: at(1, 0),
        };
        assert_eq!(s.lines(), 1..4);
        let s = BufferSelection {
            anchor: at(1, 0),
            head: at(3, 0),
        };
        assert_eq!(s.lines(), 1..3);
    }
}
