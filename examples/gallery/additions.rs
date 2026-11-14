//! "New in 0.2.1" section: status marks, empty states, still progress, JSON output and the new
//! options of buttons, lists, toolbars and cards.
//!
//! `M3_PART=<n>` starts at the n-th demo (for screenshots).

use gpui::{
    App, ClickEvent, Context, ElementId, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, px,
};
use gpui_m3::{
    Button, ButtonGroup, ButtonGroupItem, ButtonVariant, Chip, CodeOutput, EmptyState,
    ExpandableCard, Fab, HiddenContent, IconButton, ListItem, MenuItem, PageIndicator, Ring,
    ShapeName, ShimmerText, SplitButton, Status, StatusMark, Theme, Toolbar,
    ToolbarItem, Tone, WavyCircularProgress, WavyProgress,
    motion::{Enter, SharedAxis, stagger},
};

use super::{column, demo, page, row};

const STATUSES: [Status; 5] = [
    Status::Pending,
    Status::Running,
    Status::Done,
    Status::Failed,
    Status::Waiting,
];

const STATUS_NAMES: [&str; 5] = ["Pending", "Running", "Done", "Failed", "Waiting"];

const JSON: &str = r#"{"tool":"read_file","input":{"path":"src/components/status.rs","lines":[1,40],"follow":true},"ok":true,"elapsed_ms":12.5,"notes":null}"#;

const LONG_LINE: &str = "warning: this line is long and has no line breaks, so max_lines alone would not shorten it; max_chars cuts it after a number of characters and the row below shows the rest when asked. Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.";

const PAGES: [(&str, &str); 3] = [
    ("Overview", "Shared axis X: the content comes from the side you move toward."),
    ("Details", "Each change of page plays the entrance once, keyed by the page."),
    ("Activity", "With reduced motion it simply appears."),
];

pub struct AdditionDemos {
    status: [usize; 3],
    progress: f32,
    page: usize,
    previous_page: usize,
    selected_row: usize,
    swapped: bool,
    split_open: bool,
    model: usize,
    busy: bool,
    fab_busy: bool,
    card_open: bool,
}

impl AdditionDemos {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            status: [0, 2, 3],
            progress: 0.62,
            page: 0,
            previous_page: 0,
            selected_row: 1,
            swapped: false,
            split_open: false,
            model: 0,
            busy: false,
            fab_busy: false,
            card_open: false,
        }
    }

    fn on(
        cx: &mut Context<Self>,
        f: impl Fn(&mut Self) + 'static,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |_, _, cx| {
            this.update(cx, |this, cx| {
                f(this);
                cx.notify();
            })
            .ok();
        }
    }

    fn statuses(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut marks = row().gap(px(20.));
        for (i, index) in self.status.iter().enumerate() {
            let status = STATUSES[*index];
            marks = marks.child(
                row()
                    .gap(px(8.))
                    .child(StatusMark::new(("status-mark", i), status).size(px(22.)).spinner(i == 0))
                    .child(div().w(px(64.)).child(STATUS_NAMES[*index]))
                    .child(
                        Button::new(("status-next", i), "Next")
                            .text()
                            .on_click(Self::on(cx, move |this| {
                                this.status[i] = (this.status[i] + 1) % STATUSES.len();
                            })),
                    ),
            );
        }
        demo(
            "StatusMark",
            "Morphs once to the new status with the fast spatial spring and settles. The first one draws its spinner while running (spinner(true)); the others leave an empty slot for an indicator layer.",
            marks,
        )
    }

    fn empty_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        demo(
            "EmptyState",
            "A still hero shape that morphs on hover (breathe_on_hover), a title, text and an action.",
            div().rounded(px(24.)).bg(s.surface_container_low).child(
                EmptyState::new("empty-chats")
                    .shape(ShapeName::Cookie9)
                    .breathe_on_hover(ShapeName::Clover8)
                    .icon("chat")
                    .title("No chats yet")
                    .text("Start a chat to see it here. Your history stays on this device.")
                    .action(Button::new("empty-new", "New chat").filled().icon("plus")),
            ),
        )
    }

    fn progress(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let value = self.progress;
        demo(
            "Still wavy progress, ring colors",
            "WavyProgress::still and WavyCircularProgress::still request no continuous frames: the fill moves only when the value changes. Ring::warn_at(0.8) turns the arc to the warning color.",
            column()
                .child(div().w(px(320.)).child(WavyProgress::new("wavy-still", Some(value)).still(true)))
                .child(
                    row()
                        .child(WavyCircularProgress::new("wavy-circle-still", Some(value)).still(true))
                        .child(
                            WavyCircularProgress::new("wavy-circle-tertiary", Some(value))
                                .still(true)
                                .color(Theme::of(cx).tertiary),
                        )
                        .child(Ring::new("ring-warn", value).size(px(28.)).stroke(4.).warn_at(0.8))
                        .child(
                            Button::new("progress-less", "−10 %")
                                .outlined()
                                .on_click(Self::on(cx, |this| this.progress = (this.progress - 0.1).max(0.))),
                        )
                        .child(
                            Button::new("progress-more", "+10 %")
                                .outlined()
                                .on_click(Self::on(cx, |this| this.progress = (this.progress + 0.1).min(1.))),
                        ),
                ),
        )
    }

    fn shimmer_and_pages(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let (title, text) = PAGES[self.page];
        let forward = self.page >= self.previous_page;
        let content = div()
            .w(px(360.))
            .p(px(16.))
            .rounded(px(16.))
            .bg(Theme::of(cx).surface_container)
            .child(div().font_weight(gpui::FontWeight(700.)).child(title))
            .child(div().text_size(px(12.5)).child(text));
        let content = Enter::new("pages-enter")
            .shared_axis(SharedAxis::X, forward)
            .on_change(self.page as u64)
            .apply(content, window, cx);
        demo(
            "ShimmerText, PageIndicator, Enter",
            "The shimmer runs at 15 fps (plain text with reduced motion). The page indicator stretches the current dot with a spring; the card enters with motion::Enter::shared_axis.",
            column()
                .child(ShimmerText::new("thinking", "Thinking about your request…"))
                .child(content)
                .child(PageIndicator::new("pages", PAGES.len(), self.page).on_select(
                    move |page, _, cx| {
                        this.update(cx, |this, cx| {
                            this.previous_page = this.page;
                            this.page = page;
                            cx.notify();
                        })
                        .ok();
                    },
                )),
        )
    }

    fn output(&self) -> impl IntoElement {
        let spanish = |hidden| match hidden {
            HiddenContent::Lines(n) => SharedString::from(format!("Ver todo ({n} líneas más)")),
            HiddenContent::Chars(n) => format!("Ver todo ({n} caracteres más)").into(),
            _ => "Ver todo".into(),
        };
        let body = column()
            .w(px(520.))
            .child(CodeOutput::new("json-output", JSON).json(true).max_lines(8).show_all_label(spanish))
            .child(CodeOutput::new("long-output", LONG_LINE).max_chars(120).show_all_label(spanish));
        #[cfg(feature = "json")]
        let body = body.child(json_view());
        demo(
            "CodeOutput::json, max_chars, show_all_label; JsonView",
            "A one-line JSON pretty printed and cut by lines, a long line cut by characters, with the row in Spanish. JsonView (feature json) folds objects and arrays.",
            body,
        )
    }

    fn buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let models = ["Opus", "Sonnet", "Haiku"];
        let mut menu = Vec::new();
        for (i, name) in models.iter().enumerate() {
            menu.push(
                MenuItem::new(("split-model", i), *name)
                    .selected(self.model == i)
                    .on_click(Self::on(cx, move |this| {
                        this.model = i;
                        this.split_open = false;
                    })),
            );
        }
        demo(
            "Buttons",
            "ButtonGroupItem::variant, a filled SplitButton with an icon and its menu, IconButton::icon_swap, and busy buttons and FABs.",
            column()
                .child(
                    row()
                        .child(ButtonGroup::new(
                            "group-variants",
                            [
                                ButtonGroupItem::label("Cancel"),
                                ButtonGroupItem::label("Save draft"),
                                ButtonGroupItem::label("Publish").variant(ButtonVariant::Filled),
                            ],
                        ))
                        .child(
                            SplitButton::new("split-filled", models[self.model])
                                .filled()
                                .icon("spark")
                                .open(self.split_open)
                                .menu(menu)
                                .on_toggle(Self::on(cx, |this| this.split_open = !this.split_open))
                                .on_dismiss(Self::on(cx, |this| this.split_open = false)),
                        )
                        .child(SplitButton::new("split-outlined", "Allow").outlined()),
                )
                .child(
                    row()
                        .child(
                            IconButton::new("swap", if self.swapped { "moon" } else { "sun" })
                                .tonal()
                                .icon_swap(true)
                                .tooltip("Swap icon")
                                .on_click(Self::on(cx, |this| this.swapped = !this.swapped)),
                        )
                        .child(
                            IconButton::new("swap-copy", if self.swapped { "check" } else { "copy" })
                                .icon_swap(true)
                                .tooltip("Copy"),
                        )
                        .child(
                            Button::new("busy-button", if self.busy { "Saving" } else { "Save" })
                                .filled()
                                .icon("check")
                                .busy(self.busy)
                                .on_click(Self::on(cx, |this| this.busy = true)),
                        )
                        .child(
                            Button::new("busy-reset", "Reset")
                                .text()
                                .on_click(Self::on(cx, |this| {
                                    this.busy = false;
                                    this.fab_busy = false;
                                })),
                        ),
                )
                .child(
                    row()
                        .child(
                            Fab::new("fab-busy", "plus")
                                .busy(self.fab_busy)
                                .on_click(Self::on(cx, |this| this.fab_busy = true)),
                        )
                        .child(Fab::new("fab-secondary", "pen").color(Tone::Secondary))
                        .child(Fab::new("fab-tertiary", "spark").color(Tone::Tertiary))
                        .child(Fab::new("fab-disabled", "trash").disabled(true)),
                ),
        )
    }

    fn lists(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = column().gap(px(2.)).w(px(420.));
        for (i, (overline, title, supporting)) in [
            ("Today", "Design review", "Go over the new status marks and the empty state before the release."),
            ("Yesterday", "Release notes", "Write the 0.2.1 notes in Spanish and English."),
            ("Last week", "Performance pass", "Check that nothing redraws continuously at rest."),
        ]
        .into_iter()
        .enumerate()
        {
            list = list.child(
                ListItem::new(("rich-row", i), title)
                    .overline(overline)
                    .supporting(supporting)
                    .selected(self.selected_row == i)
                    .hover_actions([
                        IconButton::new(("row-pin", i), "star").size(px(28.)).tooltip("Pin"),
                        IconButton::new(("row-delete", i), "trash").size(px(28.)).tooltip("Delete"),
                    ])
                    .on_click(Self::on(cx, move |this| this.selected_row = i)),
            );
        }
        demo(
            "ListItem",
            "selected (secondary container and tighter corners, with springs), overline, supporting text and hover_actions.",
            list,
        )
    }

    fn toolbars_and_cards(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        demo(
            "Dense toolbar, error card, danger chip",
            "Toolbar::dense (48 px, 32 px buttons) with a custom item, ExpandableCard::error and Chip::danger.",
            column()
                .child(
                    row()
                        .child(
                            Toolbar::floating()
                                .dense()
                                .item(ToolbarItem::new("dense-bold", "pen").selected(true))
                                .item(ToolbarItem::new("dense-copy", "copy"))
                                .item(ToolbarItem::custom(
                                    div()
                                        .px(px(6.))
                                        .text_size(px(12.5))
                                        .text_color(s.on_surface_variant)
                                        .child("3 selected"),
                                ))
                                .item(ToolbarItem::new("dense-trash", "trash")),
                        )
                        .child(
                            Toolbar::floating()
                                .dense()
                                .vibrant()
                                .item(ToolbarItem::new("dense-v-search", "search"))
                                .item(ToolbarItem::new("dense-v-star", "star").selected(true)),
                        ),
                )
                .child(
                    div().w(px(420.)).child(
                        ExpandableCard::new("error-card")
                            .error(true)
                            .open(self.card_open)
                            .header(div().child("Bash failed (exit 1)"))
                            .on_toggle({
                                let this = cx.entity().downgrade();
                                move |open, _, cx| {
                                    this.update(cx, |this, cx| {
                                        this.card_open = open;
                                        cx.notify();
                                    })
                                    .ok();
                                }
                            })
                            .child(CodeOutput::new("error-card-out", "error: could not compile `atic`").error(true)),
                    ),
                )
                .child(
                    row()
                        .child(Chip::new("chip-danger", "Delete branch").icon("trash").danger(true))
                        .child(Chip::new("chip-plain", "Keep branch").icon("branch")),
                ),
        )
    }
}

#[cfg(feature = "json")]
fn json_view() -> impl IntoElement {
    let value: serde_json::Value = serde_json::from_str(JSON).unwrap_or_default();
    gpui_m3::JsonView::new("json-view", &value).max_depth(1).max_items(3)
}

impl Render for AdditionDemos {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let blocks: Vec<gpui::AnyElement> = vec![
            self.statuses(cx).into_any_element(),
            self.empty_state(cx).into_any_element(),
            self.progress(cx).into_any_element(),
            self.shimmer_and_pages(window, cx).into_any_element(),
            self.output().into_any_element(),
            self.buttons(cx).into_any_element(),
            self.lists(cx).into_any_element(),
            self.toolbars_and_cards(cx).into_any_element(),
        ];
        // M3_PART=<n> starts at the n-th demo (for screenshots).
        let first = std::env::var("M3_PART")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let mut out = page();
        for (i, block) in blocks.into_iter().enumerate().skip(first) {
            let block = Enter::new(ElementId::NamedInteger("addition-in".into(), i as u64))
                .delay(stagger(i, 0.03, 5))
                .apply(div().child(block), window, cx);
            out = out.child(block);
        }
        out
    }
}
