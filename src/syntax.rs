//! Syntax highlighting for code blocks and viewers, line by line.
//!
//! Built on [syntect](https://crates.io/crates/syntect) (MIT) with its bundled Sublime Text
//! syntaxes and the pure-Rust `fancy-regex` engine (no C toolchain needed). syntect parses one
//! line at a time and its state can be cloned, which is what an editor needs to re-highlight
//! from the edited line on. TypeScript/TSX, TOML and PowerShell are not in the bundled set, so
//! compact grammars for them ship in `assets/syntaxes/`.
//!
//! Tokens are reduced to a few [`TokenKind`]s, the same classes the reference design colors
//! (`--sx-k`, `--sx-s`…), and painted with a [`SyntaxPalette`] derived from the theme.
//!
//! ```ignore
//! let mut hl = SyntaxHighlighter::new("rust").unwrap();
//! for line in code.lines() {
//!     let tokens = hl.highlight_line(line); // Vec<(Range<usize>, TokenKind)>
//! }
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::OnceLock;

use gpui::{
    App, FontStyle, FontWeight, Global, HighlightStyle, Hsla, SharedString, UnderlineStyle, px,
};
use syntect::parsing::{
    ParseState, Scope, ScopeStack, SyntaxDefinition, SyntaxReference, SyntaxSet, SyntaxSetBuilder,
};

use crate::cache::{Lru, content_key};
use crate::theme::{Scheme, Theme, hex};

/// What a piece of code is, as far as coloring goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// Keywords, storage (`fn`, `const`), word operators and `self`/`this` (`--sx-k`).
    Keyword,
    /// Strings, characters, escapes and regular expressions (`--sx-s`).
    String,
    /// Function names at definitions and calls (`--sx-f`).
    Function,
    /// Types, classes, namespaces and markup tags (`--sx-t`).
    Type,
    /// Comments (`--sx-c`, italic).
    Comment,
    /// Numbers, booleans, `null` and other constants (`--sx-n`).
    Number,
    /// Macros, attributes, annotations and decorators (`--sx-m`).
    Meta,
    /// Markup headings and section names (keyword color, bold).
    Heading,
    /// Markup links (accent, underlined).
    Link,
}

/// Colors for each [`TokenKind`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyntaxPalette {
    pub keyword: Hsla,
    pub string: Hsla,
    pub function: Hsla,
    pub type_name: Hsla,
    pub comment: Hsla,
    pub number: Hsla,
    pub meta: Hsla,
    pub link: Hsla,
}

/// An app-wide override set with [`SyntaxPalette::set`].
struct PaletteOverride(SyntaxPalette);

impl Global for PaletteOverride {}

impl SyntaxPalette {
    /// The reference design's M3 Expressive syntax colors: keywords in the primary color, strings
    /// in the success color, types in the tertiary color, comments in the outline color, numbers
    /// in the warning color, plus a fixed orange for functions and blue for meta.
    pub fn from_scheme(s: &Scheme, dark: bool) -> Self {
        let (function, meta) = if dark {
            (hex("#ffb68b"), hex("#9ecaff"))
        } else {
            (hex("#9a4600"), hex("#00639b"))
        };
        Self {
            keyword: s.primary,
            string: s.success,
            function,
            type_name: s.tertiary,
            comment: s.outline,
            number: s.warning,
            meta,
            link: s.primary,
        }
    }

    /// The override set with [`SyntaxPalette::set`], or the colors derived from the current theme.
    pub fn of(cx: &App) -> Self {
        if let Some(p) = cx.try_global::<PaletteOverride>() {
            return p.0;
        }
        let theme = Theme::get(cx);
        Self::from_scheme(&theme.scheme, theme.dark)
    }

    /// Uses your own syntax colors (e.g. another visual style's tokens); `None` goes back to the
    /// theme's.
    pub fn set(palette: Option<SyntaxPalette>, cx: &mut App) {
        match palette {
            Some(p) => cx.set_global(PaletteOverride(p)),
            None => {
                if cx.has_global::<PaletteOverride>() {
                    cx.remove_global::<PaletteOverride>();
                }
            }
        }
        cx.refresh_windows();
    }

    pub fn color(&self, kind: TokenKind) -> Hsla {
        match kind {
            TokenKind::Keyword | TokenKind::Heading => self.keyword,
            TokenKind::String => self.string,
            TokenKind::Function => self.function,
            TokenKind::Type => self.type_name,
            TokenKind::Comment => self.comment,
            TokenKind::Number => self.number,
            TokenKind::Meta => self.meta,
            TokenKind::Link => self.link,
        }
    }

    /// Text style for a token: its color, italic comments, bold headings and underlined links.
    pub fn style(&self, kind: TokenKind) -> HighlightStyle {
        let color = self.color(kind);
        HighlightStyle {
            color: Some(color),
            font_style: (kind == TokenKind::Comment).then_some(FontStyle::Italic),
            font_weight: (kind == TokenKind::Heading).then_some(FontWeight::BOLD),
            underline: (kind == TokenKind::Link).then_some(UnderlineStyle {
                thickness: px(1.),
                color: Some(color),
                wavy: false,
            }),
            ..Default::default()
        }
    }

    /// Highlights for a [`gpui::StyledText`] from token ranges.
    pub fn highlights(
        &self,
        tokens: &[(Range<usize>, TokenKind)],
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        tokens
            .iter()
            .map(|(range, kind)| (range.clone(), self.style(*kind)))
            .collect()
    }
}

/* ─────────── Languages ─────────── */

/// Language of a code fence: a readable name and whether it can be highlighted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageInfo {
    /// "TypeScript", "Rust"… The fence's own word when unknown, "Text" when empty.
    pub label: SharedString,
    /// A grammar is available for it.
    pub highlighted: bool,
}

/// Fence word → (label, grammar). The grammar is a syntect syntax name; `None` is never colored.
const ALIASES: &[(&str, &str, Option<&str>)] = &[
    ("ts", "TypeScript", Some("TypeScript")),
    ("typescript", "TypeScript", Some("TypeScript")),
    ("mts", "TypeScript", Some("TypeScript")),
    ("cts", "TypeScript", Some("TypeScript")),
    ("tsx", "TSX", Some("TypeScript")),
    ("js", "JavaScript", Some("JavaScript")),
    ("javascript", "JavaScript", Some("JavaScript")),
    ("jsx", "JSX", Some("JavaScript")),
    ("mjs", "JavaScript", Some("JavaScript")),
    ("cjs", "JavaScript", Some("JavaScript")),
    ("rust", "Rust", Some("Rust")),
    ("rs", "Rust", Some("Rust")),
    ("json", "JSON", Some("JSON")),
    ("jsonc", "JSON", Some("JSON")),
    ("json5", "JSON", Some("JSON")),
    ("md", "Markdown", Some("Markdown")),
    ("markdown", "Markdown", Some("Markdown")),
    ("css", "CSS", Some("CSS")),
    ("scss", "SCSS", Some("CSS")),
    ("html", "HTML", Some("HTML")),
    ("htm", "HTML", Some("HTML")),
    ("xml", "XML", Some("XML")),
    ("svg", "SVG", Some("XML")),
    ("py", "Python", Some("Python")),
    ("python", "Python", Some("Python")),
    ("toml", "TOML", Some("TOML")),
    ("yaml", "YAML", Some("YAML")),
    ("yml", "YAML", Some("YAML")),
    ("sh", "Shell", Some("Bourne Again Shell (bash)")),
    ("bash", "Bash", Some("Bourne Again Shell (bash)")),
    ("zsh", "Zsh", Some("Bourne Again Shell (bash)")),
    ("shell", "Shell", Some("Bourne Again Shell (bash)")),
    ("console", "Console", None),
    ("powershell", "PowerShell", Some("PowerShell")),
    ("ps1", "PowerShell", Some("PowerShell")),
    ("pwsh", "PowerShell", Some("PowerShell")),
    ("ps", "PowerShell", Some("PowerShell")),
    ("diff", "Diff", Some("Diff")),
    ("patch", "Diff", Some("Diff")),
    ("text", "Text", None),
    ("txt", "Text", None),
    ("plain", "Text", None),
    ("plaintext", "Text", None),
];

/// First word of a fence's info string, lowercased (```` ```ts title="x" ```` → `ts`).
fn fence_key(info: &str) -> String {
    info.split_whitespace()
        .next()
        .unwrap_or("")
        .trim_start_matches('{')
        .trim_start_matches('.')
        .trim_end_matches('}')
        .to_lowercase()
}

/// Readable name of a fence's language and whether it can be highlighted.
pub fn language(info: &str) -> LanguageInfo {
    let key = fence_key(info);
    if key.is_empty() {
        return LanguageInfo {
            label: "Text".into(),
            highlighted: false,
        };
    }
    if let Some((_, label, grammar)) = ALIASES.iter().find(|(k, _, _)| *k == key) {
        return LanguageInfo {
            label: (*label).into(),
            highlighted: grammar.is_some(),
        };
    }
    LanguageInfo {
        highlighted: resolve(&key).is_some(),
        label: key.into(),
    }
}

fn defaults() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// The grammars gpui-m3 adds to syntect's bundled set.
fn extras() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(|| {
        let mut builder = SyntaxSetBuilder::new();
        for source in [
            include_str!("../assets/syntaxes/TypeScript.sublime-syntax"),
            include_str!("../assets/syntaxes/TOML.sublime-syntax"),
            include_str!("../assets/syntaxes/PowerShell.sublime-syntax"),
        ] {
            match SyntaxDefinition::load_from_str(source, true, None) {
                Ok(def) => builder.add(def),
                Err(err) => eprintln!("gpui_m3: invalid bundled syntax: {err}"),
            }
        }
        builder.build()
    })
}

/// The grammar for a fence word: an alias, then syntect's own names and extensions.
fn resolve(key: &str) -> Option<(&'static SyntaxSet, &'static SyntaxReference)> {
    let find = |name: &str| {
        [extras(), defaults()]
            .into_iter()
            .find_map(|set| set.find_syntax_by_name(name).map(|syntax| (set, syntax)))
    };
    if let Some((_, _, grammar)) = ALIASES.iter().find(|(k, _, _)| *k == key) {
        return find((*grammar)?);
    }
    [extras(), defaults()]
        .into_iter()
        .find_map(|set| set.find_syntax_by_token(key).map(|syntax| (set, syntax)))
}

/* ─────────── Highlighter ─────────── */

/// How a scope colors its text.
#[derive(Clone, Copy)]
enum Class {
    Kind(TokenKind),
    /// A keyword operator: colored as a keyword only when it is a word (`typeof`, `and`).
    Operator,
    /// Look at the enclosing scope.
    Pass,
    /// Embedded code or interpolation: stop, the enclosing string or tag does not apply.
    Stop,
}

fn classify(scope: &str) -> Class {
    use TokenKind::*;
    let starts = |prefixes: &[&str]| {
        prefixes
            .iter()
            .any(|p| scope == *p || scope.starts_with(&format!("{p}.")))
    };
    if starts(&["comment"]) {
        Class::Kind(Comment)
    } else if starts(&["meta.interpolation", "meta.embedded", "source", "text"])
        || scope.starts_with("punctuation.section.embedded")
    {
        Class::Stop
    } else if starts(&["string.other.link", "markup.underline.link", "meta.link"]) {
        Class::Kind(Link)
    } else if starts(&["string", "constant.character", "constant.other.placeholder"]) {
        Class::Kind(String)
    } else if starts(&["constant.numeric", "constant.language", "support.constant"]) {
        Class::Kind(Number)
    } else if starts(&["keyword.operator"]) {
        Class::Operator
    } else if starts(&[
        "support.macro",
        "entity.name.macro",
        "meta.annotation",
        "meta.decorator",
        "meta.preprocessor",
        "entity.other.attribute-name",
        "storage.type.annotation",
        "support.variable",
        "keyword.other.directive",
    ]) {
        Class::Kind(Meta)
    } else if starts(&["keyword", "storage", "variable.language"]) {
        Class::Kind(Keyword)
    } else if starts(&["markup.heading", "entity.name.section"]) {
        Class::Kind(Heading)
    } else if starts(&[
        "entity.name.function",
        "support.function",
        "variable.function",
    ]) {
        Class::Kind(Function)
    } else if starts(&[
        "entity.name",
        "entity.other.inherited-class",
        "support.type",
        "support.class",
    ]) {
        Class::Kind(Type)
    } else {
        Class::Pass
    }
}

thread_local! {
    static CLASSES: RefCell<HashMap<Scope, Class>> = RefCell::new(HashMap::new());
}

/// Kind of the text under a scope stack: the innermost scope that says something wins.
fn kind_of(stack: &[Scope], text: &str) -> Option<TokenKind> {
    CLASSES.with(|classes| {
        let mut classes = classes.borrow_mut();
        for scope in stack.iter().rev() {
            let class = *classes
                .entry(*scope)
                .or_insert_with(|| classify(&scope.build_string()));
            match class {
                Class::Kind(kind) => return Some(kind),
                Class::Operator => {
                    if text.chars().any(char::is_alphabetic) {
                        return Some(TokenKind::Keyword);
                    }
                }
                Class::Pass => {}
                Class::Stop => return None,
            }
        }
        None
    })
}

/// Highlights code one line at a time, keeping the parser state between lines (a string or
/// comment that spans lines stays colored). Clone it to snapshot the state after a line.
#[derive(Clone)]
pub struct SyntaxHighlighter {
    set: &'static SyntaxSet,
    state: ParseState,
    stack: ScopeStack,
}

impl SyntaxHighlighter {
    /// A highlighter for a fence word or file extension (`"ts"`, `"rust"`, `"py"`…), or `None`
    /// when there is no grammar for it.
    pub fn new(language: &str) -> Option<Self> {
        let (set, syntax) = resolve(&fence_key(language))?;
        Some(Self {
            set,
            state: ParseState::new(syntax),
            stack: ScopeStack::new(),
        })
    }

    /// A highlighter chosen from a file name's extension (`src/main.rs` → Rust).
    pub fn for_path(path: &str) -> Option<Self> {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let ext = name.rsplit_once('.').map(|(_, ext)| ext)?;
        Self::new(ext)
    }

    /// Tokens of one line (without its line break), as byte ranges into `line`. Uncolored text
    /// has no token. Lines must be fed in order.
    pub fn highlight_line(&mut self, line: &str) -> Vec<(Range<usize>, TokenKind)> {
        let line = line.strip_suffix('\n').unwrap_or(line);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let len = line.len();
        let mut input = String::with_capacity(len + 1);
        input.push_str(line);
        input.push('\n');
        let Ok(ops) = self.state.parse_line(&input, self.set) else {
            return Vec::new();
        };
        let mut out: Vec<(Range<usize>, TokenKind)> = Vec::new();
        let mut push = |range: Range<usize>, stack: &ScopeStack| {
            let range = range.start.min(len)..range.end.min(len);
            if range.is_empty() {
                return;
            }
            let Some(kind) = kind_of(stack.as_slice(), &line[range.clone()]) else {
                return;
            };
            match out.last_mut() {
                Some((last, k)) if *k == kind && last.end == range.start => last.end = range.end,
                _ => out.push((range, kind)),
            }
        };
        let mut at = 0;
        for (pos, op) in ops {
            if pos > at {
                push(at..pos, &self.stack);
                at = pos;
            }
            let _ = self.stack.apply(&op);
        }
        if at < len {
            push(at..len, &self.stack);
        }
        out
    }

    /// Tokens of a whole text, as byte ranges into `code`.
    pub fn highlight(&mut self, code: &str) -> Vec<(Range<usize>, TokenKind)> {
        let mut out = Vec::new();
        let mut start = 0;
        for line in code.split_inclusive('\n') {
            out.extend(
                self.highlight_line(line)
                    .into_iter()
                    .map(|(r, k)| (r.start + start..r.end + start, k)),
            );
            start += line.len();
        }
        out
    }
}

/// Tokens of `code` in `language` (byte ranges into `code`), or `None` without a grammar.
pub fn highlight(code: &str, language: &str) -> Option<Vec<(Range<usize>, TokenKind)>> {
    Some(SyntaxHighlighter::new(language)?.highlight(code))
}

/// A text highlighted up to its last complete line, kept to continue a streaming code block.
struct Partial {
    language: String,
    /// Byte length of the complete lines already highlighted.
    done: usize,
    prefix_key: u64,
    tokens: Vec<(Range<usize>, TokenKind)>,
    state: SyntaxHighlighter,
}

type Tokens = Rc<[(Range<usize>, TokenKind)]>;

thread_local! {
    static HIGHLIGHTS: RefCell<Lru<Option<Tokens>>> = const { RefCell::new(Lru::new(4096)) };
    static PARTIAL: RefCell<Option<Partial>> = const { RefCell::new(None) };
}

/// [`highlight`] with a per-thread cache, for renders: finished blocks are not highlighted again,
/// and a block that keeps growing (streaming) continues from its last complete line.
pub fn highlight_cached(code: &str, language: &str) -> Option<Tokens> {
    let key = content_key(&[language, code]);
    if let Some(hit) = HIGHLIGHTS.with(|c| c.borrow_mut().get(key)) {
        return hit;
    }
    let tokens = PARTIAL.with(|partial| {
        let mut partial = partial.borrow_mut();
        let mut resumed = partial.take().filter(|p| {
            p.language == language
                && code.len() >= p.done
                && code.is_char_boundary(p.done)
                && content_key(&[&code[..p.done]]) == p.prefix_key
        });
        let (mut tokens, mut state, start) = match resumed.take() {
            Some(p) => (p.tokens, p.state, p.done),
            None => (Vec::new(), SyntaxHighlighter::new(language)?, 0),
        };
        // Complete lines advance the saved state; the trailing partial line is highlighted on a copy.
        let done = code.rfind('\n').map_or(0, |i| i + 1).max(start);
        let mut offset = start;
        for line in code[start..done].split_inclusive('\n') {
            tokens.extend(
                state
                    .highlight_line(line)
                    .into_iter()
                    .map(|(r, k)| (r.start + offset..r.end + offset, k)),
            );
            offset += line.len();
        }
        let mut all = tokens.clone();
        if done < code.len() {
            all.extend(
                state
                    .clone()
                    .highlight_line(&code[done..])
                    .into_iter()
                    .map(|(r, k)| (r.start + done..r.end + done, k)),
            );
        }
        *partial = Some(Partial {
            language: language.to_string(),
            done,
            prefix_key: content_key(&[&code[..done]]),
            tokens,
            state,
        });
        Some(Rc::from(all))
    });
    HIGHLIGHTS.with(|c| c.borrow_mut().insert(key, tokens.clone()));
    tokens
}

/// The tokens of one line.
pub type LineTokens = Vec<(Range<usize>, TokenKind)>;

/// Feeds a [`CodeEditor`](crate::CodeEditor) its colors with a [`SyntaxHighlighter`]. It keeps
/// the parser state every [`SyntaxLines::STEP`] lines, so any visible line is highlighted by
/// re-parsing at most that many lines before it, and edits only drop what follows them.
pub struct SyntaxLines {
    start: SyntaxHighlighter,
    /// `checkpoints[k]`: the state before line `k * STEP`.
    checkpoints: Vec<SyntaxHighlighter>,
    cache: Vec<Option<LineTokens>>,
}

impl SyntaxLines {
    pub const STEP: usize = 16;

    pub fn new(highlighter: SyntaxHighlighter) -> Self {
        Self {
            checkpoints: vec![highlighter.clone()],
            start: highlighter,
            cache: Vec::new(),
        }
    }

    /// For a file name's extension, or `None` when there is no grammar for it.
    pub fn for_path(path: &str) -> Option<Self> {
        SyntaxHighlighter::for_path(path).map(Self::new)
    }

    /// The tokens of `line`, given a way to read any line of the text.
    pub fn tokens<'a>(
        &mut self,
        line: usize,
        text: impl Fn(usize) -> &'a str,
    ) -> Vec<(Range<usize>, TokenKind)> {
        if let Some(Some(hit)) = self.cache.get(line) {
            return hit.clone();
        }
        let k = line / Self::STEP;
        while self.checkpoints.len() <= k {
            let last = self.checkpoints.len() - 1;
            let mut hl = self.checkpoints[last].clone();
            for ix in last * Self::STEP..(last + 1) * Self::STEP {
                hl.highlight_line(text(ix));
            }
            self.checkpoints.push(hl);
        }
        let mut hl = self.checkpoints[k].clone();
        for ix in k * Self::STEP..line {
            hl.highlight_line(text(ix));
        }
        let tokens = hl.highlight_line(text(line));
        if self.cache.len() <= line {
            self.cache.resize(line + 1, None);
        }
        self.cache[line] = Some(tokens.clone());
        tokens
    }

    /// The text changed from `from_line` on.
    pub fn invalidate(&mut self, from_line: usize) {
        self.cache.truncate(from_line);
        self.checkpoints.truncate(from_line / Self::STEP + 1);
        if self.checkpoints.is_empty() {
            self.checkpoints.push(self.start.clone());
        }
    }
}

impl crate::LineHighlighter for SyntaxLines {
    fn highlight_line(
        &mut self,
        buffer: &crate::CodeBuffer,
        line: usize,
        cx: &App,
    ) -> crate::LineHighlights {
        let count = buffer.line_count();
        let palette = SyntaxPalette::of(cx);
        self.tokens(line, |ix| if ix < count { buffer.line(ix) } else { "" })
            .into_iter()
            .map(|(range, kind)| (range, palette.style(kind)))
            .collect()
    }

    fn invalidate(&mut self, from_line: usize) {
        SyntaxLines::invalidate(self, from_line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syntax_lines_match_a_straight_pass_and_survive_edits() {
        let lines: Vec<String> = (0..40)
            .map(|i| {
                if i == 20 {
                    "/* open".into()
                } else if i == 22 {
                    "close */".into()
                } else {
                    format!("let x{i} = \"s\"; // c")
                }
            })
            .collect();
        let mut straight = SyntaxHighlighter::new("rust").unwrap();
        let expected: Vec<_> = lines.iter().map(|l| straight.highlight_line(l)).collect();
        let mut sl = SyntaxLines::for_path("main.rs").unwrap();
        for ix in [35, 3, 21, 39, 0, 17] {
            assert_eq!(
                sl.tokens(ix, |i| lines[i].as_str()),
                expected[ix],
                "line {ix}"
            );
        }
        sl.invalidate(18);
        assert_eq!(sl.tokens(21, |i| lines[i].as_str()), expected[21]);
        assert_eq!(sl.tokens(2, |i| lines[i].as_str()), expected[2]);
    }

    fn kinds(line: &str, tokens: &[(Range<usize>, TokenKind)]) -> Vec<(String, TokenKind)> {
        tokens
            .iter()
            .map(|(r, k)| (line[r.clone()].to_string(), *k))
            .collect()
    }

    fn has(language: &str, code: &str, text: &str, kind: TokenKind) -> bool {
        let tokens = highlight(code, language).unwrap_or_else(|| panic!("no grammar: {language}"));
        kinds(code, &tokens)
            .iter()
            .any(|(t, k)| t.contains(text) && *k == kind)
    }

    #[test]
    fn every_listed_language_has_a_grammar() {
        for (key, label, grammar) in ALIASES {
            if grammar.is_some() {
                assert!(resolve(key).is_some(), "{key} ({label}) has no grammar");
            }
        }
    }

    #[test]
    fn labels_follow_the_reference_aliases() {
        assert_eq!(language("tsx").label, "TSX");
        assert_eq!(language("ts title=x.ts").label, "TypeScript");
        assert_eq!(language("").label, "Text");
        assert!(!language("console").highlighted);
        assert_eq!(language("brainfork").label, "brainfork");
        assert!(!language("brainfork").highlighted);
        // syntect's own languages work too, under the fence word.
        assert!(language("go").highlighted);
    }

    #[test]
    fn rust_tokens() {
        let code = "// note\nfn main() { let x = 42; println!(\"hi\"); }";
        assert!(has("rust", code, "// note", TokenKind::Comment));
        assert!(has("rust", code, "fn", TokenKind::Keyword));
        assert!(has("rust", code, "main", TokenKind::Function));
        assert!(has("rust", code, "42", TokenKind::Number));
        assert!(has("rust", code, "\"hi\"", TokenKind::String));
    }

    #[test]
    fn typescript_tokens() {
        let code = "interface User { name: string }\nconst n: number = 3; greet(`hi ${n}`); // ok";
        assert!(has("ts", code, "interface", TokenKind::Keyword));
        assert!(has("ts", code, "User", TokenKind::Type));
        assert!(has("ts", code, "greet", TokenKind::Function));
        assert!(has("ts", code, "`hi ", TokenKind::String));
        assert!(has("ts", code, "// ok", TokenKind::Comment));
        let tokens = highlight(code, "ts").unwrap();
        // The interpolated variable is not part of the string.
        assert!(
            !kinds(code, &tokens)
                .iter()
                .any(|(t, k)| t.contains("${n}") && *k == TokenKind::String)
        );
    }

    #[test]
    fn toml_and_powershell_tokens() {
        let toml = "[package]\nname = \"x\" # c\nversion = 2";
        assert!(has("toml", toml, "package", TokenKind::Heading));
        assert!(has("toml", toml, "name", TokenKind::Meta));
        assert!(has("toml", toml, "\"x\"", TokenKind::String));
        assert!(has("toml", toml, "2", TokenKind::Number));
        let ps = "Get-ChildItem -Path $env:HOME | Where-Object { $_.Length -gt 10 } # c";
        assert!(has("powershell", ps, "Get-ChildItem", TokenKind::Function));
        assert!(has("powershell", ps, "-gt", TokenKind::Keyword));
        assert!(has("powershell", ps, "$env:HOME", TokenKind::Meta));
        assert!(has("powershell", ps, "# c", TokenKind::Comment));
    }

    #[test]
    fn other_languages_produce_tokens() {
        for (language, code) in [
            ("js", "function f() { return 'x' }"),
            ("json", "{\"a\": 1, \"b\": true}"),
            ("md", "# Title\n[link](https://x.y)"),
            ("css", ".a { color: red; }"),
            ("html", "<div class=\"a\">x</div>"),
            ("py", "def f():\n    return \"x\"  # c"),
            ("yaml", "a: 1\nb: \"x\""),
            ("bash", "echo \"hi\" # c"),
        ] {
            let tokens = highlight(code, language).unwrap();
            assert!(!tokens.is_empty(), "{language} produced no tokens");
        }
        assert!(has("md", "# Title", "Title", TokenKind::Heading));
        assert!(has("py", "def f():\n  pass", "def", TokenKind::Keyword));
    }

    #[test]
    fn state_carries_across_lines() {
        let mut hl = SyntaxHighlighter::new("rust").unwrap();
        hl.highlight_line("/* open");
        let tokens = hl.highlight_line("still comment */ fn");
        assert_eq!(tokens[0].1, TokenKind::Comment);
        assert_eq!(tokens.last().unwrap().1, TokenKind::Keyword);
    }

    #[test]
    fn ranges_stay_inside_the_line_with_crlf() {
        let code = "let a = 1;\r\nlet b = \"x\";\r\n";
        let tokens = highlight(code, "rust").unwrap();
        for (r, _) in &tokens {
            assert!(!code[r.clone()].contains('\r') && !code[r.clone()].contains('\n'));
        }
    }

    #[test]
    fn cached_streaming_matches_a_full_pass() {
        let full = "fn a() {\n    \"one\";\n}\n/* two\nlines */ let x = 3;";
        for end in [5, 12, 20, 30, full.len()] {
            let part = &full[..end];
            let cached = highlight_cached(part, "rust").unwrap();
            let direct = highlight(part, "rust").unwrap();
            assert_eq!(&*cached, direct.as_slice(), "prefix of {end} bytes");
        }
    }

    #[test]
    fn for_path_uses_the_extension() {
        assert!(SyntaxHighlighter::for_path("src/main.rs").is_some());
        assert!(SyntaxHighlighter::for_path("C:\\x\\Cargo.toml").is_some());
        assert!(SyntaxHighlighter::for_path("README").is_none());
    }
}
