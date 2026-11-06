//! "Top app bars and search" section: the top app bars (with a scrolling frame that collapses
//! them), the docked search bar and the search view.

use gpui::{
    AnyElement, App, AppContext as _, Context, Div, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, ScrollWheelEvent, SharedString, Styled, Window, div, px,
};
use gpui_m3::{
    Avatar, Icon, IconButton, ListItem, Scheme, Search, SearchBar, SearchEvent, SearchView,
    Segment, SegmentedButtons, Slider, Theme, TopAppBar, TopAppBarSize, collapse_progress,
    collapse_range, search_field,
};

use super::{column, demo, page, row, update};

const ITEMS: [(&str, &str); 12] = [
    ("Quarterly report", "Docs · edited yesterday"),
    ("Theme migration", "Conversation · 2 h ago"),
    ("Release checklist", "Notes · Monday"),
    ("Rust compiler flags", "Bookmark"),
    ("Gallery screenshots", "Folder · 24 files"),
    ("Seed color study", "Palette · 9 colors"),
    ("Motion tokens", "Docs · edited last week"),
    ("Roadmap 2027", "Notes · 3 weeks ago"),
    ("Release notes 0.2", "Docs · today"),
    ("Spring tuning", "Conversation · yesterday"),
    ("Shape library", "Docs · edited Tuesday"),
    ("Icon audit", "Notes · last month"),
];

/// Height of the frame with the scrolling bar.
const FRAME_HEIGHT: f32 = 480.;
/// Rows of the scrolling page, and how far they scroll once the bar has collapsed.
const PAGE_ROWS: usize = 12;
const PAGE_EXTRA_SCROLL: f32 = 110.;

pub struct SearchDemos {
    /// Option of the frame with the scrolling bar (small, centered, medium, large).
    size: usize,
    /// How far that frame scrolled, in px: the bar collapses first, then the rows move.
    scroll: f32,
    /// The docked bar that opens over the page.
    docked: Entity<Search>,
    /// A view that starts open.
    open: Entity<Search>,
    /// A bar that filters in place and never opens.
    filter: Entity<Search>,
    /// The flat row of a search that takes over a page.
    page: Entity<Search>,
    focused_filter: bool,
}

impl SearchDemos {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let make = |placeholder: &'static str, text: &str, expanded: bool, cx: &mut Context<Self>| {
            let field = cx.new(|cx| search_field(cx).placeholder(placeholder));
            let search = cx.new(|cx| Search::new(field, cx).expanded(expanded));
            if !text.is_empty() {
                search.update(cx, |search, cx| search.set_text(text.to_string(), cx));
            }
            // The bar, the view and the clear button are drawn from the state of the search.
            cx.subscribe(&search, |_, _, _: &SearchEvent, cx| cx.notify())
                .detach();
            search
        };
        Self {
            size: 3,
            scroll: 0.,
            docked: make("Search your files", "", false, cx),
            open: make("Search your files", "release", true, cx),
            filter: make("Filter the list", "gpui", false, cx),
            page: make("Search the page", "spring", false, cx),
            focused_filter: false,
        }
    }

    /// Size of the scrolling bar and whether it has a subtitle.
    fn scroll_bar_size(&self) -> (TopAppBarSize, bool) {
        match self.size {
            0 | 1 => (TopAppBarSize::Small, false),
            2 => (TopAppBarSize::Medium, true),
            _ => (TopAppBarSize::Large, true),
        }
    }

    fn scroll_bar(&self) -> TopAppBar {
        let id = "tb-scroll";
        let bar = TopAppBar::new(id)
            .leading(action((id, 0), "panel-left"))
            .title("Inbox")
            .trailing(action((id, 1), "search"))
            .trailing(action((id, 2), "more"));
        let (size, subtitle) = self.scroll_bar_size();
        let bar = if subtitle { bar.subtitle("12 unread") } else { bar };
        let bar = bar.size(size);
        if self.size == 1 { bar.centered() } else { bar }
    }

    /// The largest scroll of the frame: the bar's collapse plus the rows after it.
    fn max_scroll(&self) -> f32 {
        let (size, subtitle) = self.scroll_bar_size();
        collapse_range(f32::from(size.expanded_height(subtitle))) + PAGE_EXTRA_SCROLL
    }

    /// Rows that match the query, or the recent ones while it is empty.
    fn results(&self, search: &Entity<Search>, prefix: &'static str, cx: &App) -> Vec<AnyElement> {
        let s = *Theme::of(cx);
        let query = search.read(cx).text(cx).to_lowercase();
        let recent = query.is_empty();
        ITEMS
            .iter()
            .enumerate()
            .filter(|(_, (title, _))| recent || title.to_lowercase().contains(&query))
            .take(if recent { 4 } else { 6 })
            .map(|(i, (title, kind))| {
                let search = search.clone();
                let icon = if recent { "history" } else { "search" };
                ListItem::new((prefix, i), *title)
                    .subtitle(*kind)
                    .leading(Icon::new(icon).size(px(18.)).color(s.on_surface_variant))
                    .on_click(move |_, _, cx| {
                        search.update(cx, |search, cx| {
                            search.set_text(*title, cx);
                            search.close(cx);
                        })
                    })
                    .into_any_element()
            })
            .collect()
    }

    /// A bar in a 56 px slot with its view opening over what is below it.
    fn search_block(&self, search: &Entity<Search>, id: &'static str, height: f32, cx: &App) -> Div {
        let results = self.results(search, id, cx);
        div().relative().w(px(480.)).h(px(height)).child(
            div()
                .relative()
                .h(px(56.))
                .child(SearchBar::new((id, 100usize), search).trailing(avatar()))
                .child(
                    SearchView::new((id, 101usize), search)
                        .overlay(true)
                        .empty_text("No results")
                        .trailing(avatar())
                        .children(results),
                ),
        )
    }
}

fn action(id: (&'static str, usize), icon: &'static str) -> IconButton {
    IconButton::new(id, icon).size(px(40.))
}

fn avatar() -> Avatar {
    Avatar::new("Ana Pérez").size(px(32.))
}

/// A bar over a few gray lines, like a page. The corners stay square: GPUI clips children to a
/// rectangle, so a bar's background would show past a rounded border.
fn frame(s: &Scheme, bar: TopAppBar) -> Div {
    let line = |w: f32| {
        div()
            .h(px(12.))
            .w(px(w))
            .rounded(px(6.))
            .bg(s.surface_container_highest)
    };
    div()
        .flex()
        .flex_col()
        .w(px(380.))
        .overflow_hidden()
        .border_1()
        .border_color(s.outline_variant)
        .bg(s.surface)
        .child(bar)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .p(px(16.))
                .child(line(260.))
                .child(line(300.))
                .child(line(180.)),
        )
}

impl Render for SearchDemos {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        if !self.focused_filter {
            // So that the focus ring of the filter bar shows without a click.
            self.focused_filter = true;
            self.filter.read(cx).field().read(cx).focus(window);
        }

        let small = TopAppBar::new("tb-small")
            .leading(action(("tb-small", 0), "panel-left"))
            .title("Inbox")
            .trailing(action(("tb-small", 1), "search"))
            .trailing(action(("tb-small", 2), "more"));
        let centered = TopAppBar::new("tb-centered")
            .leading(action(("tb-centered", 0), "arrow-left"))
            .title("Photos")
            .centered()
            .trailing(action(("tb-centered", 1), "more"));
        let medium = TopAppBar::new("tb-medium")
            .medium()
            .leading(action(("tb-medium", 0), "panel-left"))
            .title("Inbox")
            .trailing(action(("tb-medium", 1), "search"));
        let medium_half = TopAppBar::new("tb-medium-half")
            .medium()
            .collapse(0.55)
            .leading(action(("tb-medium-half", 0), "panel-left"))
            .title("Inbox")
            .subtitle("12 unread")
            .trailing(action(("tb-medium-half", 1), "search"));
        let large = TopAppBar::new("tb-large")
            .large()
            .leading(action(("tb-large", 0), "arrow-left"))
            .title("Trip to Valparaíso")
            .subtitle("Shared with 3 people")
            .trailing(action(("tb-large", 1), "search"))
            .trailing(action(("tb-large", 2), "more"));
        let large_shut = TopAppBar::new("tb-large-shut")
            .large()
            .collapse(1.)
            .leading(action(("tb-large-shut", 0), "arrow-left"))
            .title("Trip to Valparaíso")
            .subtitle("Shared with 3 people")
            .trailing(action(("tb-large-shut", 1), "search"))
            .trailing(action(("tb-large-shut", 2), "more"));

        // The scrolling frame: the bar collapses over the first stretch and the rows move after it.
        let bar = self.scroll_bar();
        let expanded = f32::from(bar.expanded_height());
        let progress = collapse_progress(self.scroll, expanded);
        let row_offset = (self.scroll - collapse_range(expanded)).max(0.);
        let mut rows = column().gap(px(2.)).p(px(8.)).relative().top(px(-row_offset));
        for (i, (title, kind)) in ITEMS.iter().take(PAGE_ROWS).enumerate() {
            rows = rows.child(
                ListItem::new(("page-row", i), *title)
                    .subtitle(*kind)
                    .leading(Avatar::new(*title).size(px(28.))),
            );
        }
        let scrolling = div()
            .id("scroll-frame")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(380.))
            .h(px(FRAME_HEIGHT))
                .overflow_hidden()
            .border_1()
            .border_color(s.outline_variant)
            .bg(s.surface)
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                let delta = f32::from(event.delta.pixel_delta(px(20.)).y);
                this.scroll = (this.scroll - delta).clamp(0., this.max_scroll());
                // The page behind it does not scroll along.
                cx.stop_propagation();
                cx.notify();
            }))
            .child(bar.collapse(progress))
            .child(div().flex_1().min_h_0().overflow_hidden().child(rows));

        let sizes = [
            Segment::label("Small"),
            Segment::label("Centered"),
            Segment::label("Medium"),
            Segment::label("Large"),
        ];

        let docked = self.docked.clone();
        let open = self.open.clone();
        let docked_block = self.search_block(&docked, "docked", 56., cx);
        let open_block = self.search_block(&open, "open", 330., cx);
        let filter_bar = SearchBar::new("filter-bar", &self.filter).open_on_press(false);
        let page_bar = SearchBar::new("page-bar", &self.page).active(true);

        page()
            .child(demo(
                "TopAppBar",
                "Small, centered small, and the medium and large flexible bars at collapse 0, 0.55 and 1.",
                row()
                    .items_start()
                    .gap(px(20.))
                    .child(frame(&s, small))
                    .child(frame(&s, centered))
                    .child(frame(&s, medium))
                    .child(frame(&s, medium_half))
                    .child(frame(&s, large))
                    .child(frame(&s, large_shut)),
            ))
            .child(demo(
                "Collapsing on scroll",
                "Scroll over the frame or drag the slider: collapse_progress turns the offset into the bar's collapse.",
                row()
                    .items_start()
                    .gap(px(24.))
                    .child(scrolling)
                    .child(
                        column()
                            .gap(px(16.))
                            .child(
                                SegmentedButtons::new("tb-size", sizes, self.size).on_change(
                                    update(cx, |this: &mut SearchDemos, i: usize, _| {
                                        this.size = i;
                                        this.scroll = this.scroll.min(this.max_scroll());
                                    }),
                                ),
                            )
                            .child(
                                Slider::new("tb-scroll-slider", self.scroll, 0.0..=self.max_scroll())
                                    .width(px(300.))
                                    .on_change(update(cx, |this: &mut SearchDemos, v: f32, _| {
                                        this.scroll = v
                                    })),
                            )
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(s.on_surface_variant)
                                    .child(SharedString::from(format!(
                                        "scroll {:.0} px · collapse {:.2}",
                                        self.scroll, progress
                                    ))),
                            ),
                    ),
            ))
            .child(demo(
                "SearchBar",
                "Press the first one: it opens a SearchView over the page. Below, a bar that filters in place (focus ring, clear button) and the active row.",
                column()
                    .gap(px(20.))
                    .child(docked_block)
                    .child(div().w(px(480.)).child(filter_bar))
                    .child(div().w(px(480.)).child(page_bar)),
            ))
            .child(demo(
                "SearchView",
                "The expanded search: input row, divider and results that scroll past a maximum height. Back or Escape closes it; a result fills the field.",
                open_block,
            ))
    }
}
