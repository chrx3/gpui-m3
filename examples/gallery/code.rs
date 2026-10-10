//! "Code and Markdown" section: diffs, tool output, highlighted code and Markdown.

use std::time::Duration;

use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window,
    div, px,
};
use gpui_m3::{
    Button, ButtonSize, CodeBlock, CodeOutput, DiffBar, DiffView, Markdown, Theme, split_path_line,
};

use super::{column, demo, page, row};

const OLD_FILE: &str = r#"use gpui::{App, Window};

/// Settings of the panel.
pub struct Panel {
    width: f32,
    open: bool,
}

impl Panel {
    pub fn new() -> Self {
        Self { width: 280., open: true }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn resize(&mut self, width: f32) {
        self.width = width;
    }
}
"#;

const NEW_FILE: &str = r#"use gpui::{App, Pixels, Window, px};

/// Settings of the panel.
pub struct Panel {
    width: Pixels,
    open: bool,
}

impl Panel {
    pub fn new() -> Self {
        Self { width: px(280.), open: true }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn width(&self) -> Pixels {
        self.width
    }

    pub fn resize(&mut self, width: Pixels) {
        self.width = width.max(px(200.));
    }
}
"#;

const WRITTEN: &str = "[package]\nname = \"atic-code\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\ngpui = \"0.2.2\"\ngpui-m3 = { path = \"../gpui-m3\" }\n";

const TS: &str = r#"interface Message {
  id: string;
  text: string;
  streaming?: boolean;
}

/** Joins streamed chunks into one message. */
export function append(msg: Message, chunk: string): Message {
  return { ...msg, text: `${msg.text}${chunk}` }; // keeps the id, and this comment is long enough to scroll sideways
}
"#;

const POWERSHELL: &str = r#"# Rebuild the sidecar and copy it next to the app
$target = Join-Path $PSScriptRoot "bin"
if (-not (Test-Path $target)) { New-Item -ItemType Directory $target | Out-Null }
Get-ChildItem -Path dist -Filter *.exe | Where-Object { $_.Length -gt 1MB } |
    Copy-Item -Destination $target -Force
"#;

const MARKDOWN: &str = r#"## Theme migration

I moved the panel width to **`Pixels`** and kept the *old* API working. Changes are in `src/panel.rs:12` and `src/theme.rs`; see the [GPUI docs](https://gpui.rs) or https://github.com/zed-industries/zed.

### What changed

1. `Panel::width` returns `Pixels`
2. `resize` clamps to a minimum:
   - 200 px on desktop
   - ~~150 px~~ dropped for now
3. Tests updated

- [x] Port the struct
- [ ] Update the docs

> **Note:** the old `f32` setters still compile, but they are deprecated.

| File | Lines | Status |
|:-----|------:|:------:|
| `src/panel.rs` | +4 −4 | done |
| `src/theme.rs` | +12 −1 | review |

---

```rust
pub fn resize(&mut self, width: Pixels) {
    self.width = width.max(px(200.));
}
```

![Screenshot of the panel](panel.png)
"#;

const OUTPUT: &str = "   Compiling gpui-m3 v0.1.0\n   Compiling atic-code v0.1.0\nwarning: unused variable: `window`\n  --> src/code/view.rs:405:21\n    |\n405 |     fn render(self, window: &mut Window) {\n    |                     ^^^^^^ help: prefix it with an underscore\n\nwarning: `atic-code` generated 1 warning\n    Finished `dev` profile [optimized + debuginfo] target(s) in 41.2s\n     Running `target/debug/atic-code.exe`\nlistening on 127.0.0.1:4321\nready in 312 ms\n";

/// Demos of the code components, in their own view (the streaming demo re-renders only this).
pub struct CodeDemos {
    streamed: usize,
    streaming: bool,
    clicked: SharedString,
}

impl CodeDemos {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            streamed: MARKDOWN.len(),
            streaming: false,
            clicked: SharedString::default(),
        }
    }

    /// Replays the Markdown sample a few characters at a time, like a model's reply.
    fn stream(&mut self, cx: &mut Context<Self>) {
        if self.streaming {
            return;
        }
        self.streaming = true;
        self.streamed = 0;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let done = this
                    .update(cx, |this, cx| {
                        let mut next = (this.streamed + 9).min(MARKDOWN.len());
                        while !MARKDOWN.is_char_boundary(next) {
                            next += 1;
                        }
                        this.streamed = next;
                        this.streaming = next < MARKDOWN.len();
                        cx.notify();
                        !this.streaming
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        })
        .detach();
    }

    /// Callback that shows what was clicked.
    fn clicked(&self, cx: &mut Context<Self>) -> impl Fn(&str, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |what: &str, _, cx| {
            let label: SharedString = what.to_string().into();
            this.update(cx, |this, cx| {
                this.clicked = label;
                cx.notify();
            })
            .ok();
        }
    }
}

impl Render for CodeDemos {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let on_link = self.clicked(cx);
        let path_click = self.clicked(cx);
        let on_path = move |path: &str, window: &mut Window, cx: &mut App| {
            let (file, lines) = split_path_line(path);
            let label = match lines {
                Some((start, Some(end))) => format!("{file} (lines {start}–{end})"),
                Some((start, None)) => format!("{file} (line {start})"),
                None => file.to_string(),
            };
            path_click(&label, window, cx)
        };
        let clicked = if self.clicked.is_empty() {
            "Click a link or a path in code".to_string()
        } else {
            format!("Clicked: {}", self.clicked)
        };
        let entity = cx.entity().downgrade();

        let markdown: AnyElement = div()
            .max_w(px(720.))
            .child(
                Markdown::new("md", MARKDOWN[..self.streamed].to_string())
                    .on_link(on_link)
                    .on_path(on_path),
            )
            .into_any_element();

        page()
            .child(demo(
                "Markdown",
                "GFM with headings, nested lists, tasks, quotes, tables, links, paths in code and code blocks.",
                column()
                    .child(
                        row()
                            .child(
                                Button::new("stream", "Stream it again")
                                    .size(ButtonSize::Small)
                                    .on_click(move |_, _, cx| {
                                        entity.update(cx, |this, cx| this.stream(cx)).ok();
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(s.on_surface_variant)
                                    .child(clicked),
                            ),
                    )
                    .child(markdown),
            ))
            .child(demo(
                "Diff",
                "A real line diff (similar): old and new numbers, unchanged runs folded (click to open) and a header with +/−.",
                column()
                    .max_w(px(720.))
                    .child(DiffView::new("diff", OLD_FILE, NEW_FILE).path("src/panel.rs"))
                    .child(
                        DiffView::new("diff-new", "", WRITTEN)
                            .path("Cargo.toml")
                            .max_rows(5),
                    ),
            ))
            .child(demo(
                "DiffBar",
                "Five blocks with the share of added and removed lines.",
                row()
                    .child(DiffBar::new(12, 0))
                    .child(DiffBar::new(30, 10))
                    .child(DiffBar::new(5, 5))
                    .child(DiffBar::new(1, 20))
                    .child(DiffBar::new(0, 0)),
            ))
            .child(demo(
                "CodeOutput",
                "Command output clipped to a few lines with Show all; errors in red.",
                column()
                    .max_w(px(720.))
                    .p(px(10.))
                    .rounded(px(16.))
                    .bg(s.surface_container_high)
                    .child(CodeOutput::new("out", OUTPUT).prefix("$").max_lines(6))
                    .child(
                        CodeOutput::new(
                            "out-err",
                            "error[E0425]: cannot find value `widht` in this scope\n  --> src/panel.rs:24:20",
                        )
                        .error(true),
                    ),
            ))
            .child(demo(
                "CodeBlock",
                "Language name, Copy → Copied for 1.4 s, syntax colors from the theme and horizontal scroll.",
                column()
                    .max_w(px(720.))
                    .child(CodeBlock::new("code-ts", TS).language("ts"))
                    .child(CodeBlock::new("code-ps", POWERSHELL).language("powershell"))
                    .child(CodeBlock::new("code-toml", WRITTEN).language("toml")),
            ))
    }
}
