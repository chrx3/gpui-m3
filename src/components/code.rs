//! Monospace blocks: tool output ([`CodeOutput`]) and highlighted code ([`CodeBlock`]).

use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, ClipboardItem, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, StyledText, Window, div, px,
    relative,
};

use super::{all, child_id};
use crate::interaction::{interactive, state_layer, use_interaction};
use crate::motion::{Spring, animate_color, key};
use crate::syntax::{SyntaxPalette, highlight_cached, language};
use crate::theme::{FONT_FAMILY, MONO_FONT_FAMILY, Theme};

/// How long "Copied" stays after copying a code block.
pub const COPIED_FOR: Duration = Duration::from_millis(1400);

/// A monospace block for command output that shows its first `max_lines` lines and a
/// "Show all (N more lines)" row. Wraps long lines and scrolls past 420 px; `error` paints the
/// text in the error color and `prefix` adds a prompt such as `$` in the primary color.
#[derive(IntoElement)]
pub struct CodeOutput {
    id: ElementId,
    text: SharedString,
    max_lines: usize,
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
            prefix: None,
            error: false,
            max_height: px(420.),
        }
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

impl RenderOnce for CodeOutput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let all_state = window.use_keyed_state(key(&self.id, "all"), cx, |_, _| false);
        let show_all = *all_state.read(cx);
        let (clipped, hidden) = clip_lines(&self.text, self.max_lines);
        let shown: SharedString = if show_all {
            self.text
                .strip_suffix('\n')
                .unwrap_or(&self.text)
                .to_string()
                .into()
        } else if hidden == 0 && clipped.len() == self.text.len() {
            self.text.clone()
        } else {
            clipped.to_string().into()
        };

        div()
            .flex()
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
            .when(hidden > 0 && !show_all, |d| {
                d.child(
                    div()
                        .id(child_id(&self.id, "more"))
                        .px(px(10.))
                        .py(px(4.))
                        .border_t_1()
                        .border_color(s.outline_variant)
                        .font_family(FONT_FAMILY)
                        .text_size(px(11.5))
                        .text_color(s.primary)
                        .cursor_pointer()
                        .hover(|d| d.bg(s.surface_container_high))
                        .on_click(move |_, _, cx| {
                            all_state.update(cx, |a, cx| {
                                *a = true;
                                cx.notify();
                            })
                        })
                        .child(format!("Show all ({hidden} more lines)")),
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
}

impl CodeBlock {
    pub fn new(id: impl Into<ElementId>, code: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            code: code.into(),
            language: SharedString::default(),
            max_height: px(480.),
        }
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
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let info = language(&self.language);
        let code = display_code(&self.code);
        let tokens = info
            .highlighted
            .then(|| highlight_cached(&code, &self.language))
            .flatten();
        let palette = SyntaxPalette::of(cx);
        let mut text = StyledText::new(code);
        if let Some(tokens) = tokens {
            text = text.with_highlights(palette.highlights(&tokens));
        }

        div()
            .flex()
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

/// The code block's "Copy" pill (11 px, muted, `secondary` on hover; "Copied" in the success
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
            .font_family(FONT_FAMILY)
            .text_size(px(11.))
            .text_color(fg)
            .child(layer)
            .child(if copied { "Copied" } else { "Copy" });
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
    fn display_code_normalizes_line_ends_and_tabs() {
        assert_eq!(display_code("a\r\n\tb\r\n"), "a\n  b");
    }
}
