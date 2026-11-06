//! "Selection variants" section: checkbox states (indeterminate, error), switches with icons
//! and multi-select segmented buttons.
//!
//! `M3_PART=checks|radios|switches|segments` shows one demo group only (for screenshots).

use gpui::{
    App, ClickEvent, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window,
    div, px,
};
use gpui_m3::{
    CheckState, Checkbox, ChoiceRow, Radio, Segment, SegmentedButtons, Switch, Theme,
};

use super::{column, demo, page, row};

const CHILDREN: [&str; 3] = ["Notifications", "Sounds", "Badges"];

/// Demos of the selection controls, in their own view (they keep their own state).
pub struct SelectionDemos {
    part: String,
    /// Checkbox grid: normal, error and disabled columns by unchecked, checked and dash rows.
    boxes: [CheckState; 9],
    children: [bool; 3],
    radio: usize,
    radio_error: usize,
    rows: [bool; 2],
    switches: [bool; 6],
    single: usize,
    multi: [bool; 4],
    multi_icons: [bool; 3],
    dense_single: usize,
    dense_multi: [bool; 3],
    toolbar: [bool; 3],
}

impl SelectionDemos {
    pub fn new(_: &mut Context<Self>) -> Self {
        use CheckState::{Checked, Indeterminate, Unchecked};
        Self {
            part: std::env::var("M3_PART").unwrap_or_default(),
            boxes: [
                Unchecked, Checked, Indeterminate, Unchecked, Checked, Indeterminate, Unchecked,
                Checked, Indeterminate,
            ],
            children: [true, false, true],
            radio: 1,
            radio_error: 0,
            rows: [true, false],
            switches: [true, false, true, false, true, false],
            single: 1,
            multi: [true, false, true, false],
            multi_icons: [true, false, true],
            dense_single: 0,
            dense_multi: [true, true, false],
            toolbar: [true, false, false],
        }
    }

    /// A change handler that updates this view.
    fn on<T: 'static, F: Fn(&mut Self, T) + 'static>(
        &self,
        cx: &mut Context<Self>,
        update: F,
    ) -> impl Fn(T, &mut Window, &mut App) + 'static + use<T, F> {
        let this = cx.entity().downgrade();
        move |value, _, cx| {
            this.update(cx, |d, cx| {
                update(d, value);
                cx.notify();
            })
            .ok();
        }
    }

    /// A click handler that updates this view.
    fn on_click<F: Fn(&mut Self) + 'static>(
        &self,
        cx: &mut Context<Self>,
        update: F,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static + use<F> {
        let this = cx.entity().downgrade();
        move |_, _, cx| {
            this.update(cx, |d, cx| {
                update(d);
                cx.notify();
            })
            .ok();
        }
    }

    fn shows(&self, part: &str) -> bool {
        self.part.is_empty() || self.part == part
    }

    fn set_children(&mut self, checked: bool) {
        self.children = [checked; 3];
    }

    fn checked_children(&self) -> usize {
        self.children.iter().filter(|c| **c).count()
    }

    fn checkbox_grid(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let label = |text: &'static str| {
            div()
                .w(px(96.))
                .text_size(px(12.))
                .text_color(s.on_surface_variant)
                .child(text)
        };
        let mut grid = column().gap(px(10.)).child(
            row()
                .gap(px(0.))
                .child(label(""))
                .child(label("Normal"))
                .child(label("Error"))
                .child(label("Disabled")),
        );
        for (r, name) in ["Unchecked", "Checked", "Indeterminate"].into_iter().enumerate() {
            let mut line = row().gap(px(0.)).child(label(name));
            for c in 0..3 {
                let i = c * 3 + r;
                line = line.child(
                    div().w(px(96.)).child(
                        Checkbox::new(("box", i), false)
                            .state(self.boxes[i])
                            .error(c == 1)
                            .disabled(c == 2)
                            .on_change(self.on(cx, move |d, checked: bool| {
                                d.boxes[i] = checked.into();
                            })),
                    ),
                );
            }
            grid = grid.child(line);
        }
        grid
    }

    fn parent_children(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let parent = CheckState::from_count(self.checked_children(), CHILDREN.len());
        let text = |text: &'static str, weight: f32| {
            div()
                .text_size(px(13.5))
                .font_weight(gpui::FontWeight(weight))
                .text_color(s.on_surface)
                .child(text)
        };
        let mut children = column().gap(px(10.)).pl(px(28.));
        for (i, name) in CHILDREN.into_iter().enumerate() {
            children = children.child(
                row().gap(px(10.)).child(
                    Checkbox::new(("child", i), self.children[i]).on_change(self.on(
                        cx,
                        move |d, checked: bool| d.children[i] = checked,
                    )),
                ).child(text(name, 400.)),
            );
        }
        column()
            .gap(px(10.))
            .child(
                row().gap(px(10.)).child(
                    Checkbox::new("parent", false)
                        .state(parent)
                        .on_change(self.on(cx, |d, checked: bool| d.set_children(checked))),
                ).child(text("All notifications", 600.)),
            )
            .child(children)
    }

    fn choice_rows(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let parent = CheckState::from_count(self.checked_children(), CHILDREN.len());
        let toggle_parent = self.on_click(cx, move |d| d.set_children(!parent.is_checked()));
        column()
            .w(px(320.))
            .gap(px(6.))
            .child(
                ChoiceRow::checkbox("cr-parent", "Select all", parent.is_checked())
                    .indeterminate(parent == CheckState::Indeterminate)
                    .description("Dash when only some are checked")
                    .on_click(toggle_parent),
            )
            .child(
                ChoiceRow::checkbox("cr-error-off", "Accept the terms", self.rows[0])
                    .error(true)
                    .description("Required")
                    .on_click(self.on_click(cx, |d| d.rows[0] = !d.rows[0])),
            )
            .child(
                ChoiceRow::checkbox("cr-error-on", "Subscribe to updates", self.rows[1])
                    .error(true)
                    .on_click(self.on_click(cx, |d| d.rows[1] = !d.rows[1])),
            )
    }

    fn radios(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let group = |cx: &mut Context<Self>, name: &'static str, error: bool, selected: usize| {
            let mut options = row().gap(px(14.)).child(
                div().w(px(64.)).text_size(px(12.)).text_color(s.on_surface_variant).child(name),
            );
            for i in 0..3 {
                options = options.child(
                    Radio::new((name, i), selected == i)
                        .size(px(20.))
                        .error(error)
                        .on_change(self.on(cx, move |d, _: bool| {
                            if error {
                                d.radio_error = i;
                            } else {
                                d.radio = i;
                            }
                        })),
                );
            }
            options
        };
        column()
            .child(group(cx, "Normal", false, self.radio))
            .child(group(cx, "Error", true, self.radio_error))
            .child(
                row().gap(px(14.)).child(
                    div().w(px(64.)).text_size(px(12.)).text_color(s.on_surface_variant).child("Disabled"),
                )
                .child(Radio::new("radio-off-a", false).size(px(20.)).disabled(true))
                .child(Radio::new("radio-off-b", true).size(px(20.)).disabled(true)),
            )
    }

    fn switches(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let caption = |text: &'static str| {
            div().w(px(120.)).text_size(px(12.)).text_color(s.on_surface_variant).child(text)
        };
        let toggle = |cx: &mut Context<Self>, i: usize| {
            self.on(cx, move |d, on: bool| d.switches[i] = on)
        };
        column()
            .child(
                row().child(caption("Standard"))
                    .child(Switch::new("sw-a", self.switches[0]).on_toggle(toggle(cx, 0)))
                    .child(Switch::new("sw-b", self.switches[1]).on_toggle(toggle(cx, 1))),
            )
            .child(
                row().child(caption("With icons"))
                    .child(Switch::new("sw-c", self.switches[0]).icons(true).on_toggle(toggle(cx, 0)))
                    .child(Switch::new("sw-d", self.switches[1]).icons(true).on_toggle(toggle(cx, 1))),
            )
            .child(
                row().child(caption("Compact, icons"))
                    .child(
                        Switch::new("sw-e", self.switches[2])
                            .compact(true)
                            .icons(true)
                            .on_toggle(toggle(cx, 2)),
                    )
                    .child(
                        Switch::new("sw-f", self.switches[3])
                            .compact(true)
                            .icons(true)
                            .on_toggle(toggle(cx, 3)),
                    ),
            )
            .child(
                row().child(caption("Custom icons"))
                    .child(
                        Switch::new("sw-g", self.switches[4])
                            .thumb_icons("sun", "moon")
                            .on_toggle(toggle(cx, 4)),
                    )
                    .child(
                        Switch::new("sw-h", self.switches[5])
                            .thumb_icons("sun", "moon")
                            .on_toggle(toggle(cx, 5)),
                    ),
            )
            .child(
                row().child(caption("Disabled"))
                    .child(Switch::new("sw-i", true).icons(true).disabled(true))
                    .child(Switch::new("sw-j", false).icons(true).disabled(true)),
            )
    }

    fn segments(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let words = || ["Day", "Week", "Month", "Year"].map(Segment::label);
        let status = |on: &[bool], names: &[&str]| -> SharedString {
            let picked: Vec<&str> =
                names.iter().zip(on).filter(|(_, on)| **on).map(|(n, _)| *n).collect();
            if picked.is_empty() { "Nothing selected".into() } else { picked.join(", ").into() }
        };
        let note = |text: SharedString| {
            div().text_size(px(12.)).text_color(s.on_surface_variant).child(text)
        };
        let many = |cx: &mut Context<Self>, set: fn(&mut Self, Vec<bool>)| {
            self.on(cx, move |d, flags: Vec<bool>| set(d, flags))
        };
        column()
            .gap(px(18.))
            .child(
                column()
                    .items_start()
                    .child(note("Single selection (unchanged)".into()))
                    .child(
                        SegmentedButtons::new("seg-single", words(), self.single)
                            .on_change(self.on(cx, |d, i: usize| d.single = i)),
                    ),
            )
            .child(
                column()
                    .items_start()
                    .child(note(
                        format!("Multi-select: {}", status(&self.multi, &["Day", "Week", "Month", "Year"]))
                            .into(),
                    ))
                    .child(
                        SegmentedButtons::multi("seg-multi", words(), self.multi)
                            .on_change_many(many(cx, |d, f| d.multi.copy_from_slice(&f))),
                    ),
            )
            .child(
                column()
                    .items_start()
                    .child(note("Segments with an icon keep it and show no check".into()))
                    .child(
                        SegmentedButtons::multi(
                            "seg-multi-icons",
                            [
                                Segment::label("Edit").with_icon("pen"),
                                Segment::label("Copy"),
                                Segment::label("Star").with_icon("star"),
                            ],
                            self.multi_icons,
                        )
                        .on_change_many(many(cx, |d, f| d.multi_icons.copy_from_slice(&f))),
                    ),
            )
            .child(
                row()
                    .gap(px(24.))
                    .items_end()
                    .child(
                        column().items_start().child(note("Dense, single".into())).child(
                            SegmentedButtons::new("seg-dense-single", words(), self.dense_single)
                                .dense(true)
                                .on_change(self.on(cx, |d, i: usize| d.dense_single = i)),
                        ),
                    )
                    .child(
                        column().items_start().child(note("Dense, multi".into())).child(
                            SegmentedButtons::multi(
                                "seg-dense-multi",
                                ["Bold", "Italic", "Link"].map(Segment::label),
                                self.dense_multi,
                            )
                            .dense(true)
                            .on_change_many(many(cx, |d, f| d.dense_multi.copy_from_slice(&f))),
                        ),
                    ),
            )
            .child(
                column()
                    .items_start()
                    .child(note("Toolbar: icons only, several at once".into()))
                    .child(
                        SegmentedButtons::multi(
                            "seg-toolbar",
                            ["pen", "star", "copy"].map(Segment::icon),
                            self.toolbar,
                        )
                        .on_change_many(many(cx, |d, f| d.toolbar.copy_from_slice(&f))),
                    ),
            )
    }
}

impl Render for SelectionDemos {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = page();
        if self.shows("checks") {
            content = content
                .child(demo(
                    "Checkbox states",
                    "Unchecked, checked and indeterminate, each normal, in error and disabled. A click on the dash checks it.",
                    self.checkbox_grid(cx),
                ))
                .child(demo(
                    "Parent and children",
                    "The parent shows a dash while only some children are checked; clicking it checks them all.",
                    row().items_start().gap(px(48.)).child(self.parent_children(cx)).child(self.choice_rows(cx)),
                ));
        }
        if self.shows("radios") {
            content = content.child(demo(
                "Radio",
                "The error state turns the ring red, selected or not.",
                self.radios(cx),
            ));
        }
        if self.shows("switches") {
            content = content.child(demo(
                "Switch with icons",
                "A check when on, an x when off, riding the thumb's spring. Tab to focus, Space to toggle.",
                self.switches(cx),
            ));
        }
        if self.shows("segments") {
            content = content.child(demo(
                "Segmented buttons",
                "Multi-select shows a check on selected segments without an icon; dense is 24 px high.",
                self.segments(cx),
            ));
        }
        content
    }
}
