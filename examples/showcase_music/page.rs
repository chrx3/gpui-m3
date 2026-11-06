//! The scrolling main column: large top app bar, search, carousels, top tracks and the
//! appearance settings. It sits in a cached view, so only its own changes rebuild it.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Render, ScrollWheelEvent, SharedString, StatefulInteractiveElement, Styled,
    Window, canvas, div, prelude::FluentBuilder as _, px,
};
use gpui_m3::{
    Avatar, ButtonGroup, ButtonGroupItem, Carousel, CarouselItem, Chip, Divider, DividerInset,
    FavStar, Icon, IconButton, ListGroup, ListItem, M3ButtonSize, Scheme, SchemeVariant, Search,
    SearchBar, SearchEvent, SearchView, Segment, SegmentedButtons, SettingRow, Shape, Slider,
    Switch, Theme, TopAppBar, collapse_progress, collapse_range, search_field,
    theme::{
        alpha, hex,
        type_scale::{BODY_MEDIUM, HEADLINE_SMALL, TITLE_LARGE, TypeScaleExt},
    },
};

use super::data::{
    ALBUMS, Covers, MOODS, TRACKS, artist_of, clock_text, in_mood, search as run_search,
};
use super::icons::icon;
use super::model::{Model, change};

const PAD: f32 = 16.;
/// Rows of "Top tracks".
const TOP_TRACKS: [usize; 6] = [0, 2, 5, 8, 10, 12];
const VARIANTS: [SchemeVariant; 6] = [
    SchemeVariant::TonalSpot,
    SchemeVariant::Expressive,
    SchemeVariant::Vibrant,
    SchemeVariant::Fidelity,
    SchemeVariant::Rainbow,
    SchemeVariant::Monochrome,
];

pub struct Page {
    model: Entity<Model>,
    scroll: f32,
    scroll_generation: u64,
    content_height: Rc<Cell<f32>>,
    area_height: Rc<Cell<f32>>,
    pub search: Entity<Search>,
    /// Item and request generation of each carousel (made for you, recent, mixes).
    carousels: [(usize, u64); 3],
    /// Schemes of the album cards, for the mode they were made in.
    palettes: Vec<Scheme>,
    palettes_dark: Option<bool>,
    library_show: [bool; 3],
}

impl Page {
    pub fn new(model: Entity<Model>, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| search_field(cx).placeholder("Search songs, albums, artists"));
        let search = cx.new(|cx| Search::new(field, cx));
        cx.subscribe(&search, |_, _, _: &SearchEvent, cx| cx.notify())
            .detach();
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        Self {
            model,
            scroll: 0.,
            scroll_generation: 0,
            content_height: Rc::default(),
            area_height: Rc::default(),
            search,
            carousels: [(0, 0); 3],
            palettes: Vec::new(),
            palettes_dark: None,
            library_show: [true, true, false],
        }
    }

    fn bar_expanded() -> f32 {
        f32::from(gpui_m3::TopAppBarSize::Large.expanded_height(true))
    }

    /// The farthest the page scrolls: the bar's collapse plus the content past the screen.
    fn max_scroll(&self) -> f32 {
        let expanded = Self::bar_expanded();
        let range = collapse_range(expanded);
        let progress = collapse_progress(self.scroll, expanded);
        let bar = expanded + (64. - expanded) * progress;
        let area_when_collapsed = self.area_height.get() + (bar - 64.);
        range + (self.content_height.get() - area_when_collapsed).max(0.)
    }

    /// Scrolls to `to` px over `millis` with an ease-in-out, from a timer.
    pub fn animate_scroll(&mut self, to: f32, millis: u64, cx: &mut Context<Self>) {
        self.scroll_generation += 1;
        let generation = self.scroll_generation;
        let from = self.scroll;
        let to = to.min(self.max_scroll()).max(0.);
        let start = Instant::now();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(8))
                    .await;
                let t = (start.elapsed().as_secs_f32() * 1000. / millis.max(1) as f32).min(1.);
                let eased = t * t * (3. - 2. * t);
                let alive = this
                    .update(cx, |page, cx| {
                        if page.scroll_generation != generation {
                            return false;
                        }
                        page.scroll = from + (to - from) * eased;
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !alive || t >= 1. {
                    break;
                }
            }
        })
        .detach();
    }

    /// Moves a carousel to its next item (programmatic focus, with the spring).
    pub fn carousel_next(&mut self, which: usize, cx: &mut Context<Self>) {
        let (index, generation) = self.carousels[which];
        self.carousels[which] = (index + 1, generation + 1);
        cx.notify();
    }

    pub fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.animate_scroll(0., 350, cx);
        self.search.update(cx, |search, cx| search.open(window, cx));
    }

    fn palette(&mut self, dark: bool) -> &[Scheme] {
        if self.palettes_dark != Some(dark) {
            self.palettes = ALBUMS
                .iter()
                .map(|a| Scheme::from_seed(hex(a.seed), dark))
                .collect();
            self.palettes_dark = Some(dark);
        }
        &self.palettes
    }
}

fn heading(s: &Scheme, title: &'static str, subtitle: &'static str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .pt(px(28.))
        .pb(px(12.))
        .child(div().type_emphasized(TITLE_LARGE).child(title))
        .child(
            div()
                .type_style(BODY_MEDIUM)
                .text_color(s.on_surface_variant)
                .child(subtitle),
        )
}

fn cover_shape(cx: &App, album: usize, size: f32) -> Shape {
    Shape::new(ALBUMS[album].shape)
        .image(cx.global::<Covers>().0[album].clone())
        .size(px(size))
}

impl Page {
    fn search_block(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let query = self.search.read(cx).text(cx);
        let (albums, tracks) = run_search(&query);
        let model = self.model.clone();
        let mut rows: Vec<AnyElement> = Vec::new();
        if query.trim().is_empty() {
            for (i, text) in ["Glass Tide", "Fern Hollow", "Golden Hour", "Marina Sol"]
                .into_iter()
                .enumerate()
            {
                let search = self.search.clone();
                rows.push(
                    ListItem::new(("recent", i), text)
                        .subtitle("Recent search")
                        .leading(
                            Icon::new("history")
                                .size(px(18.))
                                .color(s.on_surface_variant),
                        )
                        .on_click(move |_, _, cx| {
                            search.update(cx, |search, cx| search.set_text(text, cx))
                        })
                        .into_any_element(),
                );
            }
        } else {
            for &album in albums.iter().take(2) {
                let (search, model) = (self.search.clone(), model.clone());
                rows.push(
                    ListItem::new(("hit-album", album), ALBUMS[album].name)
                        .subtitle(format!("Album · {}", ALBUMS[album].artist))
                        .leading(cover_shape(cx, album, 36.))
                        .on_click(move |_, _, cx| {
                            search.update(cx, |search, cx| search.close(cx));
                            model.update(cx, |m, cx| m.select_album(album, cx));
                        })
                        .into_any_element(),
                );
            }
            for &track in tracks.iter().take(5 - albums.len().min(2)) {
                let (search, model) = (self.search.clone(), model.clone());
                rows.push(
                    ListItem::new(("hit-track", track), TRACKS[track].title)
                        .subtitle(format!("Song · {}", artist_of(track)))
                        .leading(cover_shape(cx, TRACKS[track].album, 36.))
                        .on_click(move |_, _, cx| {
                            search.update(cx, |search, cx| search.close(cx));
                            model.update(cx, |m, cx| m.play_track(track, cx));
                        })
                        .into_any_element(),
                );
            }
        }
        div()
            .relative()
            .h(px(56.))
            .w_full()
            .max_w(px(720.))
            .child(
                SearchBar::new("page-search-bar", &self.search)
                    .trailing(Avatar::new("Ana Pérez").size(px(32.))),
            )
            .child(
                SearchView::new("page-search-view", &self.search)
                    .overlay(true)
                    .empty_text("No results")
                    .max_height(px(330.))
                    .trailing(Avatar::new("Ana Pérez").size(px(32.)))
                    .children(rows),
            )
    }

    fn mood_chips(&self, mood: usize) -> impl IntoElement {
        let mut row = div().flex().items_center().gap(px(8.)).pt(px(16.));
        for (i, label) in MOODS.into_iter().enumerate() {
            let model = self.model.clone();
            row = row.child(Chip::new(("mood", i), label).filter(mood == i).on_click(
                move |_, _, cx| {
                    model.update(cx, |m, cx| {
                        m.mood = i;
                        cx.notify();
                    })
                },
            ));
        }
        row
    }

    fn album_card(&self, a: usize, schemes: &[Scheme], cx: &App) -> CarouselItem {
        let sc = schemes[a];
        let album = &ALBUMS[a];
        CarouselItem::element(
            div()
                .size_full()
                .flex()
                .items_center()
                .gap(px(20.))
                .px(px(20.))
                .whitespace_nowrap()
                .text_color(sc.on_primary_container)
                .child(cover_shape(cx, a, 150.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight(700.))
                                .text_color(alpha(sc.on_primary_container, 0.7))
                                .child("ALBUM"),
                        )
                        .child(div().type_emphasized(HEADLINE_SMALL).child(album.name))
                        .child(
                            div()
                                .text_size(px(15.))
                                .text_color(alpha(sc.on_primary_container, 0.8))
                                .child(album.artist),
                        )
                        .child(
                            div()
                                .mt(px(12.))
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .h(px(36.))
                                .w(px(92.))
                                .justify_center()
                                .rounded(px(18.))
                                .bg(sc.primary)
                                .text_color(sc.on_primary)
                                .text_size(px(13.5))
                                .font_weight(FontWeight(600.))
                                .child(Icon::new(icon("play")).size(px(14.)).color(sc.on_primary))
                                .child("Play"),
                        ),
                ),
        )
        .background(sc.primary_container)
    }

    fn mix_card(&self, mix: usize, schemes: &[Scheme], cx: &App) -> CarouselItem {
        const MIXES: [(&str, [usize; 3], &str); 3] = [
            (
                "Daily Mix 1",
                [0, 2, 6],
                "Nova Reyes, The Amber Coast, Orchid Club and more",
            ),
            (
                "Daily Mix 2",
                [3, 7, 5],
                "Fern Hollow, Halden, Marina Sol and more",
            ),
            (
                "Daily Mix 3",
                [4, 1, 2],
                "Kite Season, The Amber Coast, Lumen and more",
            ),
        ];
        let (name, albums, artists) = MIXES[mix];
        let sc = schemes[albums[0]];
        CarouselItem::element(
            div()
                .relative()
                .size_full()
                .whitespace_nowrap()
                .text_color(sc.on_tertiary_container)
                .child(
                    div()
                        .absolute()
                        .left(px(32.))
                        .top_0()
                        .h_full()
                        .flex()
                        .flex_col()
                        .justify_center()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight(700.))
                                .text_color(alpha(sc.on_tertiary_container, 0.7))
                                .child("MADE FOR YOU"),
                        )
                        .child(
                            div()
                                .text_size(px(34.))
                                .font_weight(FontWeight(700.))
                                .child(name),
                        )
                        .child(
                            div()
                                .text_size(px(14.))
                                .text_color(alpha(sc.on_tertiary_container, 0.8))
                                .child(artists),
                        )
                        .child(
                            div()
                                .mt(px(14.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap(px(8.))
                                .h(px(40.))
                                .w(px(112.))
                                .rounded(px(20.))
                                .bg(sc.tertiary)
                                .text_color(sc.on_tertiary)
                                .font_weight(FontWeight(600.))
                                .child(Icon::new(icon("play")).size(px(16.)).color(sc.on_tertiary))
                                .child("Play"),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .right(px(150.))
                        .top(px(26.))
                        .child(cover_shape(cx, albums[1], 120.)),
                )
                .child(
                    div()
                        .absolute()
                        .right(px(40.))
                        .top(px(44.))
                        .child(cover_shape(cx, albums[0], 150.)),
                )
                .child(
                    div()
                        .absolute()
                        .right(px(190.))
                        .top(px(120.))
                        .child(cover_shape(cx, albums[2], 76.)),
                ),
        )
        .background(sc.tertiary_container)
    }

    fn carousel_state(&self, which: usize) -> (usize, u64) {
        self.carousels[which]
    }

    fn top_tracks(
        &self,
        s: &Scheme,
        current: usize,
        liked: [bool; TRACKS.len()],
        cx: &App,
    ) -> impl IntoElement {
        let mut list = div().flex().flex_col().p(px(8.));
        for (n, &track) in TOP_TRACKS.iter().enumerate() {
            if n > 0 {
                list = list.child(Divider::horizontal().inset(DividerInset::Both));
            }
            let model = self.model.clone();
            let fav = self.model.clone();
            let playing = track == current;
            let trailing = div()
                .flex()
                .items_center()
                .gap(px(16.))
                .when(playing, |d| {
                    d.child(Icon::new(icon("waves")).size(px(18.)).color(s.primary))
                })
                .child(FavStar::new(("top-fav", track), liked[track]).on_toggle(
                    move |on, _, cx| {
                        fav.update(cx, |m, cx| {
                            m.liked[track] = on;
                            cx.notify();
                        })
                    },
                ))
                .child(
                    div()
                        .w(px(40.))
                        .text_size(px(13.))
                        .text_color(s.on_surface_variant)
                        .child(clock_text(TRACKS[track].secs as f32)),
                );
            list = list.child(
                ListItem::new(("top-track", track), TRACKS[track].title)
                    .subtitle(format!(
                        "{} · {}",
                        artist_of(track),
                        ALBUMS[TRACKS[track].album].name
                    ))
                    .leading(cover_shape(cx, TRACKS[track].album, 44.))
                    .trailing(trailing)
                    .on_click(move |_, _, cx| model.update(cx, |m, cx| m.play_track(track, cx))),
            );
        }
        let shadow = gpui_m3::Elevation::Level1.shadow(s);
        let lifted = gpui_m3::Elevation::Level3.shadow(s);
        div()
            .id("top-tracks")
            .rounded(px(28.))
            .bg(s.surface_container_low)
            .shadow(shadow)
            .hover(move |d| d.shadow(lifted))
            .child(list)
    }

    fn appearance(
        &self,
        variant: SchemeVariant,
        contrast: f32,
        dark: bool,
        this: gpui::WeakEntity<Self>,
    ) -> impl IntoElement {
        let model = self.model.clone();
        let palette = ButtonGroup::new(
            "variant-group",
            VARIANTS.map(|v| ButtonGroupItem::label(v.label())),
        )
        .connected()
        .size(M3ButtonSize::Xs)
        .selected(VARIANTS.iter().position(|v| *v == variant).unwrap_or(0))
        .on_change(change(&model, |m, i: usize, cx| {
            m.set_variant(VARIANTS[i], cx)
        }));
        let contrast_slider = Slider::new("contrast-slider", contrast, -1.0..=1.0)
            .step(0.5)
            .width(px(220.))
            .ticks(4)
            .on_change(change(&model, |m, v: f32, cx| m.set_contrast(v, cx)));
        let dark_switch = {
            let model = self.model.clone();
            Switch::new("dark-switch", dark)
                .thumb_icons("moon", "sun")
                .on_toggle(move |on, window, cx| {
                    let origin = window.mouse_position();
                    super::set_dark(&model, on, origin, window, cx);
                })
        };
        let show = SegmentedButtons::multi(
            "library-show",
            ["Albums", "Playlists", "Artists"].map(Segment::label),
            self.library_show,
        )
        .on_change_many(move |flags, _, cx| {
            this.update(cx, |page, cx| {
                page.library_show = [flags[0], flags[1], flags[2]];
                cx.notify();
            })
            .ok();
        });
        ListGroup::new()
            .title("Appearance")
            .custom_row(
                SettingRow::new("row-palette", "Palette")
                    .icon("palette")
                    .description("How an album's color becomes the app's colors")
                    .control(palette),
            )
            .custom_row(
                SettingRow::new("row-contrast", "Contrast")
                    .icon("gauge")
                    .description("From softer to easier to read")
                    .control(contrast_slider),
            )
            .custom_row(
                SettingRow::new("row-dark", "Dark theme")
                    .icon("moon")
                    .description("Changes in a circle from the switch")
                    .control(dark_switch),
            )
            .custom_row(
                SettingRow::new("row-library", "Library shows")
                    .icon(icon("library"))
                    .description("Pick what you want to see")
                    .control(show),
            )
    }

    fn home(&mut self, s: &Scheme, cx: &mut Context<Self>) -> AnyElement {
        let (mood, current, liked, variant, contrast, dark, selected) = {
            let m = self.model.read(cx);
            (
                m.mood, m.current, m.liked, m.variant, m.contrast, m.dark, m.album,
            )
        };
        let schemes: Vec<Scheme> = self.palette(Theme::get(cx).dark).to_vec();
        let visible: Vec<usize> = (0..ALBUMS.len()).filter(|&a| in_mood(a, mood)).collect();
        let model = self.model.clone();
        let this = cx.entity().downgrade();
        let selector = |model: Entity<Model>, visible: Vec<usize>| {
            move |index: usize, _: &mut Window, cx: &mut App| {
                if let Some(&album) = visible.get(index) {
                    model.update(cx, |m, cx| m.select_album(album, cx));
                }
            }
        };

        let made = Carousel::multi_browse("car-made")
            .height(px(210.))
            .items(visible.iter().map(|&a| self.album_card(a, &schemes, cx)))
            .selected(visible.iter().position(|&a| a == selected))
            .scroll_to(self.carousel_state(0).0, self.carousel_state(0).1)
            .on_select(selector(model.clone(), visible.clone()));
        let recent_albums: Vec<usize> = (0..ALBUMS.len()).rev().collect();
        let recent = Carousel::uncontained("car-recent")
            .height(px(176.))
            .item_width(px(176.))
            .items(recent_albums.iter().map(|&a| {
                CarouselItem::image(cx.global::<Covers>().0[a].clone()).caption(ALBUMS[a].name)
            }))
            .selected(recent_albums.iter().position(|&a| a == selected))
            .scroll_to(self.carousel_state(1).0, self.carousel_state(1).1)
            .on_select(selector(model.clone(), recent_albums.clone()));
        let mixes = Carousel::hero("car-mix")
            .height(px(204.))
            .items((0..3).map(|i| self.mix_card(i, &schemes, cx)))
            .scroll_to(self.carousel_state(2).0, self.carousel_state(2).1)
            .on_select({
                let model = model.clone();
                move |index, _, cx| {
                    let album = [0, 3, 4][index.min(2)];
                    model.update(cx, |m, cx| m.select_album(album, cx));
                }
            });

        div()
            .flex()
            .flex_col()
            .px(px(PAD))
            .pb(px(PAD))
            .child(self.search_block(cx))
            .child(self.mood_chips(mood))
            .child(heading(
                s,
                "Made for you",
                "Press an album and the whole app takes its colors",
            ))
            .child(made)
            .child(heading(s, "Recently played", "Pick up where you left off"))
            .child(recent)
            .child(heading(s, "Daily mix", "Fresh every morning"))
            .child(mixes)
            .child(heading(
                s,
                "Top tracks",
                "What you have on repeat this week",
            ))
            .child(self.top_tracks(s, current, liked, cx))
            .child(div().h(px(28.)))
            .child(self.appearance(variant, contrast, dark, this))
            .into_any_element()
    }

    fn library(&mut self, s: &Scheme, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.model.read(cx).album;
        let schemes: Vec<Scheme> = self.palette(Theme::get(cx).dark).to_vec();
        let playlists = self.model.read(cx).playlists.clone();
        let show = self.library_show;
        let mut content = div().flex().flex_col().px(px(PAD)).pb(px(PAD));
        let this = cx.entity().downgrade();
        content = content.child(
            div().pt(px(8.)).child(
                SegmentedButtons::multi(
                    "library-filter",
                    ["Albums", "Playlists", "Artists"].map(Segment::label),
                    show,
                )
                .on_change_many(move |flags, _, cx| {
                    this.update(cx, |page, cx| {
                        page.library_show = [flags[0], flags[1], flags[2]];
                        cx.notify();
                    })
                    .ok();
                }),
            ),
        );
        if show[1] {
            let mut wrap = div().flex().flex_wrap().gap(px(12.));
            for (i, name) in std::iter::once(SharedString::from("Liked songs"))
                .chain(playlists)
                .enumerate()
            {
                let album = [0, 3, 6, 4, 5][i % 5];
                let sc = schemes[album];
                wrap = wrap.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(14.))
                        .w(px(260.))
                        .h(px(84.))
                        .px(px(14.))
                        .rounded(px(24.))
                        .bg(sc.primary_container)
                        .text_color(sc.on_primary_container)
                        .child(cover_shape(cx, album, 56.))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(div().font_weight(FontWeight(600.)).child(name))
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .text_color(alpha(sc.on_primary_container, 0.75))
                                        .child("Playlist"),
                                ),
                        ),
                );
            }
            content = content
                .child(heading(s, "Playlists", "Yours and the ones you follow"))
                .child(wrap);
        }
        if show[0] {
            let mut wrap = div().flex().flex_wrap().gap(px(16.));
            for (a, album) in ALBUMS.iter().enumerate() {
                let model = self.model.clone();
                wrap = wrap.child(
                    div()
                        .id(("lib-album", a))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .w(px(132.))
                        .cursor_pointer()
                        .on_click(move |_, _, cx| model.update(cx, |m, cx| m.select_album(a, cx)))
                        .child(cover_shape(cx, a, 132.))
                        .child(
                            div()
                                .text_size(px(13.5))
                                .font_weight(FontWeight(if a == selected { 700. } else { 500. }))
                                .child(album.name),
                        ),
                );
            }
            content = content
                .child(heading(s, "Albums", "Saved to your library"))
                .child(wrap);
        }
        content.into_any_element()
    }
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let nav = self.model.read(cx).nav;
        let expanded = Self::bar_expanded();
        let progress = collapse_progress(self.scroll, expanded);
        let row_offset = (self.scroll - collapse_range(expanded)).max(0.);

        let search_button = {
            let this = cx.entity().downgrade();
            IconButton::new("bar-search", "search")
                .size(px(40.))
                .tooltip("Search")
                .on_click(move |_, window, cx| {
                    this.update(cx, |page, cx| page.open_search(window, cx))
                        .ok();
                })
        };
        let (title, subtitle) = if nav == 2 {
            ("Your library", "Playlists and albums")
        } else {
            ("Good evening", "Made for you")
        };
        let bar = TopAppBar::new("page-bar")
            .large()
            .title(title)
            .subtitle(subtitle)
            .trailing(search_button)
            .trailing(
                div()
                    .flex()
                    .items_center()
                    .size(px(40.))
                    .justify_center()
                    .child(Avatar::new("Ana Pérez").size(px(32.))),
            )
            .collapse(progress);

        let body = if nav == 2 {
            self.library(&s, cx)
        } else {
            self.home(&s, cx)
        };
        let (content_height, area_height) = (self.content_height.clone(), self.area_height.clone());
        let measure_content = canvas(
            move |bounds, _, _| content_height.set(f32::from(bounds.size.height)),
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let measure_area = canvas(
            move |bounds, _, _| area_height.set(f32::from(bounds.size.height)),
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        div()
            .id("page")
            .size_full()
            .flex()
            .flex_col()
            .bg(s.surface)
            .text_color(s.on_surface)
            .on_scroll_wheel(cx.listener(|page, event: &ScrollWheelEvent, _, cx| {
                let delta = f32::from(event.delta.pixel_delta(px(20.)).y);
                page.scroll_generation += 1;
                page.scroll = (page.scroll - delta).clamp(0., page.max_scroll().max(0.));
                cx.notify();
            }))
            .child(bar)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(measure_area)
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(px(-row_offset))
                            .child(measure_content)
                            .child(body),
                    ),
            )
    }
}
