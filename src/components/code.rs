//! Monospace blocks: tool output ([`CodeOutput`]) and highlighted code ([`CodeBlock`]).

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, ClipboardItem, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, StyledText, Window, div, px,
    relative,
};

use super::{Selection, all, child_id, group_for, selectable_block, selection_root};
use crate::interaction::{interactive, state_layer, use_interaction};
use crate::motion::{Spring, animate_color, key};
use crate::syntax::{SyntaxPalette, highlight_cached, language};
use crate::theme::{MONO_FONT_FAMILY, Theme, alpha, font_family};

/// How long "Copied" stays after copying a code block.
pub const COPIED_FOR: Duration = Duration::from_millis(1400);

/// What a "Show all" row reveals, for [`CodeOutput::show_all_label`] and
/// [`super::DiffView::show_all_label`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HiddenContent {
    /// This many lines are not shown.
    Lines(usize),
    /// The text was cut by length: this many characters are not shown.
    Chars(usize),
}

/// Builds the text of a "Show all" row (in your language) from what it reveals.
pub type ShowAllLabel = Rc<dyn Fn(HiddenContent) -> SharedString>;

/// The default English text of a "Show all" row.
pub(crate) fn show_all_text(hidden: HiddenContent, label: Option<&ShowAllLabel>) -> SharedString {
    if let Some(label) = label {
        return label(hidden);
    }
    match hidden {
        HiddenContent::Lines(n) => format!("Show all ({n} more lines)").into(),
        HiddenContent::Chars(n) => format!("Show all ({n} more characters)").into(),
    }
}

/// A monospace block for command output that shows its first `max_lines` lines and a
/// "Show all (N more lines)" row. Wraps long lines and scrolls past 420 px; `error` paints the
/// text in the error color and `prefix` adds a prompt such as `$` in the primary color.
///
/// [`CodeOutput::max_chars`] also cuts long text that has few line breaks (a minified JSON),
/// and [`CodeOutput::json`] pretty prints JSON before it is cut.
#[derive(IntoElement)]
pub struct CodeOutput {
    id: ElementId,
    text: SharedString,
    max_lines: usize,
    max_chars: Option<usize>,
    json: bool,
    show_all_label: Option<ShowAllLabel>,
    prefix: Option<SharedString>,
    error: bool,
    max_height: Pixels,
}

impl CodeOutput {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            max_lines: 12,
            max_chars: None,
            json: false,
            show_all_label: None,
            prefix: None,
            error: false,
            max_height: px(420.),
        }
    }

    /// Characters shown before "Show all", after [`CodeOutput::max_lines`] (no limit by
    /// default). For output with very long lines, which `max_lines` alone does not shorten.
    pub fn max_chars(mut self, chars: usize) -> Self {
        self.max_chars = Some(chars.max(1));
        self
    }

    /// Pretty prints the text as JSON (two-space indent) before cutting it, so `max_lines`
    /// folds a one-line JSON too. Text that does not look like a JSON object or array is shown
    /// as it is. "Copy all" still copies the original text.
    pub fn json(mut self, json: bool) -> Self {
        self.json = json;
        self
    }

    /// The text of the "Show all" row, from what it reveals (English by default):
    ///
    /// ```ignore
    /// CodeOutput::new("out", text).show_all_label(|hidden| match hidden {
    ///     HiddenContent::Lines(n) => format!("Ver todo ({n} líneas más)").into(),
    ///     _ => "Ver todo".into(),
    /// })
    /// ```
    pub fn show_all_label(mut self, label: impl Fn(HiddenContent) -> SharedString + 'static) -> Self {
        self.show_all_label = Some(Rc::new(label));
        self
    }

    /// Lines shown before "Show all" (12 by default).
    pub fn max_lines(mut self, lines: usize) -> Self {
        self.max_lines = lines.max(1);
        self
    }

    /// A prompt before the text (`$`, `>`), in the primary color.
    pub fn prefix(mut self, prefix: impl Into<SharedString>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Error output: the text in the error color.
    pub fn error(mut self, error: bool) -> Self {
        self.error = error;
        self
    }

    /// Height after which the block scrolls (420 px by default).
    pub fn max_height(mut self, height: Pixels) -> Self {
        self.max_height = height;
        self
    }
}

/// The first `max` lines of `text` and how many were left out (a final line break is not a line).
pub fn clip_lines(text: &str, max: usize) -> (&str, usize) {
    let text = text.strip_suffix('\n').unwrap_or(text);
    let total = text.split('\n').count();
    if total <= max {
        return (text, 0);
    }
    let end = text
        .match_indices('\n')
        .nth(max - 1)
        .map_or(text.len(), |(i, _)| i);
    (&text[..end], total - max)
}

/// The first `max` characters of `text` (cut on a character boundary) and how many were left
/// out.
pub fn clip_chars(text: &str, max: usize) -> (&str, usize) {
    match text.char_indices().nth(max) {
        Some((end, _)) => (&text[..end], text[end..].chars().count()),
        None => (text, 0),
    }
}

/// `text` pretty printed as JSON with a two-space indent, or `None` when it is not a single
/// JSON object or array (unbalanced brackets, an open string, text after the end). Strings are
/// kept as they are; nothing else is validated.
pub fn pretty_json(text: &str) -> Option<String> {
    let text = text.trim();
    if !text.starts_with(['{', '[']) {
        return None;
    }
    fn newline(out: &mut String, depth: usize) {
        out.push('\n');
        for _ in 0..depth {
            out.push_str("  ");
        }
    }
    let mut out = String::with_capacity(text.len() * 2);
    let mut stack: Vec<char> = Vec::new();
    let (mut in_string, mut escaped, mut ended) = (false, false, false);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c.is_whitespace() {
            continue;
        }
        if ended {
            return None;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '{' | '[' => {
                let close = if c == '{' { '}' } else { ']' };
                while chars.next_if(|c| c.is_whitespace()).is_some() {}
                out.push(c);
                if chars.next_if_eq(&close).is_some() {
                    out.push(close);
                    ended = stack.is_empty();
                } else {
                    stack.push(close);
                    newline(&mut out, stack.len());
                }
            }
            '}' | ']' => {
                if stack.pop() != Some(c) {
                    return None;
                }
                newline(&mut out, stack.len());
                out.push(c);
                ended = stack.is_empty();
            }
            ',' => {
                out.push(',');
                newline(&mut out, stack.len());
            }
            ':' => out.push_str(": "),
            c => out.push(c),
        }
    }
    (stack.is_empty() && !in_string && ended).then_some(out)
}

impl RenderOnce for CodeOutput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let all_state = window.use_keyed_state(key(&self.id, "all"), cx, |_, _| false);
        let show_all = *all_state.read(cx);
        let source: SharedString = match self.json.then(|| pretty_json(&self.text)).flatten() {
            Some(pretty) => pretty.into(),
            None => self.text.clone(),
        };
        let (clipped, hidden_lines) = clip_lines(&source, self.max_lines);
        let (clipped, hidden_chars) = match self.max_chars {
            Some(max) => clip_chars(clipped, max),
            None => (clipped, 0),
        };
        let hidden = if hidden_chars > 0 {
            let whole = source.strip_suffix('\n').unwrap_or(&source);
            Some(HiddenContent::Chars(
                whole.chars().count() - clipped.chars().count(),
            ))
        } else {
            (hidden_lines > 0).then_some(HiddenContent::Lines(hidden_lines))
        };
        let shown: SharedString = if show_all {
            source
                .strip_suffix('\n')
                .unwrap_or(&source)
                .to_string()
                .into()
        } else if hidden.is_none() && clipped.len() == source.len() {
            source.clone()
        } else {
            clipped.to_string().into()
        };
        // Selectable; "Copy all" takes the whole output, also the lines not shown.
        let (group, own) = group_for(child_id(&self.id, "select"), window, cx);
        let text = StyledText::new(shown.clone());
        let layout = text.layout().clone();
        let shown = selectable_block(
            &group,
            layout,
            shown,
            SharedString::default(),
            Some(self.text.clone()),
            text.into_any_element(),
            alpha(s.primary, 0.3),
        );
        let root = if own {
            selection_root(&group.state, div(), cx)
        } else {
            div()
        };

        root.flex()
            .flex_col()
            .min_w_0()
            .rounded(px(12.))
            .bg(s.surface)
            .overflow_hidden()
            .child(
                div()
                    .id(child_id(&self.id, "scroll"))
                    .max_h(self.max_height)
                    .overflow_y_scroll()
                    .px(px(10.))
                    .py(px(7.))
                    .flex()
                    .gap(px(6.))
                    .font_family(MONO_FONT_FAMILY)
                    .text_size(px(11.5))
                    .line_height(px(17.))
                    .text_color(if self.error { s.error } else { s.on_surface })
                    .when_some(self.prefix, |d, prefix| {
                        d.child(div().flex_none().text_color(s.primary).child(prefix))
                    })
                    .child(div().flex_1().min_w_0().child(shown)),
            )
            .when_some(hidden.filter(|_| !show_all), |d, hidden| {
                d.child(
                    div()
                        .id(child_id(&self.id, "more"))
                        .px(px(10.))
                        .py(px(4.))
                        .border_t_1()
                        .border_color(s.outline_variant)
                        .font_family(font_family())
                        .text_size(px(11.5))
                        .text_color(s.primary)
                        .cursor_pointer()
                        // The hover fill sits in the rounded bottom corners (GPUI clips to a rectangle).
                        .rounded_bl(px(12.))
                        .rounded_br(px(12.))
                        .hover(|d| d.bg(s.surface_container_high))
                        .on_click(move |_, _, cx| {
                            all_state.update(cx, |a, cx| {
                                *a = true;
                                cx.notify();
                            })
                        })
                        .child(show_all_text(hidden, self.show_all_label.as_ref())),
                )
            })
    }
}

/// A fenced code block: a header with the language's readable name and a Copy pill that turns
/// into "Copied" for 1.4 s, then the code highlighted with the theme's [`SyntaxPalette`],
/// without wrapping and scrolling in both directions past `max_height`.
///
/// Highlighting is cached by content, and a block that keeps growing (streaming) continues from
/// its last complete line, so re-rendering on every token is cheap.
#[derive(IntoElement)]
pub struct CodeBlock {
    id: ElementId,
    code: SharedString,
    language: SharedString,
    max_height: Pixels,
    /// The selection it shares (a Markdown's), what goes before it in a copy and what
    /// "Copy all" copies over it.
    selection: Option<(Selection, SharedString, Option<SharedString>)>,
}

impl CodeBlock {
    pub fn new(id: impl Into<ElementId>, code: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            code: code.into(),
            language: SharedString::default(),
            max_height: px(480.),
            selection: None,
        }
    }

    /// Selects as part of the Markdown it is in, with `joint` before its text in a copy and
    /// `whole` for "Copy all"; otherwise it joins the scope it is in, or is a group of its own.
    pub fn selection(
        mut self,
        group: Selection,
        joint: SharedString,
        whole: Option<SharedString>,
    ) -> Self {
        self.selection = Some((group, joint, whole));
        self
    }

    /// The fence's info string (`"ts"`, `"rust title=main.rs"`…); empty for plain text.
    pub fn language(mut self, info: impl Into<SharedString>) -> Self {
        self.language = info.into();
        self
    }

    /// Height after which the code scrolls (480 px by default).
    pub fn max_height(mut self, height: Pixels) -> Self {
        self.max_height = height;
        self
    }
}

/// Code as shown: no carriage returns, no final line break, tabs as two spaces.
fn display_code(code: &str) -> String {
    let code = code.strip_suffix('\n').unwrap_or(code);
    let code = code.strip_suffix('\r').unwrap_or(code);
    code.replace("\r\n", "\n").replace('\t', "  ")
}

impl RenderOnce for CodeBlock {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let (group, joint, whole, own) = match self.selection {
            Some((group, joint, whole)) => (group, joint, whole, false),
            None => {
                let (group, own) = group_for(child_id(&self.id, "select"), window, cx);
                (group, SharedString::default(), Some(self.code.clone()), own)
            }
        };
        let info = language(&self.language);
        let code = display_code(&self.code);
        let tokens = info
            .highlighted
            .then(|| highlight_cached(&code, &self.language))
            .flatten();
        let palette = SyntaxPalette::of(cx);
        let source: SharedString = code.clone().into();
        let mut text = StyledText::new(code);
        if let Some(tokens) = tokens {
            text = text.with_highlights(palette.highlights(&tokens));
        }
        let layout = text.layout().clone();
        let text = selectable_block(
            &group,
            layout,
            source,
            joint,
            whole,
            text.into_any_element(),
            alpha(s.primary, 0.3),
        );
        let root = if own {
            selection_root(&group.state, div(), cx)
        } else {
            div()
        };

        root.flex()
            .flex_col()
            .min_w_0()
            .bg(s.surface)
            .border_1()
            .border_color(s.outline_variant)
            .rounded(px(16.))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .pl(px(12.))
                    .pr(px(6.))
                    .py(px(4.))
                    .border_b_1()
                    .border_color(s.outline_variant)
                    .text_size(px(11.))
                    .text_color(s.outline)
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .font_family(MONO_FONT_FAMILY)
                            .child(info.label),
                    )
                    .child(CopyPill::new(child_id(&self.id, "copy"), self.code.clone())),
            )
            .child(
                div()
                    .id(child_id(&self.id, "code"))
                    .max_h(self.max_height)
                    .overflow_scroll()
                    // A row lets the text keep its full width, so long lines scroll sideways.
                    .flex()
                    .px(px(12.))
                    .py(px(10.))
                    .font_family(MONO_FONT_FAMILY)
                    .text_size(px(12.5))
                    .line_height(relative(1.55))
                    .text_color(s.on_surface)
                    .child(div().flex_none().whitespace_nowrap().child(text)),
            )
    }
}

/// The code block's "Copiar" pill (11 px, muted, `secondary` on hover; "Copiado" in the success
/// color afterwards).
#[derive(IntoElement)]
struct CopyPill {
    id: ElementId,
    text: SharedString,
}

impl CopyPill {
    fn new(id: ElementId, text: SharedString) -> Self {
        Self { id, text }
    }
}

impl RenderOnce for CopyPill {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let state = use_interaction(&self.id, window, cx);
        let copied_at = window.use_keyed_state(key(&self.id, "at"), cx, |_, _| None::<Instant>);
        let copied = copied_at.read(cx).is_some_and(|t| t.elapsed() < COPIED_FOR);
        let hovered = state.read(cx).hovered;
        let fg = if copied {
            s.success
        } else if hovered {
            s.on_surface
        } else {
            s.on_surface_variant
        };
        let fg = animate_color(key(&self.id, "fg"), Spring::EFFECTS, fg, window, cx);
        let layer = state_layer(&self.id, &state, all(px(10.)), s.on_surface, window, cx);
        let el = div()
            .id(self.id.clone())
            .relative()
            .px(px(10.))
            .py(px(2.))
            .rounded(px(10.))
            .font_family(font_family())
            .text_size(px(11.))
            .text_color(fg)
            .child(layer)
            .child(if copied { "Copiado" } else { "Copiar" });
        let text = self.text;
        interactive(el, &state, true, true).on_click(move |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
            copied_at.update(cx, |at, cx| {
                *at = Some(Instant::now());
                cx.notify();
            });
            let at = copied_at.clone();
            let timer = cx.background_executor().timer(COPIED_FOR);
            cx.spawn(async move |cx| {
                timer.await;
                at.update(cx, |_, cx| cx.notify()).ok();
            })
            .detach();
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_to_whole_lines() {
        assert_eq!(clip_lines("a\nb\nc\n", 5), ("a\nb\nc", 0));
        assert_eq!(clip_lines("a\nb\nc\nd", 2), ("a\nb", 2));
        assert_eq!(clip_lines("", 1), ("", 0));
    }

    #[test]
    fn clips_to_whole_characters() {
        assert_eq!(clip_chars("abcdef", 4), ("abcd", 2));
        assert_eq!(clip_chars("abc", 4), ("abc", 0));
        assert_eq!(clip_chars("añob", 2), ("añ", 2));
    }

    #[test]
    fn pretty_prints_json() {
        assert_eq!(
            pretty_json(r#"{"a":1,"b":[true,null],"c":{}}"#).as_deref(),
            Some("{\n  \"a\": 1,\n  \"b\": [\n    true,\n    null\n  ],\n  \"c\": {}\n}")
        );
        assert_eq!(pretty_json(" [ ] ").as_deref(), Some("[]"));
        // Brackets, commas and colons inside strings are left alone, escaped quotes too.
        assert_eq!(
            pretty_json(r#"{"k":"a,{b}:\"c\""}"#).as_deref(),
            Some("{\n  \"k\": \"a,{b}:\\\"c\\\"\"\n}")
        );
    }

    #[test]
    fn leaves_what_is_not_json() {
        assert_eq!(pretty_json("hello"), None);
        assert_eq!(pretty_json("{\"a\": 1"), None);
        assert_eq!(pretty_json("[1}"), None);
        assert_eq!(pretty_json("{} {}"), None);
        assert_eq!(pretty_json("{\"a\": \"open}"), None);
    }

    #[test]
    fn show_all_text_defaults_to_english() {
        assert_eq!(show_all_text(HiddenContent::Lines(3), None), "Show all (3 more lines)");
        assert_eq!(
            show_all_text(HiddenContent::Chars(9), None),
            "Show all (9 more characters)"
        );
        let label: ShowAllLabel = Rc::new(|hidden| match hidden {
            HiddenContent::Lines(n) => format!("Ver todo ({n} líneas más)").into(),
            _ => "Ver todo".into(),
        });
        assert_eq!(
            show_all_text(HiddenContent::Lines(2), Some(&label)),
            "Ver todo (2 líneas más)"
        );
    }

    #[test]
    fn display_code_normalizes_line_ends_and_tabs() {
        assert_eq!(display_code("a\r\n\tb\r\n"), "a\n  b");
    }
}
