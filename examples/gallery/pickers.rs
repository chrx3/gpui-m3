//! "Date and time" section: date pickers (single and range), time pickers (12 h and 24 h) and
//! their modal variants.
//!
//! `M3_OPEN=date|range|time` opens a modal picker on start, `M3_PART=date|range|time|modal` shows
//! one group only and `M3_TIME_INPUT=1` starts the time pickers on their numeric fields (all for
//! screenshots).

use gpui::{
    App, ClickEvent, Context, ElementId, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, prelude::FluentBuilder as _, px,
};
use gpui_m3::{
    Button, Date, DateNames, DatePicker, DateRange, Theme, Time, TimeNames, TimePicker, Weekday,
    motion::{key, presence},
};

use super::{column, demo, page, row};

/// Demos of the pickers, in their own view.
pub struct PickerDemos {
    date: Option<Date>,
    range: Option<DateRange>,
    limited: Option<Date>,
    time_12: Time,
    time_24: Time,
    modal_date: bool,
    modal_range: bool,
    modal_time: bool,
    /// What the modal pickers show while they are open; OK copies it to the chosen values.
    draft_date: Option<Date>,
    draft_range: Option<DateRange>,
    draft_time: Time,
    chosen_date: Option<Date>,
    chosen_range: Option<DateRange>,
    chosen_time: Option<Time>,
    part: String,
    time_input: bool,
}

impl PickerDemos {
    pub fn new(_: &mut Context<Self>) -> Self {
        let open = std::env::var("M3_OPEN").unwrap_or_default();
        let day = |day| Date::new(2026, 11, day);
        Self {
            date: day(12),
            range: day(9).zip(day(17)).map(|(a, b)| DateRange::new(a, b)),
            limited: None,
            time_12: Time {
                hour: 14,
                minute: 30,
            },
            time_24: Time { hour: 9, minute: 5 },
            modal_date: open == "date",
            modal_range: open == "range",
            modal_time: open == "time",
            draft_date: day(12),
            draft_range: day(9).zip(day(17)).map(|(a, b)| DateRange::new(a, b)),
            draft_time: Time {
                hour: 14,
                minute: 30,
            },
            chosen_date: None,
            chosen_range: None,
            chosen_time: None,
            part: std::env::var("M3_PART").unwrap_or_default(),
            time_input: std::env::var("M3_TIME_INPUT").is_ok_and(|v| v == "1"),
        }
    }

    /// A callback that updates the view and redraws it.
    fn set<T: 'static>(
        cx: &mut Context<Self>,
        f: impl Fn(&mut Self, T) + 'static,
    ) -> impl Fn(T, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |value, _, cx| {
            this.update(cx, |this, cx| {
                f(this, value);
                cx.notify();
            })
            .ok();
        }
    }

    /// A click handler with the same effect.
    fn click(
        cx: &mut Context<Self>,
        f: impl Fn(&mut Self) + 'static,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let set = Self::set(cx, move |this, _: ()| f(this));
        move |_, window, cx| set((), window, cx)
    }
}

fn note(text: impl Into<SharedString>, s: &gpui_m3::Scheme) -> impl IntoElement {
    div()
        .text_size(px(12.5))
        .text_color(s.on_surface_variant)
        .child(text.into())
}

impl Render for PickerDemos {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let shows = |part: &str| self.part.is_empty() || self.part == part;
        let progress =
            |name: &'static str, open: bool, window: &mut Window, cx: &mut Context<Self>| {
                presence(key(&ElementId::Name(name.into()), "p"), open, window, cx)
            };
        let date_modal = progress("pk-date", self.modal_date, window, cx);
        let range_modal = progress("pk-range", self.modal_range, window, cx);
        let time_modal = progress("pk-time", self.modal_time, window, cx);

        let date_text = |date: Option<Date>| match date {
            Some(date) => date.to_string(),
            None => "none".to_string(),
        };
        let range_text = |range: Option<DateRange>| match range {
            Some(r) if r.start == r.end => format!("{} (waiting for the end)", r.start),
            Some(r) => format!("{} to {} ({} days)", r.start, r.end, r.days()),
            None => "none".to_string(),
        };

        let min = Date::new(2026, 11, 3).unwrap();
        let max = Date::new(2026, 12, 20).unwrap();

        let single = column()
            .child(
                DatePicker::docked("pk-single")
                    .header(true)
                    .value(self.date)
                    .on_change(Self::set(cx, |this, date| this.date = Some(date))),
            )
            .child(note(format!("Picked: {}", date_text(self.date)), &s));
        let ranged = column()
            .child(
                DatePicker::docked("pk-range-inline")
                    .range(true)
                    .header(true)
                    .range_value(self.range)
                    .on_range_change(Self::set(cx, |this, range| this.range = Some(range))),
            )
            .child(note(format!("Picked: {}", range_text(self.range)), &s));
        let limited = column()
            .child(
                DatePicker::docked("pk-limited")
                    .header(true)
                    .names(DateNames::spanish())
                    .first_day_of_week(Weekday::Sunday)
                    .min(min)
                    .max(max)
                    .value(self.limited)
                    .on_change(Self::set(cx, |this, date| this.limited = Some(date))),
            )
            .child(note(
                format!(
                    "Spanish, weeks start on Sunday, only {min} to {max}: {}",
                    date_text(self.limited)
                ),
                &s,
            ));
        let docked = column()
            .child(
                DatePicker::docked("pk-plain")
                    .value(self.date)
                    .on_change(Self::set(cx, |this, date| this.date = Some(date))),
            )
            .child(note(
                "Docked without a header: the surface of a text field's menu.",
                &s,
            ));

        let time_12 = column()
            .child(
                TimePicker::docked("pk-time-12")
                    .value(self.time_12)
                    .start_with_input(self.time_input)
                    .on_change(Self::set(cx, |this, time| this.time_12 = time)),
            )
            .child(note(format!("12 h: {}", self.time_12), &s));
        let time_24 = column()
            .child(
                TimePicker::docked("pk-time-24")
                    .value(self.time_24)
                    .twenty_four_hour(true)
                    .minute_step(5)
                    .names(TimeNames::spanish())
                    .start_with_input(self.time_input)
                    .on_change(Self::set(cx, |this, time| this.time_24 = time)),
            )
            .child(note(
                format!("24 h, Spanish, minutes snap to 5: {}", self.time_24),
                &s,
            ));

        let modal = column()
            .child(
                row()
                    .child(
                        Button::new("pk-open-date", "Pick a date")
                            .filled()
                            .on_click(Self::click(cx, |this| {
                                this.draft_date = this.chosen_date.or(this.date);
                                this.modal_date = true;
                            })),
                    )
                    .child(
                        Button::new("pk-open-range", "Pick a range")
                            .tonal()
                            .on_click(Self::click(cx, |this| {
                                this.draft_range = this.chosen_range.or(this.range);
                                this.modal_range = true;
                            })),
                    )
                    .child(
                        Button::new("pk-open-time", "Pick a time")
                            .outlined()
                            .on_click(Self::click(cx, |this| {
                                this.draft_time = this.chosen_time.unwrap_or(this.draft_time);
                                this.modal_time = true;
                            })),
                    ),
            )
            .child(note(
                format!(
                    "Confirmed: date {}, range {}, time {}",
                    date_text(self.chosen_date),
                    range_text(self.chosen_range),
                    self.chosen_time
                        .map_or("none".to_string(), |t| t.to_string()),
                ),
                &s,
            ));

        page()
            .when(shows("date"), |d| {
                d.child(demo(
                    "Date picker",
                    "Docked calendar with the header: the picked day, a switch to typing it (YYYY-MM-DD), month arrows, a year list under the month, today's ring, arrows move the day once it has focus, PageUp and PageDown change the month.",
                    row().items_start().gap(px(24.)).child(single).child(docked).child(limited),
                ))
            })
            .when(shows("range"), |d| {
                d.child(demo(
                    "Date range picker",
                    "The first click starts the range, the second ends it; the days between use the secondary container and the pointer previews the end.",
                    ranged,
                ))
            })
            .when(shows("time"), |d| {
                d.child(demo(
                    "Time picker",
                    "Drag the dial: the hour first, then the minutes. The header numbers choose what the dial edits, the pen swaps the dial for two fields (type digits, Up and Down step).",
                    row().items_start().gap(px(24.)).child(time_12).child(time_24),
                ))
            })
            .when(shows("modal"), |d| {
                d.child(demo(
                    "Modal pickers",
                    "Over a scrim, with OK and Cancel; Escape and a click on the scrim cancel. The values only change on OK.",
                    modal,
                ))
            })
            .when_some(date_modal, |d, p| {
                d.child(
                    DatePicker::modal("pk-modal-date")
                        .exit(p)
                        .value(self.draft_date)
                        .on_change(Self::set(cx, |this, date| this.draft_date = Some(date)))
                        .on_confirm(Self::click(cx, |this| {
                            this.chosen_date = this.draft_date;
                            this.modal_date = false;
                        }))
                        .on_cancel(Self::click(cx, |this| this.modal_date = false)),
                )
            })
            .when_some(range_modal, |d, p| {
                d.child(
                    DatePicker::modal("pk-modal-range")
                        .range(true)
                        .exit(p)
                        .range_value(self.draft_range)
                        .on_range_change(Self::set(cx, |this, range| this.draft_range = Some(range)))
                        .on_confirm(Self::click(cx, |this| {
                            this.chosen_range = this.draft_range;
                            this.modal_range = false;
                        }))
                        .on_cancel(Self::click(cx, |this| this.modal_range = false)),
                )
            })
            .when_some(time_modal, |d, p| {
                d.child(
                    TimePicker::modal("pk-modal-time")
                        .exit(p)
                        .value(self.draft_time)
                        .start_with_input(self.time_input)
                        .on_change(Self::set(cx, |this, time| this.draft_time = time))
                        .on_confirm(Self::click(cx, |this| {
                            this.chosen_time = Some(this.draft_time);
                            this.modal_time = false;
                        }))
                        .on_cancel(Self::click(cx, |this| this.modal_time = false)),
                )
            })
    }
}
