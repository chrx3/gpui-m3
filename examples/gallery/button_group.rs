//! "Button groups and toolbars" section.

use gpui::{
    App, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_m3::{
    ButtonGroup, ButtonGroupItem, Fab, M3ButtonSize, Segment, SegmentedButtons, Theme, Toolbar,
    ToolbarItem,
};

use super::{column, demo, page, row};

const SIZES: [(&str, M3ButtonSize); 5] = [
    ("XS", M3ButtonSize::Xs),
    ("S", M3ButtonSize::S),
    ("M", M3ButtonSize::M),
    ("L", M3ButtonSize::L),
    ("XL", M3ButtonSize::Xl),
];

fn labels(names: &[&'static str]) -> Vec<ButtonGroupItem> {
    names.iter().map(|n| ButtonGroupItem::label(*n)).collect()
}

fn icons() -> Vec<ButtonGroupItem> {
    ["pen", "copy", "star", "trash"]
        .into_iter()
        .map(|icon| ButtonGroupItem::icon(icon).tooltip(icon))
        .collect()
}

fn with_icons() -> Vec<ButtonGroupItem> {
    vec![
        ButtonGroupItem::label("Edit").with_icon("pen"),
        ButtonGroupItem::label("Copy").with_icon("copy"),
        ButtonGroupItem::label("Delete").with_icon("trash"),
    ]
}

/// Demos of the button groups and toolbars, in their own view (they keep their own state).
pub struct ButtonGroupDemos {
    /// Standard, connected, sizes or toolbars (M3_PAGE=<n> opens one directly).
    page: usize,
    clicked: SharedString,
    single: [usize; 6],
    multi: [[bool; 4]; 2],
    tool: usize,
    marks: [bool; 3],
}

impl ButtonGroupDemos {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            page: std::env::var("M3_PAGE")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0)
                .min(3),
            clicked: SharedString::default(),
            single: [1, 0, 2, 1, 0, 1],
            multi: [[true, false, true, false], [false, true, false, false]],
            tool: 1,
            marks: [true, false, true],
        }
    }

    fn set_single(
        &self,
        cx: &mut Context<Self>,
        demo: usize,
    ) -> impl Fn(usize, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |i, _, cx| {
            this.update(cx, |d, cx| {
                d.single[demo] = i;
                cx.notify();
            })
            .ok();
        }
    }

    fn set_multi(
        &self,
        cx: &mut Context<Self>,
        demo: usize,
    ) -> impl Fn(Vec<bool>, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |flags, _, cx| {
            this.update(cx, |d, cx| {
                for (slot, flag) in d.multi[demo].iter_mut().zip(flags) {
                    *slot = flag;
                }
                cx.notify();
            })
            .ok();
        }
    }

    fn set_clicked(
        &self,
        cx: &mut Context<Self>,
    ) -> impl Fn(usize, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |i, _, cx| {
            this.update(cx, |d, cx| {
                d.clicked = format!("Clicked button {}", i + 1).into();
                cx.notify();
            })
            .ok();
        }
    }

    fn select_tool(
        &self,
        cx: &mut Context<Self>,
        tool: usize,
    ) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |_, _, cx| {
            this.update(cx, |d, cx| {
                d.tool = tool;
                cx.notify();
            })
            .ok();
        }
    }

    fn toggle_mark(
        &self,
        cx: &mut Context<Self>,
        mark: usize,
    ) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |_, _, cx| {
            this.update(cx, |d, cx| {
                d.marks[mark] = !d.marks[mark];
                cx.notify();
            })
            .ok();
        }
    }

    fn standard(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let on_click = self.set_clicked(cx);
        let variants = column()
            .child(
                ButtonGroup::new("std-filled", labels(&["Save", "Share", "Archive"]))
                    .filled()
                    .on_click(self.set_clicked(cx)),
            )
            .child(
                ButtonGroup::new("std-tonal", labels(&["Save", "Share", "Archive"]))
                    .tonal()
                    .on_click(on_click),
            )
            .child(ButtonGroup::new("std-outlined", labels(&["Save", "Share", "Archive"])).outlined())
            .child(ButtonGroup::new("std-text", labels(&["Save", "Share", "Archive"])).text());
        let content = column()
            .child(
                ButtonGroup::new("std-icons", with_icons())
                    .tonal()
                    .size(M3ButtonSize::M),
            )
            .child(ButtonGroup::new("std-icon-only", icons()).outlined());
        page()
            .child(demo(
                "Standard group",
                "Press and hold a button: it widens and its neighbours give way.",
                variants,
            ))
            .child(demo("Icons and labels", "Icon with label, and icons only.", content))
    }

    fn connected(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut variants = column();
        let kinds = ["Filled", "Tonal", "Outlined", "Text"];
        for (k, kind) in kinds.into_iter().enumerate() {
            let group = ButtonGroup::new(("con-single", k), labels(&["Day", "Week", "Month", "Year"]))
                .connected()
                .selected(self.single[k])
                .on_change(self.set_single(cx, k));
            let group = match k {
                0 => group.filled(),
                1 => group.tonal(),
                2 => group.outlined(),
                _ => group.text(),
            };
            variants = variants.child(
                row()
                    .gap(px(16.))
                    .child(div().w(px(70.)).child(kind))
                    .child(group),
            );
        }
        let multi = column()
            .child(
                ButtonGroup::new("con-multi-tonal", labels(&["Bold", "Italic", "Underline", "Strike"]))
                    .connected()
                    .tonal()
                    .selected_many(self.multi[0])
                    .on_change_many(self.set_multi(cx, 0)),
            )
            .child(
                ButtonGroup::new("con-multi-filled", icons())
                    .connected()
                    .filled()
                    .selected_many(self.multi[1])
                    .on_change_many(self.set_multi(cx, 1)),
            );
        let icons = column()
            .child(
                ButtonGroup::new("con-with-icons", with_icons())
                    .connected()
                    .filled()
                    .size(M3ButtonSize::M)
                    .selected(self.single[4])
                    .on_change(self.set_single(cx, 4)),
            )
            .child(
                ButtonGroup::new("con-icon-only", icons())
                    .connected()
                    .outlined()
                    .selected(self.single[5])
                    .on_change(self.set_single(cx, 5)),
            )
            .child(
                ButtonGroup::new(
                    "con-disabled",
                    vec![
                        ButtonGroupItem::label("Left"),
                        ButtonGroupItem::label("Middle").disabled(true),
                        ButtonGroupItem::label("Right"),
                    ],
                )
                .connected()
                .tonal()
                .selected(0)
                .on_change(|_, _, _| {}),
            );
        page()
            .child(demo(
                "Connected group, single selection",
                "The selected button turns into a pill; the group's ends stay round.",
                variants,
            ))
            .child(demo(
                "Multiple selection",
                "Each button toggles on its own.",
                multi,
            ))
            .child(demo("Icons, labels and disabled", "Icon with label, icons only and a disabled button.", icons))
    }

    fn sizes(&self) -> impl IntoElement {
        let group = |id: &'static str, i: usize, size: M3ButtonSize, connected: bool| {
            let items = if matches!(size, M3ButtonSize::L | M3ButtonSize::Xl) {
                labels(&["One", "Two"])
            } else {
                labels(&["One", "Two", "Three", "Four"])
            };
            let group = ButtonGroup::new((id, i), items).filled().size(size);
            if connected {
                group.connected().selected(1).on_change(|_, _, _| {})
            } else {
                group
            }
        };
        let mut standard = column();
        let mut connected = column();
        for (i, (name, size)) in SIZES.into_iter().enumerate() {
            let label = || div().w(px(28.)).child(name);
            standard = standard.child(row().gap(px(16.)).child(label()).child(group("std-size", i, size, false)));
            connected = connected.child(row().gap(px(16.)).child(label()).child(group("con-size", i, size, true)));
        }
        page()
            .child(demo(
                "Standard group sizes",
                "Extra small to extra large; the gap grows from 8 to 12 px.",
                standard,
            ))
            .child(demo("Connected group sizes", "The gap stays 2 px.", connected))
    }

    fn toolbars(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tools = ["pen", "copy", "star", "search"];
        let mut floating = |id: &'static str, vibrant: bool, vertical: bool, with_fab: bool| {
            let mut bar = Toolbar::floating();
            if vibrant {
                bar = bar.vibrant();
            }
            if vertical {
                bar = bar.vertical();
            }
            for (i, icon) in tools.into_iter().enumerate() {
                bar = bar.item(
                    ToolbarItem::new((id, i), icon)
                        .tooltip(icon)
                        .selected(self.tool == i)
                        .on_click(self.select_tool(cx, i)),
                );
            }
            if with_fab {
                bar = bar.fab(Fab::new((id, 99usize), "plus"));
            }
            bar
        };
        let horizontal = row()
            .gap(px(32.))
            .child(floating("fl-std", false, false, false))
            .child(floating("fl-std-fab", false, false, true))
            .child(floating("fl-vib", true, false, false))
            .child(floating("fl-vib-fab", true, false, true));
        let vertical = row()
            .items_start()
            .gap(px(32.))
            .child(floating("fv-std", false, true, false))
            .child(floating("fv-std-fab", false, true, true))
            .child(floating("fv-vib", true, true, false))
            .child(floating("fv-vib-fab", true, true, true));
        let mut docked = |id: &'static str, vibrant: bool| {
            let mut bar = Toolbar::docked();
            if vibrant {
                bar = bar.vibrant();
            }
            for (i, icon) in ["pen", "copy", "star"].into_iter().enumerate() {
                bar = bar.item(
                    ToolbarItem::new((id, i), icon)
                        .selected(self.marks[i])
                        .on_click(self.toggle_mark(cx, i)),
                );
            }
            bar.item(ToolbarItem::new((id, 3usize), "trash").disabled(true))
                .fab(Fab::new((id, 99usize), "plus"))
        };
        let docked = column()
            .max_w(px(480.))
            .child(div().rounded(px(16.)).overflow_hidden().child(docked("dk-std", false)))
            .child(div().rounded(px(16.)).overflow_hidden().child(docked("dk-vib", true)))
            .child(
                div().rounded(px(16.)).overflow_hidden().child(
                    Toolbar::docked()
                        .item(ToolbarItem::new("dk-a", "pen"))
                        .item(ToolbarItem::new("dk-b", "copy").selected(true))
                        .item(ToolbarItem::new("dk-c", "star")),
                ),
            );
        page()
            .child(demo(
                "Floating toolbar",
                "Standard and vibrant, with and without a FAB. Tap a button to select it.",
                horizontal,
            ))
            .child(demo("Vertical", "The same toolbars stacked.", vertical))
            .child(demo("Docked toolbar", "Full width, 64 px; icons toggle, the FAB sits at the end.", docked))
    }
}

impl Render for ButtonGroupDemos {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let clicked = if self.clicked.is_empty() {
            "Click a button of the filled or tonal standard group".to_string()
        } else {
            self.clicked.to_string()
        };
        let this = cx.entity().downgrade();
        let pages = SegmentedButtons::new(
            "button-group-page",
            ["Standard", "Connected", "Sizes", "Toolbars"].map(Segment::label),
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
            0 => self.standard(cx).into_any_element(),
            1 => self.connected(cx).into_any_element(),
            2 => self.sizes().into_any_element(),
            _ => self.toolbars(cx).into_any_element(),
        };
        page().child(pages).child(content).when(self.page == 0, |d| {
            d.child(
                div()
                    .text_size(px(12.5))
                    .text_color(s.on_surface_variant)
                    .child(clicked),
            )
        })
    }
}
