//! "Carousel and FAB menu" section: the four carousel layouts, cards, a vertical and a
//! full-screen carousel, and the FAB menu in the corner of a framed area.
//!
//! `M3_PAGE=0..2` opens one page (layouts, cards and more, FAB menu) and `M3_FAB=open` starts
//! with the menus open (both for screenshots).

use std::sync::Arc;

use gpui::{
    App, ClickEvent, Context, IntoElement, ParentElement, Render, RenderImage, SharedString,
    Styled, Window, div, px,
};
use gpui_m3::{
    Carousel, CarouselItem, FabMenu, FabMenuColor, Segment, SegmentedButtons, Theme, theme::alpha,
};

use super::{column, demo, page, row};

const CAPTIONS: [&str; 10] = [
    "Dawn", "Harbor", "Orchard", "Dunes", "Glacier", "Meadow", "Canyon", "Lagoon", "Summit",
    "Twilight",
];

/// Demos of the carousel and the FAB menu, in their own view.
pub struct CarouselDemos {
    page: usize,
    photos: Vec<Arc<RenderImage>>,
    /// Selected item of each carousel.
    selected: [Option<usize>; 6],
    fab_open: [bool; 3],
    last_action: SharedString,
}

impl CarouselDemos {
    pub fn new(_: &mut Context<Self>) -> Self {
        let open = std::env::var("M3_FAB").is_ok_and(|v| v == "open");
        Self {
            page: std::env::var("M3_PAGE")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0)
                .min(2),
            photos: (0..CAPTIONS.len())
                .map(|i| super::demo_photo_sized(i as f32 / CAPTIONS.len() as f32, 256))
                .collect(),
            selected: [None; 6],
            fab_open: [open; 3],
            last_action: SharedString::default(),
        }
    }

    fn photo_items(&self) -> Vec<CarouselItem> {
        self.photos
            .iter()
            .zip(CAPTIONS)
            .map(|(photo, caption)| CarouselItem::image(photo.clone()).caption(caption))
            .collect()
    }

    fn select(
        &self,
        cx: &mut Context<Self>,
        which: usize,
    ) -> impl Fn(usize, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |index, _, cx| {
            this.update(cx, |d, cx| {
                d.selected[which] = Some(index);
                cx.notify();
            })
            .ok();
        }
    }

    fn layouts(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let multi = Carousel::multi_browse("car-multi")
            .width(px(560.))
            .height(px(190.))
            .items(self.photo_items())
            .selected(self.selected[0])
            .on_select(self.select(cx, 0));
        let uncontained = Carousel::uncontained("car-uncontained")
            .width(px(560.))
            .height(px(190.))
            .item_width(px(170.))
            .items(self.photo_items())
            .selected(self.selected[1])
            .on_select(self.select(cx, 1));
        let hero = Carousel::hero("car-hero")
            .width(px(560.))
            .height(px(190.))
            .items(self.photo_items())
            .selected(self.selected[2])
            .on_select(self.select(cx, 2));
        let center = Carousel::center_hero("car-center")
            .width(px(560.))
            .height(px(190.))
            .items(self.photo_items())
            .selected(self.selected[3])
            .on_select(self.select(cx, 3));
        page()
            .gap(px(22.))
            .child(demo(
                "Multi-browse",
                "A large, a medium and a small item; they shrink as they scroll out. Wheel, drag, or Tab to it and use the arrows.",
                multi,
            ))
            .child(demo(
                "Uncontained",
                "Equal items that scroll freely and keep going after a flick.",
                uncontained,
            ))
            .child(demo("Hero", "One large item with a small one peeking.", hero))
            .child(demo(
                "Center-aligned hero",
                "The large item stays in the middle.",
                center,
            ))
    }

    fn more(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let cards = (0..6).map(|i| {
            let tint = [
                s.primary_container,
                s.secondary_container,
                s.tertiary_container,
            ][i % 3];
            CarouselItem::element(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .p(px(20.))
                    .text_color(s.on_surface)
                    .whitespace_nowrap()
                    .child(div().text_size(px(28.)).child(format!("{}", i + 1)))
                    .child(div().text_size(px(15.)).child(CAPTIONS[i]))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(s.on_surface_variant)
                            .child("A card keeps its layout while it shrinks."),
                    ),
            )
            .background(tint)
        });
        let cards = Carousel::multi_browse("car-cards")
            .width(px(560.))
            .height(px(150.))
            .items(cards)
            .selected(self.selected[5])
            .on_select(self.select(cx, 5));
        let full = Carousel::full_screen("car-full")
            .width(px(420.))
            .height(px(240.))
            .items(self.photo_items())
            .selected(self.selected[4])
            .on_select(self.select(cx, 4));
        let vertical = Carousel::multi_browse("car-vertical")
            .vertical()
            .width(px(220.))
            .height(px(420.))
            .items(self.photo_items());
        page()
            .gap(px(22.))
            .child(demo(
                "Cards",
                "Any element can be an item; it is clipped, not squeezed.",
                cards,
            ))
            .child(
                row()
                    .items_start()
                    .gap(px(40.))
                    .child(demo(
                        "Full-screen",
                        "One item per page, taller; arrows or drag.",
                        full,
                    ))
                    .child(demo("Vertical", "The same layout from top to bottom.", vertical)),
            )
    }

    fn fab_menus(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let mut frames = row().items_start().gap(px(20.));
        for (i, (name, color)) in [
            ("Primary", FabMenuColor::Primary),
            ("Secondary", FabMenuColor::Secondary),
            ("Tertiary", FabMenuColor::Tertiary),
        ]
        .into_iter()
        .enumerate()
        {
            let this = cx.entity().downgrade();
            let toggle = {
                let this = this.clone();
                move |open: bool, _: &mut Window, cx: &mut App| {
                    this.update(cx, |d, cx| {
                        d.fab_open[i] = open;
                        cx.notify();
                    })
                    .ok();
                }
            };
            let act = |label: &'static str| {
                let this = this.clone();
                move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    this.update(cx, |d, cx| {
                        d.last_action = format!("{label} ({})", name.to_lowercase()).into();
                        cx.notify();
                    })
                    .ok();
                }
            };
            let menu = FabMenu::new(("fab-menu", i))
                .color(color)
                .open(self.fab_open[i])
                .item("pen", "Edit", act("Edit"))
                .item("copy", "Duplicate", act("Duplicate"))
                .item("star", "Favorite", act("Favorite"))
                .item("trash", "Delete", act("Delete"))
                .on_toggle(toggle);
            let frame = div()
                .relative()
                .w(px(240.))
                .h(px(380.))
                .flex_none()
                .rounded(px(28.))
                .bg(s.surface_container_low)
                .border_1()
                .border_color(s.outline_variant)
                .child(
                    column().p(px(20.)).gap(px(10.)).children((0..5).map(|n| {
                        div()
                            .h(px(10.))
                            .w(px(200. - n as f32 * 24.))
                            .rounded(px(5.))
                            .bg(alpha(s.on_surface, 0.08))
                    })),
                )
                .child(div().absolute().right(px(16.)).bottom(px(16.)).child(menu));
            frames = frames.child(column().gap(px(8.)).child(name).child(frame));
        }
        let note = if self.last_action.is_empty() {
            "Open a menu and choose an action.".to_string()
        } else {
            format!("Chose: {}", self.last_action)
        };
        page()
            .child(demo(
                "FAB menu",
                "The plus turns into an x and the FAB into a circle; the actions rise from it one by one. Escape or a click outside closes it.",
                frames,
            ))
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(s.on_surface_variant)
                    .child(note),
            )
    }
}

impl Render for CarouselDemos {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let pages = SegmentedButtons::new(
            "carousel-page",
            ["Layouts", "Cards and more", "FAB menu"].map(Segment::label),
            self.page,
        )
        .on_change(move |i, _, cx| {
            this.update(cx, |d, cx| {
                d.page = i;
                cx.notify();
            })
            .ok();
        });
        let content = match self.page {
            0 => self.layouts(cx).into_any_element(),
            1 => self.more(cx).into_any_element(),
            _ => self.fab_menus(cx).into_any_element(),
        };
        page().child(pages).child(content)
    }
}
