//! Line diffs: the pure model ([`Diff`]: lines, hunks, folding, stats) and its views
//! ([`DiffView`], [`DiffBar`]).

use std::collections::BTreeSet;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, relative,
};
use similar::{ChangeTag, DiffTag, TextDiff};

use super::child_id;
use crate::cache::{Lru, content_key};
use crate::motion::key;
use crate::theme::{FONT_FAMILY, MONO_FONT_FAMILY, Scheme, Theme, alpha};

/// Whether a diff line was added, removed or kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffLineKind {
    Added,
    Removed,
    Context,
}

/// One line of a diff with its numbers in the old and new texts (1-based).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
    pub old: Option<usize>,
    pub new: Option<usize>,
}

/// Lines added and removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DiffStats {
    pub added: usize,
    pub removed: usize,
}

/// A unified-diff hunk: a group of changes with up to `context` unchanged lines around them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffHunk {
    /// First line in the old text (1-based; for an empty side, the line before it, as in `@@`).
    pub old_start: usize,
    pub old_len: usize,
    pub new_start: usize,
    pub new_len: usize,
    /// The hunk's lines, as indices into [`Diff::lines`].
    pub lines: Range<usize>,
}

impl DiffHunk {
    /// `@@ -a,b +c,d @@`
    pub fn header(&self) -> String {
        format!(
            "@@ -{},{} +{},{} @@",
            self.old_start, self.old_len, self.new_start, self.new_len
        )
    }
}

/// What a [`DiffView`] shows at each row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffRow {
    /// A line, as an index into [`Diff::lines`].
    Line(usize),
    /// Unchanged lines folded away; `id` identifies the fold to open it.
    Fold { id: usize, lines: Range<usize> },
    /// Lines a patch does not include (between the hunks of [`Diff::from_unified`]).
    Skipped { lines: usize },
}

/// Line diff between two texts, computed once and cheap to render.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diff {
    lines: Vec<DiffLine>,
    stats: DiffStats,
    new_file: bool,
}

/// Without a final line break, a line diff would mark the last line as changed even when equal.
fn with_eol(text: &str) -> std::borrow::Cow<'_, str> {
    if text.is_empty() || text.ends_with('\n') {
        text.into()
    } else {
        format!("{text}\n").into()
    }
}

fn strip_eol(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

fn line_diff<'a>(old: &'a str, new: &'a str) -> TextDiff<'a, 'a, 'a, str> {
    TextDiff::configure()
        .timeout(Duration::from_millis(400))
        .diff_lines(old, new)
}

/// Lines added and removed between two texts (a real line diff, as `diff` does).
pub fn diff_stats(old: &str, new: &str) -> DiffStats {
    let (old, new) = (with_eol(old), with_eol(new));
    let diff = line_diff(&old, &new);
    let mut stats = DiffStats::default();
    for op in diff.ops() {
        let (tag, old, new) = op.as_tag_tuple();
        match tag {
            DiffTag::Equal => {}
            DiffTag::Delete => stats.removed += old.len(),
            DiffTag::Insert => stats.added += new.len(),
            DiffTag::Replace => {
                stats.removed += old.len();
                stats.added += new.len();
            }
        }
    }
    stats
}

impl Diff {
    /// Diff of `old` → `new`. An empty `old` is a new file.
    pub fn new(old: &str, new: &str) -> Self {
        let (old_text, new_text) = (with_eol(old), with_eol(new));
        let diff = line_diff(&old_text, &new_text);
        let mut lines = Vec::new();
        let mut stats = DiffStats::default();
        for change in diff.iter_all_changes() {
            let text = strip_eol(change.value()).to_string();
            let old = change.old_index().map(|i| i + 1);
            let new = change.new_index().map(|i| i + 1);
            let kind = match change.tag() {
                ChangeTag::Insert => {
                    stats.added += 1;
                    DiffLineKind::Added
                }
                ChangeTag::Delete => {
                    stats.removed += 1;
                    DiffLineKind::Removed
                }
                ChangeTag::Equal => DiffLineKind::Context,
            };
            lines.push(DiffLine {
                kind,
                text,
                old,
                new,
            });
        }
        Self {
            lines,
            stats,
            new_file: old.is_empty(),
        }
    }

    /// Lines of a unified diff (`git diff` output); everything outside the hunks is ignored.
    pub fn from_unified(patch: &str) -> Self {
        let mut lines = Vec::new();
        let mut stats = DiffStats::default();
        let (mut old, mut new) = (0usize, 0usize);
        let mut in_hunk = false;
        let mut new_file = false;
        for raw in patch.lines() {
            if let Some(rest) = raw.strip_prefix("@@") {
                in_hunk = true;
                let mut parts = rest.split_whitespace();
                let start = |part: Option<&str>, sign: char| {
                    part.and_then(|p| p.strip_prefix(sign))
                        .and_then(|p| p.split(',').next())
                        .and_then(|n| n.parse::<usize>().ok())
                        .unwrap_or(1)
                };
                old = start(parts.next(), '-');
                new = start(parts.next(), '+');
                continue;
            }
            if !in_hunk {
                if raw.starts_with("--- /dev/null") || raw.starts_with("new file mode") {
                    new_file = true;
                }
                continue;
            }
            let (kind, body) = match raw.chars().next() {
                Some('+') => (DiffLineKind::Added, &raw[1..]),
                Some('-') => (DiffLineKind::Removed, &raw[1..]),
                Some(' ') => (DiffLineKind::Context, &raw[1..]),
                Some('\\') => continue,
                None => (DiffLineKind::Context, ""),
                _ => {
                    // The next file's header.
                    in_hunk = false;
                    continue;
                }
            };
            let (o, n) = match kind {
                DiffLineKind::Added => {
                    stats.added += 1;
                    new += 1;
                    (None, Some(new - 1))
                }
                DiffLineKind::Removed => {
                    stats.removed += 1;
                    old += 1;
                    (Some(old - 1), None)
                }
                DiffLineKind::Context => {
                    old += 1;
                    new += 1;
                    (Some(old - 1), Some(new - 1))
                }
            };
            lines.push(DiffLine {
                kind,
                text: strip_eol(body).to_string(),
                old: o,
                new: n,
            });
        }
        Self {
            lines,
            stats,
            new_file,
        }
    }

    pub fn lines(&self) -> &[DiffLine] {
        &self.lines
    }

    pub fn stats(&self) -> DiffStats {
        self.stats
    }

    /// The old text was empty (the view hides the old line numbers).
    pub fn is_new_file(&self) -> bool {
        self.new_file
    }

    /// Nothing changed.
    pub fn is_empty(&self) -> bool {
        self.stats == DiffStats::default()
    }

    /// Unified-diff hunks with `context` unchanged lines around each group of changes.
    pub fn hunks(&self, context: usize) -> Vec<DiffHunk> {
        let changed: Vec<usize> = (0..self.lines.len())
            .filter(|&i| self.lines[i].kind != DiffLineKind::Context)
            .collect();
        let mut hunks = Vec::new();
        let mut i = 0;
        while i < changed.len() {
            let start = changed[i].saturating_sub(context);
            let mut end = changed[i] + 1;
            // Merge the next change while the gap fits in the context of both.
            while i + 1 < changed.len() && changed[i + 1] <= end + 2 * context {
                i += 1;
                end = changed[i] + 1;
            }
            let end = (end + context).min(self.lines.len());
            hunks.push(self.hunk(start..end));
            i += 1;
        }
        hunks
    }

    fn hunk(&self, range: Range<usize>) -> DiffHunk {
        let lines = &self.lines[range.clone()];
        let old_len = lines.iter().filter(|l| l.old.is_some()).count();
        let new_len = lines.iter().filter(|l| l.new.is_some()).count();
        // The number before the hunk, for a side with no lines.
        let before = |pick: fn(&DiffLine) -> Option<usize>| {
            self.lines[..range.start]
                .iter()
                .rev()
                .find_map(pick)
                .unwrap_or(0)
        };
        let old_start = lines
            .iter()
            .find_map(|l| l.old)
            .unwrap_or_else(|| before(|l| l.old));
        let new_start = lines
            .iter()
            .find_map(|l| l.new)
            .unwrap_or_else(|| before(|l| l.new));
        DiffHunk {
            old_start,
            old_len,
            new_start,
            new_len,
            lines: range,
        }
    }

    /// Rows to show: runs of unchanged lines longer than `context` before and after a change are
    /// folded, unless their id is in `open`. A fold of a single line is not worth it and stays.
    pub fn rows(&self, context: usize, open: &BTreeSet<usize>) -> Vec<DiffRow> {
        let lines = &self.lines;
        let mut rows = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            if i > 0
                && let Some(gap) = skipped(&lines[i - 1], &lines[i])
            {
                rows.push(DiffRow::Skipped { lines: gap });
            }
            if lines[i].kind != DiffLineKind::Context {
                rows.push(DiffRow::Line(i));
                i += 1;
                continue;
            }
            let mut j = i;
            while j < lines.len()
                && lines[j].kind == DiffLineKind::Context
                && (j == i || skipped(&lines[j - 1], &lines[j]).is_none())
            {
                j += 1;
            }
            let head = if i == 0 { 0 } else { context };
            let tail = if j == lines.len() { 0 } else { context };
            let hidden = (j - i).saturating_sub(head + tail);
            if hidden > 1 && !open.contains(&i) {
                rows.extend((i..i + head).map(DiffRow::Line));
                rows.push(DiffRow::Fold {
                    id: i,
                    lines: i + head..j - tail,
                });
                rows.extend((j - tail..j).map(DiffRow::Line));
            } else {
                rows.extend((i..j).map(DiffRow::Line));
            }
            i = j;
        }
        rows
    }
}

/// Lines a patch skips between two consecutive lines (their numbers jump).
fn skipped(prev: &DiffLine, next: &DiffLine) -> Option<usize> {
    let gap = |a: Option<usize>, b: Option<usize>| b?.checked_sub(a? + 1);
    gap(prev.old, next.old)
        .or_else(|| gap(prev.new, next.new))
        .filter(|&n| n > 0)
}

thread_local! {
    static DIFFS: std::cell::RefCell<Lru<Rc<Diff>>> = const { std::cell::RefCell::new(Lru::new(1024)) };
}

/// [`Diff::new`] cached by content (renders do not recompute it).
pub fn diff_cached(old: &str, new: &str) -> Rc<Diff> {
    let key = content_key(&[old, new]);
    DIFFS.with(|c| {
        c.borrow_mut()
            .get_or_insert_with(key, || Rc::new(Diff::new(old, new)))
    })
}

/* ─────────── Views ─────────── */

/// Background of added and removed rows (`--add`, `--del`).
fn row_colors(s: &Scheme, dark: bool) -> (Hsla, Hsla) {
    if dark {
        (alpha(s.success, 0.14), alpha(s.error, 0.13))
    } else {
        (alpha(s.success, 0.12), alpha(s.error, 0.10))
    }
}

fn expand_tabs(text: &str) -> String {
    text.replace('\t', "    ")
}

/// A line diff like the reference design's: old and new line numbers, `+`/`−` marks, unchanged
/// runs folded as "⋯ N unchanged lines" (click to open), "Show all" past `max_rows`, and an
/// optional header with the file name and `+N −M`. A new file hides the old numbers.
///
/// The diff is computed once per content and cached; pass a shared [`Diff`] with
/// [`DiffView::from_diff`] to skip even the lookup.
#[derive(IntoElement)]
pub struct DiffView {
    id: ElementId,
    diff: Rc<Diff>,
    path: Option<SharedString>,
    context: usize,
    max_rows: usize,
    bordered: bool,
}

impl DiffView {
    pub fn new(id: impl Into<ElementId>, old: &str, new: &str) -> Self {
        Self::from_diff(id, diff_cached(old, new))
    }

    pub fn from_diff(id: impl Into<ElementId>, diff: Rc<Diff>) -> Self {
        Self {
            id: id.into(),
            diff,
            path: None,
            context: 3,
            max_rows: 400,
            bordered: true,
        }
    }

    /// Shows a header with the file name (from the path) and the `+N −M` counts.
    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Unchanged lines kept around each change (3 by default).
    pub fn context(mut self, lines: usize) -> Self {
        self.context = lines;
        self
    }

    /// Rows shown before "Show all" (400 by default; the reference design uses 40 for Write).
    pub fn max_rows(mut self, rows: usize) -> Self {
        self.max_rows = rows.max(1);
        self
    }

    /// Without its own border and radius (inside a panel that already has them).
    pub fn bare(mut self) -> Self {
        self.bordered = false;
        self
    }
}

impl RenderOnce for DiffView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::get(cx);
        let s = theme.scheme;
        let (add_bg, del_bg) = row_colors(&s, theme.dark);
        let open =
            window.use_keyed_state(key(&self.id, "open"), cx, |_, _| BTreeSet::<usize>::new());
        let all = window.use_keyed_state(key(&self.id, "all"), cx, |_, _| false);
        let rows = self.diff.rows(self.context, open.read(cx));
        let show_all = *all.read(cx);
        let total = rows.len();
        let shown = if show_all {
            total
        } else {
            total.min(self.max_rows)
        };
        let new_file = self.diff.is_new_file();
        let lines = self.diff.lines();

        // Width of the number columns from the largest number (min-width 3.2em in the reference).
        let biggest = lines
            .iter()
            .filter_map(|l| l.old.max(l.new))
            .max()
            .unwrap_or(0);
        let digits = biggest.to_string().len().max(3) as f32;
        let num_w = px(digits * 7.2 + 12.);

        let num = |n: Option<usize>| {
            div()
                .flex_none()
                .w(num_w)
                .px(px(6.))
                .text_right()
                .text_color(s.outline)
                .child(n.map(|n| n.to_string()).unwrap_or_default())
        };

        let mut body = div().flex().flex_col().flex_none().min_w_full();
        for row in rows.into_iter().take(shown) {
            body = body.child(match row {
                DiffRow::Line(i) => {
                    let line = &lines[i];
                    let (bg, mark, mark_color) = match line.kind {
                        DiffLineKind::Added => (Some(add_bg), "+", s.success),
                        DiffLineKind::Removed => (Some(del_bg), "−", s.error),
                        DiffLineKind::Context => (None, " ", s.on_surface_variant),
                    };
                    div()
                        .flex()
                        .w_full()
                        .whitespace_nowrap()
                        .when_some(bg, |d, bg| d.bg(bg))
                        .when(!new_file, |d| d.child(num(line.old)))
                        .child(num(line.new))
                        .child(
                            div()
                                .flex_none()
                                .w(px(17.))
                                .text_center()
                                .text_color(mark_color)
                                .child(mark),
                        )
                        .child(div().flex_none().pr(px(12.)).child(expand_tabs(&line.text)))
                        .into_any_element()
                }
                DiffRow::Fold { id, lines } => {
                    let open = open.clone();
                    div()
                        .id(("fold", id))
                        .w_full()
                        .px(px(12.))
                        .py(px(2.))
                        .bg(s.surface_container)
                        .text_color(s.on_surface_variant)
                        .whitespace_nowrap()
                        .cursor_pointer()
                        .hover(|d| d.bg(s.surface_container_high).text_color(s.on_surface))
                        .on_click(move |_, _, cx| {
                            open.update(cx, |o, cx| {
                                o.insert(id);
                                cx.notify();
                            })
                        })
                        .child(format!("⋯ {} unchanged lines", lines.len()))
                        .into_any_element()
                }
                DiffRow::Skipped { lines } => div()
                    .w_full()
                    .px(px(12.))
                    .py(px(2.))
                    .bg(s.surface_container)
                    .text_color(s.outline)
                    .whitespace_nowrap()
                    .child(format!("⋯ {lines} unchanged lines"))
                    .into_any_element(),
            });
        }

        let header = self.path.as_ref().map(|path| {
            let name = path
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(path.as_ref())
                .to_string();
            let stats = self.diff.stats();
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .px(px(10.))
                .py(px(5.))
                .border_b_1()
                .border_color(s.outline_variant)
                .font_family(FONT_FAMILY)
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .font_weight(FontWeight::MEDIUM)
                        .child(name),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .gap(px(6.))
                        .font_family(MONO_FONT_FAMILY)
                        .child(
                            div()
                                .text_color(s.success)
                                .child(format!("+{}", stats.added)),
                        )
                        .child(
                            div()
                                .text_color(s.error)
                                .child(format!("−{}", stats.removed)),
                        ),
                )
        });

        let more = (!show_all && total > self.max_rows).then(|| {
            let hidden = total - self.max_rows;
            div()
                .id(child_id(&self.id, "more"))
                .w_full()
                .px(px(10.))
                .py(px(5.))
                .border_t_1()
                .border_color(s.outline_variant)
                .font_family(FONT_FAMILY)
                .text_center()
                .text_color(s.primary)
                .cursor_pointer()
                .hover(|d| d.bg(s.surface_container_high))
                .on_click(move |_, _, cx| {
                    all.update(cx, |a, cx| {
                        *a = true;
                        cx.notify();
                    })
                })
                .child(format!("Show all ({hidden} more lines)"))
        });

        div()
            .flex()
            .flex_col()
            .min_w_0()
            .overflow_hidden()
            .font_family(MONO_FONT_FAMILY)
            .text_size(px(12.))
            .line_height(relative(1.5))
            .text_color(s.on_surface)
            .bg(s.surface)
            .when(self.bordered, |d| {
                d.border_1()
                    .border_color(s.outline_variant)
                    .rounded(px(16.))
            })
            .children(header)
            .child(
                div()
                    .id(child_id(&self.id, "body"))
                    .overflow_x_scroll()
                    // A row lets the lines keep their full width, so long ones scroll sideways.
                    .flex()
                    .child(body),
            )
            .children(more)
    }
}

/// Five blocks with the share of added (green) and removed (red) lines, like GitHub's; gray
/// when nothing changed. Round blocks, as in the reference design's M3 Expressive style.
#[derive(IntoElement)]
pub struct DiffBar {
    stats: DiffStats,
}

impl DiffBar {
    pub fn new(added: usize, removed: usize) -> Self {
        Self {
            stats: DiffStats { added, removed },
        }
    }

    pub fn from_stats(stats: DiffStats) -> Self {
        Self { stats }
    }
}

/// How many of the five blocks are "added" (`None` when there are no changes).
pub fn diff_bar_blocks(stats: DiffStats) -> Option<usize> {
    let total = stats.added + stats.removed;
    (total > 0).then(|| ((stats.added as f32 / total as f32) * 5.).round() as usize)
}

impl RenderOnce for DiffBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = Theme::of(cx);
        let added = diff_bar_blocks(self.stats);
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(1.5))
            .ml(px(2.))
            .children((0..5).map(|i| {
                let color = match added {
                    None => s.outline_variant,
                    Some(n) if i < n => s.success,
                    Some(_) => s.error,
                };
                div().size(px(6.)).rounded_full().bg(color)
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(diff: &Diff) -> Vec<(char, Option<usize>, Option<usize>, &str)> {
        diff.lines()
            .iter()
            .map(|l| {
                let c = match l.kind {
                    DiffLineKind::Added => '+',
                    DiffLineKind::Removed => '-',
                    DiffLineKind::Context => ' ',
                };
                (c, l.old, l.new, l.text.as_str())
            })
            .collect()
    }

    #[test]
    fn numbers_old_and_new_lines() {
        let diff = Diff::new("a\nb\nc\n", "a\nB\nc\nd\n");
        assert_eq!(
            summary(&diff),
            vec![
                (' ', Some(1), Some(1), "a"),
                ('-', Some(2), None, "b"),
                ('+', None, Some(2), "B"),
                (' ', Some(3), Some(3), "c"),
                ('+', None, Some(4), "d"),
            ]
        );
        assert_eq!(
            diff.stats(),
            DiffStats {
                added: 2,
                removed: 1
            }
        );
    }

    #[test]
    fn a_missing_final_newline_is_not_a_change() {
        assert!(Diff::new("a\nb", "a\nb\n").is_empty());
        assert_eq!(
            diff_stats("a\nb", "a\nc"),
            DiffStats {
                added: 1,
                removed: 1
            }
        );
    }

    #[test]
    fn stats_match_the_full_diff() {
        let old = "one\ntwo\nthree\nfour\nfive\n";
        let new = "zero\none\nthree\nfour!\nfive\nsix\n";
        assert_eq!(diff_stats(old, new), Diff::new(old, new).stats());
        assert_eq!(
            diff_stats("", "x\ny"),
            DiffStats {
                added: 2,
                removed: 0
            }
        );
    }

    #[test]
    fn new_files_are_all_added() {
        let diff = Diff::new("", "x\ny\n");
        assert!(diff.is_new_file());
        assert_eq!(
            diff.stats(),
            DiffStats {
                added: 2,
                removed: 0
            }
        );
        assert!(!Diff::new("a", "b").is_new_file());
    }

    fn numbered(n: usize) -> String {
        (1..=n).map(|i| format!("line {i}\n")).collect()
    }

    #[test]
    fn folds_long_unchanged_runs() {
        let old = numbered(20);
        let new = old.replace("line 10\n", "line ten\n");
        let diff = Diff::new(&old, &new);
        let rows = diff.rows(3, &BTreeSet::new());
        // Lines 1–6 fold (the start has no head context), 7–9 stay, change, 11–13 stay, 14–20 fold.
        assert_eq!(rows[0], DiffRow::Fold { id: 0, lines: 0..6 });
        assert_eq!(rows[1], DiffRow::Line(6));
        assert!(matches!(rows.last(), Some(DiffRow::Fold { lines, .. }) if lines.len() == 7));
        assert_eq!(rows.len(), 1 + 3 + 2 + 3 + 1);
        // Opening a fold shows its lines.
        let open = BTreeSet::from([0]);
        assert_eq!(diff.rows(3, &open)[0], DiffRow::Line(0));
    }

    #[test]
    fn a_single_hidden_line_is_not_folded() {
        // 7 unchanged lines between two changes: 3 + 1 + 3 → no fold.
        let old = numbered(9);
        let new = old
            .replace("line 1\n", "first\n")
            .replace("line 9\n", "last\n");
        let rows = Diff::new(&old, &new).rows(3, &BTreeSet::new());
        assert!(rows.iter().all(|r| matches!(r, DiffRow::Line(_))));
    }

    #[test]
    fn hunks_group_nearby_changes() {
        let old = numbered(30);
        let new = old
            .replace("line 5\n", "five\n")
            .replace("line 8\n", "eight\n")
            .replace("line 25\n", "twenty-five\n");
        let diff = Diff::new(&old, &new);
        let hunks = diff.hunks(3);
        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].header(), "@@ -2,10 +2,10 @@");
        assert_eq!(hunks[1].header(), "@@ -22,7 +22,7 @@");
        // Pure insertion at the top of an empty file.
        let hunks = Diff::new("", "a\n").hunks(3);
        assert_eq!(hunks[0].header(), "@@ -0,0 +1,1 @@");
    }

    #[test]
    fn parses_unified_patches_with_gaps() {
        let patch = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -3,3 +3,4 @@ fn main\n uno\n-dos\n+DOS\n+tres\n cuatro\n\\ No newline at end of file\n@@ -20,2 +21,2 @@\n veinte\n-x\n+y\n";
        let diff = Diff::from_unified(patch);
        assert_eq!(
            &summary(&diff)[..5],
            &[
                (' ', Some(3), Some(3), "uno"),
                ('-', Some(4), None, "dos"),
                ('+', None, Some(4), "DOS"),
                ('+', None, Some(5), "tres"),
                (' ', Some(5), Some(6), "cuatro"),
            ]
        );
        assert_eq!(
            diff.stats(),
            DiffStats {
                added: 3,
                removed: 2
            }
        );
        let rows = diff.rows(3, &BTreeSet::new());
        assert!(rows.contains(&DiffRow::Skipped { lines: 14 }));
    }

    #[test]
    fn diff_bar_proportions() {
        assert_eq!(
            diff_bar_blocks(DiffStats {
                added: 0,
                removed: 0
            }),
            None
        );
        assert_eq!(
            diff_bar_blocks(DiffStats {
                added: 10,
                removed: 0
            }),
            Some(5)
        );
        assert_eq!(
            diff_bar_blocks(DiffStats {
                added: 1,
                removed: 1
            }),
            Some(3)
        );
        assert_eq!(
            diff_bar_blocks(DiffStats {
                added: 1,
                removed: 9
            }),
            Some(1)
        );
    }
}
