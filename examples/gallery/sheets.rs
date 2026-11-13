//! "Sheets and dividers" section: dividers, bottom and side sheets and rich tooltips.
//!
//! `M3_OPEN=bottom-sheet|bottom-sheet-bouncy|side-sheet|side-sheet-left` opens a modal sheet on
//! start and `M3_PART=dividers|bottom|side|tooltip` shows one demo only (both for screenshots).

use gpui::{
    AnyElement, App, ClickEvent, Context, ElementId, Hsla, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};
use gpui_m3::{
    BottomSheet, Button, Divider, DividerInset, Icon, ListItem, RichTooltip, Scheme, SheetSide,
    SideSheet, Theme,
    motion::{Spring, key, presence},
    theme::alpha,
};

use super::{column, demo, page, row};

/// Demos of the sheets, in their own view.
pub struct SheetDemos {
    bottom: bool,
    bottom_bouncy: bool,
    side_right: bool,
    side_left: bool,
    tooltip_action: SharedString,
    part: String,
}

impl SheetDemos {
    pub fn new(_: &mut Context<Self>) -> Self {
        let open = std::env::var("M3_OPEN").unwrap_or_default();
        Self {
            bottom: open == "bottom-sheet",
            bottom_bouncy: open == "bottom-sheet-bouncy",
            side_right: open == "side-sheet",
            side_left: open == "side-sheet-left",
            tooltip_action: SharedString::default(),
            part: std::env::var("M3_PART").unwrap_or_default(),
        }
    }

    /// A click handler that sets one of the flags that open the modal sheets.
    fn setter(
        cx: &mut Context<Self>,
        set: fn(&mut SheetDemos, bool),
        value: bool,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |_, _, cx| {
            this.update(cx, |this, cx| {
                set(this, value);
                cx.notify();
            })
            .ok();
        }
    }
}

/// A few list rows to fill a sheet.
fn rows(prefix: &'static str, color: Hsla) -> AnyElement {
    let mut list = column().gap(px(0.));
    for (i, (icon, title, subtitle)) in [
        ("export", "Share", "Send a link to the chat"),
        ("copy", "Copy link", "Copies it to the clipboard"),
        ("bookmark", "Save for later", "Find it in your bookmarks"),
        ("trash", "Delete", "Removes it from every device"),
    ]
    .into_iter()
    .enumerate()
    {
        list = list.child(
            ListItem::new(ElementId::NamedInteger(prefix.into(), i as u64), title)
                .subtitle(subtitle)
                .leading(Icon::new(icon).size(px(20.)).color(color)),
        );
    }
    list.into_any_element()
}

fn stage() -> gpui::Div {
    div()
        .flex()
        .overflow_hidden()
        .rounded(px(12.))
        .w(px(480.))
        .h(px(360.))
}

fn page_content(s: &Scheme) -> gpui::Div {
    div()
        .flex_1()
        .w_full()
        .p(px(20.))
        .text_size(px(13.))
        .text_color(s.on_surface_variant)
        .child("Page content")
}

impl Render for SheetDemos {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let frame = |d: gpui::Div| {
            d.bg(s.surface_container_lowest)
                .border_1()
                .border_color(s.outline_variant)
        };

        // Dividers: one list per inset, and vertical ones between texts.
        let mut dividers = row().items_start().gap(px(24.));
        for (name, inset) in [
            ("None", DividerInset::None),
            ("Start", DividerInset::Start),
            ("Both", DividerInset::Both),
            ("Middle", DividerInset::Middle),
        ] {
            let mut card = column()
                .gap(px(0.))
                .w(px(172.))
                .rounded(px(12.))
                .bg(s.surface_container_low)
                .py(px(8.))
                .child(
                    div()
                        .px(px(16.))
                        .pb(px(8.))
                        .text_size(px(12.5))
                        .text_color(s.on_surface_variant)
                        .child(name),
                );
            for item in ["Inbox", "Drafts", "Sent"] {
                card = card
                    .child(
                        div()
                            .px(px(16.))
                            .py(px(10.))
                            .text_size(px(13.5))
                            .child(item),
                    )
                    .child(Divider::horizontal().inset(inset));
            }
            dividers = dividers.child(card);
        }
        let vertical = row()
            .gap(px(16.))
            .h(px(56.))
            .child("Day")
            .child(Divider::vertical())
            .child("Week")
            .child(Divider::vertical().inset(DividerInset::Both))
            .child("Month");

        // Modal sheets: drawn while they are open and while they leave.
        let progress =
            |name: &'static str, open: bool, window: &mut Window, cx: &mut Context<Self>| {
                presence(key(&ElementId::Name(name.into()), "p"), open, window, cx)
            };
        let bottom = progress("sheet-bottom", self.bottom, window, cx);
        let bottom_bouncy = progress("sheet-bottom-bouncy", self.bottom_bouncy, window, cx);
        let right = progress("sheet-right", self.side_right, window, cx);
        let left = progress("sheet-left", self.side_left, window, cx);

        // Standard sheets, inline.
        let standard_bottom = frame(stage().flex_col().justify_end().items_center())
            .child(page_content(&s))
            .child(
                BottomSheet::standard("std-bottom")
                    .padding(px(8.))
                    .child(rows("std-bottom-row", s.on_surface_variant)),
            );
        let standard_side = frame(stage()).child(page_content(&s)).child(
            SideSheet::standard("std-side")
                .width(px(240.))
                .title("Details")
                .on_close(|_, _, _| {})
                .child(
                    div()
                        .text_size(px(13.5))
                        .text_color(s.on_surface_variant)
                        .child(
                            "A standard side sheet sits next to the page and does not block it.",
                        ),
                )
                .action(Button::new("std-side-cancel", "Cancel").text())
                .action(Button::new("std-side-save", "Save").filled()),
        );
        let standard_side_left = frame(stage())
            .child(
                SideSheet::standard("std-side-left")
                    .side(SheetSide::Left)
                    .width(px(240.))
                    .title("Filters")
                    .on_back(|_, _, _| {})
                    .child(
                        div()
                            .text_size(px(13.5))
                            .text_color(s.on_surface_variant)
                            .child("With a back button and no actions."),
                    ),
            )
            .child(page_content(&s));

        // Rich tooltips: live (hover) and drawn in place.
        let clicked = |label: &'static str, cx: &mut Context<Self>| {
            let this = cx.entity().downgrade();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                this.update(cx, |this, cx| {
                    this.tooltip_action = label.into();
                    cx.notify();
                })
                .ok();
            }
        };
        let hover_target = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .child(Button::new(ElementId::Name(format!("{id}-b").into()), label).tonal())
        };
        let live = row()
            .child(
                hover_target("rt-plain", "Text only").tooltip(
                    RichTooltip::new(
                        "rt-live-1",
                        "Rich tooltips explain a control in a sentence or two.",
                    )
                    .build(),
                ),
            )
            .child(
                hover_target("rt-title", "With subhead").tooltip(
                    RichTooltip::new(
                        "rt-live-2",
                        "Keeps your notes in step on every device you use.",
                    )
                    .title("Sync")
                    .build(),
                ),
            )
            .child(
                hover_target("rt-actions", "With actions").hoverable_tooltip(
                    RichTooltip::new(
                        "rt-live-3",
                        "Backups run every night while the app is open and the device is charging.",
                    )
                    .title("Backups")
                    .action("Learn more", clicked("Learn more", cx))
                    .action("Got it", clicked("Got it", cx))
                    .build(),
                ),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(s.on_surface_variant)
                    .child(if self.tooltip_action.is_empty() {
                        SharedString::from("Hover a button for half a second")
                    } else {
                        format!("Clicked: {}", self.tooltip_action).into()
                    }),
            );
        let static_row = row()
            .items_start()
            .gap(px(24.))
            .child(RichTooltip::new(
                "rt-static-1",
                "Rich tooltips explain a control in a sentence or two.",
            ))
            .child(
                RichTooltip::new(
                    "rt-static-2",
                    "Keeps your notes in step on every device you use.",
                )
                .title("Sync"),
            )
            .child(
                RichTooltip::new(
                    "rt-static-3",
                    "Backups run every night while the app is open and the device is charging.",
                )
                .title("Backups")
                .action("Learn more", |_, _, _| {})
                .action("Got it", |_, _, _| {}),
            );

        let open_bottom = Button::new("open-bottom", "Open modal bottom sheet")
            .filled()
            .on_click(Self::setter(cx, |d, v| d.bottom = v, true));
        let open_bottom_bouncy = Button::new("open-bottom-bouncy", "Open with a bouncier spring")
            .tonal()
            .on_click(Self::setter(cx, |d, v| d.bottom_bouncy = v, true));
        let open_right = Button::new("open-right", "Open modal side sheet (right)")
            .filled()
            .on_click(Self::setter(cx, |d, v| d.side_right = v, true));
        let open_left = Button::new("open-left", "Open modal side sheet (left)")
            .filled()
            .on_click(Self::setter(cx, |d, v| d.side_left = v, true));

        let shows = |part: &str| self.part.is_empty() || self.part == part;
        page()
            .when(shows("dividers"), |d| d.child(demo(
                "Dividers",
                "1 dp in outline_variant. None, start (16 dp), both and middle insets; vertical ones need a parent with a height.",
                column().gap(px(20.)).child(dividers).child(vertical),
            )))
            .when(shows("bottom"), |d| d.child(demo(
                "Bottom sheet",
                "Modal: scrim, spring slide-up, dismissed by the scrim, Escape or the handler. spring() picks that spring. Standard: the same surface in place.",
                column()
                    .gap(px(16.))
                    .child(row().child(open_bottom).child(open_bottom_bouncy))
                    .child(standard_bottom),
            )))
            .when(shows("side"), |d| d.child(demo(
                "Side sheet",
                "256–400 dp wide, header with back and close, scrolling content and an actions row. Modal from either edge, or standard.",
                column()
                    .gap(px(16.))
                    .child(row().child(open_right).child(open_left))
                    .child(row().items_start().gap(px(24.)).child(standard_side).child(standard_side_left)),
            )))
            .when(shows("tooltip"), |d| d.child(demo(
                "Rich tooltip",
                "Subhead, supporting text and text buttons. The one with actions uses hoverable_tooltip: it stays open while the pointer is over it.",
                column().gap(px(16.)).child(static_row).child(live),
            )))
            .when_some(bottom, |d, progress| {
                d.child(
                    BottomSheet::modal("modal-bottom")
                        .exit(progress)
                        .on_dismiss(Self::setter(cx, |d, v| d.bottom = v, false))
                        .child(
                            div()
                                .pb(px(8.))
                                .text_size(px(22.))
                                .text_color(s.on_surface)
                                .child("Share this chat"),
                        )
                        .child(rows("modal-bottom-row", s.on_surface_variant))
                        .child(Divider::horizontal().inset(DividerInset::Both))
                        .child(row().justify_end().pt(px(8.)).child(
                            Button::new("modal-bottom-close", "Close")
                                .text()
                                .on_click(Self::setter(cx, |d, v| d.bottom = v, false)),
                        )),
                )
            })
            .when_some(bottom_bouncy, |d, progress| {
                d.child(
                    BottomSheet::modal("modal-bottom-bouncy")
                        // A custom spring: quicker and with a deeper overshoot.
                        .spring(Spring {
                            stiffness: 700.,
                            damping_ratio: 0.55,
                        })
                        .exit(progress)
                        .on_dismiss(Self::setter(cx, |d, v| d.bottom_bouncy = v, false))
                        .child(
                            div()
                                .pb(px(8.))
                                .text_size(px(22.))
                                .text_color(s.on_surface)
                                .child("A bouncier entrance"),
                        )
                        .child(
                            div()
                                .text_size(px(13.5))
                                .text_color(alpha(s.on_surface, 0.8))
                                .child("The sheet's own spring, instead of Spring::SPATIAL."),
                        )
                        .child(row().justify_end().pt(px(16.)).child(
                            Button::new("modal-bottom-bouncy-close", "Close")
                                .text()
                                .on_click(Self::setter(cx, |d, v| d.bottom_bouncy = v, false)),
                        )),
                )
            })
            .when_some(right, |d, progress| {
                d.child(modal_side(SheetSide::Right, "modal-right", progress, &s, cx, |d, v| {
                    d.side_right = v
                }))
            })
            .when_some(left, |d, progress| {
                d.child(modal_side(SheetSide::Left, "modal-left", progress, &s, cx, |d, v| {
                    d.side_left = v
                }))
            })
    }
}

fn modal_side(
    side: SheetSide,
    id: &'static str,
    progress: f32,
    s: &Scheme,
    cx: &mut Context<SheetDemos>,
    set: fn(&mut SheetDemos, bool),
) -> impl IntoElement {
    SideSheet::modal(id)
        .side(side)
        .title("Settings")
        .exit(progress)
        .on_back(|_, _, _| {})
        .on_close(SheetDemos::setter(cx, set, false))
        .on_dismiss(SheetDemos::setter(cx, set, false))
        .child(
            div()
                .pb(px(16.))
                .text_size(px(13.5))
                .text_color(alpha(s.on_surface, 0.8))
                .child("Modal side sheets keep the focus until they are dismissed: click the scrim, press Escape or use the close button."),
        )
        .child(rows(if side == SheetSide::Right { "modal-right-row" } else { "modal-left-row" }, s.on_surface_variant))
        .action(
            Button::new(ElementId::Name(format!("{id}-cancel").into()), "Cancel")
                .text()
                .on_click(SheetDemos::setter(cx, set, false)),
        )
        .action(
            Button::new(ElementId::Name(format!("{id}-save").into()), "Save")
                .filled()
                .on_click(SheetDemos::setter(cx, set, false)),
        )
}
