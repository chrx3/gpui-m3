//! Date picker: a calendar with a single date or a range, docked in place or modal over a scrim.
//!
//! The calendar maths is done here, without a date library: [`Date`], [`DateRange`], [`Weekday`]
//! and the week grid of a month are plain values with unit tests. [`DateNames`] holds the words
//! (months and week days) so an app can show them in its own language, like `KeyNames` does for
//! keys.

use std::f32::consts::PI;
use std::fmt;
use std::rc::Rc;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Div, ElementId, Entity, Hsla, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, ScrollHandle, SharedString, Stateful, StatefulInteractiveElement,
    Styled, Window, div, px, relative,
};

use super::overlays::{ground, over};
use super::{
    Button, ChangeHandler, ClickHandler, Dialog, Icon, RoundedExt, TextField, TextFieldEvent, all,
    child_id, corners,
};
use crate::interaction::{interactive, state_layer, use_interaction};
use crate::motion::{Spring, animate_color, animate_f32, entrance, key, request_frame};
use crate::theme::type_scale::{
    BODY_LARGE, HEADLINE_LARGE, LABEL_LARGE, TypeScaleExt,
};
use crate::theme::{Scheme, Theme, alpha, font_family};

// ---------------------------------------------------------------------------------------------
// Calendar maths
// ---------------------------------------------------------------------------------------------

/// A day of the week. The default first day of a week is Monday.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Weekday {
    #[default]
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    const ALL: [Weekday; 7] = [
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Friday,
        Weekday::Saturday,
        Weekday::Sunday,
    ];

    /// 0 for Monday up to 6 for Sunday.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The day with that index (taken modulo 7): 0 is Monday.
    pub fn from_index(index: usize) -> Self {
        Self::ALL[index % 7]
    }
}

/// A calendar date in the proleptic Gregorian calendar. Fields are public for reading; build one
/// with [`Date::new`], which checks it. Dates order by year, month and day, and display as
/// ISO `YYYY-MM-DD`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: i32,
    /// 1 to 12.
    pub month: u8,
    /// 1 to the length of the month.
    pub day: u8,
}

/// Error of [`Date::from_str`]: the text is not a valid `YYYY-MM-DD` date.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseDateError;

impl fmt::Display for ParseDateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a valid YYYY-MM-DD date")
    }
}

impl std::error::Error for ParseDateError {}

/// Days since 1970-01-01 of a civil date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let year_of_era = y - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The civil date of a number of days since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 { month_index + 3 } else { month_index - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

impl Date {
    /// A date, or `None` when the month or the day do not exist.
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        if day == 0 || day > Self::days_in_month(year, month) {
            return None;
        }
        Some(Self { year, month, day })
    }

    /// Today in UTC (the system clock has no time zone without a date library). Use
    /// [`DatePicker::today`] to set the local date.
    pub fn today_utc() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        Self::from_days(seconds.div_euclid(86_400))
    }

    pub fn is_leap_year(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    /// Length of a month (0 for a month that does not exist).
    pub fn days_in_month(year: i32, month: u8) -> u8 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if Self::is_leap_year(year) => 29,
            2 => 28,
            _ => 0,
        }
    }

    fn from_days(days: i64) -> Self {
        let (year, month, day) = civil_from_days(days);
        Self { year: year as i32, month: month as u8, day: day as u8 }
    }

    fn to_days(self) -> i64 {
        days_from_civil(self.year.into(), self.month.into(), self.day.into())
    }

    /// The day of the week (Sakamoto's method).
    pub fn weekday(self) -> Weekday {
        const OFFSETS: [i64; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let year = i64::from(self.year) - i64::from(self.month < 3);
        let sunday_first = (year + year.div_euclid(4) - year.div_euclid(100)
            + year.div_euclid(400)
            + OFFSETS[usize::from(self.month) - 1]
            + i64::from(self.day))
        .rem_euclid(7);
        Weekday::from_index((sunday_first as usize + 6) % 7)
    }

    /// The date `days` days later (earlier when negative).
    pub fn add_days(self, days: i64) -> Self {
        Self::from_days(self.to_days() + days)
    }

    /// The date `months` months later, keeping the day or the last day of a shorter month.
    pub fn add_months(self, months: i32) -> Self {
        let total = i64::from(self.year) * 12 + i64::from(self.month) - 1 + i64::from(months);
        let year = total.div_euclid(12) as i32;
        let month = total.rem_euclid(12) as u8 + 1;
        Self { year, month, day: self.day.min(Self::days_in_month(year, month)) }
    }

    /// The date kept between `min` and `max` (either may be missing).
    pub fn clamp_between(self, min: Option<Date>, max: Option<Date>) -> Self {
        let date = min.map_or(self, |min| self.max(min));
        max.map_or(date, |max| date.min(max))
    }

    /// Parses `YYYY-MM-DD`; there is no other format.
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.trim().split('-');
        let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
        let digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
        if parts.next().is_some()
            || !digits(year)
            || !digits(month)
            || !digits(day)
            || year.len() != 4
            || !(1..=2).contains(&month.len())
            || !(1..=2).contains(&day.len())
        {
            return None;
        }
        Self::new(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?)
    }

    /// The calendar of a month: six rows of seven days that start on `first_day`. The days
    /// before and after the month belong to the neighbouring months.
    pub fn month_grid(year: i32, month: u8, first_day: Weekday) -> [[Date; 7]; 6] {
        let first = Date { year, month, day: 1 };
        let lead = (first.weekday().index() + 7 - first_day.index()) % 7;
        let start = first.add_days(-(lead as i64));
        std::array::from_fn(|row| std::array::from_fn(|col| start.add_days((row * 7 + col) as i64)))
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl FromStr for Date {
    type Err = ParseDateError;

    fn from_str(text: &str) -> Result<Self, ParseDateError> {
        Self::parse(text).ok_or(ParseDateError)
    }
}

/// A span of dates, both ends included. `start` is never after `end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DateRange {
    pub start: Date,
    pub end: Date,
}

impl DateRange {
    /// A range between two dates, in whichever order they come.
    pub fn new(a: Date, b: Date) -> Self {
        Self { start: a.min(b), end: a.max(b) }
    }

    pub fn contains(&self, date: Date) -> bool {
        self.start <= date && date <= self.end
    }

    /// Number of days, both ends included.
    pub fn days(&self) -> i64 {
        self.end.to_days() - self.start.to_days() + 1
    }
}

/// What a click on `clicked` does to a range being picked: the first click starts it (a range of
/// one day, waiting for the end), the second one ends it. Returns the range and the date still
/// waiting for its end, if any.
fn range_click(anchor: Option<Date>, clicked: Date) -> (DateRange, Option<Date>) {
    match anchor {
        None => (DateRange::new(clicked, clicked), Some(clicked)),
        Some(start) => (DateRange::new(start, clicked), None),
    }
}

// ---------------------------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------------------------

/// The words of a date picker, so an app can show it in its own language. Arrays start on
/// January and on Monday. [`DateNames::english`] is the default; [`DateNames::spanish`] is the
/// other one. Change a word by assigning the field.
#[derive(Clone, Debug)]
pub struct DateNames {
    pub months: [SharedString; 12],
    /// Used in the headline ("Mon, Mar 9").
    pub months_short: [SharedString; 12],
    pub weekdays_short: [SharedString; 7],
    /// One letter, above the day columns.
    pub weekdays_narrow: [SharedString; 7],
    pub select_date: SharedString,
    pub select_range: SharedString,
    /// Headline while no date is picked.
    pub no_date: SharedString,
    /// Headline while no range is picked.
    pub no_range: SharedString,
    /// Label of the text field in input mode.
    pub input_label: SharedString,
    /// Hint of the text field in input mode.
    pub input_hint: SharedString,
    /// Shown under the text field while its text is not an allowed date.
    pub invalid_date: SharedString,
    pub ok: SharedString,
    pub cancel: SharedString,
    /// "9 Mar" instead of "Mar 9".
    pub day_first: bool,
}

fn names<const N: usize>(words: [&str; N]) -> [SharedString; N] {
    words.map(|word| SharedString::from(word.to_string()))
}

impl Default for DateNames {
    fn default() -> Self {
        Self::english()
    }
}

impl DateNames {
    pub fn english() -> Self {
        Self {
            months: names([
                "January", "February", "March", "April", "May", "June", "July", "August",
                "September", "October", "November", "December",
            ]),
            months_short: names([
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ]),
            weekdays_short: names(["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]),
            weekdays_narrow: names(["M", "T", "W", "T", "F", "S", "S"]),
            select_date: "Select date".into(),
            select_range: "Select dates".into(),
            no_date: "Pick a date".into(),
            no_range: "Start date – End date".into(),
            input_label: "Date".into(),
            input_hint: "YYYY-MM-DD".into(),
            invalid_date: "Not a valid date".into(),
            ok: "OK".into(),
            cancel: "Cancel".into(),
            day_first: false,
        }
    }

    pub fn spanish() -> Self {
        Self {
            months: names([
                "Enero", "Febrero", "Marzo", "Abril", "Mayo", "Junio", "Julio", "Agosto",
                "Septiembre", "Octubre", "Noviembre", "Diciembre",
            ]),
            months_short: names([
                "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
            ]),
            weekdays_short: names(["lun", "mar", "mié", "jue", "vie", "sáb", "dom"]),
            weekdays_narrow: names(["L", "M", "X", "J", "V", "S", "D"]),
            select_date: "Elige una fecha".into(),
            select_range: "Elige las fechas".into(),
            no_date: "Sin fecha".into(),
            no_range: "Inicio – Fin".into(),
            input_label: "Fecha".into(),
            input_hint: "AAAA-MM-DD".into(),
            invalid_date: "Fecha no válida".into(),
            ok: "Aceptar".into(),
            cancel: "Cancelar".into(),
            day_first: true,
        }
    }

    /// "Mon, Mar 9" (or "lun, 9 mar" when [`DateNames::day_first`]).
    pub fn day_text(&self, date: Date) -> String {
        let weekday = &self.weekdays_short[date.weekday().index()];
        let month = &self.months_short[usize::from(date.month) - 1];
        if self.day_first {
            format!("{weekday}, {} {month}", date.day)
        } else {
            format!("{weekday}, {month} {}", date.day)
        }
    }

    /// "Mar 9" without the week day.
    fn short_text(&self, date: Date) -> String {
        let month = &self.months_short[usize::from(date.month) - 1];
        if self.day_first {
            format!("{} {month}", date.day)
        } else {
            format!("{month} {}", date.day)
        }
    }

    /// "March 2026".
    pub fn month_year(&self, year: i32, month: u8) -> String {
        format!("{} {year}", self.months[usize::from(month) - 1])
    }
}

// ---------------------------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Days,
    Years,
    Input,
}

/// What the picker remembers between frames. The date itself belongs to the app.
struct State {
    year: i32,
    month: u8,
    view: View,
    /// The day the keyboard is on.
    cursor: Option<Date>,
    /// The key was used last: show the cursor ring.
    keyboard: bool,
    /// A range's first date, waiting for its end.
    anchor: Option<Date>,
    /// Which way the month last moved (1 later, -1 earlier) and how many times it moved.
    slide: i8,
    generation: u32,
    scroll_years: bool,
    field: Option<Entity<TextField>>,
    // Copies of the picker's settings for the text field's events, refreshed on every render.
    value: Option<Date>,
    min: Option<Date>,
    max: Option<Date>,
    on_change: Option<ChangeHandler<Date>>,
}

impl State {
    fn new(
        start: Date,
        with_field: bool,
        input_label: SharedString,
        input_hint: SharedString,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let field = with_field.then(|| {
            let field = cx.new(|cx| {
                TextField::new(cx)
                    .outlined()
                    .label(input_label)
                    .placeholder(input_hint)
                    .select_all_on_focus(true)
            });
            cx.subscribe_in(&field, window, |state: &mut State, _, event: &TextFieldEvent, window, cx| {
                let TextFieldEvent::Changed(text) = event else { return };
                // The "not a valid date" line follows the text.
                cx.notify();
                let Some(date) = Date::parse(text) else { return };
                if state.value == Some(date) || !state.allows(date) {
                    return;
                }
                state.value = Some(date);
                state.cursor = Some(date);
                state.show_month(date.year, date.month);
                if let Some(on_change) = state.on_change.clone() {
                    on_change(date, window, cx);
                }
                cx.notify();
            })
            .detach();
            field
        });
        Self {
            year: start.year,
            month: start.month,
            view: View::Days,
            cursor: None,
            keyboard: false,
            anchor: None,
            slide: 0,
            generation: 0,
            scroll_years: false,
            field,
            value: None,
            min: None,
            max: None,
            on_change: None,
        }
    }

    fn allows(&self, date: Date) -> bool {
        self.min.is_none_or(|min| date >= min) && self.max.is_none_or(|max| date <= max)
    }

    /// Shows another month; the grid slides in from the side it lies on.
    fn show_month(&mut self, year: i32, month: u8) {
        let direction = (year, month).cmp(&(self.year, self.month));
        if direction.is_eq() {
            return;
        }
        self.slide = if direction.is_gt() { 1 } else { -1 };
        self.generation += 1;
        self.year = year;
        self.month = month;
    }
}

// ---------------------------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------------------------

const WIDTH: f32 = 360.;
const CELL: f32 = 48.;
const CIRCLE: f32 = 40.;
const GRID_HEIGHT: f32 = CELL * 6. + 40.;

/// Material 3 date picker.
///
/// It does not keep the date: pass it with [`DatePicker::value`] (or [`DatePicker::range_value`])
/// and update your own state in [`DatePicker::on_change`] (or [`DatePicker::on_range_change`]).
/// The month on screen, the year list and the keyboard position are its own.
///
/// A docked picker is a surface to place anywhere; a modal one covers the window with a scrim
/// and has OK and Cancel buttons ([`DatePicker::on_confirm`], [`DatePicker::on_cancel`]; Escape
/// and a click on the scrim cancel). Render a modal one only while it is open, and drive its exit
/// with [`DatePicker::exit`] like a [`Dialog`].
///
/// Keyboard, once the calendar has focus (a click on a day gives it, a modal takes it): the
/// arrows move the day, Enter or Space picks it, PageUp and PageDown change the month (with
/// Shift, the year).
#[derive(IntoElement)]
pub struct DatePicker {
    id: ElementId,
    modal: bool,
    range: bool,
    header: bool,
    input_mode: bool,
    title: Option<SharedString>,
    value: Option<Date>,
    range_value: Option<DateRange>,
    min: Option<Date>,
    max: Option<Date>,
    first_day: Weekday,
    names: DateNames,
    today: Date,
    width: gpui::Pixels,
    shown: f32,
    on_change: Option<ChangeHandler<Date>>,
    on_range_change: Option<ChangeHandler<DateRange>>,
    on_confirm: Option<ClickHandler>,
    on_cancel: Option<ClickHandler>,
}

impl DatePicker {
    fn with_modal(id: impl Into<ElementId>, modal: bool) -> Self {
        Self {
            id: id.into(),
            modal,
            range: false,
            header: modal,
            input_mode: true,
            title: None,
            value: None,
            range_value: None,
            min: None,
            max: None,
            first_day: Weekday::Monday,
            names: DateNames::english(),
            today: Date::today_utc(),
            width: px(WIDTH),
            shown: 1.,
            on_change: None,
            on_range_change: None,
            on_confirm: None,
            on_cancel: None,
        }
    }

    /// A picker drawn in place, without header (add one with [`DatePicker::header`]).
    pub fn docked(id: impl Into<ElementId>) -> Self {
        Self::with_modal(id, false)
    }

    /// A picker over a scrim, with header and OK / Cancel buttons.
    pub fn modal(id: impl Into<ElementId>) -> Self {
        Self::with_modal(id, true)
    }

    /// Picks a range of dates instead of one: the first click is the start, the second the end.
    pub fn range(mut self, range: bool) -> Self {
        self.range = range;
        self
    }

    /// Shows the header with the picked date (a docked picker has none by default).
    pub fn header(mut self, header: bool) -> Self {
        self.header = header;
        self
    }

    /// Lets the header switch to typing the date (on by default; single dates with a header).
    pub fn input_mode(mut self, input_mode: bool) -> Self {
        self.input_mode = input_mode;
        self
    }

    /// Replaces the small title above the headline.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The picked date.
    pub fn value(mut self, value: Option<Date>) -> Self {
        self.value = value;
        self
    }

    /// The picked range, for a [`DatePicker::range`] picker.
    pub fn range_value(mut self, value: Option<DateRange>) -> Self {
        self.range_value = value;
        self
    }

    /// The earliest date that can be picked; earlier days are disabled.
    pub fn min(mut self, min: Date) -> Self {
        self.min = Some(min);
        self
    }

    /// The latest date that can be picked; later days are disabled.
    pub fn max(mut self, max: Date) -> Self {
        self.max = Some(max);
        self
    }

    /// First column of the calendar (Monday by default).
    pub fn first_day_of_week(mut self, day: Weekday) -> Self {
        self.first_day = day;
        self
    }

    /// Month and day names, and the labels (English by default).
    pub fn names(mut self, names: DateNames) -> Self {
        self.names = names;
        self
    }

    /// The date drawn with a ring (the UTC date by default).
    pub fn today(mut self, today: Date) -> Self {
        self.today = today;
        self
    }

    /// Width of a docked picker (360 px by default).
    pub fn width(mut self, width: gpui::Pixels) -> Self {
        self.width = width;
        self
    }

    /// Draws a modal picker while it leaves: `progress` goes from 1 to 0, as
    /// [`crate::motion::presence`] returns it.
    pub fn exit(mut self, progress: f32) -> Self {
        self.shown = progress.clamp(0., 1.);
        self
    }

    /// Called with the date every time one is picked or typed.
    pub fn on_change(mut self, f: impl Fn(Date, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    /// Called with the range on every click of a range picker: after the first click it is one
    /// day long, after the second it is complete.
    pub fn on_range_change(mut self, f: impl Fn(DateRange, &mut Window, &mut App) + 'static) -> Self {
        self.on_range_change = Some(Rc::new(f));
        self
    }

    /// Called by the OK button of a modal picker.
    pub fn on_confirm(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_confirm = Some(Rc::new(f));
        self
    }

    /// Called by the Cancel button, Escape and a click on the scrim of a modal picker.
    pub fn on_cancel(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(f));
        self
    }
}

/// What picking a date does.
type SelectDate = Rc<dyn Fn(Date, &mut Window, &mut App)>;

/// A round 40 px button with an icon, in the calendar's header row.
#[allow(clippy::too_many_arguments)]
fn icon_circle(
    id: ElementId,
    icon: &'static str,
    rotate: f32,
    enabled: bool,
    color: Hsla,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let state = use_interaction(&id, window, cx);
    let fg = if enabled { color } else { alpha(color, 0.38) };
    let layer = state_layer(&id, &state, all(px(CIRCLE / 2.)), fg, window, cx);
    let button = div()
        .id(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(CIRCLE))
        .rounded(px(CIRCLE / 2.))
        .child(layer)
        .child(Icon::new(icon).size(px(20.)).color(fg).rotate(rotate));
    let button = interactive(button, &state, enabled, true);
    if enabled { button.on_click(move |_, window, cx| on_click(window, cx)) } else { button }
}

/// Number that identifies a date in element ids: the colors of a day animate where the day is.
fn date_key(date: Date) -> u32 {
    date.year.unsigned_abs() * 10_000 + u32::from(date.month) * 100 + u32::from(date.day)
}

fn cell_id(id: &ElementId, part: &'static str, n: u32) -> ElementId {
    ElementId::NamedChild(Box::new(key(id, part)), n.to_string().into())
}

/// How a day looks.
#[derive(Clone, Copy, PartialEq)]
enum Mark {
    None,
    /// Between the ends of a range.
    Inside,
    /// The picked date, or an end of a range.
    Edge,
}

impl RenderOnce for DatePicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let id = self.id.clone();
        let picked_start = self.value.or(self.range_value.map(|r| r.start));
        let start = picked_start.unwrap_or(self.today).clamp_between(self.min, self.max);
        let with_field = self.input_mode && !self.range && self.header;
        let (label, hint) = (self.names.input_label.clone(), self.names.input_hint.clone());
        let state = window.use_keyed_state(key(&id, "state"), cx, move |window, cx| {
            State::new(start, with_field, label, hint, window, cx)
        });
        state.update(cx, |st, _| {
            st.value = self.value;
            st.min = self.min;
            st.max = self.max;
            st.on_change = self.on_change.clone();
        });
        let focus = window.use_keyed_state(key(&id, "focus"), cx, |_, cx| cx.focus_handle());
        let handle = focus.read(cx).clone();
        if self.modal {
            let asked = window.use_keyed_state(key(&id, "autofocus"), cx, |_, _| false);
            if !*asked.read(cx) {
                asked.update(cx, |asked, _| *asked = true);
                window.focus(&handle);
            }
        }

        let (year, month, view, cursor, keyboard, anchor, slide, generation, scroll_years) = {
            let st = state.read(cx);
            (
                st.year, st.month, st.view, st.cursor, st.keyboard, st.anchor, st.slide,
                st.generation, st.scroll_years,
            )
        };
        let (min, max, range_mode, today) = (self.min, self.max, self.range, self.today);
        let enabled = move |date: Date| min.is_none_or(|m| date >= m) && max.is_none_or(|m| date <= m);

        // What picking a date does.
        let select: SelectDate = {
            let state = state.clone();
            let handle = handle.clone();
            let on_change = self.on_change.clone();
            let on_range = self.on_range_change.clone();
            Rc::new(move |date, window, cx| {
                if !enabled(date) {
                    return;
                }
                window.focus(&handle);
                let range = state.update(cx, |st, cx| {
                    st.cursor = Some(date);
                    st.show_month(date.year, date.month);
                    let range = range_mode.then(|| {
                        let (range, anchor) = range_click(st.anchor, date);
                        st.anchor = anchor;
                        range
                    });
                    cx.notify();
                    range
                });
                match range {
                    Some(range) => {
                        if let Some(f) = &on_range {
                            f(range, window, cx);
                        }
                    }
                    None => {
                        if let Some(f) = &on_change {
                            f(date, window, cx);
                        }
                    }
                }
            })
        };

        // The month on screen: move it by months, or to a date.
        let go_to_month = {
            let state = state.clone();
            move |delta: i32, cx: &mut App| {
                state.update(cx, |st, cx| {
                    let moved = Date { year: st.year, month: st.month, day: 1 }.add_months(delta);
                    st.show_month(moved.year, moved.month);
                    cx.notify();
                })
            }
        };

        // ----- header
        let mut surface = div()
            .id(key(&id, "root"))
            .track_focus(&handle)
            .flex()
            .flex_col()
            .w_full()
            .text_color(s.on_surface)
            .font_family(font_family());

        let cancel = self.on_cancel.clone();
        {
            let state = state.clone();
            let select = select.clone();
            let cancel = self.modal.then(|| cancel.clone()).flatten();
            let modal = self.modal;
            let picked = picked_start;
            surface = surface.on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "escape" && modal {
                    if let Some(cancel) = &cancel {
                        cx.stop_propagation();
                        cancel(&ClickEvent::default(), window, cx);
                    }
                    return;
                }
                let (view, shown, cursor) = {
                    let st = state.read(cx);
                    (st.view, Date { year: st.year, month: st.month, day: 1 }, st.cursor)
                };
                if view != View::Days {
                    return;
                }
                // The arrows start from the picked day when it is on screen, else the 1st.
                let from = cursor
                    .or(picked.filter(|d| (d.year, d.month) == (shown.year, shown.month)))
                    .unwrap_or(shown);
                let shift = event.keystroke.modifiers.shift;
                let target = match key {
                    "left" => from.add_days(-1),
                    "right" => from.add_days(1),
                    "up" => from.add_days(-7),
                    "down" => from.add_days(7),
                    "pageup" => from.add_months(if shift { -12 } else { -1 }),
                    "pagedown" => from.add_months(if shift { 12 } else { 1 }),
                    "enter" | "space" if cursor.is_some() => {
                        cx.stop_propagation();
                        select(from, window, cx);
                        return;
                    }
                    _ => return,
                };
                cx.stop_propagation();
                let target = target.clamp_between(min, max);
                state.update(cx, |st, cx| {
                    st.cursor = Some(target);
                    st.keyboard = true;
                    st.show_month(target.year, target.month);
                    cx.notify();
                });
            });
        }

        let ranged = if range_mode {
            self.range_value.map(|r| (r.start, r.end))
        } else {
            None
        };
        // The range being drawn: the ends picked so far, or the pick and the day under the pointer.
        let dates = Date::month_grid(year, month, self.first_day);
        let hovered = anchor.and_then(|_| {
            dates.iter().flatten().copied().find(|d| {
                let id = cell_id(&id, "day", date_key(*d));
                use_interaction(&id, window, cx).read(cx).hovered
            })
        });
        let shown_range = match (anchor, hovered) {
            (Some(a), Some(h)) => Some(DateRange::new(a, h)),
            (Some(a), None) => Some(DateRange::new(a, a)),
            (None, _) => ranged.map(|(a, b)| DateRange::new(a, b)),
        };

        if self.header {
            let title = self.title.clone().unwrap_or_else(|| {
                if range_mode { self.names.select_range.clone() } else { self.names.select_date.clone() }
            });
            let headline: SharedString = if range_mode {
                match (shown_range, anchor) {
                    (None, _) => self.names.no_range.clone(),
                    (Some(r), Some(_)) if r.start == r.end => {
                        format!("{} –", self.names.short_text(r.start)).into()
                    }
                    (Some(r), _) if r.start == r.end => self.names.day_text(r.start).into(),
                    (Some(r), _) => format!(
                        "{} – {}",
                        self.names.short_text(r.start),
                        self.names.short_text(r.end)
                    )
                    .into(),
                }
            } else {
                match self.value {
                    Some(date) => self.names.day_text(date).into(),
                    None => self.names.no_date.clone(),
                }
            };
            let empty = if range_mode { shown_range.is_none() } else { self.value.is_none() };
            let toggle = with_field.then(|| {
                let state = state.clone();
                let typing = view == View::Input;
                let value = self.value;
                icon_circle(
                    key(&id, "toggle"),
                    if typing { "blocks" } else { "pen" },
                    0.,
                    true,
                    s.on_surface_variant,
                    move |window, cx| {
                        let field = state.update(cx, |st, cx| {
                            st.view = if typing { View::Days } else { View::Input };
                            cx.notify();
                            st.field.clone()
                        });
                        if let (false, Some(field)) = (typing, field) {
                            field.update(cx, |f, cx| {
                                let text = value.map_or_else(String::new, |d| d.to_string());
                                f.set_text(text, cx);
                            });
                            field.read(cx).focus(window);
                        }
                    },
                    window,
                    cx,
                )
            });
            surface = surface
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .px(px(24.))
                        .pt(px(16.))
                        .pb(px(12.))
                        .child(
                            div()
                                .h(px(20.))
                                .type_style(LABEL_LARGE)
                                .text_color(s.on_surface_variant)
                                .child(title),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .h(px(52.))
                                .child(
                                    div()
                                        .min_w_0()
                                        .type_style(HEADLINE_LARGE)
                                        .text_size(if range_mode { px(28.) } else { px(32.) })
                                        .text_color(if empty { s.on_surface_variant } else { s.on_surface })
                                        .child(headline),
                                )
                                .children(toggle),
                        ),
                )
                .child(div().h(px(1.)).w_full().bg(s.outline_variant));
        }

        // ----- body
        if view != View::Input {
            // Month row.
            let prev_end = Date { year, month, day: 1 }.add_days(-1);
            let next_start = Date { year, month, day: 1 }.add_months(1);
            let can_prev = min.is_none_or(|m| prev_end >= m);
            let can_next = max.is_none_or(|m| next_start <= m);
            let years_open = view == View::Years;
            let month_button = {
                let button_id = key(&id, "month");
                let interaction = use_interaction(&button_id, window, cx);
                let layer = state_layer(
                    &button_id,
                    &interaction,
                    all(px(20.)),
                    s.on_surface_variant,
                    window,
                    cx,
                );
                let arrow = animate_f32(
                    key(&id, "arrow"),
                    Spring::SPATIAL_FAST,
                    if years_open { PI } else { 0. },
                    window,
                    cx,
                );
                let state = state.clone();
                let el = div()
                    .id(button_id)
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .h(px(CIRCLE))
                    .pl(px(12.))
                    .pr(px(8.))
                    .rounded(px(20.))
                    .type_style(LABEL_LARGE)
                    .text_color(s.on_surface_variant)
                    .child(layer)
                    .child(self.names.month_year(year, month))
                    .child(Icon::new("chevron-down").size(px(18.)).rotate(arrow));
                interactive(el, &interaction, true, true).on_click(move |_, _, cx| {
                    state.update(cx, |st, cx| {
                        st.view = if years_open { View::Days } else { View::Years };
                        st.scroll_years = !years_open;
                        cx.notify();
                    })
                })
            };
            let nav = {
                let (a, b) = (go_to_month.clone(), go_to_month.clone());
                div()
                    .flex()
                    .child(icon_circle(
                        key(&id, "prev"),
                        "chevron-right",
                        PI,
                        can_prev,
                        s.on_surface_variant,
                        move |_, cx| a(-1, cx),
                        window,
                        cx,
                    ))
                    .child(icon_circle(
                        key(&id, "next"),
                        "chevron-right",
                        0.,
                        can_next,
                        s.on_surface_variant,
                        move |_, cx| b(1, cx),
                        window,
                        cx,
                    ))
            };
            surface = surface.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(56.))
                    .pl(px(12.))
                    .pr(px(12.))
                    .child(month_button)
                    .when(!years_open, |d| d.child(nav)),
            );

        }
        match view {
            View::Input => {
                if let Some(field) = state.read(cx).field.clone() {
                    let text = field.read(cx).text().clone();
                    let invalid = !text.trim().is_empty()
                        && Date::parse(&text).is_none_or(|date| !enabled(date));
                    surface = surface.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .px(px(24.))
                            .pt(px(24.))
                            .pb(px(16.))
                            .child(field)
                            .child(
                                div()
                                    .type_style(crate::theme::type_scale::BODY_SMALL)
                                    .text_color(if invalid { s.error } else { s.on_surface_variant })
                                    .child(if invalid {
                                        self.names.invalid_date.clone()
                                    } else {
                                        self.names.input_hint.clone()
                                    }),
                            ),
                    );
                }
            }
            View::Years => {
                surface = surface.child(self.year_grid(&state, &s, (year, month), scroll_years, window, cx));
            }
            View::Days => {
                // Week day letters.
                let mut letters = div().flex().px(px(12.)).h(px(40.));
                for col in 0..7 {
                    let day = Weekday::from_index(self.first_day.index() + col);
                    letters = letters.child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .type_style(BODY_LARGE)
                            .text_color(s.on_surface)
                            .child(self.names.weekdays_narrow[day.index()].clone()),
                    );
                }
                surface = surface.child(letters);

                // The six weeks.
                let slide_p = if generation == 0 {
                    1.
                } else {
                    let n = generation.to_string();
                    entrance(
                        ElementId::NamedChild(Box::new(key(&id, "slide")), n.into()),
                        Spring::SPATIAL_FAST,
                        0.,
                        window,
                        cx,
                    )
                };
                let mut grid = div()
                    .relative()
                    .left(px(f32::from(slide) * 28. * (1. - slide_p)))
                    .opacity(slide_p.clamp(0., 1.))
                    .flex()
                    .flex_col()
                    .px(px(12.));
                for week in dates {
                    let mut row = div().flex().h(px(CELL));
                    for (col, date) in week.into_iter().enumerate() {
                        let inside_month = date.month == month;
                        let day_enabled = enabled(date);
                        let (mark, band_left, band_right) = match shown_range {
                            Some(r) if range_mode && r.contains(date) => {
                                let edge = date == r.start || date == r.end;
                                (
                                    if edge { Mark::Edge } else { Mark::Inside },
                                    r.start != r.end && date != r.start,
                                    r.start != r.end && date != r.end,
                                )
                            }
                            _ if !range_mode && self.value == Some(date) => (Mark::Edge, false, false),
                            _ => (Mark::None, false, false),
                        };
                        row = row.child(day_cell(
                            &id,
                            date,
                            col,
                            DayLook {
                                mark,
                                band_left,
                                band_right,
                                today: date == today,
                                inside_month,
                                enabled: day_enabled,
                                ring: keyboard && cursor == Some(date) && handle.is_focused(window),
                            },
                            &s,
                            select.clone(),
                            window,
                            cx,
                        ));
                    }
                    grid = grid.child(row);
                }
                surface = surface.child(grid.h(px(CELL * 6.)));
            }
        }

        if self.modal {
            let mut buttons = div().flex().justify_end().gap(px(8.)).px(px(12.)).pb(px(8.)).pt(px(8.));
            let cancel_button = Button::new(child_id(&id, "cancel"), self.names.cancel.clone()).text();
            let ok_button = Button::new(child_id(&id, "ok"), self.names.ok.clone()).text();
            buttons = buttons
                .child(match cancel {
                    Some(f) => cancel_button.on_click(move |e, w, cx| f(e, w, cx)),
                    None => cancel_button,
                })
                .child(match self.on_confirm {
                    Some(f) => ok_button.on_click(move |e, w, cx| f(e, w, cx)),
                    None => ok_button,
                });
            surface = surface.child(buttons);
            let dialog = Dialog::new(key(&id, "dialog"))
                .width(px(WIDTH))
                .padding(px(0.))
                .exit(self.shown)
                .child(surface);
            return match self.on_cancel {
                Some(f) => dialog.on_dismiss(move |e, w, cx| f(e, w, cx)),
                None => dialog,
            }
            .into_any_element();
        }

        let background = if s.surface.a < 1. {
            over(s.surface_container_high, ground(&s))
        } else {
            s.surface_container_high
        };
        div()
            .flex_none()
            .w(self.width)
            .rounded(px(16.))
            .overflow_hidden()
            .bg(background)
            .child(surface)
            .into_any_element()
    }
}

impl DatePicker {
    /// The list of years, three to a row, that replaces the calendar.
    fn year_grid(
        &self,
        state: &Entity<State>,
        s: &Scheme,
        (shown_year, shown_month): (i32, u8),
        scroll: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let id = &self.id;
        let low = self.min.map_or(1900, |d| d.year).min(shown_year);
        let high = self.max.map_or(2100, |d| d.year).max(shown_year);
        let scroll_handle = window
            .use_keyed_state(key(id, "years-scroll"), cx, |_, _| ScrollHandle::new())
            .read(cx)
            .clone();
        let columns = 3;
        let rows = (high - low) as usize / columns + 1;
        if scroll {
            // Opens with the shown year near the top. The list must have been laid out once
            // before it can scroll, so this waits for its first frame.
            if scroll_handle.bounds().size.height > px(0.) {
                scroll_handle.scroll_to_top_of_item(((shown_year - low) as usize / columns).saturating_sub(1));
                state.update(cx, |st, _| st.scroll_years = false);
            } else {
                request_frame(window);
            }
        }
        let this_year = self.today.year;
        let picked = self.value.map(|d| d.year);
        let mut list = div()
            .id(key(id, "years"))
            .track_scroll(&scroll_handle)
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .h(px(GRID_HEIGHT + 56.))
            .px(px(12.))
            .pb(px(8.));
        for r in 0..rows {
            let mut row = div().flex().flex_none().h(px(52.));
            for c in 0..columns {
                let year = low + (r * columns + c) as i32;
                if year > high {
                    row = row.child(div().flex_1());
                    continue;
                }
                let on = year == shown_year;
                let button_id = cell_id(id, "year", year.unsigned_abs());
                let interaction = use_interaction(&button_id, window, cx);
                let (bg, fg) = if on {
                    (s.primary, s.on_primary)
                } else {
                    (alpha(s.primary, 0.), s.on_surface_variant)
                };
                let bg = animate_color(key(&button_id, "bg"), Spring::EFFECTS, bg, window, cx);
                let layer = state_layer(&button_id, &interaction, all(px(18.)), fg, window, cx);
                let state = state.clone();
                let pill = div()
                    .id(button_id)
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(72.))
                    .h(px(36.))
                    .rounded(px(18.))
                    .bg(bg)
                    .type_style(BODY_LARGE)
                    .text_color(if on { s.on_primary } else if year == this_year { s.primary } else { s.on_surface })
                    .when(!on && (year == this_year || Some(year) == picked), |d| {
                        d.border_1().border_color(s.outline)
                    })
                    .child(layer)
                    .child(year.to_string());
                let pill = interactive(pill, &interaction, true, true).on_click(move |_, _, cx| {
                    state.update(cx, |st, cx| {
                        let day = Date { year, month: shown_month, day: 1 };
                        st.show_month(day.year, day.month);
                        st.view = View::Days;
                        cx.notify();
                    })
                });
                row = row.child(div().flex_1().flex().items_center().justify_center().child(pill));
            }
            list = list.child(row);
        }
        list.into_any_element()
    }
}

/// How one day cell is drawn.
struct DayLook {
    mark: Mark,
    /// The range band continues to the left / right of the day.
    band_left: bool,
    band_right: bool,
    today: bool,
    inside_month: bool,
    enabled: bool,
    /// The keyboard cursor is here.
    ring: bool,
}

#[allow(clippy::too_many_arguments)]
fn day_cell(
    id: &ElementId,
    date: Date,
    column: usize,
    look: DayLook,
    s: &Scheme,
    select: SelectDate,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let day_id = cell_id(id, "day", date_key(date));
    let interaction = use_interaction(&day_id, window, cx);
    let (bg, fg) = match look.mark {
        _ if !look.enabled => (alpha(s.primary, 0.), alpha(s.on_surface, 0.38)),
        Mark::Edge => (s.primary, s.on_primary),
        Mark::Inside => (alpha(s.primary, 0.), s.on_secondary_container),
        Mark::None if look.today => (alpha(s.primary, 0.), s.primary),
        Mark::None if look.inside_month => (alpha(s.primary, 0.), s.on_surface),
        Mark::None => (alpha(s.primary, 0.), alpha(s.on_surface, 0.5)),
    };
    let bg = animate_color(key(&day_id, "bg"), Spring::EFFECTS, bg, window, cx);
    let fg = animate_color(key(&day_id, "fg"), Spring::EFFECTS, fg, window, cx);
    let layer = state_layer(&day_id, &interaction, all(px(CIRCLE / 2.)), fg, window, cx);

    let radius = px(CIRCLE / 2.);
    let band = |left: f32, right: f32, round: [f32; 4]| {
        div()
            .absolute()
            .top(px((CELL - CIRCLE) / 2.))
            .h(px(CIRCLE))
            .left(relative(left))
            .right(relative(right))
            .corners(corners(round))
            .bg(s.secondary_container)
    };
    let mut cell = div().relative().flex_1().h(px(CELL)).flex().items_center().justify_center();
    if look.mark != Mark::None && look.enabled {
        // The band runs under the days of the range, and rounds at the ends of a week.
        let first_in_week = column == 0;
        let last_in_week = column == 6;
        let left_round = if first_in_week { CIRCLE / 2. } else { 0. };
        let right_round = if last_in_week { CIRCLE / 2. } else { 0. };
        match look.mark {
            Mark::Inside => {
                cell = cell.child(band(0., 0., [left_round, right_round, right_round, left_round]));
            }
            Mark::Edge => {
                if look.band_left {
                    cell = cell.child(band(0., 0.5, [left_round, 0., 0., left_round]));
                }
                if look.band_right {
                    cell = cell.child(band(0.5, 0., [0., right_round, right_round, 0.]));
                }
            }
            Mark::None => {}
        }
    }
    let mut circle = div()
        .id(day_id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(CIRCLE))
        .rounded(radius)
        .bg(bg)
        .type_style(BODY_LARGE)
        .text_color(fg)
        .when(look.today && look.mark != Mark::Edge, |d| d.border_1().border_color(s.outline))
        .child(layer)
        .child(date.day.to_string());
    if look.ring {
        circle = circle.child(
            div()
                .absolute()
                .top(px(-3.))
                .left(px(-3.))
                .right(px(-3.))
                .bottom(px(-3.))
                .border_2()
                .border_color(s.primary)
                .rounded(radius + px(3.)),
        );
    }
    let circle = interactive(circle, &interaction, look.enabled, true);
    let circle = if look.enabled {
        circle.on_click(move |_, window, cx| select(date, window, cx))
    } else {
        circle.cursor_default()
    };
    cell.child(circle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::new(year, month, day).unwrap()
    }

    #[test]
    fn leap_years_follow_the_gregorian_rule() {
        assert!(Date::is_leap_year(2024));
        assert!(Date::is_leap_year(2000));
        assert!(!Date::is_leap_year(1900));
        assert!(!Date::is_leap_year(2023));
        assert!(Date::is_leap_year(-4));
    }

    #[test]
    fn months_have_their_length() {
        let lengths: Vec<u8> = (1..=12).map(|m| Date::days_in_month(2023, m)).collect();
        assert_eq!(lengths, [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]);
        assert_eq!(Date::days_in_month(2024, 2), 29);
        assert_eq!(Date::days_in_month(2024, 0), 0);
        assert_eq!(Date::days_in_month(2024, 13), 0);
    }

    #[test]
    fn new_rejects_dates_that_do_not_exist() {
        assert!(Date::new(2023, 2, 29).is_none());
        assert!(Date::new(2024, 2, 29).is_some());
        assert!(Date::new(2024, 4, 31).is_none());
        assert!(Date::new(2024, 0, 10).is_none());
        assert!(Date::new(2024, 13, 10).is_none());
        assert!(Date::new(2024, 5, 0).is_none());
    }

    #[test]
    fn weekdays_of_known_dates() {
        assert_eq!(date(1970, 1, 1).weekday(), Weekday::Thursday);
        assert_eq!(date(2000, 1, 1).weekday(), Weekday::Saturday);
        assert_eq!(date(2024, 2, 29).weekday(), Weekday::Thursday);
        assert_eq!(date(2026, 3, 9).weekday(), Weekday::Monday);
        assert_eq!(date(1900, 1, 1).weekday(), Weekday::Monday);
        assert_eq!(date(1999, 12, 31).weekday(), Weekday::Friday);
    }

    #[test]
    fn weekday_agrees_with_counting_days() {
        let mut day = date(1969, 12, 25);
        for n in 0..20_000 {
            assert_eq!(day.weekday().index(), ((n + 3) % 7) as usize, "{day}");
            let next = day.add_days(1);
            assert!(next > day);
            day = next;
        }
    }

    #[test]
    fn adding_days_crosses_months_and_years() {
        assert_eq!(date(2023, 12, 31).add_days(1), date(2024, 1, 1));
        assert_eq!(date(2024, 3, 1).add_days(-1), date(2024, 2, 29));
        assert_eq!(date(2023, 3, 1).add_days(-1), date(2023, 2, 28));
        assert_eq!(date(2026, 3, 9).add_days(366), date(2027, 3, 10));
        assert_eq!(date(1999, 12, 31).add_days(0), date(1999, 12, 31));
    }

    #[test]
    fn adding_months_keeps_the_day_when_it_can() {
        assert_eq!(date(2024, 1, 31).add_months(1), date(2024, 2, 29));
        assert_eq!(date(2023, 1, 31).add_months(1), date(2023, 2, 28));
        assert_eq!(date(2024, 12, 15).add_months(1), date(2025, 1, 15));
        assert_eq!(date(2024, 1, 15).add_months(-1), date(2023, 12, 15));
        assert_eq!(date(2024, 5, 31).add_months(-12), date(2023, 5, 31));
        assert_eq!(date(2024, 3, 31).add_months(-1), date(2024, 2, 29));
    }

    #[test]
    fn clamping_respects_either_limit() {
        let (min, max) = (date(2024, 3, 10), date(2024, 3, 20));
        assert_eq!(date(2024, 3, 5).clamp_between(Some(min), Some(max)), min);
        assert_eq!(date(2024, 3, 25).clamp_between(Some(min), Some(max)), max);
        assert_eq!(date(2024, 3, 15).clamp_between(Some(min), Some(max)), date(2024, 3, 15));
        assert_eq!(date(2024, 3, 5).clamp_between(None, Some(max)), date(2024, 3, 5));
        assert_eq!(date(2024, 3, 25).clamp_between(Some(min), None), date(2024, 3, 25));
        assert_eq!(date(2024, 3, 25).clamp_between(None, None), date(2024, 3, 25));
    }

    #[test]
    fn dates_order_and_display_as_iso() {
        assert!(date(2023, 12, 31) < date(2024, 1, 1));
        assert!(date(2024, 1, 31) < date(2024, 2, 1));
        assert!(date(2024, 2, 1) < date(2024, 2, 2));
        assert_eq!(date(2024, 3, 9).to_string(), "2024-03-09");
        assert_eq!(date(987, 11, 1).to_string(), "0987-11-01");
    }

    #[test]
    fn parsing_is_strict_about_the_shape() {
        assert_eq!(Date::parse("2024-03-09"), Some(date(2024, 3, 9)));
        assert_eq!("2024-3-9".parse::<Date>(), Ok(date(2024, 3, 9)));
        assert_eq!(Date::parse(" 2024-03-09 "), Some(date(2024, 3, 9)));
        assert_eq!(Date::parse("2024-02-30"), None);
        assert_eq!(Date::parse("24-03-09"), None);
        assert_eq!(Date::parse("2024-03"), None);
        assert_eq!(Date::parse("2024-03-09-01"), None);
        assert_eq!(Date::parse("2024-003-09"), None);
        assert_eq!(Date::parse("2024/03/09"), None);
        assert_eq!(Date::parse("+024-03-09"), None);
        assert_eq!(Date::parse(""), None);
        assert_eq!("nope".parse::<Date>(), Err(ParseDateError));
        let day = date(2031, 8, 4);
        assert_eq!(day.to_string().parse::<Date>(), Ok(day));
    }

    #[test]
    fn today_is_a_real_date() {
        let today = Date::today_utc();
        assert!(today.year >= 2024);
        assert!(Date::new(today.year, today.month, today.day).is_some());
    }

    #[test]
    fn grid_starts_on_the_first_day_of_the_week() {
        // March 2026 begins on a Sunday.
        let monday_first = Date::month_grid(2026, 3, Weekday::Monday);
        assert_eq!(monday_first[0][0], date(2026, 2, 23));
        assert_eq!(monday_first[0][6], date(2026, 3, 1));
        assert_eq!(monday_first[5][6], date(2026, 4, 5));
        let sunday_first = Date::month_grid(2026, 3, Weekday::Sunday);
        assert_eq!(sunday_first[0][0], date(2026, 3, 1));
        assert_eq!(sunday_first[4][6], date(2026, 4, 4));
        // A month that starts on the first day has no days of the month before.
        let february = Date::month_grid(2021, 2, Weekday::Monday);
        assert_eq!(february[0][0], date(2021, 2, 1));
        assert_eq!(february[3][6], date(2021, 2, 28));
        assert_eq!(february[4][0], date(2021, 3, 1));
    }

    #[test]
    fn grid_is_consecutive_and_holds_the_whole_month() {
        for first_day in Weekday::ALL {
            for (year, month) in [(2024, 2), (2023, 2), (2026, 3), (2025, 12), (1999, 1)] {
                let grid = Date::month_grid(year, month, first_day);
                let days: Vec<Date> = grid.iter().flatten().copied().collect();
                assert_eq!(days.len(), 42);
                assert!(days.windows(2).all(|pair| pair[1] == pair[0].add_days(1)));
                assert_eq!(days[0].weekday(), first_day);
                let inside = days.iter().filter(|d| d.month == month && d.year == year).count();
                assert_eq!(inside, usize::from(Date::days_in_month(year, month)));
            }
        }
    }

    #[test]
    fn ranges_order_their_ends() {
        let (a, b) = (date(2024, 3, 20), date(2024, 3, 10));
        let range = DateRange::new(a, b);
        assert_eq!(range.start, b);
        assert_eq!(range.end, a);
        assert_eq!(range, DateRange::new(b, a));
        assert!(range.contains(date(2024, 3, 10)));
        assert!(range.contains(date(2024, 3, 15)));
        assert!(range.contains(date(2024, 3, 20)));
        assert!(!range.contains(date(2024, 3, 9)));
        assert!(!range.contains(date(2024, 3, 21)));
        assert_eq!(range.days(), 11);
        assert_eq!(DateRange::new(a, a).days(), 1);
    }

    #[test]
    fn two_clicks_make_a_range_in_any_order() {
        let (first, second) = (date(2024, 3, 20), date(2024, 3, 10));
        let (range, anchor) = range_click(None, first);
        assert_eq!(range, DateRange::new(first, first));
        assert_eq!(anchor, Some(first));
        let (range, anchor) = range_click(anchor, second);
        assert_eq!(range, DateRange { start: second, end: first });
        assert_eq!(anchor, None);
        // The next click starts over.
        assert_eq!(range_click(anchor, second).1, Some(second));
    }

    #[test]
    fn names_are_complete_and_text_follows_the_order() {
        for names in [DateNames::english(), DateNames::spanish()] {
            assert!(names.months.iter().chain(&names.months_short).all(|m| !m.is_empty()));
            assert!(names.weekdays_short.iter().chain(&names.weekdays_narrow).all(|d| !d.is_empty()));
        }
        let day = date(2026, 3, 9);
        assert_eq!(DateNames::english().day_text(day), "Mon, Mar 9");
        assert_eq!(DateNames::spanish().day_text(day), "lun, 9 mar");
        assert_eq!(DateNames::english().month_year(2026, 3), "March 2026");
        assert_eq!(DateNames::spanish().month_year(2026, 1), "Enero 2026");
    }
}
