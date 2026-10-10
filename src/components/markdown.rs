//! GitHub-flavored Markdown rendered with gpui-m3 components, parsed with
//! [pulldown-cmark](https://crates.io/crates/pulldown-cmark).
//!
//! [`parse_markdown`] turns text into [`MarkdownBlock`]s (pure, tested); [`Markdown`] renders
//! them and caches the parse by text, so a message that streams in token by token is parsed once
//! per change and not on every frame.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, ElementId, Font, FontFeatures, FontStyle, FontWeight, Hsla,
    InteractiveElement, InteractiveText, IntoElement, ParentElement, Pixels, RenderOnce,
    SharedString, StatefulInteractiveElement, StrikethroughStyle, Styled, StyledText, TextRun,
    Window, div, px, relative,
};
use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use super::{CodeBlock, Icon};
use crate::cache::{Lru, content_key};
use crate::theme::{FONT_FAMILY, MONO_FONT_FAMILY, Scheme, Theme, text};

/* ─────────── Model ─────────── */

/// Inline formatting of a run of text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    /// `inline code`
    pub code: bool,
    /// Part of a link (see [`RichText::links`]).
    pub link: bool,
}

/// Text with inline formatting: `spans` cover the whole text in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RichText {
    pub text: String,
    pub spans: Vec<(Range<usize>, InlineStyle)>,
    /// Link ranges and their destinations (bare `https://` URLs included).
    pub links: Vec<(Range<usize>, String)>,
}

impl RichText {
    /// Byte ranges of the `inline code` runs.
    pub fn code_ranges(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        self.spans
            .iter()
            .filter(|(_, s)| s.code)
            .map(|(r, _)| r.clone())
    }

    /// The text without formatting, for tests and plain-text fallbacks.
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

/// An image reference (`![alt](url "title")`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkdownImage {
    pub url: String,
    pub alt: String,
    pub title: String,
}

/// Horizontal alignment of a table column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColumnAlign {
    #[default]
    None,
    Left,
    Center,
    Right,
}

/// One list item: an optional task checkbox and its blocks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkdownItem {
    /// `Some(done)` for a task list item (`- [x]`).
    pub task: Option<bool>,
    pub blocks: Vec<MarkdownBlock>,
}

/// A block of a Markdown document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkdownBlock {
    Paragraph(RichText),
    Heading {
        level: u8,
        text: RichText,
    },
    /// A fenced or indented code block; `language` is the fence's info string.
    Code {
        language: String,
        code: String,
    },
    Quote(Vec<MarkdownBlock>),
    /// `start` is the first number of an ordered list, `None` for bullets.
    List {
        start: Option<u64>,
        items: Vec<MarkdownItem>,
    },
    Table {
        align: Vec<ColumnAlign>,
        head: Vec<RichText>,
        rows: Vec<Vec<RichText>>,
    },
    Rule,
    /// Images become blocks of their own, after the text of their paragraph.
    Image(MarkdownImage),
}

/* ─────────── Parser ─────────── */

#[derive(Default)]
struct Inline {
    rich: RichText,
    bold: u32,
    italic: u32,
    strike: u32,
    links: Vec<(usize, String)>,
    images: Vec<MarkdownImage>,
}

impl Inline {
    fn style(&self, code: bool) -> InlineStyle {
        InlineStyle {
            bold: self.bold > 0,
            italic: self.italic > 0,
            strike: self.strike > 0,
            code,
            link: !self.links.is_empty(),
        }
    }

    fn push(&mut self, text: &str, style: InlineStyle) {
        if text.is_empty() {
            return;
        }
        let start = self.rich.text.len();
        self.rich.text.push_str(text);
        let end = self.rich.text.len();
        match self.rich.spans.last_mut() {
            Some((range, last)) if *last == style && range.end == start => range.end = end,
            _ => self.rich.spans.push((start..end, style)),
        }
    }

    /// Plain text, with bare URLs turned into links outside links and code.
    fn text(&mut self, text: &str) {
        let style = self.style(false);
        if style.link {
            self.push(text, style);
            return;
        }
        let mut at = 0;
        for url in find_urls(text) {
            self.push(&text[at..url.start], style);
            let start = self.rich.text.len();
            self.push(
                &text[url.clone()],
                InlineStyle {
                    link: true,
                    ..style
                },
            );
            self.rich
                .links
                .push((start..self.rich.text.len(), text[url.clone()].to_string()));
            at = url.end;
        }
        self.push(&text[at..], style);
    }
}

enum Frame {
    Blocks(Vec<MarkdownBlock>),
    Quote(Vec<MarkdownBlock>),
    List {
        start: Option<u64>,
        items: Vec<MarkdownItem>,
    },
    Item(MarkdownItem),
    Table {
        align: Vec<ColumnAlign>,
        head: Vec<RichText>,
        rows: Vec<Vec<RichText>>,
        row: Vec<RichText>,
    },
}

struct Builder {
    frames: Vec<Frame>,
    inline: Option<Inline>,
    heading: Option<u8>,
    code: Option<(String, String)>,
    html: Option<String>,
    image: Option<MarkdownImage>,
    in_cell: bool,
}

impl Builder {
    fn push_block(&mut self, block: MarkdownBlock) {
        match self.frames.last_mut() {
            Some(Frame::Blocks(blocks) | Frame::Quote(blocks)) => blocks.push(block),
            Some(Frame::Item(item)) => item.blocks.push(block),
            // A list only holds items and a table only cells; nothing else lands there.
            Some(Frame::List { .. } | Frame::Table { .. }) | None => {}
        }
    }

    fn inline(&mut self) -> &mut Inline {
        self.inline.get_or_insert_with(Inline::default)
    }

    /// Ends the current run of inline content as a paragraph (or heading), with its images after.
    fn flush(&mut self) {
        let Some(inline) = self.inline.take() else {
            return;
        };
        let images = inline.images;
        let mut rich = inline.rich;
        trim_rich(&mut rich);
        if !rich.text.is_empty() {
            match self.heading.take() {
                Some(level) => self.push_block(MarkdownBlock::Heading { level, text: rich }),
                None => self.push_block(MarkdownBlock::Paragraph(rich)),
            }
        }
        for image in images {
            self.push_block(MarkdownBlock::Image(image));
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => self.flush(),
            Tag::Heading { level, .. } => {
                self.flush();
                self.heading = Some(level as u8);
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.frames.push(Frame::Quote(Vec::new()));
            }
            Tag::CodeBlock(kind) => {
                self.flush();
                let language = match kind {
                    CodeBlockKind::Fenced(info) => info.trim().to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((language, String::new()));
            }
            Tag::HtmlBlock => {
                self.flush();
                self.html = Some(String::new());
            }
            Tag::List(start) => {
                self.flush();
                self.frames.push(Frame::List {
                    start,
                    items: Vec::new(),
                });
            }
            Tag::Item => {
                self.flush();
                self.frames.push(Frame::Item(MarkdownItem::default()));
            }
            Tag::Table(align) => {
                self.flush();
                self.frames.push(Frame::Table {
                    align: align
                        .into_iter()
                        .map(|a| match a {
                            Alignment::None => ColumnAlign::None,
                            Alignment::Left => ColumnAlign::Left,
                            Alignment::Center => ColumnAlign::Center,
                            Alignment::Right => ColumnAlign::Right,
                        })
                        .collect(),
                    head: Vec::new(),
                    rows: Vec::new(),
                    row: Vec::new(),
                });
            }
            Tag::TableCell => {
                self.in_cell = true;
                self.inline = Some(Inline::default());
            }
            Tag::Emphasis => self.inline().italic += 1,
            Tag::Strong => self.inline().bold += 1,
            Tag::Strikethrough => self.inline().strike += 1,
            Tag::Link { dest_url, .. } => {
                let inline = self.inline();
                let at = inline.rich.text.len();
                inline.links.push((at, dest_url.to_string()));
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                self.image = Some(MarkdownImage {
                    url: dest_url.to_string(),
                    alt: String::new(),
                    title: title.to_string(),
                });
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) => self.flush(),
            TagEnd::BlockQuote(_) => {
                self.flush();
                if let Some(Frame::Quote(blocks)) = self.frames.pop() {
                    self.push_block(MarkdownBlock::Quote(blocks));
                }
            }
            TagEnd::CodeBlock => {
                if let Some((language, mut code)) = self.code.take() {
                    if code.ends_with('\n') {
                        code.pop();
                    }
                    self.push_block(MarkdownBlock::Code { language, code });
                }
            }
            TagEnd::HtmlBlock => {
                if let Some(html) = self.html.take() {
                    let plain = strip_tags(&html);
                    let plain = plain.trim();
                    if !plain.is_empty() {
                        let mut inline = Inline::default();
                        inline.text(plain);
                        self.push_block(MarkdownBlock::Paragraph(inline.rich));
                    }
                }
            }
            TagEnd::Item => {
                self.flush();
                if let Some(Frame::Item(item)) = self.frames.pop() {
                    if let Some(Frame::List { items, .. }) = self.frames.last_mut() {
                        items.push(item);
                    }
                }
            }
            TagEnd::List(_) => {
                self.flush();
                if let Some(Frame::List { start, items }) = self.frames.pop() {
                    self.push_block(MarkdownBlock::List { start, items });
                }
            }
            TagEnd::TableCell => {
                self.in_cell = false;
                let mut rich = self.inline.take().unwrap_or_default().rich;
                trim_rich(&mut rich);
                if let Some(Frame::Table { row, .. }) = self.frames.last_mut() {
                    row.push(rich);
                }
            }
            TagEnd::TableHead => {
                if let Some(Frame::Table { head, row, .. }) = self.frames.last_mut() {
                    *head = std::mem::take(row);
                }
            }
            TagEnd::TableRow => {
                if let Some(Frame::Table { rows, row, .. }) = self.frames.last_mut() {
                    rows.push(std::mem::take(row));
                }
            }
            TagEnd::Table => {
                if let Some(Frame::Table {
                    align, head, rows, ..
                }) = self.frames.pop()
                {
                    self.push_block(MarkdownBlock::Table { align, head, rows });
                }
            }
            TagEnd::Emphasis => self.inline().italic = self.inline().italic.saturating_sub(1),
            TagEnd::Strong => self.inline().bold = self.inline().bold.saturating_sub(1),
            TagEnd::Strikethrough => self.inline().strike = self.inline().strike.saturating_sub(1),
            TagEnd::Link => {
                let inline = self.inline();
                if let Some((start, url)) = inline.links.pop() {
                    let end = inline.rich.text.len();
                    if end > start {
                        inline.rich.links.push((start..end, url));
                    }
                }
            }
            TagEnd::Image => {
                if let Some(image) = self.image.take() {
                    if self.in_cell || self.heading.is_some() {
                        // No room for a picture: keep its description in the text.
                        let alt = image.alt.clone();
                        self.inline().text(&alt);
                    } else {
                        self.inline().images.push(image);
                    }
                }
            }
            _ => {}
        }
    }

    fn event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => {
                if let Some((_, code)) = &mut self.code {
                    code.push_str(&text);
                } else if let Some(image) = &mut self.image {
                    image.alt.push_str(&text);
                } else {
                    self.inline().text(&text);
                }
            }
            Event::Code(code) => {
                if let Some(image) = &mut self.image {
                    image.alt.push_str(&code);
                } else {
                    let inline = self.inline();
                    let style = inline.style(true);
                    inline.push(&code, style);
                }
            }
            Event::Html(html) => {
                if let Some(buffer) = &mut self.html {
                    buffer.push_str(&html);
                }
            }
            Event::InlineHtml(html) => {
                let tag = html.trim().to_ascii_lowercase();
                if matches!(tag.as_str(), "<br>" | "<br/>" | "<br />") {
                    let inline = self.inline();
                    let style = inline.style(false);
                    inline.push("\n", style);
                }
            }
            Event::SoftBreak => {
                if self.image.is_none() {
                    let inline = self.inline();
                    let style = inline.style(false);
                    inline.push(" ", style);
                }
            }
            Event::HardBreak => {
                let inline = self.inline();
                let style = inline.style(false);
                inline.push("\n", style);
            }
            Event::Rule => {
                self.flush();
                self.push_block(MarkdownBlock::Rule);
            }
            Event::TaskListMarker(done) => {
                if let Some(Frame::Item(item)) = self
                    .frames
                    .iter_mut()
                    .rev()
                    .find(|f| matches!(f, Frame::Item(_)))
                {
                    item.task = Some(done);
                }
            }
            _ => {}
        }
    }
}

/// Removes leading and trailing whitespace, keeping the spans and links in place.
fn trim_rich(rich: &mut RichText) {
    let start = rich.text.len() - rich.text.trim_start().len();
    let end = rich.text.trim_end().len();
    if start == 0 && end == rich.text.len() {
        return;
    }
    let end = end.max(start);
    let clip =
        |r: &Range<usize>| r.start.clamp(start, end) - start..r.end.clamp(start, end) - start;
    rich.text = rich.text[start..end].to_string();
    rich.spans = rich
        .spans
        .iter()
        .map(|(r, s)| (clip(r), *s))
        .filter(|(r, _)| !r.is_empty())
        .collect();
    rich.links = rich
        .links
        .iter()
        .map(|(r, u)| (clip(r), u.clone()))
        .filter(|(r, _)| !r.is_empty())
        .collect();
}

/// Removes `<…>` tags from raw HTML (an HTML block is shown as its text).
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// Bare `http(s)://` URLs in plain text (GFM autolinks), without trailing punctuation.
fn find_urls(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find("http") {
        let start = from + found;
        let rest = &text[start..];
        let scheme = if rest.starts_with("https://") {
            8
        } else if rest.starts_with("http://") {
            7
        } else {
            from = start + 4;
            continue;
        };
        let boundary = text[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let len = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`'))
            .unwrap_or(rest.len());
        let mut url = &rest[..len];
        loop {
            let trimmed = url.trim_end_matches(['.', ',', ';', ':', '!', '?', '\'', '*', '_']);
            let trimmed = if trimmed.ends_with(')')
                && trimmed.matches(')').count() > trimmed.matches('(').count()
            {
                &trimmed[..trimmed.len() - 1]
            } else {
                trimmed
            };
            if trimmed.len() == url.len() {
                break;
            }
            url = trimmed;
        }
        if boundary && url.len() > scheme {
            out.push(start..start + url.len());
        }
        from = start + len.max(1);
    }
    out
}

/// Whether inline code looks like a file path (`src/x.ts`, `x.ts:42`, `x.rs:3-9`), like the
/// reference design's clickable paths.
pub fn looks_like_path(text: &str) -> bool {
    let (path, _) = split_path_line(text.trim());
    let path = match path.as_bytes() {
        [drive, b':', b'\\' | b'/', ..] if drive.is_ascii_alphabetic() => &path[2..],
        _ => path,
    };
    let Some((stem, ext)) = path.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_.-/\\@~".contains(c))
        && (1..=8).contains(&ext.len())
        && ext.chars().all(|c| c.is_ascii_alphanumeric())
        && ext.chars().any(|c| c.is_ascii_alphabetic())
}

/// Splits a `path:line` or `path:start-end` reference into the path and its lines.
pub fn split_path_line(text: &str) -> (&str, Option<(u32, Option<u32>)>) {
    if let Some((path, lines)) = text.rsplit_once(':') {
        let (start, end) = match lines.split_once('-') {
            Some((a, b)) => (a, Some(b)),
            None => (lines, None),
        };
        if let Ok(start) = start.parse::<u32>() {
            match end.map(str::parse::<u32>) {
                None => return (path, Some((start, None))),
                Some(Ok(end)) => return (path, Some((start, Some(end)))),
                Some(Err(_)) => {}
            }
        }
    }
    (text, None)
}

/// Parses GitHub-flavored Markdown (tables, task lists, strikethrough, autolinks) into blocks.
pub fn parse_markdown(text: &str) -> Vec<MarkdownBlock> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM;
    let mut builder = Builder {
        frames: vec![Frame::Blocks(Vec::new())],
        inline: None,
        heading: None,
        code: None,
        html: None,
        image: None,
        in_cell: false,
    };
    for event in Parser::new_ext(text, options) {
        builder.event(event);
    }
    builder.flush();
    // An unclosed container (cut off mid-stream) is closed in order.
    while builder.frames.len() > 1 {
        let end = match builder.frames.last() {
            Some(Frame::Quote(_)) => TagEnd::BlockQuote(None),
            Some(Frame::List { start, .. }) => TagEnd::List(start.is_some()),
            Some(Frame::Item(_)) => TagEnd::Item,
            Some(Frame::Table { .. }) => TagEnd::Table,
            _ => break,
        };
        builder.end(end);
    }
    match builder.frames.pop() {
        Some(Frame::Blocks(blocks)) => blocks,
        _ => Vec::new(),
    }
}

thread_local! {
    static PARSED: RefCell<Lru<Rc<Vec<MarkdownBlock>>>> = const { RefCell::new(Lru::new(128)) };
}

/// [`parse_markdown`] with a per-thread cache keyed by the text.
pub fn parse_markdown_cached(text: &str) -> Rc<Vec<MarkdownBlock>> {
    let key = content_key(&[text]);
    PARSED.with(|c| {
        c.borrow_mut()
            .get_or_insert_with(key, || Rc::new(parse_markdown(text)))
    })
}

/* ─────────── View ─────────── */

type LinkHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;
type ImageRenderer = Rc<dyn Fn(&MarkdownImage, &mut Window, &mut App) -> AnyElement>;

/// Rendered Markdown with the reference design's look: 1.6 line height, headings by level,
/// nested and numbered lists, task boxes, quotes with a side rule, tables, [`CodeBlock`]s and
/// links in the primary color.
///
/// GPUI does not select text; offer a Copy action next to messages instead.
#[derive(IntoElement)]
pub struct Markdown {
    id: ElementId,
    text: SharedString,
    size: Pixels,
    on_link: Option<LinkHandler>,
    on_path: Option<LinkHandler>,
    on_image: Option<ImageRenderer>,
}

impl Markdown {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            size: text::BODY,
            on_link: None,
            on_path: None,
            on_image: None,
        }
    }

    /// Body text size (13.5 px by default); headings scale from it.
    pub fn text_size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// Called with a link's destination when it is clicked (open it in the browser, say).
    pub fn on_link(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_link = Some(Rc::new(f));
        self
    }

    /// Makes `inline code` that looks like a path (`src/x.ts:42`) clickable and calls `f` with
    /// it; see [`split_path_line`] to separate the line number.
    pub fn on_path(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_path = Some(Rc::new(f));
        self
    }

    /// Renders images yourself (load the URL, a thumbnail…). Without it, an image shows as a
    /// placeholder row with its description that opens through `on_link`.
    pub fn on_image(
        mut self,
        f: impl Fn(&MarkdownImage, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.on_image = Some(Rc::new(f));
        self
    }
}

/// What a click on a range of rich text does.
enum Target {
    Link(String),
    Path(String),
}

struct Render<'a> {
    id: &'a ElementId,
    count: usize,
    size: f32,
    s: Scheme,
    on_link: Option<LinkHandler>,
    on_path: Option<LinkHandler>,
    on_image: Option<ImageRenderer>,
}

impl Render<'_> {
    fn next_id(&mut self, part: &'static str) -> ElementId {
        self.count += 1;
        ElementId::NamedChild(
            Box::new(self.id.clone()),
            format!("{part}{}", self.count).into(),
        )
    }

    fn rich(&mut self, rich: &RichText, weight: FontWeight) -> AnyElement {
        let s = self.s;
        let runs: Vec<TextRun> = rich
            .spans
            .iter()
            .map(|(range, style)| {
                let color = if style.link { s.primary } else { s.on_surface };
                TextRun {
                    len: range.len(),
                    font: Font {
                        family: if style.code {
                            MONO_FONT_FAMILY.into()
                        } else {
                            FONT_FAMILY.into()
                        },
                        features: FontFeatures::default(),
                        fallbacks: None,
                        weight: if style.bold {
                            FontWeight(weight.0.max(FontWeight::SEMIBOLD.0))
                        } else {
                            weight
                        },
                        style: if style.italic {
                            FontStyle::Italic
                        } else {
                            FontStyle::Normal
                        },
                    },
                    color,
                    background_color: style.code.then_some(s.surface_container_high),
                    underline: None,
                    strikethrough: style.strike.then_some(StrikethroughStyle {
                        thickness: px(1.),
                        color: Some(color),
                    }),
                }
            })
            .collect();
        let styled = StyledText::new(rich.text.clone()).with_runs(runs);

        let mut targets: Vec<(Range<usize>, Target)> = Vec::new();
        if self.on_link.is_some() {
            targets.extend(
                rich.links
                    .iter()
                    .map(|(r, url)| (r.clone(), Target::Link(url.clone()))),
            );
        }
        if self.on_path.is_some() {
            targets.extend(rich.code_ranges().filter_map(|r| {
                let text = &rich.text[r.clone()];
                looks_like_path(text).then(|| (r.clone(), Target::Path(text.trim().to_string())))
            }));
        }
        if targets.is_empty() {
            return styled.into_any_element();
        }
        targets.sort_by_key(|(r, _)| r.start);
        let ranges = targets.iter().map(|(r, _)| r.clone()).collect();
        let (on_link, on_path) = (self.on_link.clone(), self.on_path.clone());
        InteractiveText::new(self.next_id("text"), styled)
            .on_click(ranges, move |ix, window, cx| match &targets[ix].1 {
                Target::Link(url) => {
                    if let Some(f) = &on_link {
                        f(url, window, cx)
                    }
                }
                Target::Path(path) => {
                    if let Some(f) = &on_path {
                        f(path, window, cx)
                    }
                }
            })
            .into_any_element()
    }

    fn blocks(
        &mut self,
        blocks: &[MarkdownBlock],
        depth: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        blocks
            .iter()
            .enumerate()
            .map(|(i, block)| self.block(block, i == 0, depth, window, cx))
            .collect()
    }

    fn block(
        &mut self,
        block: &MarkdownBlock,
        first: bool,
        depth: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let s = self.s;
        let size = self.size;
        match block {
            MarkdownBlock::Paragraph(rich) => div()
                .child(self.rich(rich, FontWeight::NORMAL))
                .into_any_element(),
            MarkdownBlock::Heading { level, text } => {
                let scale = match level {
                    1 => 1.3,
                    2 => 1.18,
                    3 => 1.06,
                    _ => 1.,
                };
                let muted = *level >= 4;
                let rich = self.rich(text, FontWeight::SEMIBOLD);
                div()
                    .when(!first, |d| d.pt(px(size * 0.5)))
                    .text_size(px(size * scale))
                    .line_height(relative(1.3))
                    .when(muted, |d| d.text_color(s.on_surface_variant))
                    .child(rich)
                    .into_any_element()
            }
            MarkdownBlock::Code { language, code } => {
                CodeBlock::new(self.next_id("code"), code.clone())
                    .language(language.clone())
                    .into_any_element()
            }
            MarkdownBlock::Quote(blocks) => {
                let children = self.blocks(blocks, depth, window, cx);
                div()
                    .flex()
                    .gap(px(size * 0.9 - 3.))
                    .text_color(s.on_surface_variant)
                    .child(
                        div()
                            .flex_none()
                            .w(px(3.))
                            .rounded(px(1.5))
                            .bg(s.outline_variant),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(size * 0.75))
                            .flex_1()
                            .min_w_0()
                            .children(children),
                    )
                    .into_any_element()
            }
            MarkdownBlock::List { start, items } => {
                let mut list = div().flex().flex_col().gap(px(size * 0.2));
                for (i, item) in items.iter().enumerate() {
                    let marker = match (item.task, start) {
                        (Some(done), _) => task_box(done, size, &s).into_any_element(),
                        (None, Some(n)) => div()
                            .text_color(s.on_surface_variant)
                            .child(format!("{}.", n + i as u64))
                            .into_any_element(),
                        (None, None) => div()
                            .text_color(s.on_surface_variant)
                            .child(["•", "◦", "▪"][depth % 3])
                            .into_any_element(),
                    };
                    let children = self.blocks(&item.blocks, depth + 1, window, cx);
                    list = list.child(
                        div()
                            .flex()
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(size * 1.5))
                                    .pr(px(size * 0.4))
                                    .flex()
                                    .justify_end()
                                    .child(marker),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(size * 0.2))
                                    .flex_1()
                                    .min_w_0()
                                    .when(item.task == Some(true), |d| {
                                        d.text_color(s.on_surface_variant)
                                    })
                                    .children(children),
                            ),
                    );
                }
                list.into_any_element()
            }
            MarkdownBlock::Table { align, head, rows } => self.table(align, head, rows),
            MarkdownBlock::Rule => div()
                .my(px(size * 0.35))
                .h(px(1.))
                .bg(s.outline_variant)
                .into_any_element(),
            MarkdownBlock::Image(image) => match self.on_image.clone() {
                Some(f) => f(image, window, cx),
                None => self.image_placeholder(image),
            },
        }
    }

    fn table(
        &mut self,
        align: &[ColumnAlign],
        head: &[RichText],
        rows: &[Vec<RichText>],
    ) -> AnyElement {
        let s = self.s;
        let columns = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
        if columns == 0 {
            return div().into_any_element();
        }
        // Column widths follow their longest cell (clamped), like an auto table layout.
        let mut weights = vec![3f32; columns];
        for row in std::iter::once(head).chain(rows.iter().map(Vec::as_slice)) {
            for (c, cell) in row.iter().enumerate() {
                weights[c] = weights[c].max(cell.text.chars().count().min(40) as f32);
            }
        }
        let total: f32 = weights.iter().sum();
        let row_el = |cells: &[RichText], header: bool, this: &mut Self| {
            let mut row = div()
                .flex()
                .w_full()
                .border_b_1()
                .border_color(s.outline_variant);
            if header {
                row = row.bg(s.surface_container);
            }
            for c in 0..columns {
                let content = cells.get(c).map(|cell| {
                    this.rich(
                        cell,
                        if header {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        },
                    )
                });
                let cell = div()
                    .w(relative(weights[c] / total))
                    .min_w(px(48.))
                    .px(px(10.))
                    .py(px(4.))
                    .when(c + 1 < columns, |d| {
                        d.border_r_1().border_color(s.outline_variant)
                    })
                    .map(|d| match align.get(c).copied().unwrap_or_default() {
                        ColumnAlign::Center => d.text_center(),
                        ColumnAlign::Right => d.text_right(),
                        _ => d,
                    })
                    .children(content);
                row = row.child(cell);
            }
            row
        };
        let mut table = div()
            .flex()
            .flex_col()
            .min_w(px(columns as f32 * 72.))
            .border_1()
            .border_color(s.outline_variant)
            .rounded(px(8.))
            .overflow_hidden()
            .text_size(px(self.size * 0.95));
        if !head.is_empty() {
            table = table.child(row_el(head, true, self));
        }
        for row in rows {
            table = table.child(row_el(row, false, self));
        }
        div()
            .id(self.next_id("table"))
            .overflow_x_scroll()
            .child(table)
            .into_any_element()
    }

    fn image_placeholder(&mut self, image: &MarkdownImage) -> AnyElement {
        let s = self.s;
        let label = if !image.alt.is_empty() {
            image.alt.clone()
        } else if !image.title.is_empty() {
            image.title.clone()
        } else {
            image.url.clone()
        };
        let url = image.url.clone();
        let on_link = self.on_link.clone();
        div()
            .id(self.next_id("image"))
            .flex()
            .items_center()
            .gap(px(8.))
            .max_w_full()
            .px(px(12.))
            .py(px(8.))
            .rounded(px(12.))
            .bg(s.surface_container_high)
            .text_color(s.on_surface_variant)
            .child(Icon::new("file").size(px(16.)).color(s.on_surface_variant))
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(label),
            )
            .when_some(on_link, |d, f| {
                d.cursor_pointer()
                    .hover(|d| d.bg(s.surface_container_highest))
                    .on_click(move |_, window, cx| f(&url, window, cx))
            })
            .into_any_element()
    }
}

/// A static task checkbox (done: filled with a check).
fn task_box(done: bool, size: f32, s: &Scheme) -> impl IntoElement {
    let side = (size * 1.05).round();
    let color: Hsla = if done { s.primary } else { s.outline };
    div()
        .mt(px((size * 1.6 - side) / 2.))
        .size(px(side))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .when(done, |d| d.bg(color))
        .when(!done, |d| d.border_2().border_color(color))
        .when(done, |d| {
            d.child(Icon::new("check").size(px(side - 3.)).color(s.on_primary))
        })
}

impl RenderOnce for Markdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let blocks = parse_markdown_cached(&self.text);
        let size = f32::from(self.size);
        let mut r = Render {
            id: &self.id,
            count: 0,
            size,
            s,
            on_link: self.on_link,
            on_path: self.on_path,
            on_image: self.on_image,
        };
        let children = r.blocks(&blocks, 0, window, cx);
        div()
            .flex()
            .flex_col()
            .gap(px(size * 0.75))
            .min_w_0()
            .font_family(FONT_FAMILY)
            .text_size(self.size)
            .line_height(relative(1.6))
            .text_color(s.on_surface)
            .children(children)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn para(blocks: &[MarkdownBlock], i: usize) -> &RichText {
        match &blocks[i] {
            MarkdownBlock::Paragraph(rich) => rich,
            other => panic!("not a paragraph: {other:?}"),
        }
    }

    fn styled<'a>(rich: &'a RichText, f: impl Fn(&InlineStyle) -> bool) -> Vec<&'a str> {
        rich.spans
            .iter()
            .filter(|(_, s)| f(s))
            .map(|(r, _)| &rich.text[r.clone()])
            .collect()
    }

    #[test]
    fn paragraphs_and_inline_styles() {
        let blocks = parse_markdown(
            "Some **bold**, *italic*, ~~gone~~ and `code`.\nSame paragraph.\n\nSecond.",
        );
        assert_eq!(blocks.len(), 2);
        let p = para(&blocks, 0);
        assert_eq!(p.text, "Some bold, italic, gone and code. Same paragraph.");
        assert_eq!(styled(p, |s| s.bold), ["bold"]);
        assert_eq!(styled(p, |s| s.italic), ["italic"]);
        assert_eq!(styled(p, |s| s.strike), ["gone"]);
        assert_eq!(styled(p, |s| s.code), ["code"]);
        // Spans cover the whole text.
        assert_eq!(
            p.spans.iter().map(|(r, _)| r.len()).sum::<usize>(),
            p.text.len()
        );
        assert_eq!(para(&blocks, 1).text, "Second.");
    }

    #[test]
    fn headings_keep_their_level() {
        let blocks = parse_markdown("# One\n### Three *x*\n###### Six");
        let levels: Vec<(u8, &str)> = blocks
            .iter()
            .map(|b| match b {
                MarkdownBlock::Heading { level, text } => (*level, text.as_str()),
                _ => panic!(),
            })
            .collect();
        assert_eq!(levels, [(1, "One"), (3, "Three x"), (6, "Six")]);
    }

    #[test]
    fn nested_numbered_and_task_lists() {
        let blocks = parse_markdown(
            "3. three\n4. four\n   - nested\n     - deeper\n\n- [x] done\n- [ ] todo",
        );
        let MarkdownBlock::List { start, items } = &blocks[0] else {
            panic!("{blocks:?}")
        };
        assert_eq!(*start, Some(3));
        assert_eq!(items.len(), 2);
        assert_eq!(para(&items[0].blocks, 0).text, "three");
        let MarkdownBlock::List {
            start: None,
            items: nested,
        } = &items[1].blocks[1]
        else {
            panic!("{:?}", items[1].blocks)
        };
        assert!(matches!(&nested[0].blocks[1], MarkdownBlock::List { .. }));
        let MarkdownBlock::List { items: tasks, .. } = &blocks[1] else {
            panic!()
        };
        assert_eq!(tasks[0].task, Some(true));
        assert_eq!(tasks[1].task, Some(false));
        assert_eq!(para(&tasks[1].blocks, 0).text, "todo");
    }

    #[test]
    fn quotes_rules_and_code() {
        let blocks =
            parse_markdown("> quoted\n> > inner\n\n---\n\n```rust title=x\nfn x() {}\n```\n");
        let MarkdownBlock::Quote(inner) = &blocks[0] else {
            panic!()
        };
        assert_eq!(para(inner, 0).text, "quoted");
        assert!(matches!(inner[1], MarkdownBlock::Quote(_)));
        assert_eq!(blocks[1], MarkdownBlock::Rule);
        assert_eq!(
            blocks[2],
            MarkdownBlock::Code {
                language: "rust title=x".into(),
                code: "fn x() {}".into()
            }
        );
    }

    #[test]
    fn an_unclosed_fence_streams_as_code() {
        let blocks = parse_markdown("Look:\n\n```ts\nconst a = 1;\nconst b");
        assert_eq!(
            blocks[1],
            MarkdownBlock::Code {
                language: "ts".into(),
                code: "const a = 1;\nconst b".into()
            }
        );
    }

    #[test]
    fn tables_with_alignment() {
        let blocks = parse_markdown("| A | B |\n|:--|--:|\n| `x` | **2** |\n| y | 3 |");
        let MarkdownBlock::Table { align, head, rows } = &blocks[0] else {
            panic!("{blocks:?}")
        };
        assert_eq!(align, &[ColumnAlign::Left, ColumnAlign::Right]);
        assert_eq!(
            head.iter().map(RichText::as_str).collect::<Vec<_>>(),
            ["A", "B"]
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(styled(&rows[0][0], |s| s.code), ["x"]);
        assert_eq!(styled(&rows[0][1], |s| s.bold), ["2"]);
    }

    #[test]
    fn links_and_bare_urls() {
        let blocks = parse_markdown(
            "See [the docs](https://a.dev/x) or https://b.dev/y. (Also https://c.dev/(z)).",
        );
        let p = para(&blocks, 0);
        let links: Vec<(&str, &str)> = p
            .links
            .iter()
            .map(|(r, url)| (&p.text[r.clone()], url.as_str()))
            .collect();
        assert_eq!(
            links,
            [
                ("the docs", "https://a.dev/x"),
                ("https://b.dev/y", "https://b.dev/y"),
                ("https://c.dev/(z)", "https://c.dev/(z)"),
            ]
        );
        assert_eq!(
            styled(p, |s| s.link),
            ["the docs", "https://b.dev/y", "https://c.dev/(z)"]
        );
    }

    #[test]
    fn images_become_blocks_after_their_text() {
        let blocks = parse_markdown("Before ![a cat](cat.png \"Cat\") after\n\n![only](x.png)");
        assert_eq!(para(&blocks, 0).text, "Before  after");
        assert_eq!(
            blocks[1],
            MarkdownBlock::Image(MarkdownImage {
                url: "cat.png".into(),
                alt: "a cat".into(),
                title: "Cat".into()
            })
        );
        assert!(matches!(&blocks[2], MarkdownBlock::Image(i) if i.alt == "only"));
        assert_eq!(blocks.len(), 3);
    }

    #[test]
    fn hard_breaks_and_html() {
        let blocks = parse_markdown("one  \ntwo<br>three\n\n<div>\nraw <b>html</b>\n</div>");
        assert_eq!(para(&blocks, 0).text, "one\ntwo\nthree");
        assert_eq!(para(&blocks, 1).text, "raw html");
    }

    #[test]
    fn path_detection() {
        for yes in [
            "src/x.ts",
            "x.ts:42",
            "a/b-c.rs:3-9",
            "C:\\x\\y.rs",
            "Cargo.toml",
            "./m.mjs",
        ] {
            assert!(looks_like_path(yes), "{yes}");
        }
        for no in ["hello", "1.5", "e.g.", "a b.rs", "x.toolongext", "fn()"] {
            assert!(!looks_like_path(no), "{no}");
        }
        assert_eq!(split_path_line("x.ts:42"), ("x.ts", Some((42, None))));
        assert_eq!(split_path_line("x.ts:3-9"), ("x.ts", Some((3, Some(9)))));
        assert_eq!(split_path_line("x.ts"), ("x.ts", None));
    }

    #[test]
    fn cached_parse_is_shared() {
        let a = parse_markdown_cached("# same");
        let b = parse_markdown_cached("# same");
        assert!(Rc::ptr_eq(&a, &b));
    }
}
