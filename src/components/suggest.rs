//! Suggestion lists for completions such as @-mentions: matching, keyboard navigation and an
//! anchored, highlighted list.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Corner, ElementId, InteractiveElement, IntoElement, KeyBinding, ParentElement, Pixels,
    RenderOnce, SharedString, Styled, Window, actions, anchored, deferred, div, px,
};

use super::{ChangeHandler, Menu, MenuItem, PopDirection};
use crate::motion::{Spring, entrance, key};

actions!(
    m3_suggestions,
    [
        /// Moves the highlight up (wraps around).
        SuggestionPrevious,
        /// Moves the highlight down (wraps around).
        SuggestionNext,
        /// Picks the highlighted suggestion (Enter or Tab).
        SuggestionAccept,
        /// Closes the list (Escape).
        SuggestionDismiss
    ]
);

/// Key context flag that a [`super::TextArea`] or [`super::TextField`] adds while a suggestion
/// list is open for it (see `set_suggesting`). While it is set, ↑ ↓ Enter Tab Esc dispatch the
/// suggestion actions instead of moving the cursor, submitting or cancelling.
pub const SUGGESTING_CONTEXT: &str = "m3_suggesting";

/// Registers ↑ ↓ Enter Tab Esc for open suggestion lists. Called by [`crate::init`]; it must run
/// after the text input bindings so these take precedence.
pub fn bind_suggestion_keys(cx: &mut App) {
    cx.bind_keys(suggestion_bindings());
}

pub(crate) fn suggestion_bindings() -> Vec<KeyBinding> {
    let mut bindings = Vec::new();
    for input in ["M3TextArea", "M3TextField"] {
        let ctx = format!("{input} && {SUGGESTING_CONTEXT}");
        let ctx = Some(ctx.as_str());
        bindings.extend([
            KeyBinding::new("up", SuggestionPrevious, ctx),
            KeyBinding::new("down", SuggestionNext, ctx),
            KeyBinding::new("enter", SuggestionAccept, ctx),
            KeyBinding::new("tab", SuggestionAccept, ctx),
            KeyBinding::new("escape", SuggestionDismiss, ctx),
        ]);
    }
    bindings
}

/// Keyboard highlight of a suggestion list: wraps around at both ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SuggestionNav {
    selected: usize,
    len: usize,
}

impl SuggestionNav {
    pub fn new(len: usize) -> Self {
        Self { selected: 0, len }
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The highlighted index, if the list has rows.
    pub fn current(&self) -> Option<usize> {
        (self.selected < self.len).then_some(self.selected)
    }

    /// New results: highlights the first row (the query changed, the old row means nothing).
    pub fn reset(&mut self, len: usize) {
        self.len = len;
        self.selected = 0;
    }

    /// Same results with a different length: keeps the row, clamped to the new end.
    pub fn set_len(&mut self, len: usize) {
        self.len = len;
        self.selected = self.selected.min(len.saturating_sub(1));
    }

    pub fn next(&mut self) {
        if self.len > 0 {
            self.selected = (self.selected + 1) % self.len;
        }
    }

    pub fn previous(&mut self) {
        if self.len > 0 {
            self.selected = (self.selected + self.len - 1) % self.len;
        }
    }

    /// The pointer moved over `row`. Returns whether the highlight changed.
    pub fn hover(&mut self, row: usize) -> bool {
        let changed = row < self.len && row != self.selected;
        if changed {
            self.selected = row;
        }
        changed
    }
}

fn chars_eq(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

/// Byte ranges of `text` to highlight for `query`, ignoring case: the first contiguous match if
/// there is one, otherwise the characters of a subsequence match ("cmpsr" in "Composer.rs").
/// Empty when the query is empty or does not match.
#[allow(clippy::single_range_in_vec_init)] // One range for a contiguous match is intended.
pub fn match_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    let needle: Vec<char> = query.chars().collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let hay: Vec<(usize, char)> = text.char_indices().collect();
    // Contiguous match.
    if hay.len() >= needle.len() {
        for start in 0..=hay.len() - needle.len() {
            if (0..needle.len()).all(|i| chars_eq(hay[start + i].1, needle[i])) {
                let end_char = start + needle.len();
                let end = hay.get(end_char).map_or(text.len(), |&(i, _)| i);
                return vec![hay[start].0..end];
            }
        }
    }
    // Subsequence, merging adjacent characters into one range.
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut n = 0;
    for &(i, ch) in &hay {
        if n < needle.len() && chars_eq(ch, needle[n]) {
            let end = i + ch.len_utf8();
            match ranges.last_mut() {
                Some(last) if last.end == i => last.end = end,
                _ => ranges.push(i..end),
            }
            n += 1;
        }
    }
    if n == needle.len() {
        ranges
    } else {
        Vec::new()
    }
}

/// Ranks a path for a mention query: lower is better, `None` if it does not match.
/// The file name starting with the query beats the file name containing it, which beats the
/// path containing it, which beats a subsequence; ties go to the shorter path.
pub fn mention_score(path: &str, query: &str) -> Option<usize> {
    if query.is_empty() {
        // No query yet: keep the given order (the first files come first).
        return Some(0);
    }
    let rel = path.to_lowercase();
    let needle = query.to_lowercase();
    let base = rel.rsplit(['/', '\\']).next().unwrap_or(&rel);
    let tier = if base.starts_with(&needle) {
        0
    } else if base.contains(&needle) {
        1
    } else if rel.contains(&needle) {
        2
    } else {
        let mut wanted = needle.chars().peekable();
        for ch in rel.chars() {
            if wanted.peek() == Some(&ch) {
                wanted.next();
            }
        }
        if wanted.peek().is_some() {
            return None;
        }
        3
    };
    Some(tier * 1000 + rel.chars().count())
}

/// The best `limit` of `items` for `query`, using [`mention_score`] on the text `text_of` gives.
pub fn rank_mentions<T>(
    items: impl IntoIterator<Item = T>,
    query: &str,
    limit: usize,
    text_of: impl Fn(&T) -> &str,
) -> Vec<T> {
    let mut scored: Vec<(usize, usize, T)> = items
        .into_iter()
        .enumerate()
        .filter_map(|(i, item)| mention_score(text_of(&item), query).map(|s| (s, i, item)))
        .collect();
    scored.sort_by_key(|(score, i, _)| (*score, *i));
    scored.truncate(limit);
    scored.into_iter().map(|(_, _, item)| item).collect()
}

/// One row of a [`SuggestionList`].
#[derive(Clone, Debug)]
pub struct Suggestion {
    pub icon: SharedString,
    pub label: SharedString,
    /// Secondary text on the same line, truncated (a file's folder).
    pub hint: Option<SharedString>,
}

impl Suggestion {
    pub fn new(icon: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            icon: icon.into(),
            label: label.into(),
            hint: None,
        }
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

/// Anchored completion list (an @-mention popover): a menu of suggestions with the matching
/// part of each label highlighted. Rows rise in with a short stagger.
///
/// The list does not take focus; the text input keeps it. Wire the keys like this:
///
/// 1. While the list is open, call `set_suggesting(true, cx)` on the `TextArea`/`TextField`, so
///    ↑ ↓ Enter Tab Esc dispatch [`SuggestionPrevious`], [`SuggestionNext`],
///    [`SuggestionAccept`] and [`SuggestionDismiss`].
/// 2. Handle those actions with `on_action` on an ancestor of the input, updating a
///    [`SuggestionNav`].
/// 3. Render the list with the nav's `selected()`, `on_hover` (the pointer moves the
///    highlight) and `on_pick` (a click).
#[derive(IntoElement)]
pub struct SuggestionList {
    id: ElementId,
    items: Vec<Suggestion>,
    query: SharedString,
    selected: usize,
    width: f32,
    max_height: Pixels,
    anchor: Option<Corner>,
    direction: PopDirection,
    on_hover: Option<ChangeHandler<usize>>,
    on_pick: Option<ChangeHandler<usize>>,
}

impl SuggestionList {
    pub fn new(id: impl Into<ElementId>, items: Vec<Suggestion>) -> Self {
        Self {
            id: id.into(),
            items,
            query: SharedString::default(),
            selected: 0,
            width: 460.,
            max_height: px(320.),
            anchor: None,
            direction: PopDirection::Up,
            on_hover: None,
            on_pick: None,
        }
    }

    /// The text being completed; its match is highlighted in each label (see [`match_ranges`]).
    pub fn query(mut self, query: impl Into<SharedString>) -> Self {
        self.query = query.into();
        self
    }

    /// Highlighted row (from [`SuggestionNav::selected`]).
    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self
    }

    /// Width in pixels (460 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn max_h(mut self, max_height: Pixels) -> Self {
        self.max_height = max_height;
        self
    }

    /// Floats over the window from where it is placed: `corner` is the list corner that sits on
    /// that point (`BottomLeft` opens upward, above a composer). Without it the list is laid out
    /// in place.
    pub fn anchor(mut self, corner: Corner) -> Self {
        self.anchor = Some(corner);
        self.direction = match corner {
            Corner::TopLeft | Corner::TopRight => PopDirection::Down,
            Corner::BottomLeft | Corner::BottomRight => PopDirection::Up,
        };
        self
    }

    /// Entrance direction when laid out in place (up by default).
    pub fn direction(mut self, direction: PopDirection) -> Self {
        self.direction = direction;
        self
    }

    /// The pointer moved over a row (make it the highlight).
    pub fn on_hover(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(f));
        self
    }

    /// A row was clicked.
    pub fn on_pick(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for SuggestionList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut menu = Menu::new(self.id.clone())
            .width(self.width)
            .padding(px(4.))
            .radius(px(20.))
            .direction(self.direction)
            .max_h(self.max_height);
        for (row, item) in self.items.iter().enumerate() {
            let mut entry = MenuItem::new(("m3-suggestion", row), item.label.clone())
                .icon(item.icon.clone())
                .dense(true)
                .inline_sublabel(true)
                .highlighted(row == self.selected)
                .hover_fill(false)
                .label_highlights(match_ranges(&item.label, &self.query));
            if let Some(hint) = &item.hint {
                entry = entry.sublabel(hint.clone());
            }
            if let Some(f) = self.on_pick.clone() {
                entry = entry.on_click(move |_, w, cx| f(row, w, cx));
            }
            // Each row drops 6 px into place, 25 ms after the previous one.
            let delay = row.min(4) as f32 * 0.025;
            let p = entrance(
                ElementId::NamedChild(Box::new(key(&self.id, "row-in")), row.to_string().into()),
                Spring::SPATIAL,
                delay,
                window,
                cx,
            );
            let mut wrapper = div()
                .id(("m3-suggestion-row", row))
                .relative()
                .top(px((1. - p) * -6.))
                .opacity(p.clamp(0., 1.));
            if let Some(f) = self.on_hover.clone() {
                wrapper = wrapper.on_mouse_move(move |_, w, cx| f(row, w, cx));
            }
            menu = menu.item(wrapper.child(entry));
        }
        let menu = div()
            .id(key(&self.id, "list"))
            .occlude()
            .child(menu)
            .into_any_element();
        match self.anchor {
            Some(corner) => deferred(
                anchored()
                    .anchor(corner)
                    .snap_to_window_with_margin(px(8.))
                    .child(menu),
            )
            .with_priority(1)
            .into_any_element(),
            None => menu,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the flag on, the suggestion bindings must beat the input's own (registered first, as
    /// `init` does); without it, the input keeps its keys.
    #[test]
    fn suggestion_keys_take_precedence_only_while_suggesting() {
        use gpui::{KeyContext, Keymap, Keystroke};
        let mut bindings = super::super::text_area::text_area_bindings();
        bindings.extend(super::super::text_field::text_field_bindings());
        bindings.extend(suggestion_bindings());
        let keymap = Keymap::new(bindings);
        let first = |keys: &str, context: &str| {
            let stack = [KeyContext::parse(context).unwrap()];
            let (found, _) = keymap.bindings_for_input(&[Keystroke::parse(keys).unwrap()], &stack);
            found.first().map(|b| b.action().name())
        };
        for input in ["M3TextArea", "M3TextField"] {
            let on = format!("{input} {SUGGESTING_CONTEXT}");
            assert_eq!(first("up", &on), Some("m3_suggestions::SuggestionPrevious"));
            assert_eq!(first("down", &on), Some("m3_suggestions::SuggestionNext"));
            assert_eq!(
                first("enter", &on),
                Some("m3_suggestions::SuggestionAccept")
            );
            assert_eq!(first("tab", &on), Some("m3_suggestions::SuggestionAccept"));
            assert_eq!(
                first("escape", &on),
                Some("m3_suggestions::SuggestionDismiss")
            );
            assert_ne!(
                first("enter", input),
                Some("m3_suggestions::SuggestionAccept")
            );
            assert_ne!(
                first("escape", input),
                Some("m3_suggestions::SuggestionDismiss")
            );
        }
        assert_eq!(first("down", "M3TextArea"), Some("m3_text_area::Down"));
    }

    #[test]
    fn nav_wraps_both_ways() {
        let mut nav = SuggestionNav::new(3);
        nav.previous();
        assert_eq!(nav.selected(), 2);
        nav.next();
        assert_eq!(nav.selected(), 0);
        nav.next();
        nav.next();
        nav.next();
        assert_eq!(nav.selected(), 0);
    }

    #[test]
    fn nav_handles_empty_and_shrinking_lists() {
        let mut nav = SuggestionNav::new(0);
        nav.next();
        nav.previous();
        assert_eq!(nav.current(), None);
        nav.reset(5);
        nav.previous();
        assert_eq!(nav.current(), Some(4));
        nav.set_len(2);
        assert_eq!(nav.current(), Some(1));
        nav.reset(4);
        assert_eq!(nav.current(), Some(0));
    }

    #[test]
    fn hover_moves_the_highlight_once() {
        let mut nav = SuggestionNav::new(4);
        assert!(nav.hover(2));
        assert!(!nav.hover(2));
        assert!(!nav.hover(9));
        assert_eq!(nav.selected(), 2);
    }

    #[test]
    fn contiguous_matches_ignore_case() {
        assert_eq!(match_ranges("Composer.tsx", "pos"), vec![3..6]);
        assert_eq!(match_ranges("Composer.tsx", "COMP"), vec![0..4]);
        assert!(match_ranges("Composer.tsx", "").is_empty());
        assert!(match_ranges("abc", "abcd").is_empty());
    }

    #[test]
    fn subsequence_matches_merge_adjacent_chars() {
        assert_eq!(
            match_ranges("Composer.tsx", "cmpsr"),
            vec![0..1, 2..4, 5..6, 7..8]
        );
        assert!(match_ranges("Composer.tsx", "zz").is_empty());
    }

    #[test]
    fn ranges_respect_multibyte_text() {
        let text = "Configuración.md";
        let ranges = match_ranges(text, "ción");
        assert_eq!(ranges.len(), 1);
        assert_eq!(&text[ranges[0].clone()], "ción");
        let ranges = match_ranges("ÁRBOL", "árb");
        assert_eq!(&"ÁRBOL"[ranges[0].clone()], "ÁRB");
    }

    #[test]
    fn mention_ranking_puts_name_matches_first() {
        let files = [
            "src/components/Agent.tsx",
            "src/styles/agent-notes.css",
            "src/agent/readme.md",
            "src/components/Composer.tsx",
        ];
        let ranked = rank_mentions(files, "agent", 8, |f| f);
        assert_eq!(
            ranked,
            vec![
                "src/components/Agent.tsx",
                "src/styles/agent-notes.css",
                "src/agent/readme.md",
            ]
        );
        assert_eq!(
            rank_mentions(files, "cmpsr", 8, |f| f),
            vec!["src/components/Composer.tsx"]
        );
        assert_eq!(rank_mentions(files, "", 2, |f| f).len(), 2);
        assert_eq!(mention_score("a/b.rs", "zz"), None);
    }
}
