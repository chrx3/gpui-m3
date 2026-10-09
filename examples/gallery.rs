//! Gallery for trying out each component and comparing it with the reference design.
//!
//! `cargo run --example gallery`

use gpui::{
    AnyElement, AnyView, App, AppContext, Application, Bounds, Context, ElementId, Entity,
    FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement, PathBuilder,
    Pixels, Point, Render, SharedString, StatefulInteractiveElement, StyleRefinement, Styled,
    TitlebarOptions, Window, WindowAppearance, WindowBounds, WindowOptions, anchored, canvas,
    deferred, div, point, prelude::FluentBuilder, px, size,
};
use gpui_m3::{
    Avatar, Badge, Bubble, BubbleKind, Button, ButtonSize, Card, Chip, Dialog, Fab, IconButton,
    IconTab, IconTabs, LinearProgress, ListGroup, LoadingIndicator, Menu, MenuItem, MotionSettings,
    NavItem, RailItem, Ring, Segment, SegmentedButtons, Shape, ShapeName, SplitButton, Spring,
    StopSlider, Switch, TextField, TextFieldEvent, Theme, Toast, Tone, WavyProgress, context_menu,
    motion::{animate_f32, key},
    theme::{FONT_FAMILY, alpha, hex},
};

const SECTIONS: [(&str, &str); 9] = [
    ("Buttons", "bolt"),
    ("Selection", "check"),
    ("Navigation", "panel-left"),
    ("Menus", "layers"),
    ("Shapes", "spark"),
    ("Progress", "gauge"),
    ("Surfaces", "blocks"),
    ("Text and feedback", "chat"),
    ("Motion", "refresh"),
];

/// the reference design's seed colors for M3 Expressive; the first one is the base scheme.
const SEEDS: [&str; 9] = [
    "#6750a4", "#0b57d0", "#006a6a", "#386a20", "#7d5700", "#b3261e", "#984061", "#5b5f97",
    "#00639b",
];

const MODELS: [&str; 3] = ["Opus 4.1", "Sonnet 4.5", "Haiku 4.5"];

const EFFORTS: [&str; 5] = ["Low", "Medium", "High", "Very high", "Max"];

/// Window root: sidebar and toolbar. The content lives in a separate cached view, so the
/// logo and toolbar animations do not rebuild it.
struct Gallery {
    dark: bool,
    seed: usize,
    slow: bool,
    demos: Entity<Demos>,
    logo: Entity<Logo>,
}

/// Content of the current section and the state of its demos.
struct Demos {
    section: usize,
    toggles: [bool; 4],
    icon_toggle: bool,
    switches: [bool; 3],
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
    name: Entity<TextField>,
    submitted: SharedString,
    dialog_open: bool,
    toast_open: bool,
    context_menu: Option<Point<Pixels>>,
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
        }
    }

    fn apply_theme(&self, cx: &mut App) {
        let seed = if self.seed == 0 {
            None
        } else {
            Some(hex(SEEDS[self.seed]))
        };
        Theme::set(self.dark, seed, cx);
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
        Self {
            search,
            name,
            submitted: SharedString::default(),
            dialog_open: false,
            toast_open: false,
            context_menu: None,
            section: 0,
            toggles: [true, false, false, false],
            icon_toggle: true,
            switches: [true, false, true],
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
            .bg(s.surface_container)
            .text_color(s.on_surface)
            .font_family(FONT_FAMILY)
            .text_size(px(13.5))
            .pr(px(8.))
            .pb(px(8.))
            .pt(px(8.))
            .child(sidebar)
            .child(panel)
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
            7 => self.inputs(cx),
            _ => self.motion(window, cx),
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
                        this.apply_theme(cx);
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
                .on_change(update(cx, |this, i: usize, cx| {
                    this.dark = i == 1;
                    this.apply_theme(cx);
                })),
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
            .child(div().h(px(220.)).text_color(s.on_surface_variant))
            .into_any_element()
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

    fn inputs(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = *Theme::of(cx);
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
            .into_any_element()
    }
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
            // M3_REDUCED=1 disables animations (useful for measuring idle cost).
            if std::env::var("M3_REDUCED").is_ok_and(|v| v == "1") {
                cx.set_global(MotionSettings {
                    time_scale: 1.,
                    reduced: true,
                });
            }
            let bounds = Bounds::centered(None, size(px(1280.), px(860.)), cx);
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
                        gallery
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
