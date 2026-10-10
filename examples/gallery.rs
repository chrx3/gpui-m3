//! Gallery for trying out each component and comparing it with the reference design.
//!
//! `cargo run --example gallery`

use std::sync::Arc;

use gpui::{
    AnyElement, AnyView, App, AppContext, Application, Bounds, ClickEvent, Context, ElementId, Entity,
    ExternalPaths, FontStyle, FontWeight, HighlightStyle, Hsla, InteractiveElement, IntoElement,
    KeyBinding, MouseButton, ParentElement, PathBuilder, Pixels, Point, Render, RenderImage, Rgba,
    SharedString, StatefulInteractiveElement, StyleRefinement, Styled, TitlebarOptions, Window,
    WindowAppearance, WindowBounds, WindowOptions, actions, anchored, canvas, deferred, div, point,
    prelude::FluentBuilder, px, size,
};
use gpui_m3::{
    Avatar, Badge, Banner, Bubble, BubbleKind, Button, ButtonSize, ButtonVariant, Card, Chip,
    ChipSize, ChoiceRow, CodeBuffer, CodeEditor, CodeEditorEvent, ColorSwatches, Command,
    CommandPalette, CommandPaletteEvent, Dialog, DividerLabel, DotShape, DropZone, Exit,
    ExpandableCard, Fab, FavStar, HsvPicker, Icon, IconButton, IconTab, IconTabs, ImageCropper,
    ImageCropperEvent, ImageThumb, Kbd, KeyCombo, KeyNames, LineHighlighter, LineHighlights, LinearProgress, ListGroup,
    ListItem, LoadingIndicator, Menu, MenuItem, MorphDot, MotionSettings, NavItem, PopDirection,
    Popover, Presence, RailItem, ReorderEvent, ReorderList, Ring, Segment, SegmentedButtons,
    SelectCard, SettingRow, Shape, ShapeName, Slider, SplitButton, Splitter, Spring, StopSlider, Suggestion,
    SuggestionAccept, SuggestionDismiss, SuggestionList, SuggestionNav, SuggestionNext,
    SuggestionPrevious, Switch, Tab, TabStrip, TextArea, TextAreaEvent, TextField, TextFieldEvent,
    Theme, ThemeReveal, Toast, Tone, TreeRow, WavyProgress, animate_theme, apply_reorder,
    context_menu,
    motion::{animate_color, animate_f32, key, pulse},
    rank_mentions,
    theme::{FONT_FAMILY, alpha, hex},
    theme_reveal,
};

#[path = "gallery/code.rs"]
mod code;

const SECTIONS: [(&str, &str); 16] = [
    ("Buttons", "bolt"),
    ("Selection", "check"),
    ("Navigation", "panel-left"),
    ("Menus", "layers"),
    ("Shapes", "spark"),
    ("Progress", "gauge"),
    ("Surfaces", "blocks"),
    ("Text and feedback", "chat"),
    ("Composer", "up"),
    ("Conversation", "diff"),
    ("Panels", "panel-right"),
    ("Appearance", "palette"),
    ("Attachments and notices", "clip"),
    ("Code and Markdown", "terminal"),
    ("Motion", "refresh"),
    ("Editor and layout", "terminal"),
];

/// the reference design's seed colors for M3 Expressive; the first one is the base scheme.
const SEEDS: [&str; 9] = [
    "#6750a4", "#0b57d0", "#006a6a", "#386a20", "#7d5700", "#b3261e", "#984061", "#5b5f97",
    "#00639b",
];

const MODELS: [&str; 3] = ["Opus 4.1", "Sonnet 4.5", "Haiku 4.5"];

const EFFORTS: [&str; 5] = ["Low", "Medium", "High", "Very high", "Max"];

/// Project files for the @-mention demo.
const MENTION_FILES: [&str; 12] = [
    "src/components/Agent.tsx",
    "src/components/Composer.tsx",
    "src/components/chat/Thread.tsx",
    "src/components/Profile.tsx",
    "src/components/Sidebar.tsx",
    "src/components/ui.tsx",
    "src/lib/motion.ts",
    "src/lib/shapes.ts",
    "src/styles/chat.css",
    "src/styles/expressive.css",
    "src/styles/motion.css",
    "README.md",
];

actions!(gallery, [FocusNext, FocusPrevious]);

/// A small generated "photo" (sky, sun and hills) for the image demos, in GPUI's BGRA order.
fn demo_photo(hue: f32) -> Arc<RenderImage> {
    demo_photo_sized(hue, 128)
}

fn demo_photo_sized(hue: f32, n: usize) -> Arc<RenderImage> {
    let mut data = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 / n as f32, y as f32 / n as f32);
            let hill = 0.68 + 0.08 * (fx * 7.).sin();
            let sun = ((fx - 0.68).powi(2) + (fy - 0.32).powi(2)).sqrt() < 0.14;
            let color = if fy > hill {
                gpui::hsla((hue + 0.25) % 1., 0.45, 0.32 + 0.1 * (1. - fy), 1.)
            } else if sun {
                gpui::hsla((hue + 0.12) % 1., 0.9, 0.78, 1.)
            } else {
                gpui::hsla(hue, 0.55, 0.45 + 0.3 * fy, 1.)
            };
            let c = Rgba::from(color);
            let o = (y * n + x) * 4;
            data[o] = (c.b * 255.) as u8;
            data[o + 1] = (c.g * 255.) as u8;
            data[o + 2] = (c.r * 255.) as u8;
            data[o + 3] = 255;
        }
    }
    let buffer = image::RgbaImage::from_raw(n as u32, n as u32, data).unwrap();
    Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]))
}

/// Window root: sidebar and toolbar. The content lives in a separate cached view, so the
/// logo and toolbar animations do not rebuild it.
struct Gallery {
    dark: bool,
    seed: usize,
    slow: bool,
    demos: Entity<Demos>,
    logo: Entity<Logo>,
    /// Focused on open so Tab (bound to `FocusNext`) has a dispatch path to the root.
    focus: gpui::FocusHandle,
}

/// Content of the current section and the state of its demos.
struct Demos {
    section: usize,
    code_demos: Entity<code::CodeDemos>,
    toggles: [bool; 4],
    icon_toggle: bool,
    switches: [bool; 3],
    keys_capturing: bool,
    segment: usize,
    icon_segment: usize,
    effort: usize,
    default_effort: Option<usize>,
    filters: [bool; 3],
    attachments: Vec<&'static str>,
    model: usize,
    split_open: bool,
    nav: usize,
    rail: usize,
    tab: usize,
    menu_open: bool,
    menu_switch: bool,
    menu_effort: usize,
    progress: f32,
    ring: f32,
    motion_flip: bool,
    search: Entity<TextField>,
    filter: Entity<TextField>,
    agent_tab: usize,
    agent_sub: bool,
    agent_model: usize,
    agent_highlight: usize,
    launches: u64,
    mode: usize,
    agents_live: bool,
    usage_open: bool,
    answer: usize,
    answers: [bool; 3],
    tool_open: bool,
    thinking_open: bool,
    permission_shown: bool,
    tree_open: bool,
    tree_selected: usize,
    tabs: Vec<&'static str>,
    tab_selected: SharedString,
    zoom: f32,
    nav_unread: bool,
    deleted: usize,
    dot_key: u64,
    palette: Entity<CommandPalette>,
    palette_open: bool,
    palette_ran: SharedString,
    favorites: [bool; 3],
    style_choice: usize,
    accent: Hsla,
    picker_open: bool,
    filled_field: Entity<TextField>,
    outlined_field: Entity<TextField>,
    rename_field: Entity<TextField>,
    name: Entity<TextField>,
    submitted: SharedString,
    dialog_open: bool,
    toast_open: bool,
    context_menu: Option<Point<Pixels>>,
    composer: Entity<TextArea>,
    sent: SharedString,
    replay_key: u64,
    offer_shown: bool,
    notice_level: usize,
    last_notice_level: usize,
    presence_on: bool,
    dropped: SharedString,
    photos: Vec<Arc<RenderImage>>,
    thumbs: Vec<usize>,
    mention_input: Entity<TextArea>,
    mention_matches: Vec<&'static str>,
    mention_query: SharedString,
    mention_at: Option<usize>,
    mention_nav: SuggestionNav,
    editor: Entity<CodeEditor>,
    editor_saves: usize,
    side_width: Pixels,
    term_height: Pixels,
    open_files: Vec<&'static str>,
    favorite_projects: Vec<&'static str>,
    other_projects: Vec<&'static str>,
    cropper: Option<Entity<ImageCropper>>,
    cropped: Option<Arc<RenderImage>>,
    pop_key: u64,
    stagger_key: u64,
    accent_step: usize,
    rewound: SharedString,
    long_card_open: bool,
}

/// Rust-ish colors for the code editor demo, read from the theme for every line it is asked for.
struct DemoColors;

const KEYWORDS: [&str; 22] = [
    "fn", "let", "mut", "pub", "struct", "impl", "use", "mod", "for", "in", "if", "else", "match",
    "return", "self", "Self", "const", "enum", "while", "loop", "as", "where",
];

impl LineHighlighter for DemoColors {
    fn highlight_line(&mut self, buffer: &CodeBuffer, line: usize, cx: &App) -> LineHighlights {
        let s = Theme::of(cx);
        let text = buffer.line(line);
        let style = |color: Hsla| HighlightStyle {
            color: Some(color),
            ..Default::default()
        };
        let mut out = Vec::new();
        let code_end = match text.find("//") {
            Some(i) => {
                out.push((
                    i..text.len(),
                    HighlightStyle {
                        color: Some(s.outline),
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    },
                ));
                i
            }
            None => text.len(),
        };
        let code = &text[..code_end];
        let mut chars = code.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            if c == '"' {
                let mut end = code.len();
                for (j, d) in chars.by_ref() {
                    if d == '"' {
                        end = j + 1;
                        break;
                    }
                }
                out.push((i..end, style(s.success)));
            } else if c.is_ascii_digit() {
                let mut end = i + 1;
                while let Some((j, d)) = chars.peek().copied() {
                    if !d.is_ascii_alphanumeric() && d != '.' && d != '_' {
                        break;
                    }
                    end = j + d.len_utf8();
                    chars.next();
                }
                out.push((i..end, style(s.warning)));
            } else if c.is_alphabetic() || c == '_' {
                let mut end = i + c.len_utf8();
                while let Some((j, d)) = chars.peek().copied() {
                    if !d.is_alphanumeric() && d != '_' {
                        break;
                    }
                    end = j + d.len_utf8();
                    chars.next();
                }
                let word = &code[i..end];
                if KEYWORDS.contains(&word) {
                    out.push((
                        i..end,
                        HighlightStyle {
                            color: Some(s.primary),
                            font_weight: Some(FontWeight(700.)),
                            ..Default::default()
                        },
                    ));
                } else if word.starts_with(char::is_uppercase) {
                    out.push((i..end, style(s.tertiary)));
                }
            }
        }
        out
    }
}

/// A long generated Rust file, to show that only the visible lines are laid out.
fn demo_source() -> String {
    let mut out = String::from(
        "//! Generated demo: scroll, select, type, Tab / Shift+Tab, Ctrl+Z, Ctrl+S.\n\nuse std::collections::HashMap;\n\n",
    );
    for i in 0..400 {
        out.push_str(&format!(
            "/// Spring number {i}.\npub fn spring_{i}(stiffness: f32, damping: f32) -> Spring {{\n    let ratio = damping / (2.0 * stiffness.sqrt());\n    // Bouncy when the ratio is below 1.\n    Spring {{ stiffness, damping_ratio: ratio + {i}.0 * 0.001, label: \"spring {i}\" }}\n}}\n\n",
        ));
    }
    out
}

/// The animated logo, in its own view: its continuous animation does not rebuild the content.
struct Logo;

impl Render for Logo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        Shape::new(ShapeName::Cookie9)
            .size(px(30.))
            .color(Theme::of(cx).primary)
            .breathe(ShapeName::Clover4, 7.)
            .spin(40.)
    }
}

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        Self {
            dark,
            seed: 0,
            slow: false,
            demos: cx.new(Demos::new),
            logo: cx.new(|_| Logo),
            focus: cx.focus_handle(),
        }
    }

    fn seed_color(&self) -> Option<Hsla> {
        (self.seed != 0).then(|| hex(SEEDS[self.seed]))
    }

    fn apply_theme(&self, cx: &mut App) {
        Theme::set(self.dark, self.seed_color(), cx);
    }

    fn demo_url(&self) -> String {
        let mode = if self.dark { "dark" } else { "light" };
        let accent = if self.seed == 0 {
            String::new()
        } else {
            format!("&accent={}", &SEEDS[self.seed][1..])
        };
        format!("http://localhost:1420/?style=expressive&mode={mode}{accent}&demo&chat")
    }
}

impl Demos {
    fn new(cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            TextField::new(cx)
                .placeholder("Search chats")
                .icon("search")
        });
        let name = cx.new(|cx| TextField::new(cx).placeholder("Project name"));
        for field in [&search, &name] {
            cx.subscribe(field, |this: &mut Demos, _, event: &TextFieldEvent, cx| {
                if let TextFieldEvent::Submitted(text) = event {
                    this.submitted = text.clone();
                    cx.notify();
                }
            })
            .detach();
        }
        let palette = cx.new(|cx| {
            let mut palette =
                CommandPalette::new(cx).placeholder("Search for a command or a chat…", cx);
            palette.set_commands(
                vec![
                    Command::new("plus", "New conversation").hint("Ctrl+N"),
                    Command::new("chat", "New chat without a project"),
                    Command::new("folder-open", "Open workspace…").hint("Ctrl+O"),
                    Command::new("history", "Conversation history"),
                    Command::new("gear", "Settings").hint("Ctrl+,"),
                    Command::new("spark", "Theme migration").hint("Conversation"),
                ],
                cx,
            );
            palette
        });
        cx.subscribe(
            &palette,
            |this: &mut Demos, _, event: &CommandPaletteEvent, cx| {
                if let CommandPaletteEvent::Run(i) = event {
                    this.palette_ran = format!("Ran command #{i}").into();
                }
                this.palette_open = false;
                cx.notify();
            },
        )
        .detach();
        let filter = cx.new(|cx| {
            TextField::new(cx)
                .compact()
                .icon("search")
                .placeholder("Filter actions or commands…")
        });
        let composer = cx.new(|cx| {
            TextArea::new(cx)
                .placeholder("Ask anything · @ to mention · / for actions")
                .min_rows(2)
                .max_height(px(240.))
        });
        cx.subscribe(
            &composer,
            |this: &mut Demos, composer, event: &TextAreaEvent, cx| {
                if let TextAreaEvent::Submit(text) = event {
                    this.sent = text.clone();
                    composer.update(cx, |composer, cx| composer.clear(cx));
                    cx.notify();
                }
            },
        )
        .detach();
        let mention_input = cx.new(|cx| {
            TextArea::new(cx)
                .placeholder("Mention a file with @…")
                .min_rows(2)
                .max_height(px(160.))
        });
        cx.subscribe(
            &mention_input,
            |this: &mut Demos, _, event: &TextAreaEvent, cx| {
                if let TextAreaEvent::Changed(_) = event {
                    this.refresh_mentions(cx);
                }
            },
        )
        .detach();
        let editor = cx.new(|cx| {
            CodeEditor::new(cx)
                .path("src/springs.rs")
                .with_text(&demo_source())
                .highlighter(DemoColors)
        });
        cx.subscribe(
            &editor,
            |this: &mut Demos, editor, event: &CodeEditorEvent, cx| {
                if *event == CodeEditorEvent::Save {
                    this.editor_saves += 1;
                    editor.update(cx, |e, cx| e.mark_saved(cx));
                }
                cx.notify();
            },
        )
        .detach();
        // Syntax colors come from the theme: ask for them again when it changes.
        let themed = editor.clone();
        cx.observe_global::<Theme>(move |_, cx| {
            themed.update(cx, |e, cx| e.refresh_highlights(cx));
        })
        .detach();
        Self {
            editor,
            editor_saves: 0,
            side_width: px(200.),
            term_height: px(140.),
            open_files: vec!["springs.rs", "theme.rs", "motion.rs", "Cargo.toml"],
            favorite_projects: vec!["gpui-m3", "atic"],
            other_projects: vec!["demo", "zed", "material-colors"],
            cropper: None,
            cropped: None,
            pop_key: 0,
            stagger_key: 0,
            accent_step: 0,
            rewound: SharedString::default(),
            long_card_open: false,
            composer,
            sent: SharedString::default(),
            replay_key: 0,
            offer_shown: true,
            notice_level: 1,
            last_notice_level: 1,
            presence_on: true,
            dropped: SharedString::default(),
            photos: vec![demo_photo(0.58), demo_photo(0.05), demo_photo(0.75)],
            thumbs: vec![0, 1, 2],
            mention_input,
            mention_matches: Vec::new(),
            mention_query: SharedString::default(),
            mention_at: None,
            mention_nav: SuggestionNav::default(),
            search,
            filter,
            agent_tab: 0,
            agent_sub: false,
            agent_model: 0,
            agent_highlight: 0,
            launches: 0,
            mode: 0,
            agents_live: true,
            usage_open: false,
            answer: 0,
            answers: [true, false, false],
            tool_open: true,
            thinking_open: false,
            permission_shown: true,
            tree_open: true,
            tree_selected: 2,
            tabs: vec!["main.rs", "theme.rs", "Cargo.toml"],
            tab_selected: "theme.rs".into(),
            zoom: 1.6,
            nav_unread: true,
            deleted: 0,
            dot_key: 0,
            palette,
            palette_open: false,
            palette_ran: SharedString::default(),
            favorites: [true, false, false],
            style_choice: 1,
            accent: hex("#6750a4"),
            picker_open: false,
            filled_field: cx.new(|cx| {
                TextField::new(cx)
                    .filled()
                    .label("Name")
                    .placeholder("Your name")
            }),
            outlined_field: cx.new(|cx| {
                TextField::new(cx)
                    .outlined()
                    .label("Name")
                    .placeholder("My project")
            }),
            rename_field: cx.new(|cx| {
                let mut field = TextField::new(cx)
                    .inline()
                    .select_all_on_focus(true)
                    .restore_on_cancel(true);
                field.set_text("Theme migration", cx);
                field
            }),
            name,
            submitted: SharedString::default(),
            dialog_open: false,
            toast_open: false,
            context_menu: None,
            section: 0,
            code_demos: cx.new(code::CodeDemos::new),
            toggles: [true, false, false, false],
            icon_toggle: true,
            switches: [true, false, true],
            keys_capturing: false,
            segment: 1,
            icon_segment: 0,
            effort: 2,
            default_effort: None,
            filters: [true, false, false],
            attachments: vec!["theme.rs", "screenshot.png", "notes.md"],
            model: 0,
            split_open: false,
            nav: 1,
            rail: 0,
            tab: 0,
            menu_open: false,
            menu_switch: true,
            menu_effort: 1,
            progress: 0.42,
            ring: 0.65,
            motion_flip: false,
        }
    }
}

/// Callback that updates a view and redraws it.
fn update<V: 'static, T: 'static, F: Fn(&mut V, T, &mut App) + 'static>(
    cx: &Context<V>,
    f: F,
) -> impl Fn(T, &mut Window, &mut App) + use<V, T, F> {
    let view = cx.entity().downgrade();
    move |value, _, cx| {
        view.update(cx, |this, cx| {
            f(this, value, cx);
            cx.notify();
        })
        .ok();
    }
}

/// Click callback that updates a view.
fn click<V: 'static, F: Fn(&mut V, &mut App) + 'static>(
    cx: &Context<V>,
    f: F,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + use<V, F> {
    let set = update(cx, move |this, _: (), cx| f(this, cx));
    move |_, window, cx| set((), window, cx)
}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        // The mode can also change from the motion demos (theme reveal).
        self.dark = Theme::get(cx).dark;
        let section = self.demos.read(cx).section;

        let mut nav = div().flex().flex_col().gap(px(2.));
        for (i, (name, icon)) in SECTIONS.iter().enumerate() {
            let demos = self.demos.clone();
            nav = nav.child(
                NavItem::new(("section", i), *name)
                    .icon(*icon)
                    .selected(section == i)
                    .on_click(move |_, _, cx| {
                        demos.update(cx, |d, cx| {
                            d.section = i;
                            cx.notify();
                        })
                    }),
            );
        }

        let sidebar = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(244.))
            .h_full()
            .px(px(12.))
            .pt(px(18.))
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(8.))
                    .pb(px(10.))
                    .child(self.logo.clone())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight(700.))
                                    .child("M3 Expressive"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(s.on_surface_variant)
                                    .child("GPUI · the reference design components"),
                            ),
                    ),
            )
            .child(
                Fab::new("fab-demo", "export")
                    .label("Open in the reference design")
                    .on_click(click(cx, |this: &mut Gallery, cx| {
                        cx.open_url(&this.demo_url())
                    })),
            )
            .child(div().h(px(6.)))
            .child(
                div()
                    .px(px(12.))
                    .pb(px(4.))
                    .text_size(px(12.))
                    .font_weight(FontWeight(700.))
                    .text_color(s.primary)
                    .child("Components"),
            )
            .child(nav);

        let panel = div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .min_w_0()
            .rounded(px(28.))
            .bg(s.surface)
            .overflow_hidden()
            .child(self.toolbar(section, cx))
            .child(div().flex_1().min_h_0().child(
                AnyView::from(self.demos.clone()).cached(StyleRefinement::default().size_full()),
            ));

        div()
            .size_full()
            .flex()
            .key_context("Gallery")
            .track_focus(&self.focus)
            .on_action(|_: &FocusNext, window, _| window.focus_next())
            .on_action(|_: &FocusPrevious, window, _| window.focus_prev())
            .bg(s.surface_container)
            .text_color(s.on_surface)
            .font_family(FONT_FAMILY)
            .text_size(px(13.5))
            .pr(px(8.))
            .pb(px(8.))
            .pt(px(8.))
            .child(sidebar)
            .child(panel)
            .child(ThemeReveal::new())
    }
}

impl Render for Demos {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.section {
            0 => self.buttons(cx),
            1 => self.selection(cx),
            2 => self.navigation(cx),
            3 => self.menus(cx),
            4 => self.shapes(cx),
            5 => self.progress(cx),
            6 => self.surfaces(cx),
            7 => self.inputs(window, cx),
            8 => self.composer(cx),
            9 => self.conversation(window, cx),
            10 => self.panels(cx),
            11 => self.appearance(cx),
            12 => self.attachments(window, cx),
            13 => self.code_demos.clone().into_any_element(),
            14 => self.motion(window, cx),
            _ => self.editor_section(window, cx),
        };
        div()
            .id(("content", self.section))
            .size_full()
            .overflow_y_scroll()
            .px(px(28.))
            .pb(px(40.))
            .child(content)
    }
}

impl Gallery {
    fn toolbar(&self, section: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let mut swatches = div().flex().items_center().gap(px(6.));
        for (i, seed) in SEEDS.iter().enumerate() {
            let color = hex(seed);
            let on = self.seed == i;
            swatches = swatches.child(
                div()
                    .id(("seed", i))
                    .size(px(22.))
                    .rounded(px(11.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_2()
                    .border_color(if on {
                        s.on_surface
                    } else {
                        alpha(s.on_surface, 0.)
                    })
                    .cursor_pointer()
                    .child(div().size(px(14.)).rounded(px(7.)).bg(color))
                    .on_click(click(cx, move |this, cx| {
                        this.seed = i;
                        animate_theme(this.dark, this.seed_color(), cx);
                    })),
            );
        }
        div()
            .flex()
            .items_center()
            .gap(px(14.))
            .h(px(64.))
            .px(px(28.))
            .flex_none()
            .child(
                div()
                    .flex_1()
                    .text_size(px(22.))
                    .font_weight(FontWeight(700.))
                    .child(SECTIONS[section].0),
            )
            .child(swatches)
            .child(
                SegmentedButtons::new(
                    "mode",
                    [Segment::icon("sun"), Segment::icon("moon")],
                    if self.dark { 1 } else { 0 },
                )
                .on_change({
                    // Circular reveal from the pointer, like the reference design's M3 theme switch.
                    let view = cx.entity().downgrade();
                    move |i: usize, window, cx| {
                        view.update(cx, |this, cx| {
                            this.dark = i == 1;
                            let scheme = Theme::new(this.dark, this.seed_color()).scheme;
                            theme_reveal(scheme, this.dark, window, cx);
                        })
                        .ok();
                    }
                }),
            )
            .child(
                SegmentedButtons::new(
                    "speed",
                    [Segment::label("1×"), Segment::label("Slow ×5")],
                    if self.slow { 1 } else { 0 },
                )
                .on_change(update(cx, |this, i: usize, cx| {
                    this.slow = i == 1;
                    cx.set_global(MotionSettings {
                        time_scale: if this.slow { 5. } else { 1. },
                        reduced: false,
                    });
                })),
            )
    }
}

impl Demos {
    fn buttons(&self, cx: &mut Context<Self>) -> AnyElement {
        let labels = ["Files", "Changes", "Terminal", "History"];
        let mut toggles = row();
        for (i, label) in labels.iter().enumerate() {
            let mut toggle = Button::new(("toggle", i), *label)
                .selected(self.toggles[i])
                .on_click(click(cx, move |this, _| this.toggles[i] = !this.toggles[i]));
            if i == 1 {
                toggle = toggle.trailing(Badge::new("98"));
            }
            toggles = toggles.child(toggle);
        }
        page()
            .child(demo(
                "Common buttons",
                "A pill that tightens its corners to 8 px when pressed, using the fast spatial spring (.btn).",
                column()
                    .child(
                        row()
                            .child(Button::new("b-filled", "Update").filled())
                            .child(Button::new("b-tonal", "Tonal"))
                            .child(Button::new("b-outlined", "Outlined").outlined())
                            .child(Button::new("b-text", "Text").text())
                            .child(Button::new("b-icon", "New project").filled().icon("folder-plus"))
                            .child(Button::new("b-disabled", "Disabled").filled().disabled(true)),
                    )
                    .child(
                        row()
                            .child(Button::new("b-small", "Small").size(ButtonSize::Small))
                            .child(Button::new("b-medium", "Medium").size(ButtonSize::Medium))
                            .child(Button::new("b-large", "Large").filled().size(ButtonSize::Large))
                            .child(Button::new("b-large-t", "Large tonal").size(ButtonSize::Large)),
                    ),
            ))
            .child(demo(
                "Toggle buttons",
                "Round; when selected they turn square (radius 12) and switch to secondary container (.tb-btn).",
                toggles,
            ))
            .child(demo(
                "Icon buttons",
                "32 px, round. The send button (36 px) gets squarer on hover and shrinks when pressed.",
                row()
                    .child(IconButton::new("i-std", "search"))
                    .child(IconButton::new("i-filled", "plus").filled())
                    .child(IconButton::new("i-tonal", "pen").tonal())
                    .child(IconButton::new("i-outlined", "copy").outlined())
                    .child(
                        IconButton::new("i-toggle", "star")
                            .tonal()
                            .selected(self.icon_toggle)
                            .on_click(click(cx, |this, _| this.icon_toggle = !this.icon_toggle)),
                    )
                    .child(IconButton::new("i-send", "up").filled().hover_morph(true).size(px(36.)))
                    .child(IconButton::new("i-send-off", "up").filled().size(px(36.)).disabled(true))
                    .child(IconButton::new("i-stop", "stop").filled().size(px(36.))),
            ))
            .child(demo(
                "FAB",
                "56 px with 16 px corners; rounds on hover and shrinks when pressed (.rail-fab, .nav-row.new).",
                row().child(Fab::new("fab", "pen")).child(Fab::new("fab-ext", "plus").label("New chat")),
            ))
            .into_any_element()
    }

    fn selection(&self, cx: &mut Context<Self>) -> AnyElement {
        let model_item = |i: usize, label: &'static str, sub: &'static str, icon: &'static str| {
            MenuItem::new(("model", i), label)
                .sublabel(sub)
                .icon(icon)
                .selected(self.model == i)
                .on_click(click(cx, move |this, _| {
                    this.model = i;
                    this.split_open = false;
                }))
        };
        let mut attachments = row();
        for (i, name) in self.attachments.iter().enumerate() {
            let name = *name;
            attachments =
                attachments.child(Chip::input(("attachment", i), name).icon("file").on_remove(
                    click(cx, move |this, _| this.attachments.retain(|n| *n != name)),
                ));
        }
        let mut switches = row();
        for (i, label) in ["Notifications", "Compact mode", "Sounds"]
            .iter()
            .enumerate()
        {
            switches = switches.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        Switch::new(("sw", i), self.switches[i])
                            .compact(i == 1)
                            .on_toggle(update(cx, move |this, v: bool, _| this.switches[i] = v)),
                    )
                    .child(*label),
            );
        }
        switches = switches.child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(Switch::new("sw-off", true).disabled(true))
                .child("Disabled"),
        );

        let mut filters = row();
        for (i, label) in ["Remote", "Agents", "Favorites"].iter().enumerate() {
            filters = filters.child(
                Chip::new(("filter", i), *label)
                    .filter(self.filters[i])
                    .on_click(click(cx, move |this, _| this.filters[i] = !this.filters[i])),
            );
        }

        page()
            .child(demo(
                "Switch",
                "44 × 26 (36 × 22 in menus). The thumb grows when turned on and stretches while held down.",
                switches,
            ))
            .child(demo(
                "Connected button group",
                "Round ends and nearly square inner corners; the selected one becomes a pill (.segmented).",
                column()
                    .child(
                        SegmentedButtons::new(
                            "seg-text",
                            [Segment::label("Light"), Segment::label("Dark"), Segment::label("System")],
                            self.segment,
                        )
                        .on_change(update(cx, |this, i: usize, _| this.segment = i)),
                    )
                    .child(
                        SegmentedButtons::new(
                            "seg-icon",
                            [
                                Segment::label("List").with_icon("layers"),
                                Segment::label("Grid").with_icon("blocks"),
                                Segment::label("Tree").with_icon("branch"),
                            ],
                            self.icon_segment,
                        )
                        .on_change(update(cx, |this, i: usize, _| this.icon_segment = i)),
                    ),
            ))
            .child(demo(
                "Stepped slider",
                "The effort picker: the current stop becomes a vertical bar (.effort).",
                row()
                    .child(StopSlider::new("effort", 5, self.effort).on_change(update(cx, |this, i: usize, _| this.effort = i)))
                    .child(div().text_color(Theme::of(cx).on_surface_variant).child(EFFORTS[self.effort]))
                    .child(StopSlider::new("effort-compact", 5, self.effort).compact(true).on_change(update(cx, |this, i: usize, _| this.effort = i))),
            ))
            .child(demo(
                "Slider without a value",
                "With `None` no stop is marked; picking one sets it.",
                row()
                    .child(
                        StopSlider::new("effort-default", 5, self.default_effort)
                            .on_change(update(cx, |this, i: usize, _| this.default_effort = Some(i))),
                    )
                    .child(
                        div()
                            .text_color(Theme::of(cx).on_surface_variant)
                            .child(self.default_effort.map_or("Default", |i| EFFORTS[i])),
                    )
                    .child(
                        Button::new("effort-reset", "Reset")
                            .text()
                            .size(ButtonSize::Small)
                            .on_click(click(cx, |this, _| this.default_effort = None)),
                    ),
            ))
            .child(demo(
                "Chips",
                "32 px with 8 px corners; they round when pressed. Toggleable chips become pills when selected.",
                column()
                    .child(
                        row()
                            .child(Chip::new("c-assist", "Explain this code").icon("spark"))
                            .child(Chip::new("c-assist2", "Write tests").icon("check"))
                            .child(Chip::new("c-project", "demo").icon("folder").filled())
                            .child(
                                Chip::new("c-dot", "Running")
                                    .leading(div().size(px(8.)).rounded(px(4.)).bg(Theme::of(cx).success)),
                            ),
                    )
                    .child(filters)
                    .child(attachments),
            ))
            .child(demo(
                "Split button",
                "When opened, the trailing button becomes a primary-colored circle and the arrow rotates (.split).",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        SplitButton::new("split", MODELS[self.model])
                            .sublabel(EFFORTS[self.effort])
                            .open(self.split_open)
                            .on_toggle(click(cx, |this, _| this.split_open = !this.split_open)),
                    )
                    .when(self.split_open, |d| {
                        d.child(
                            Menu::new("split-menu")
                                .width(240.)
                                .max_h(px(200.))
                                .section("Model")
                                .item(model_item(0, "Opus 4.1", "Most capable", "brain"))
                                .item(model_item(1, "Sonnet 4.5", "Balanced", "bolt"))
                                .item(model_item(2, "Haiku 4.5", "Fastest", "spark"))
                                .section("Legacy")
                                .item(MenuItem::new("m-old", "Opus 3").sublabel("Retired").icon("history").disabled(true)),
                        )
                    }),
            ))
            .into_any_element()
    }

    fn navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
        let projects = ["demo", "dashboard-php", "api-clientes"];
        let mut drawer = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .w(px(244.))
            .p(px(12.))
            .rounded(px(24.))
            .bg(s.surface_container);
        drawer = drawer.child(Fab::new("new-chat", "plus").label("New chat"));
        drawer = drawer.child(div().h(px(8.)));
        for (i, (label, icon)) in [("Search", "search"), ("Chats", "chat"), ("Agents", "cpu")]
            .iter()
            .enumerate()
        {
            let mut item = NavItem::new(("nav", i), *label)
                .icon(*icon)
                .selected(self.nav == i)
                .on_click(click(cx, move |this, _| this.nav = i));
            if i == 2 {
                item = item.trailing(Badge::new("3"));
            }
            drawer = drawer.child(item);
        }
        drawer = drawer.child(
            div()
                .px(px(12.))
                .pt(px(12.))
                .pb(px(4.))
                .text_size(px(12.))
                .font_weight(FontWeight(700.))
                .text_color(s.primary)
                .child("Projects"),
        );
        for (j, name) in projects.iter().enumerate() {
            let i = 3 + j;
            drawer = drawer.child(
                NavItem::new(("nav", i), *name)
                    .leading(Avatar::new(*name))
                    .selected(self.nav == i)
                    .on_click(click(cx, move |this, _| this.nav = i)),
            );
        }
        for (j, conv) in ["Theme refactor", "Review migrations"].iter().enumerate() {
            let i = 6 + j;
            drawer = drawer.child(
                div().pl(px(26.)).child(
                    NavItem::new(("nav", i), *conv)
                        .dense(true)
                        .selected(self.nav == i)
                        .on_click(click(cx, move |this, _| this.nav = i)),
                ),
            );
        }

        let mut rail = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.))
            .w(px(80.))
            .py(px(12.))
            .rounded(px(24.))
            .bg(s.surface_container);
        rail = rail
            .child(Fab::new("rail-fab", "pen"))
            .child(div().h(px(8.)));
        for (i, (label, icon)) in [
            ("Chats", "chat"),
            ("Files", "files"),
            ("Changes", "diff"),
            ("Settings", "gear"),
        ]
        .iter()
        .enumerate()
        {
            rail = rail.child(
                RailItem::new(("rail", i), *icon, *label)
                    .selected(self.rail == i)
                    .on_click(click(cx, move |this, _| this.rail = i)),
            );
        }

        let tabs = IconTabs::new(
            "tabs",
            [
                IconTab::new("bolt", "Actions"),
                IconTab::new("brain", "Model"),
                IconTab::new("plug", "MCP"),
                IconTab::new("hook", "Hooks"),
            ],
            self.tab,
        )
        .on_change(update(cx, |this, i: usize, _| this.tab = i));

        page()
            .child(demo(
                "Navigation drawer",
                "Pill rows; the selected row's indicator grows from the center and stretches with the spring (.nav-row).",
                row().items_start().child(drawer).child(
                    demo_note(
                        "Try it: switch rows quickly several times. The indicator always grows from the center, as in the reference design.",
                        cx,
                    ),
                ),
            ))
            .child(demo(
                "Navigation rail",
                "A 56 × 32 pill that stretches out from 24 px when selected (.rail-item).",
                rail,
            ))
            .child(demo(
                "Icon tabs",
                "Round icons; the active one stretches into a pill with its name and the icon bounces (.menu-tab).",
                div().w(px(340.)).child(tabs),
            ))
            .into_any_element()
    }

    fn menus(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
        let actions = |id: &'static str,
                       grouped: bool,
                       menu_switch: bool,
                       menu_effort: usize,
                       cx: &mut Context<Self>| {
            Menu::new(id)
                .grouped(grouped)
                .width(300.)
                .section("Session")
                .item(
                    MenuItem::new((id, 0usize), "Attach file")
                        .icon("clip")
                        .trailing("Ctrl U"),
                )
                .item(
                    MenuItem::new((id, 1usize), "Mention")
                        .icon("at")
                        .trailing("@"),
                )
                .separator()
                .section("Settings")
                .item(
                    MenuItem::new((id, 2usize), "Remote control")
                        .icon("plug")
                        .sublabel("Continue from your phone")
                        .trailing(
                            Switch::new((id, 3usize), menu_switch)
                                .compact(true)
                                .on_toggle(update(cx, |this, v: bool, _| this.menu_switch = v)),
                        ),
                )
                .item(
                    MenuItem::new((id, 4usize), "Effort")
                        .icon("gauge")
                        .trailing(
                            StopSlider::new((id, 5usize), 5, menu_effort)
                                .compact(true)
                                .on_change(update(cx, |this, i: usize, _| this.menu_effort = i)),
                        ),
                )
                .separator()
                .item(
                    MenuItem::new((id, 6usize), "Commands")
                        .icon("slash")
                        .trailing("/"),
                )
        };

        page()
            .child(demo(
                "Single-surface menu",
                "24 px corners, items that round into pills on hover. It rises in with the spring.",
                actions("menu-single", false, self.menu_switch, self.menu_effort, cx),
            ))
            .child(demo(
                "Grouped menu",
                "Each group is a card (radius 18) and they enter staggered by 35 ms (.agent-menu .menu-group).",
                actions("menu-grouped", true, self.menu_switch, self.menu_effort, cx),
            ))
            .child(demo(
                "Popover",
                "Opens over the content (deferred + anchored). Click outside to close it.",
                div()
                    .child(
                        Button::new("open-menu", if self.menu_open { "Close menu" } else { "Open menu" })
                            .filled()
                            .icon("more")
                            .on_click(click(cx, |this, _| this.menu_open = !this.menu_open)),
                    )
                    .when(self.menu_open, |d| {
                        let close = update(cx, |this, _: (), _| this.menu_open = false);
                        d.child(deferred(
                            anchored().child(
                                div()
                                    .id("popover")
                                    .mt(px(8.))
                                    .on_mouse_down_out(move |_, w, cx| close((), w, cx))
                                    .child(
                                        Menu::new("popover-menu")
                                            .width(240.)
                                            .item(MenuItem::new("p-0", "Rename").icon("pen"))
                                            .item(MenuItem::new("p-1", "Duplicate").icon("copy"))
                                            .item(MenuItem::new("p-2", "Export").icon("export"))
                                            .separator()
                                            .item(MenuItem::new("p-3", "Delete").icon("trash")),
                                    ),
                            ),
                        ))
                    }),
            ))
            .child(demo(
                "Two-line rows with actions",
                "The Rewind list: the message preview is cut at two lines and the choices sit under it (MenuItem::action, line_clamp).",
                column()
                    .items_start()
                    .child(self.rewind_menu(cx))
                    .when(!self.rewound.is_empty(), |d| {
                        d.child(div().text_color(s.on_surface_variant).child(self.rewound.clone()))
                    }),
            ))
            .child(div().h(px(220.)).text_color(s.on_surface_variant))
            .into_any_element()
    }

    fn rewind_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let turns = [
            "Port the code editor to GPUI with line numbers, undo and a virtualized list so large files stay fast",
            "Add a splitter for the terminal height",
            "Why does the theme flash when switching to dark mode? It should reveal from the click like the reference design",
        ];
        let mut menu = Menu::new("rewind-menu").width(340.).section("Rewind to");
        for (i, text) in turns.into_iter().enumerate() {
            let choice = |label: &'static str, j: usize, cx: &mut Context<Self>| {
                Chip::new(("rewind-chip", i * 3 + j), label)
                    .size(ChipSize::Small)
                    .on_click(click(cx, move |this: &mut Demos, _| {
                        this.rewound = format!("Rewound message {} ({label})", i + 1).into();
                    }))
            };
            menu = menu.item(
                MenuItem::new(("rewind", i), text)
                    .icon("history")
                    .line_clamp(2)
                    .static_row(true)
                    .action(choice("Code", 0, cx))
                    .action(choice("Conversation", 1, cx))
                    .action(choice("Both", 2, cx)),
            );
        }
        menu
    }

    fn shapes(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
        let mut grid = div().flex().flex_wrap().gap(px(16.));
        for shape in ShapeName::ALL {
            grid = grid.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(6.))
                    .w(px(84.))
                    .child(Shape::new(shape).size(px(64.)).color(s.primary_container))
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(s.on_surface_variant)
                            .child(shape.label()),
                    ),
            );
        }
        let mut avatars = row();
        for name in [
            "demo",
            "dashboard-php",
            "api-clientes",
            "chrsx3-web",
            "danza-web",
            "halftone-studio",
            "leo",
            "memoria",
        ] {
            avatars = avatars.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.))
                    .child(Avatar::new(name).size(px(36.)))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(s.on_surface_variant)
                            .child(name),
                    ),
            );
        }
        page()
            .child(demo("Shape library", "The 14 M3 Expressive shapes, fitted to their box as in the reference design (shapes.ts).", grid))
            .child(demo(
                "Breathing shapes",
                "the reference design's home screen composition: three shapes that slowly morph and rotate (.hero-shapes).",
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(200.))
                    .h(px(84.))
                    .child(div().mt(px(-30.)).child(Shape::new(ShapeName::Cookie12).size(px(34.)).color(s.tertiary_container).breathe(ShapeName::Sunny, 9.).spin(-26.)))
                    .child(
                        Shape::new(ShapeName::Cookie9)
                            .size(px(72.))
                            .color(s.primary_container)
                            .breathe(ShapeName::Clover8, 7.)
                            .spin(40.)
                            .child(gpui_m3::Icon::new("spark").size(px(28.)).color(s.on_primary_container)),
                    )
                    .child(div().mt(px(34.)).child(Shape::new(ShapeName::Pentagon).size(px(28.)).color(s.secondary_container).breathe(ShapeName::Gem, 5.).spin(18.))),
            ))
            .child(demo("Project avatars", "Shape and tone are derived from the name (same hash as the reference design).", avatars))
            .into_any_element()
    }

    fn progress(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
        page()
            .child(demo(
                "Loading indicator",
                "A shape that spins and morphs into the next one (softBurst → cookie9 → pentagon → oval → sunny → cookie4 → clover4).",
                row()
                    .gap(px(24.))
                    .child(LoadingIndicator::new().size(px(24.)))
                    .child(LoadingIndicator::new().size(px(48.)))
                    .child(LoadingIndicator::new().size(px(48.)).contained(true))
                    .child(LoadingIndicator::new().size(px(96.)).contained(true)),
            ))
            .child(demo(
                "Wavy progress",
                "The wave advances one wavelength per second; the value changes with the spatial spring.",
                column()
                    .w(px(520.))
                    .child(WavyProgress::new("wavy", Some(self.progress)))
                    .child(
                        row()
                            .child(Button::new("p-0", "0 %").size(ButtonSize::Small).on_click(click(cx, |this, _| this.progress = 0.)))
                            .child(Button::new("p-25", "25 %").size(ButtonSize::Small).on_click(click(cx, |this, _| this.progress = 0.25)))
                            .child(Button::new("p-70", "70 %").size(ButtonSize::Small).on_click(click(cx, |this, _| this.progress = 0.7)))
                            .child(Button::new("p-100", "100 %").size(ButtonSize::Small).on_click(click(cx, |this, _| this.progress = 1.))),
                    )
                    .child(div().text_size(px(12.)).text_color(s.on_surface_variant).child("Indeterminate"))
                    .child(WavyProgress::new("wavy-ind", None)),
            ))
            .child(demo(
                "Ring",
                "The context indicator from the composer and the usage panel.",
                row()
                    .gap(px(20.))
                    .child(Ring::new("ring-s", self.ring))
                    .child(Ring::new("ring-m", self.ring).size(px(32.)).stroke(4.))
                    .child(Ring::new("ring-l", self.ring).size(px(56.)).stroke(6.))
                    .child(
                        Button::new("ring-cycle", "Change value")
                            .size(ButtonSize::Small)
                            .on_click(click(cx, |this, _| this.ring = if this.ring > 0.9 { 0.12 } else { (this.ring + 0.3).min(1.) })),
                    ),
            ))
            .into_any_element()
    }

    fn surfaces(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
        page()
            .child(demo(
                "Tonal cards",
                "28 px corners and color containers (the Claude version card in Settings).",
                row()
                    .items_start()
                    .child(
                        div().w(px(300.)).child(
                            Card::tonal(Tone::Primary)
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight(700.))
                                        .opacity(0.8)
                                        .child("Claude Code"),
                                )
                                .child(
                                    div()
                                        .text_size(px(40.))
                                        .font_weight(FontWeight(800.))
                                        .child("2.1.4"),
                                )
                                .child(
                                    row().child(
                                        Button::new("card-btn", "Update")
                                            .filled()
                                            .size(ButtonSize::Large),
                                    ),
                                ),
                        ),
                    )
                    .child(
                        div().w(px(260.)).child(
                            Card::tonal(Tone::Tertiary)
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight(700.))
                                        .opacity(0.8)
                                        .child("Outdated"),
                                )
                                .child(
                                    div()
                                        .text_size(px(40.))
                                        .font_weight(FontWeight(800.))
                                        .child("1.9.0"),
                                ),
                        ),
                    )
                    .child(
                        div().w(px(220.)).child(
                            Card::tonal(Tone::Secondary)
                                .child(div().font_weight(FontWeight(700.)).child("Secondary"))
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .child("Secondary container for grouping content."),
                                ),
                        ),
                    ),
            ))
            .child(demo(
                "Segmented list",
                "Rows separated by 2 px; 22 px corners only at the ends (.settings-group).",
                div().w(px(460.)).child(
                    ListGroup::new()
                        .title("General")
                        .row(
                            "Notifications",
                            Switch::new("lg-0", self.switches[0])
                                .on_toggle(update(cx, |this, v: bool, _| this.switches[0] = v)),
                        )
                        .row(
                            "Language",
                            div().text_color(s.on_surface_variant).child("English"),
                        )
                        .row("Shortcuts", Badge::new("12")),
                ),
            ))
            .child(demo(
                "Keyboard keys",
                "Kbd is one key cap; KeyCombo turns a text such as ctrl-shift-k into a row of them, in a fixed modifier order, with the words of a KeyNames table (English or Spanish). Muted keys cannot be changed; clicking a combo that has on_click starts capturing, shown as a pulsing pill.",
                column()
                    .child(
                        row()
                            .child(Kbd::new("Ctrl"))
                            .child(Kbd::new("K").active(true))
                            .child(Kbd::new("Esc").muted(true))
                            .child(Kbd::new("F5").small(true)),
                    )
                    .child(
                        row()
                            .gap(px(24.))
                            .child(KeyCombo::new("kc-en", "ctrl-shift-k"))
                            .child(KeyCombo::new("kc-es", "ctrl-shift-k").names(KeyNames::spanish()))
                            .child(KeyCombo::new("kc-seq", "escape escape").names(KeyNames::spanish()).muted(true))
                            .child(KeyCombo::new("kc-arrows", "alt-left").small(true))
                            .child(KeyCombo::new("kc-pg", "pagedown").names(KeyNames::spanish())),
                    )
                    .child(
                        row().child(
                            KeyCombo::new("kc-capture", "ctrl-k")
                                .names(KeyNames::spanish())
                                .capturing(self.keys_capturing)
                                .prompt("Presiona la nueva combinación…")
                                .tooltip("Click to change")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.keys_capturing = !this.keys_capturing;
                                    cx.notify();
                                })),
                        ),
                    ),
            ))
            .child(demo(
                "Setting rows",
                "SettingRow puts a title, a description and a control (a switch, a key combo, a button) in one row; it goes inside a ListGroup. With on_click the whole row is a button.",
                div().w(px(520.)).child(
                    ListGroup::new()
                        .title("Shortcuts")
                        .custom_row(
                            SettingRow::new("sr-0", "Open the command palette")
                                .description("Factory key: Ctrl+K")
                                .control(
                                    KeyCombo::new("sr-0-keys", "ctrl-k")
                                        .on_click(|_, _, _| {}),
                                ),
                        )
                        .custom_row(
                            SettingRow::new("sr-1", "Close the menu")
                                .description("This one cannot be changed")
                                .muted(true)
                                .control(KeyCombo::new("sr-1-keys", "escape").muted(true)),
                        )
                        .custom_row(
                            SettingRow::new("sr-2", "Notifications")
                                .icon("bolt")
                                .control(
                                    Switch::new("sr-2-sw", self.switches[0])
                                        .on_toggle(update(cx, |this, v: bool, _| this.switches[0] = v)),
                                ),
                        )
                        .custom_row(
                            SettingRow::new("sr-3", "Language")
                                .description("Applies to menus and messages")
                                .on_click(|_, _, _| {})
                                .control(div().text_color(s.on_surface_variant).child("English")),
                        ),
                ),
            ))
            .child(demo(
                "Message bubbles",
                "The tail corner tightens to 4 px (.msg.user, .msg.notice).",
                column()
                    .child(div().flex().justify_end().w(px(520.)).child(Bubble::new(
                        BubbleKind::User,
                        "Can you check why the theme migration is failing?",
                    )))
                    .child(Bubble::new(
                        BubbleKind::Notice,
                        "Claude asked for permission to edit 3 files.",
                    ))
                    .child(
                        div().flex().justify_end().w(px(520.)).child(
                            Bubble::custom(BubbleKind::User)
                                .max_w(px(300.))
                                .child("Here is the screenshot:")
                                .child(
                                    div()
                                        .pt(px(8.))
                                        .child(Chip::new("b-file", "screenshot.png").icon("file")),
                                ),
                        ),
                    ),
            ))
            .into_any_element()
    }

    fn inputs(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
        let composer_focused = self.composer.read(cx).is_focused(window);
        let composer_bg = animate_color(
            "composer-bg",
            Spring::EFFECTS,
            if composer_focused {
                s.surface_container_highest
            } else {
                s.surface_container_high
            },
            window,
            cx,
        );
        let sent = if self.sent.is_empty() {
            "Enter sends, Shift+Enter adds a line.".to_string()
        } else {
            format!("Sent: {}", self.sent)
        };
        let submitted = if self.submitted.is_empty() {
            "Type and press Enter.".to_string()
        } else {
            format!("Submitted: {}", self.submitted)
        };
        let menu_position = self.context_menu;
        page()
            .child(demo(
                "Text field",
                "A tonal pill that deepens and opens up its corners when focused. Editing, selection, clipboard and IME.",
                column()
                    .w(px(420.))
                    .child(self.search.clone())
                    .child(self.name.clone())
                    .child(div().text_size(px(12.5)).text_color(s.on_surface_variant).child(submitted)),
            ))
            .child(demo(
                "Text area",
                "The composer: grows with its text up to 240 px and then scrolls. Wraps lines, moves by row with the arrows.",
                column()
                    .w(px(560.))
                    .child(
                        div()
                            .pt(px(15.))
                            .px(px(20.))
                            .pb(px(12.))
                            .rounded(px(26.))
                            .bg(composer_bg)
                            .child(self.composer.clone()),
                    )
                    .child(div().text_size(px(12.5)).text_color(s.on_surface_variant).child(sent)),
            ))
            .child(demo(
                "Tooltips",
                "Inverse surface with small corners. Hover over the icons.",
                row()
                    .child(IconButton::new("tt-copy", "copy").tooltip("Copy"))
                    .child(IconButton::new("tt-pen", "pen").tooltip("Rename"))
                    .child(IconButton::new("tt-trash", "trash").tooltip("Delete")),
            ))
            .child(demo(
                "Linear bar",
                "The usage panel meter; the value changes with the spatial spring.",
                column()
                    .w(px(420.))
                    .child(LinearProgress::new("meter", self.progress))
                    .child(
                        row()
                            .child(Button::new("m-20", "20 %").size(ButtonSize::Small).on_click(click(cx, |this: &mut Demos, _| this.progress = 0.2)))
                            .child(Button::new("m-60", "60 %").size(ButtonSize::Small).on_click(click(cx, |this: &mut Demos, _| this.progress = 0.6)))
                            .child(Button::new("m-90", "90 %").size(ButtonSize::Small).on_click(click(cx, |this: &mut Demos, _| this.progress = 0.9))),
                    ),
            ))
            .child(demo(
                "Dialog and toast",
                "The dialog covers the window with a scrim; the toast uses the inverse surface and can have an action.",
                column()
                    .items_start()
                    .child(
                        row()
                            .child(Button::new("open-dialog", "Open dialog").filled().on_click(click(cx, |this: &mut Demos, _| this.dialog_open = true)))
                            .child(Button::new("open-toast", "Show toast").on_click(click(cx, |this: &mut Demos, _| this.toast_open = !this.toast_open))),
                    )
                    .when(self.toast_open, |d| {
                        d.child(
                            div().w(px(420.)).child(
                                Toast::new("toast", "Chat archived.")
                                    .action("Undo", click(cx, |this: &mut Demos, _| this.toast_open = false)),
                            ),
                        )
                    }),
            ))
            .child(demo(
                "Context menu",
                "Opens at the pointer position and stays inside the window.",
                div()
                    .id("context-area")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(420.))
                    .h(px(120.))
                    .rounded(px(16.))
                    .border_1()
                    .border_color(s.outline_variant)
                    .text_color(s.on_surface_variant)
                    .child("Right-click here")
                    .on_mouse_down(MouseButton::Right, {
                        let set = update(cx, |this: &mut Demos, position: Point<Pixels>, _| this.context_menu = Some(position));
                        move |event, window, cx| set(event.position, window, cx)
                    }),
            ))
            .when_some(menu_position, |d, position| {
                let close = update(cx, |this: &mut Demos, _: (), _| this.context_menu = None);
                d.child(context_menu(
                    position,
                    div().id("ctx").on_mouse_down_out(move |_, w, cx| close((), w, cx)).child(
                        Menu::new("ctx-menu")
                            .width(220.)
                            .item(MenuItem::new("ctx-0", "Open").icon("folder-open"))
                            .item(MenuItem::new("ctx-1", "Copy path").icon("copy"))
                            .separator()
                            .item(MenuItem::new("ctx-2", "Delete").icon("trash")),
                    ),
                ))
            })
            .when(self.dialog_open, |d| {
                d.child(
                    Dialog::new("dialog")
                        .icon("trash")
                        .title("Delete this chat?")
                        .child("Its messages will be deleted. This can't be undone.")
                        .action(Button::new("d-cancel", "Cancel").text().on_click(click(cx, |this: &mut Demos, _| this.dialog_open = false)))
                        .action(Button::new("d-ok", "Delete").filled().on_click(click(cx, |this: &mut Demos, _| this.dialog_open = false)))
                        .on_dismiss(click(cx, |this: &mut Demos, _| this.dialog_open = false)),
                )
            })
            .into_any_element()
    }

    fn composer(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
        const TABS: [(&str, &str); 6] = [
            ("clip", "Context"),
            ("cpu", "Model"),
            ("palette", "Customize"),
            ("plug", "Integrations"),
            ("history", "Session"),
            ("shield", "Account"),
        ];
        const MODES: [(&str, DotShape); 4] = [
            ("Ask", DotShape::Square),
            ("Auto edit", DotShape::Circle),
            ("Plan", DotShape::Diamond),
            ("Bypass", DotShape::Square),
        ];
        let mode_color = [s.outline, s.primary, s.tertiary, s.error][self.mode];
        let (mode_label, mode_shape) = MODES[self.mode];

        let mut menu = Menu::new("agent-menu")
            .width(340.)
            .max_h(px(440.))
            .content_key(self.agent_tab as u64 + if self.agent_sub { 100 } else { 0 });
        if self.agent_sub {
            menu = menu
                .back("Model", click(cx, |this, _| this.agent_sub = false))
                .item(
                    MenuItem::new("am-effort", "Effort")
                        .dense(true)
                        .icon("gauge")
                        .sublabel(EFFORTS[self.effort])
                        .inline_sublabel(true)
                        .static_row(true)
                        .trailing(
                            StopSlider::new("am-effort-slider", 5, self.effort)
                                .compact(true)
                                .on_change(update(cx, |this, i: usize, _| this.effort = i)),
                        ),
                )
                .separator();
            for (i, (name, sub)) in [
                ("Opus 4.1", "Most capable"),
                ("Sonnet 4.5", "Balanced"),
                ("Haiku 4.5", "Fastest"),
            ]
            .into_iter()
            .enumerate()
            {
                menu = menu.item(
                    MenuItem::new(("am-model", i), name)
                        .dense(true)
                        .radio(self.agent_model == i)
                        .sublabel(sub)
                        .on_click(click(cx, move |this, _| this.agent_model = i)),
                );
            }
        } else {
            let tabs = IconTabs::new(
                "am-tabs",
                TABS.iter().map(|(icon, label)| IconTab::new(*icon, *label)),
                self.agent_tab,
            )
            .on_change(update(cx, |this, i: usize, _| this.agent_tab = i));
            menu = menu.header(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .pb(px(6.))
                    .child(self.filter.clone())
                    .child(tabs),
            );
            let rows: [(&str, &str, Option<&str>); 5] = [
                ("clip", "Attach file…", Some("Ctrl+U")),
                ("at", "Mention project file…", Some("@")),
                ("trash", "Clear conversation", None),
                ("history", "Rewind", None),
                ("export", "Export conversation", None),
            ];
            for (i, (icon, label, shortcut)) in rows.into_iter().enumerate() {
                let mut item = MenuItem::new(("am-row", i), label)
                    .dense(true)
                    .icon(icon)
                    .highlighted(self.agent_highlight == i)
                    .on_click(click(cx, move |this, _| this.agent_highlight = i));
                if let Some(shortcut) = shortcut {
                    item = item.shortcut(shortcut);
                }
                menu = menu.item(item);
            }
            menu = menu.item(
                MenuItem::new("am-model-row", "Change model…")
                    .dense(true)
                    .icon("cpu")
                    .trailing(MODELS[self.agent_model])
                    .on_click(click(cx, |this, _| this.agent_sub = true)),
            );
        }

        let launches = self.launches;
        let bar = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .w(px(560.))
            .p(px(8.))
            .rounded(px(26.))
            .bg(s.surface_container_high)
            .child(IconButton::new("cb-attach", "plus").tooltip("Attach (Ctrl+U)"))
            .child(IconButton::new("cb-actions", "slash").tooltip("Actions (/)"))
            .child(
                Button::new("cb-model", MODELS[self.agent_model])
                    .size(ButtonSize::Compact)
                    .sublabel(EFFORTS[self.effort])
                    .open(self.agent_sub)
                    .on_click(click(cx, |this, _| this.agent_sub = !this.agent_sub)),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("cb-ring")
                    .cursor_pointer()
                    .p(px(8.))
                    .child(Ring::new("cb-ring-value", 0.42))
                    .on_click(click(cx, |this, _| this.usage_open = !this.usage_open)),
            )
            .child(
                IconButton::new("cb-send", "up")
                    .filled()
                    .hover_morph(true)
                    .size(px(36.))
                    .launch(launches)
                    .on_click(click(cx, |this, _| this.launches += 1)),
            );

        let under = row()
            .child(
                Chip::new("cu-branch", "main")
                    .icon("branch")
                    .muted(true)
                    .inert(true),
            )
            .child(
                Chip::new("cu-mode", mode_label)
                    .tone(if self.mode == 2 {
                        Tone::Tertiary
                    } else {
                        Tone::Secondary
                    })
                    .leading(MorphDot::new("cu-mode-dot", mode_shape).color(mode_color))
                    .on_click(click(cx, |this, _| this.mode = (this.mode + 1) % 4)),
            )
            .child(
                Chip::new("cu-time", "12m")
                    .icon("history")
                    .muted(true)
                    .inert(true),
            )
            .child(
                Chip::new("cu-agents", "2 agents")
                    .icon("layers")
                    .muted(!self.agents_live)
                    .filter(self.agents_live)
                    .show_check(false)
                    .pulse_icon(self.agents_live)
                    .on_click(click(cx, |this, _| this.agents_live = !this.agents_live)),
            )
            .child(
                Chip::input("cu-file", "src/components/very_long_file_name.rs")
                    .tone(Tone::Secondary)
                    .icon("file")
                    .max_w(px(220.)),
            )
            .child(
                Chip::new("cu-context", "theme.rs")
                    .icon("file")
                    .mono(true)
                    .sublabel(":12-30")
                    .size(ChipSize::Small),
            );

        page()
            .child(demo(
                "Agent menu",
                "Fixed filter and tabs over the scrolling items; dense rows with shortcuts; a submenu with a back row and radios. The content fades in on tab changes.",
                menu,
            ))
            .child(demo(
                "Composer bar",
                "Model pill with sublabel (opens the model submenu); the send button plays the launch on each click.",
                column()
                    .child(bar)
                    .when(self.usage_open, |d| {
                        d.child(
                            Popover::new("usage")
                                .width(px(380.))
                                .padding(px(18.))
                                .direction(PopDirection::Down)
                                .child(div().text_size(px(20.)).font_weight(FontWeight(750.)).child("Account and usage"))
                                .child(WavyProgress::new("usage-wave", Some(0.42))),
                        )
                    }),
            ))
            .child(demo(
                "Composer chips",
                "Muted static chips, a tonal mode chip with a morphing dot (click to cycle), a filter that keeps its icon and pulses, a tonal file chip and a small mono chip.",
                under,
            ))
            .into_any_element()
    }

    fn conversation(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
        let options = [
            (
                "Keep the current API",
                "Add the new builders next to the old ones.",
            ),
            ("Break it", "Rename everything now, before 0.1."),
            ("Ask me later", ""),
        ];
        let mut single = column().w(px(460.)).gap(px(4.));
        let mut multi = column().w(px(460.)).gap(px(4.));
        for (i, (label, description)) in options.into_iter().enumerate() {
            let mut radio = ChoiceRow::radio(("answer", i), label, self.answer == i)
                .on_click(click(cx, move |this, _| this.answer = i));
            let mut check = ChoiceRow::checkbox(("answers", i), label, self.answers[i])
                .on_click(click(cx, move |this, _| this.answers[i] = !this.answers[i]));
            if !description.is_empty() {
                radio = radio.description(description);
                check = check.description(description);
            }
            single = single.child(radio);
            multi = multi.child(check);
        }

        let tool = ExpandableCard::new("tool-edit")
            .open(self.tool_open)
            .on_toggle(update(cx, |this, open: bool, _| this.tool_open = open))
            .header(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .child(Icon::new("check").size(px(13.)).color(s.success))
                    .child(div().font_weight(FontWeight(600.)).child("Edit"))
                    .child(div().text_color(s.on_surface_variant).child("theme.rs"))
                    .child(Badge::tonal(Tone::Secondary, "3 changes"))
                    .child(Badge::tonal(Tone::Tertiary, "all")),
            )
            .child(
                div()
                    .p(px(10.))
                    .rounded(px(12.))
                    .bg(s.surface)
                    .text_color(s.on_surface_variant)
                    .child("- primary: hex(0x6750a4)\n+ primary: hex(0x65558f)"),
            );
        let subagent = ExpandableCard::new("tool-agent")
            .tone(Tone::Secondary)
            .header(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .child(LoadingIndicator::new().size(px(14.)))
                    .child(div().font_weight(FontWeight(600.)).child("Subagent"))
                    .child("Explore"),
            )
            .under(
                row()
                    .gap(px(4.))
                    .pl(px(33.))
                    .pb(px(7.))
                    .child(Badge::tonal(Tone::Primary, "Grep").mono(true))
                    .child(Badge::tonal(Tone::Primary, "Read").mono(true)),
            )
            .child("Summary of what the subagent found.");
        let thinking = ExpandableCard::new("thinking")
            .plain(true)
            .open(self.thinking_open)
            .on_toggle(update(cx, |this, open: bool, _| this.thinking_open = open))
            .header(div().opacity(pulse(1.6, window, cx)).child("Reasoning…"))
            .preview("so the card radius should be 16 on the high container")
            .child(
                "The user wants a 1:1 copy, so the card radius should be 16 on the high container.",
            );

        let permission = Card::tonal(Tone::Tertiary)
            .id(("permission", self.permission_shown as u64))
            .entrance(true)
            .padding_xy(px(18.), px(16.))
            .gap(px(10.))
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight(600.))
                    .child("Claude wants to run a command"),
            )
            .child(
                div()
                    .p(px(10.))
                    .rounded(px(16.))
                    .bg(s.surface)
                    .text_color(s.on_surface)
                    .child("$ cargo test"),
            )
            .child(
                row()
                    .gap(px(6.))
                    .child(
                        Button::new("perm-allow", "Allow")
                            .filled()
                            .size(ButtonSize::Comfortable),
                    )
                    .child(Button::new("perm-always", "Always allow").size(ButtonSize::Comfortable))
                    .child(
                        Button::new("perm-deny", "Deny")
                            .outlined()
                            .size(ButtonSize::Comfortable),
                    ),
            );

        page()
            .child(demo(
                "Choice rows",
                "AskUserQuestion options: radio or checkbox mark, label and description; selected rows fill with the primary color.",
                row().items_start().gap(px(24.)).child(single).child(multi),
            ))
            .child(demo(
                "Expandable cards",
                "Tool card (radius 16, chevron rotates, corners tighten on press while closed), a tonal subagent card with an always-visible row, and the plain reasoning disclosure with pulsing text. Tab to a header: Enter or Space toggles it and a focus ring shows.",
                column().w(px(520.)).gap(px(8.)).child(tool).child(subagent).child(thinking),
            ))
            .child(demo(
                "Permission card",
                "Tonal card with the shape entrance (corners 48 → 28, rises 24 px) and 40 px buttons.",
                column()
                    .w(px(520.))
                    .items_start()
                    .child(permission)
                    .child(
                        Button::new("perm-replay", "Replay entrance")
                            .text()
                            .size(ButtonSize::Small)
                            .on_click(click(cx, |this, _| this.permission_shown = !this.permission_shown)),
                    ),
            ))
            .child(demo(
                "Copy action",
                "Ghost button whose label slides out on hover and confirms with a check for 1.4 s.",
                row().child(
                    Button::new("copy", "Copy")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Tiny)
                        .icon("copy")
                        .label_on_hover(true)
                        .confirm("Copied", std::time::Duration::from_millis(1400))
                        .on_click(|_, _, _| {}),
                ),
            ))
            .into_any_element()
    }

    fn panels(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
        let tree = column()
            .w(px(320.))
            .gap(px(0.))
            .child(
                TreeRow::new("tree-root", "gpui-m3")
                    .root(true)
                    .expanded(Some(true))
                    .icon("folder-open")
                    .icon_color(s.primary),
            )
            .child(
                TreeRow::new("tree-src", "src")
                    .depth(1)
                    .expanded(Some(self.tree_open))
                    .icon(if self.tree_open {
                        "folder-open"
                    } else {
                        "folder"
                    })
                    .icon_color(s.primary)
                    .on_click(click(cx, |this, _| this.tree_open = !this.tree_open)),
            )
            .when(self.tree_open, |d| {
                d.child(
                    TreeRow::new("tree-lib", "lib.rs")
                        .depth(2)
                        .icon("file")
                        .icon_color(s.primary)
                        .selected(self.tree_selected == 2)
                        .on_click(click(cx, |this, _| this.tree_selected = 2)),
                )
                .child(
                    TreeRow::new("tree-theme", "theme.rs")
                        .depth(2)
                        .icon("file")
                        .icon_color(s.primary)
                        .selected(self.tree_selected == 3)
                        .on_click(click(cx, |this, _| this.tree_selected = 3)),
                )
            })
            .child(
                TreeRow::new("tree-env", ".gitignore")
                    .depth(1)
                    .icon("file")
                    .icon_color(s.warning)
                    .dimmed(true),
            );

        let changes = column()
            .w(px(320.))
            .gap(px(2.))
            .child(
                ListItem::new("chg-0", "theme.rs")
                    .subtitle("src")
                    .badge("M", s.warning, alpha(s.warning, 0.16))
                    .trailing(div().text_color(s.success).child("+12")),
            )
            .child(
                ListItem::new("chg-1", "lists.rs")
                    .subtitle("src/components")
                    .badge("A", s.success, alpha(s.success, 0.16))
                    .trailing(div().text_color(s.success).child("+480")),
            );

        let mut strip = TabStrip::new("strip")
            .selected(self.tab_selected.clone())
            .leading(IconButton::new("strip-back", "arrow-left").size(px(28.)))
            .trailing(
                IconButton::new("strip-add", "plus")
                    .size(px(28.))
                    .on_click(click(cx, |this, _| this.tabs.push("new.rs"))),
            )
            .on_select(update(cx, |this, key: SharedString, _| {
                this.tab_selected = key
            }))
            .on_close(update(cx, |this, key: SharedString, _| {
                this.tabs.retain(|t| *t != key.as_ref())
            }));
        for (i, name) in self.tabs.iter().enumerate() {
            strip = strip.tab(
                Tab::new(*name, *name)
                    .icon("file")
                    .tooltip(*name)
                    .dirty(i == 1),
            );
        }

        let nav = column()
            .w(px(280.))
            .gap(px(2.))
            .child(
                NavItem::new("nav-unread", "Theme migration")
                    .icon("chat")
                    .emphasized(self.nav_unread)
                    .meta("2m")
                    .trailing(
                        Badge::dot()
                            .size(px(8.))
                            .strong(Tone::Tertiary)
                            .pop(("dot", self.dot_key)),
                    )
                    .on_click(click(cx, |this, _| {
                        this.nav_unread = !this.nav_unread;
                        this.dot_key += 1;
                    })),
            )
            .child(NavItem::new("nav-read", "Old chat").icon("chat").meta("3d"))
            .child(
                NavItem::new("nav-large", "Appearance")
                    .icon("palette")
                    .large(true)
                    .tone(Tone::Primary)
                    .selected(true),
            );

        let menu = Menu::new("danger-menu")
            .width(240.)
            .item(MenuItem::new("dm-0", "Rename").icon("pen"))
            .item(
                MenuItem::new("dm-1", "Delete")
                    .icon("trash")
                    .danger(true)
                    .confirm("Click again to delete")
                    .on_click(click(cx, |this, _| this.deleted += 1)),
            );

        page()
            .child(demo(
                "File tree",
                "30 px pills indented 14 px per level with guides; the chevron turns; hidden files dim.",
                tree,
            ))
            .child(demo(
                "Change rows",
                "Two-line rows; corners tighten from 20 to 12 and the status badge opens from 9 to 13 on hover.",
                changes,
            ))
            .child(demo(
                "Tab strip",
                "Closable tabs with an unsaved dot, a back button and +. Hover a tab to see its close button.",
                div().w(px(520.)).child(strip),
            ))
            .child(demo(
                "Continuous slider",
                "16 px track split by a 4 × 28 handle (the photo cropper's zoom). Drag it.",
                row()
                    .child(
                        Slider::new("zoom", self.zoom, 1.0..=5.0)
                            .step(0.01)
                            .on_change(update(cx, |this, v: f32, _| this.zoom = v)),
                    )
                    .child(div().text_color(s.on_surface_variant).child(format!("{:.2}×", self.zoom))),
            ))
            .child(demo(
                "Navigation extras",
                "Unread row (click to toggle) with meta text and a dot that pops in; a large tonal settings tab.",
                nav,
            ))
            .child(demo(
                "Danger item",
                "Error colors; the first click arms it, the second runs it; moving away disarms it.",
                row()
                    .items_start()
                    .child(menu)
                    .child(div().text_color(s.on_surface_variant).child(format!("Deleted {} times", self.deleted))),
            ))
            .child(demo(
                "Command palette",
                "Modal near the top: type to filter (matches are highlighted), ↑/↓ or the pointer move the selection, Enter or a click runs, Escape or a click outside closes.",
                row()
                    .child(
                        Button::new("open-palette", "Open palette")
                            .filled()
                            .icon("search")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.palette_open = true;
                                this.palette.update(cx, |p, cx| p.reset(cx));
                                this.palette.read(cx).focus(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(div().text_color(s.on_surface_variant).child(self.palette_ran.clone()))
                    .when(self.palette_open, |d| d.child(self.palette.clone())),
            ))
            .child(demo(
                "Bubbles",
                "Error notice and bubbles with the entrance from the tail corner.",
                column()
                    .w(px(520.))
                    .child(
                        div().flex().justify_end().child(
                            Bubble::new(BubbleKind::User, "Run the tests again.")
                                .entrance(("bubble-user", self.dot_key), 0.),
                        ),
                    )
                    .child(
                        Bubble::new(BubbleKind::Error, "The process exited with code 101.")
                            .entrance(("bubble-error", self.dot_key), 0.12),
                    ),
            ))
            .into_any_element()
    }

    fn appearance(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
        let mut projects = column().w(px(280.)).gap(px(2.));
        for (i, name) in ["gpui-m3", "demo", "atic"].into_iter().enumerate() {
            let group: SharedString = format!("project-{i}").into();
            projects = projects.child(
                NavItem::new(("project", i), name)
                    .icon("folder")
                    .group(group.clone())
                    .trailing(
                        FavStar::new(("fav", i), self.favorites[i])
                            .hover_group(group)
                            .on_toggle(update(cx, move |this, on: bool, _| this.favorites[i] = on)),
                    ),
            );
        }

        let styles = [
            ("Formal", "Sober, dense and fast"),
            ("Material 3 Expressive", "Color, shapes and motion"),
            ("Liquid Glass", "Glass, light and depth"),
        ];
        let mut cards = div().flex().gap(px(8.)).w(px(420.));
        for (i, (title, hint)) in styles.into_iter().enumerate() {
            cards = cards.child(
                div().flex_1().child(
                    SelectCard::new(("style", i), title)
                        .hint(hint)
                        .preview(
                            div()
                                .h(px(52.))
                                .rounded(px(14.))
                                .bg(s.surface_container)
                                .p(px(6.))
                                .flex()
                                .gap(px(5.))
                                .child(div().flex_1().rounded(px(10.)).bg(s.primary_container))
                                .child(div().w(px(20.)).rounded(px(10.)).bg(s.primary)),
                        )
                        .selected(self.style_choice == i)
                        .on_click(click(cx, move |this, _| this.style_choice = i)),
                ),
            );
        }

        let presets = SEEDS.map(hex);
        page()
            .child(demo(
                "Favorite star",
                "Hover a row to reveal its star; turning it on spins it in and throws a burst.",
                projects,
            ))
            .child(demo("Select cards", "The style picker: a preview, a title and a hint.", cards))
            .child(demo(
                "Accent swatches",
                "The chosen swatch turns into a rounded diamond with a double ring; the wheel opens an HSV picker.",
                column()
                    .items_start()
                    .child(
                        ColorSwatches::new("accents", presets)
                            .current(self.accent)
                            .custom(click(cx, |this, _| this.picker_open = !this.picker_open))
                            .on_change(update(cx, |this, color: Hsla, _| this.accent = color)),
                    )
                    .when(self.picker_open, |d| {
                        d.child(
                            HsvPicker::new("accent-picker", self.accent)
                                .on_change(update(cx, |this, color: Hsla, _| this.accent = color)),
                        )
                    }),
            ))
            .child(demo(
                "Form fields",
                "Filled with a label and an underline, outlined, and the inline rename pill (select-all on focus, Escape restores).",
                column()
                    .w(px(320.))
                    .gap(px(14.))
                    .child(self.filled_field.clone())
                    .child(self.outlined_field.clone())
                    .child(self.rename_field.clone()),
            ))
            .into_any_element()
    }

    fn attachments(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
        let k = self.replay_key;

        // Dividers inside a conversation.
        let dividers = column()
            .w(px(560.))
            .gap(px(4.))
            .child(DividerLabel::new(("div-compact", k), "Context compacted"))
            .child(
                DividerLabel::new(("div-model", k), "Switched to Opus 4.1")
                    .pill()
                    .icon("cpu"),
            )
            .child(
                DividerLabel::new(("div-pending", k), "Haiku 4.5 on the next message")
                    .pill()
                    .icon("cpu")
                    .pending(true),
            )
            .child(
                row().child(
                    Button::new("div-replay", "Replay")
                        .size(ButtonSize::Small)
                        .icon("refresh")
                        .on_click(click(cx, |this: &mut Demos, _| this.replay_key += 1)),
                ),
            );

        // Banners above the composer, kept on screen while they leave.
        let offer = Presence::new("offer-presence", self.offer_shown)
            .exit(Exit::Sink)
            .child(
                Banner::new(("offer", k))
                    .icon("spark")
                    .title("Theme migration notes")
                    .text("Artifact published · open it in the browser?")
                    .action(
                        Button::new("offer-open", "Open")
                            .filled()
                            .trailing(Icon::new("export").size(px(13.)))
                            .on_click(click(cx, |this: &mut Demos, _| this.offer_shown = false)),
                    )
                    .action(
                        Button::new("offer-copy", "Copy link")
                            .text()
                            .on_click(click(cx, |this: &mut Demos, _| this.offer_shown = false)),
                    )
                    .on_dismiss(click(cx, |this: &mut Demos, _| this.offer_shown = false)),
            );
        let level = self.notice_level.max(self.last_notice_level);
        let notice_text = if level >= 2 {
            "The context is almost full: Claude will compact soon and may lose details. A new chat is recommended."
        } else {
            "This conversation is already long. For better results and lower usage, start a new chat."
        };
        let notice = Banner::new(("notice", level))
            .icon("gauge")
            .text(notice_text)
            .action(Chip::new("notice-new", "New chat"))
            .action(Chip::new("notice-compact", "Compact"))
            .dismiss_label("Hide notice")
            .on_dismiss(click(cx, |this: &mut Demos, _| this.notice_level = 0));
        let notice = if level >= 2 {
            notice.warning()
        } else {
            notice.subtle()
        };
        let notice = Presence::new("notice-presence", self.notice_level > 0)
            .exit(Exit::Sink)
            .child(notice);
        let banners = column().w(px(560.)).child(offer).child(notice).child(
            row()
                .child(
                    Button::new("offer-show", "Show artifact offer")
                        .size(ButtonSize::Small)
                        .on_click(click(cx, |this: &mut Demos, _| {
                            this.offer_shown = !this.offer_shown
                        })),
                )
                .child(
                    SegmentedButtons::new(
                        "notice-level",
                        [
                            Segment::label("No notice"),
                            Segment::label("Long"),
                            Segment::label("Almost full"),
                        ],
                        self.notice_level,
                    )
                    .on_change(update(cx, |this: &mut Demos, i: usize, _| {
                        this.notice_level = i;
                        if i > 0 {
                            this.last_notice_level = i;
                        }
                    })),
                ),
        );

        // Presence on its own: a chip that fades out before it is removed.
        let presence_demo = row()
            .child(
                Switch::new("presence-switch", self.presence_on)
                    .on_toggle(update(cx, |this: &mut Demos, on: bool, _| {
                        this.presence_on = on
                    })),
            )
            .when_some(
                gpui_m3::motion::presence("presence-chip", self.presence_on, window, cx),
                |d, p| {
                    d.child(Exit::Rise.apply(
                        div().child(
                            Chip::new("presence-chip-el", "Leaves before unmounting").icon("spark"),
                        ),
                        p,
                    ))
                },
            );

        // Drop zone around a composer-like area.
        let dropped = if self.dropped.is_empty() {
            "Drag files from your file manager onto the box.".to_string()
        } else {
            format!("Dropped: {}", self.dropped)
        };
        let drop = column()
            .w(px(560.))
            .child(
                DropZone::new("drop-zone")
                    .label("Drop to attach")
                    .on_drop({
                        let set = update(cx, |this: &mut Demos, names: SharedString, _| {
                            this.dropped = names
                        });
                        move |paths: &ExternalPaths, window, cx| {
                            let names = paths
                                .paths()
                                .iter()
                                .filter_map(|p| p.file_name())
                                .map(|n| n.to_string_lossy().into_owned())
                                .collect::<Vec<_>>()
                                .join(", ");
                            set(names.into(), window, cx)
                        }
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(px(110.))
                            .rounded(px(26.))
                            .bg(s.surface_container_high)
                            .text_color(s.on_surface_variant)
                            .child("Ask anything · drop files to attach"),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(s.on_surface_variant)
                    .child(dropped),
            );

        // Thumbnails: composer attachments (remove on hover) and a sent message.
        let mut thumbs = row().gap(px(8.));
        for (i, &photo) in self.thumbs.iter().enumerate() {
            thumbs = thumbs.child(
                ImageThumb::new(("thumb", photo), self.photos[photo].clone()).on_remove(click(
                    cx,
                    move |this: &mut Demos, _| {
                        if i < this.thumbs.len() {
                            this.thumbs.remove(i);
                        }
                    },
                )),
            );
        }
        let sent = Bubble::custom(BubbleKind::User)
            .child(
                row()
                    .gap(px(6.))
                    .mb(px(6.))
                    .child(ImageThumb::new("sent-0", self.photos[0].clone()).size(px(64.)))
                    .child(ImageThumb::new("sent-1", self.photos[1].clone()).size(px(64.))),
            )
            .child("Can you match these two palettes?");
        let images = row()
            .items_end()
            .gap(px(28.))
            .child(
                column().gap(px(8.)).child(thumbs).child(
                    Button::new("thumbs-restore", "Restore")
                        .size(ButtonSize::Small)
                        .on_click(click(cx, |this: &mut Demos, _| this.thumbs = vec![0, 1, 2])),
                ),
            )
            .child(sent);

        // Photo avatars and shapes that breathe when hovered.
        let avatars = row()
            .gap(px(20.))
            .child(
                Avatar::image(self.photos[2].clone())
                    .name("Ada Lovelace")
                    .size(px(64.))
                    .breathe_on_hover("ava-photo", ShapeName::Clover8),
            )
            .child(
                Avatar::image(self.photos[0].clone())
                    .name("Grace Hopper")
                    .size(px(40.))
                    .breathe_on_hover("ava-photo-small", ShapeName::Clover8),
            )
            .child(
                Avatar::image(std::path::PathBuf::from("missing-photo.png"))
                    .name("Grace Hopper")
                    .size(px(40.)),
            )
            .child(
                Shape::new(ShapeName::Cookie9)
                    .size(px(56.))
                    .color(s.tertiary_container)
                    .breathe_on_hover("shape-hover", ShapeName::Clover8)
                    .child(
                        Icon::new("spark")
                            .size(px(20.))
                            .color(s.on_tertiary_container),
                    ),
            );

        // @-mentions: a composer with an anchored suggestion list.
        let suggestions: Vec<Suggestion> = self
            .mention_matches
            .iter()
            .map(|path| {
                let name = path.rsplit('/').next().unwrap_or(path);
                Suggestion::new("file", name.to_string()).hint(path.to_string())
            })
            .collect();
        let list = (!suggestions.is_empty()).then(|| {
            div().absolute().top_0().left(px(12.)).child(
                SuggestionList::new("mentions", suggestions)
                    .query(self.mention_query.clone())
                    .selected(self.mention_nav.selected())
                    .width(420.)
                    .anchor(gpui::Corner::BottomLeft)
                    .on_hover(update(cx, |this: &mut Demos, row: usize, _| {
                        this.mention_nav.hover(row);
                    }))
                    .on_pick({
                        let view = cx.entity().downgrade();
                        move |row, _, cx| {
                            view.update(cx, |this, cx| this.pick_mention(row, cx)).ok();
                        }
                    }),
            )
        });
        let mentions = column()
            .w(px(560.))
            .child(
                div()
                    .relative()
                    .key_context("GalleryMentions")
                    .on_action(cx.listener(|this, _: &SuggestionNext, _, cx| {
                        this.mention_nav.next();
                        cx.notify();
                    }))
                    .on_action(cx.listener(|this, _: &SuggestionPrevious, _, cx| {
                        this.mention_nav.previous();
                        cx.notify();
                    }))
                    .on_action(cx.listener(|this, _: &SuggestionAccept, _, cx| {
                        if let Some(row) = this.mention_nav.current() {
                            this.pick_mention(row, cx);
                        }
                    }))
                    .on_action(cx.listener(|this, _: &SuggestionDismiss, _, cx| {
                        this.close_mentions(cx);
                    }))
                    .pt(px(15.))
                    .px(px(20.))
                    .pb(px(12.))
                    .rounded(px(26.))
                    .bg(s.surface_container_high)
                    .child(self.mention_input.clone())
                    .children(list),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(s.on_surface_variant)
                    .child("Type @ and part of a file name. ↑/↓ or the pointer move, Enter or Tab picks, Escape closes."),
            );

        page()
            .child(demo(
                "Divider labels",
                "Separators inside a conversation: a compaction (thin lines) and a model switch (wavy lines and a tonal pill; outlined and flowing while pending). The lines grow out from the label.",
                dividers,
            ))
            .child(demo(
                "Banners",
                "The artifact offer (tonal, shape mark, pill buttons) and the long-conversation notice (subtle, or tinted when the context is almost full). Both leave through Presence.",
                banners,
            ))
            .child(demo(
                "Presence",
                "Keeps an element on screen while it animates out with the fast effects spring, then removes it.",
                presence_demo,
            ))
            .child(demo(
                "Drop zone",
                "While files are dragged over the area, a \"Drop to attach\" layer springs in over it.",
                drop,
            ))
            .child(demo(
                "Image thumbnails",
                "Composer attachments (56 px, a round × on hover) and the images of a sent message (64 px).",
                images,
            ))
            .child(demo(
                "Photo avatars",
                "A photo clipped to an M3 shape with smooth edges; hover to morph Cookie9 → Clover8 and back. The third one falls back to initials (missing file).",
                avatars,
            ))
            .child(demo(
                "Mentions (autocomplete)",
                "An anchored suggestion list driven by the composer's keys; the matching part of each name is highlighted.",
                mentions,
            ))
            .into_any_element()
    }

    fn refresh_mentions(&mut self, cx: &mut Context<Self>) {
        let input = self.mention_input.read(cx);
        let text = input.text().to_string();
        let cursor = input.cursor_offset().min(text.len());
        let before = &text[..cursor];
        let at = before
            .rfind('@')
            .filter(|&i| i == 0 || before[..i].ends_with(char::is_whitespace));
        let query = at
            .map(|i| &before[i + 1..])
            .filter(|q| !q.contains(char::is_whitespace));
        match (at, query) {
            (Some(at), Some(query)) => {
                self.mention_matches =
                    rank_mentions(MENTION_FILES.iter().copied(), query, 8, |f| f);
                self.mention_query = query.to_string().into();
                self.mention_at = Some(at);
                self.mention_nav.reset(self.mention_matches.len());
            }
            _ => {
                self.mention_at = None;
                self.mention_matches.clear();
                self.mention_nav.reset(0);
            }
        }
        let open = !self.mention_matches.is_empty();
        self.mention_input
            .update(cx, |input, cx| input.set_suggesting(open, cx));
        cx.notify();
    }

    fn pick_mention(&mut self, row: usize, cx: &mut Context<Self>) {
        let (Some(at), Some(path)) = (self.mention_at, self.mention_matches.get(row).copied())
        else {
            return;
        };
        let cursor = self.mention_input.read(cx).cursor_offset();
        self.mention_input.update(cx, |input, cx| {
            input.replace_range(at..cursor, &format!("@{path} "), cx)
        });
        self.close_mentions(cx);
    }

    fn close_mentions(&mut self, cx: &mut Context<Self>) {
        self.mention_at = None;
        self.mention_matches.clear();
        self.mention_nav.reset(0);
        self.mention_input
            .update(cx, |input, cx| input.set_suggesting(false, cx));
        cx.notify();
    }

    fn motion(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
        let springs: [(&str, Spring, Hsla); 6] = [
            ("Spatial fast", Spring::SPATIAL_FAST, s.primary),
            ("Spatial", Spring::SPATIAL, s.tertiary),
            (
                "Spatial slow",
                Spring::SPATIAL_SLOW,
                s.on_secondary_container,
            ),
            ("Effects fast", Spring::EFFECTS_FAST, s.outline),
            ("Effects", Spring::EFFECTS, s.error),
            ("Effects slow", Spring::EFFECTS_SLOW, s.success),
        ];

        let plot = {
            let (grid, text) = (s.outline_variant, s.on_surface_variant);
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let (x0, y0) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
                    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                    let base = y0 + h * 0.85;
                    let top = y0 + h * 0.15;
                    for y in [base, top] {
                        let mut b = PathBuilder::stroke(px(1.));
                        b.move_to(point(px(x0), px(y)));
                        b.line_to(point(px(x0 + w), px(y)));
                        if let Ok(p) = b.build() {
                            window.paint_path(p, if y == top { text } else { grid });
                        }
                    }
                    for (_, spring, color) in springs {
                        let mut b = PathBuilder::stroke(px(2.));
                        let steps = 160;
                        for i in 0..=steps {
                            let t = i as f32 / steps as f32 * 0.8;
                            let v = spring.step(t);
                            let p: Point<gpui::Pixels> = point(
                                px(x0 + w * i as f32 / steps as f32),
                                px(base - (base - top) * v),
                            );
                            if i == 0 { b.move_to(p) } else { b.line_to(p) }
                        }
                        if let Ok(p) = b.build() {
                            window.paint_path(p, color);
                        }
                    }
                },
            )
            .w(px(520.))
            .h(px(200.))
        };

        let mut legend = column().gap(px(6.));
        let mut tracks = column().gap(px(10.)).w(px(520.));
        for (i, (name, spring, color)) in springs.iter().enumerate() {
            legend = legend.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().size(px(10.)).rounded(px(5.)).bg(*color))
                    .child(format!(
                        "{name} · stiffness {} · damping {}",
                        spring.stiffness, spring.damping_ratio
                    )),
            );
            let x = animate_f32(
                key(&ElementId::from(("motion", i)), "x"),
                *spring,
                if self.motion_flip { 1. } else { 0. },
                window,
                cx,
            );
            tracks = tracks.child(
                div()
                    .relative()
                    .h(px(28.))
                    .rounded(px(14.))
                    .bg(s.surface_container_high)
                    .child(
                        div()
                            .absolute()
                            .top(px(4.))
                            .left(px(4. + x * (520. - 28.)))
                            .size(px(20.))
                            .rounded(px(10.))
                            .bg(*color),
                    ),
            );
        }

        page()
            .child(demo(
                "M3 Expressive springs",
                "Step response over 0.8 s. Spatial springs bounce (position, size, shape); effects springs do not (color, opacity).",
                row().items_start().gap(px(24.)).child(plot).child(legend),
            ))
            .child(demo(
                "Compare them in motion",
                "Press repeatedly: the spring keeps its velocity when the target changes. Use \"Slow ×5\" above to watch it slowly.",
                column()
                    .items_start()
                    .child(Button::new("flip", "Animate").filled().icon("refresh").on_click(click(cx, |this, _| this.motion_flip = !this.motion_flip)))
                    .child(tracks),
            ))
            .child(self.theme_motion(cx))
            .child(self.small_motion(cx))
            .into_any_element()
    }

    fn theme_motion(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let dark = Theme::get(cx).dark;
        let reveal = {
            let view = cx.entity().downgrade();
            move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                let dark = !Theme::get(cx).dark;
                let scheme = Theme::new(dark, None).scheme;
                theme_reveal(scheme, dark, window, cx);
                view.update(cx, |_, cx| cx.notify()).ok();
            }
        };
        demo(
            "Theme changes",
            "theme_reveal: a circle of the new background grows from the click (GPUI cannot capture the old frame, so it covers instead of revealing). animate_scheme / animate_theme: every color blends for 0.45 s. The toolbar's mode switch and seed swatches use them too.",
            row()
                .child(
                    Button::new("reveal", if dark { "Reveal light" } else { "Reveal dark" })
                        .filled()
                        .icon(if dark { "sun" } else { "moon" })
                        .on_click(reveal),
                )
                .child(
                    Button::new("blend", "Blend accent")
                        .icon("palette")
                        .on_click(click(cx, move |this: &mut Demos, cx| {
                            this.accent_step = (this.accent_step + 1) % SEEDS.len();
                            let seed =
                                (this.accent_step != 0).then(|| hex(SEEDS[this.accent_step]));
                            animate_theme(Theme::get(cx).dark, seed, cx);
                        })),
                )
                .child(div().size(px(28.)).rounded(px(14.)).bg(s.primary))
                .child(
                    div()
                        .size(px(28.))
                        .rounded(px(14.))
                        .bg(s.tertiary_container),
                )
                .child(
                    div()
                        .size(px(28.))
                        .rounded(px(14.))
                        .bg(s.secondary_container),
                ),
        )
    }

    fn small_motion(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let mut avatars = row().gap(px(18.));
        for (i, name) in ["the reference design", "Bruno", "Carla", "Diego"].into_iter().enumerate() {
            avatars = avatars.child(Avatar::new(name).size(px(36.)).hover_spin(("spin", i)));
        }
        let pop = self.pop_key;
        let stagger = self.stagger_key;
        column()
            .gap(px(28.))
            .child(demo(
                "Icon pop and avatar spin",
                "Icon::pop grows from 20 % while turning from -60° (a copy check, an unread dot). Avatar::hover_spin turns 40° and grows 8 % on hover; the initial stays upright.",
                row()
                    .gap(px(28.))
                    .child(
                        row()
                            .child(Button::new("pop-again", "Copy").icon("copy").on_click(click(cx, |this: &mut Demos, _| this.pop_key += 1)))
                            .when(pop > 0, |d| {
                                d.child(Icon::new("check").size(px(22.)).color(s.primary).pop(("pop", pop)))
                            }),
                    )
                    .child(avatars),
            ))
            .child(demo(
                "Staggered menu items",
                "Menu items enter one by one, 25 ms apart (Menu::stagger, on by default).",
                column()
                    .items_start()
                    .child(Button::new("stagger-again", "Replay").icon("refresh").on_click(click(cx, |this: &mut Demos, _| this.stagger_key += 1)))
                    .child(
                        Menu::new(("stagger-menu", stagger))
                            .width(240.)
                            .item(MenuItem::new(("st", 0usize), "Rename").icon("pen"))
                            .item(MenuItem::new(("st", 1usize), "Duplicate").icon("copy"))
                            .item(MenuItem::new(("st", 2usize), "Export").icon("export"))
                            .item(MenuItem::new(("st", 3usize), "Pin").icon("star"))
                            .separator()
                            .item(MenuItem::new(("st", 4usize), "Delete").icon("trash").danger(true)),
                    ),
            ))
            .child(demo(
                "Expandable card height",
                "The body opens and closes with the spatial spring instead of only fading. Dialogs and toasts (Text and feedback) now enter growing with large corners.",
                div().w(px(420.)).child(
                    ExpandableCard::new("long-card")
                        .open(self.long_card_open)
                        .header(div().child("Read 4 files"))
                        .on_toggle(update(cx, |this: &mut Demos, open: bool, _| this.long_card_open = open))
                        .child("src/components/code_editor.rs")
                        .child("src/components/code_buffer.rs")
                        .child("src/components/resize.rs")
                        .child("src/components/reorder.rs"),
                ),
            ))
    }
}

impl Demos {
    fn editor_section(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
        let editor = self.editor.read(cx);
        let context = editor.selection_context();
        let status = format!(
            "{} · {} · {} saves · {} lines",
            context.label(),
            if editor.is_dirty() {
                "modified"
            } else {
                "saved"
            },
            self.editor_saves,
            editor.buffer().line_count(),
        );

        // Open files: a reorderable list in the side pane.
        let mut files = ReorderList::new("open-files").gap(px(2.));
        for name in &self.open_files {
            files = files.row(
                *name,
                0,
                ListItem::new(ElementId::Name(format!("file-{name}").into()), *name)
                    .leading(Icon::new("file").size(px(16.)).color(s.on_surface_variant)),
            );
        }
        let files = files.on_reorder(update(cx, |this: &mut Demos, e: ReorderEvent, _| {
            apply_reorder(&mut this.open_files, e)
        }));
        let side = div()
            .size_full()
            .p(px(8.))
            .bg(s.surface_container)
            .child(files);
        let terminal = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(s.surface_container_high)
            .px(px(14.))
            .py(px(8.))
            .gap(px(4.))
            .font_family(gpui_m3::theme::MONO_FONT_FAMILY)
            .text_size(px(12.))
            .text_color(s.on_surface_variant)
            .child(div().text_color(s.primary).child("Terminal"))
            .child("$ cargo test --lib")
            .child("test result: ok. 49 passed; 0 failed");
        let workbench = div()
            .w_full()
            .h(px(480.))
            .rounded(px(20.))
            .overflow_hidden()
            .bg(s.surface)
            .child(
                Splitter::horizontal("wb-side", self.side_width)
                    .min(px(140.))
                    .max(px(320.))
                    .on_resize(update(cx, |this: &mut Demos, w: Pixels, _| {
                        this.side_width = w
                    }))
                    .first(side)
                    .second(
                        Splitter::vertical("wb-term", self.term_height)
                            .invert(true)
                            .min(px(72.))
                            .max(px(300.))
                            .on_resize(update(cx, |this: &mut Demos, h: Pixels, _| {
                                this.term_height = h
                            }))
                            .first(div().size_full().pt(px(6.)).child(self.editor.clone()))
                            .second(terminal),
                    ),
            );

        // Projects with a favorites group.
        let mut projects = ReorderList::new("projects").header("Favorites");
        for name in &self.favorite_projects {
            projects = projects.row(*name, 0, project_row(name, true));
        }
        projects = projects.header("Projects");
        for name in &self.other_projects {
            projects = projects.row(*name, 1, project_row(name, false));
        }
        let projects = projects.on_reorder(update(cx, |this: &mut Demos, e: ReorderEvent, _| {
            let favs = this.favorite_projects.len();
            if e.from < favs {
                apply_reorder(&mut this.favorite_projects, e);
            } else {
                apply_reorder(
                    &mut this.other_projects,
                    ReorderEvent {
                        from: e.from - favs,
                        to: e.to - favs,
                    },
                );
            }
        }));

        let open_cropper = {
            let view = cx.entity().downgrade();
            move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| this.open_cropper(window, cx))
                    .ok();
            }
        };
        let cropped = self.cropped.clone();
        page()
            .child(demo(
                "Code editor",
                "CodeEditor: line numbers, a virtualized list (1,600+ lines), selection with mouse and keys, clipboard, undo/redo, Tab / Shift+Tab, auto-indent, bracket pairs, Ctrl+S and a LineHighlighter hook. Drag the dividers: the side pane and the terminal resize (Splitter).",
                column()
                    .w_full()
                    .child(workbench)
                    .child(div().text_size(px(12.5)).text_color(s.on_surface_variant).child(status)),
            ))
            .child(demo(
                "Reorder with groups",
                "Drag a project: favorites stay with favorites. A line marks where it lands and the rows slide into place. Avatars spin on hover.",
                div().w(px(300.)).child(projects),
            ))
            .child(demo(
                "Image cropper",
                "Drag to move, wheel or slider to zoom, Enter to save and Esc to cancel. Outside the shape the photo is dimmed.",
                row()
                    .child(Button::new("crop", "Frame a photo").filled().icon("palette").on_click(open_cropper))
                    .when_some(cropped, |d, image| {
                        d.child(Avatar::image(image).size(px(56.)))
                    }),
            ))
            .when_some(self.cropper.clone(), |d, cropper| {
                let confirm = cropper.clone();
                let cancel = cropper.clone();
                d.child(
                    Dialog::new("crop-dialog")
                        .title("Frame your photo")
                        .width(px(340.))
                        .child(cropper)
                        .action(Button::new("crop-cancel", "Cancel").text().on_click(move |_, _, cx| {
                            cancel.update(cx, |c, cx| c.cancel(cx));
                        }))
                        .action(Button::new("crop-save", "Save").filled().on_click(move |_, _, cx| {
                            confirm.update(cx, |c, cx| c.confirm(cx));
                        })),
                )
            })
            .into_any_element()
    }

    fn open_cropper(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cropper = cx.new(|cx| ImageCropper::new(demo_photo_sized(0.6, 512), cx));
        cx.subscribe(
            &cropper,
            |this: &mut Demos, _, event: &ImageCropperEvent, cx| {
                if let ImageCropperEvent::Cropped(image) = event {
                    this.cropped = Some(image.clone());
                }
                this.cropper = None;
                cx.notify();
            },
        )
        .detach();
        cropper.read(cx).focus(window);
        self.cropper = Some(cropper);
        cx.notify();
    }
}

fn project_row(name: &'static str, favorite: bool) -> impl IntoElement {
    ListItem::new(ElementId::Name(format!("project-row-{name}").into()), name)
        .leading(
            Avatar::new(name)
                .size(px(24.))
                .hover_spin(ElementId::Name(format!("project-spin-{name}").into())),
        )
        .when(favorite, |item| {
            item.trailing(Icon::new("star-fill").size(px(14.)))
        })
}

fn page() -> gpui::Div {
    div().flex().flex_col().gap(px(28.)).pt(px(4.))
}

fn row() -> gpui::Div {
    div().flex().flex_wrap().items_center().gap(px(12.))
}

fn column() -> gpui::Div {
    div().flex().flex_col().gap(px(12.))
}

fn demo(
    title: impl Into<SharedString>,
    note: impl Into<SharedString>,
    body: impl IntoElement,
) -> impl IntoElement {
    DemoBlock {
        title: title.into(),
        note: note.into(),
        body: body.into_any_element(),
    }
}

#[derive(IntoElement)]
struct DemoBlock {
    title: SharedString,
    note: SharedString,
    body: AnyElement,
}

impl gpui::RenderOnce for DemoBlock {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight(700.))
                            .child(self.title),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(s.on_surface_variant)
                            .child(self.note),
                    ),
            )
            .child(self.body)
    }
}

fn demo_note(text: &'static str, cx: &App) -> impl IntoElement {
    let s = Theme::of(cx);
    div()
        .max_w(px(260.))
        .p(px(14.))
        .rounded(px(16.))
        .bg(s.surface_container)
        .text_size(px(12.5))
        .text_color(s.on_surface_variant)
        .child(text)
}

fn main() {
    Application::new()
        .with_assets(gpui_m3::Assets)
        .run(|cx: &mut App| {
            gpui_m3::init(cx);
            // Tab moves the keyboard focus (expandable card headers show their focus ring).
            cx.bind_keys([
                // Under the root's context, not globally: a context-less binding would win over
                // the code editor's own Tab (indent).
                KeyBinding::new("tab", FocusNext, Some("Gallery")),
                KeyBinding::new("shift-tab", FocusPrevious, Some("Gallery")),
            ]);
            // M3_SECTION=<n> opens a section directly (handy for screenshots).
            // M3_REDUCED=1 disables animations (useful for measuring idle cost).
            if std::env::var("M3_REDUCED").is_ok_and(|v| v == "1") {
                cx.set_global(MotionSettings {
                    time_scale: 1.,
                    reduced: true,
                });
            }
            // M3_SIZE=<width>x<height> sets the window size (for screenshots).
            let (width, height) = std::env::var("M3_SIZE")
                .ok()
                .and_then(|v| {
                    let (w, h) = v.split_once('x')?;
                    Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?))
                })
                .unwrap_or((1280., 860.));
            let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("M3 Expressive · GPUI".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        let gallery = Gallery::new(window, cx);
                        gallery.apply_theme(cx);
                        window.focus(&gallery.focus);
                        if let Some(section) = std::env::var("M3_SECTION")
                            .ok()
                            .and_then(|v| v.parse::<usize>().ok())
                        {
                            gallery
                                .demos
                                .update(cx, |d, _| d.section = section.min(SECTIONS.len() - 1));
                        }
                        gallery
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
