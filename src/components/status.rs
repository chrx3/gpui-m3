//! Status and placeholder pieces: a status mark that morphs between states, an empty state, a
//! shimmering label and a page indicator.

use std::f32::consts::PI;
use std::ops::Range;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Bounds, ElementId, HighlightStyle, Hsla, InteractiveElement, IntoElement,
    ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    StyledText, Window, canvas, div, point, px,
};

use super::{ChangeHandler, Icon, LoadingIndicator, Shape};
use crate::motion::{
    MotionSettings, Spring, animate_color, animate_px, continuous_at, element_state, key, replay,
};
use crate::shapes::{Morph, ShapeName, Tone};
use crate::theme::type_scale::{BODY_MEDIUM, TITLE_MEDIUM, TypeScaleExt};
use crate::theme::{Scheme, Theme, font_family, mix};

/// The state shown by a [`StatusMark`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Status {
    /// Not started: a small dot in the outline color.
    #[default]
    Pending,
    /// In progress: an empty slot of the mark's size, or a [`LoadingIndicator`] with
    /// [`StatusMark::spinner`].
    Running,
    /// Finished: a check in a primary Cookie9.
    Done,
    /// Failed: a cross in an error-colored rounded square.
    Failed,
    /// Waiting for someone (a permission, an answer): a clock in a tertiary Cookie4.
    Waiting,
}

/// How a [`Status`] is drawn: its shape, the share of the mark it fills and its icon.
#[derive(Clone, Copy, Debug, PartialEq)]
struct MarkLook {
    shape: ShapeName,
    scale: f32,
    icon: Option<&'static str>,
}

fn mark_look(status: Status) -> MarkLook {
    let (shape, scale, icon) = match status {
        // Running morphs out of the pending dot (its own drawing is the spinner or nothing).
        Status::Pending | Status::Running => (ShapeName::Circle, 0.42, None),
        Status::Done => (ShapeName::Cookie9, 1., Some("check")),
        Status::Failed => (ShapeName::Square, 0.86, Some("x")),
        Status::Waiting => (ShapeName::Cookie4, 0.94, Some("history")),
    };
    MarkLook { shape, scale, icon }
}

/// Container and icon colors of a status mark.
fn mark_colors(s: &Scheme, status: Status) -> (Hsla, Hsla) {
    match status {
        Status::Pending | Status::Running => (s.outline, s.outline),
        Status::Done => (s.primary, s.on_primary),
        Status::Failed => (s.error, s.on_error),
        Status::Waiting => (s.tertiary, s.on_tertiary),
    }
}

/// What the mark remembers between frames: the status it shows and the one it came from.
/// Returns the new state and the status to morph from.
fn advance_status(state: Option<(Status, Status)>, status: Status) -> ((Status, Status), Status) {
    match state {
        Some((current, previous)) if current == status => ((current, previous), previous),
        Some((current, _)) => ((status, current), current),
        None => ((status, status), status),
    }
}

/// A small mark for the state of a task, a tool call or a step: pending, running, done, failed
/// or waiting.
///
/// When the status changes, the mark morphs from the old shape to the new one once, with the
/// fast spatial spring (a slight turn and bounce), and settles: it requests no frames at rest,
/// and nothing plays when it first appears or with reduced motion. `id` must be unique within
/// the view.
///
/// While [`Status::Running`] it leaves an empty slot of its size, so an app can draw its
/// loading indicator there from a separate layer (a continuous animation redraws the view that
/// holds it); [`StatusMark::spinner`] draws a [`LoadingIndicator`] in place instead.
///
/// ```ignore
/// StatusMark::new(("step", i), Status::Done).size(px(20.))
/// ```
#[derive(IntoElement)]
pub struct StatusMark {
    id: ElementId,
    status: Status,
    size: Pixels,
    spinner: bool,
}

impl StatusMark {
    pub fn new(id: impl Into<ElementId>, status: Status) -> Self {
        Self {
            id: id.into(),
            status,
            size: px(18.),
            spinner: false,
        }
    }

    /// Side of the mark (18 px by default). It takes this room in every status.
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// Draws a [`LoadingIndicator`] while running (continuous, 15 fps) instead of leaving an
    /// empty slot. Off by default.
    pub fn spinner(mut self, spinner: bool) -> Self {
        self.spinner = spinner;
        self
    }
}

impl RenderOnce for StatusMark {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let status = self.status;
        let from = element_state(key(&self.id, "status"), window, |state, _| {
            let (state, from) = advance_status(state, status);
            (from, state)
        });
        let (bg, fg) = mark_colors(&s, status);
        let bg = animate_color(key(&self.id, "bg"), Spring::EFFECTS, bg, window, cx);
        let fg = animate_color(key(&self.id, "fg"), Spring::EFFECTS, fg, window, cx);
        let slot = div().relative().flex().flex_none().items_center().justify_center().size(self.size);
        if status == Status::Running {
            return slot
                .when(self.spinner, |d| d.child(LoadingIndicator::new().size(self.size)))
                .into_any_element();
        }

        // One settle per change of status (the generation), never on mount.
        let spring = Spring::SPATIAL_FAST;
        let p = replay(
            key(&self.id, "settle"),
            status as u64,
            spring.settle_time().as_secs_f32(),
            window,
            cx,
        )
        .map_or(1., |t| spring.step(t));
        let (old, new) = (mark_look(from), mark_look(status));
        let morph = Morph {
            from: old.shape,
            to: new.shape,
            t: p,
            rotation: -PI / 3. * (1. - p),
            toward: None,
        };
        let scale = (old.scale + (new.scale - old.scale) * p).max(0.);
        let size = f32::from(self.size);
        slot.child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let side = size * scale;
                    let half = point(px(side / 2.), px(side / 2.));
                    let bounds = Bounds::new(bounds.center() - half, gpui::size(px(side), px(side)));
                    morph.paint(bounds, bg, window);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .children(new.icon.map(|icon| {
            Icon::new(icon)
                .size(px((size * 0.62).round()))
                .color(fg)
                .scale(p.max(0.))
                .rotate(-PI / 4. * (1. - p))
        }))
        .into_any_element()
    }
}

/// An empty state: a quiet hero shape with an icon, a title, a line of text and actions
/// ("No chats yet" and a "New chat" button).
///
/// The shape holds still; [`EmptyState::breathe_on_hover`] morphs it toward another shape while
/// the pointer is over the block, with a spring that settles. It never animates continuously.
///
/// ```ignore
/// EmptyState::new("no-chats")
///     .shape(ShapeName::Cookie9)
///     .icon("chat")
///     .title("No chats yet")
///     .text("Start one to see it here.")
///     .action(Button::new("new-chat", "New chat").filled())
/// ```
#[derive(IntoElement)]
pub struct EmptyState {
    id: ElementId,
    shape: ShapeName,
    other: Option<ShapeName>,
    tone: Tone,
    size: Pixels,
    icon: Option<SharedString>,
    title: Option<SharedString>,
    text: Option<SharedString>,
    actions: Vec<AnyElement>,
}

impl EmptyState {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            shape: ShapeName::Cookie9,
            other: None,
            tone: Tone::Secondary,
            size: px(88.),
            icon: None,
            title: None,
            text: None,
            actions: Vec::new(),
        }
    }

    /// The hero shape (Cookie9 by default).
    pub fn shape(mut self, shape: ShapeName) -> Self {
        self.shape = shape;
        self
    }

    /// While the pointer is over the empty state, the shape morphs toward `other` (and back on
    /// leave) with the spatial spring.
    pub fn breathe_on_hover(mut self, other: ShapeName) -> Self {
        self.other = Some(other);
        self
    }

    /// Container tone of the shape (secondary by default).
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Side of the shape (88 px by default; the icon takes 40 % of it).
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Supporting text under the title (centered, up to 360 px wide).
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Adds an action under the text, usually a [`super::Button`]. Several sit in a row.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }
}

impl RenderOnce for EmptyState {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let (container, content) = self.tone.colors(&s);
        let hovered = self
            .other
            .map(|_| window.use_keyed_state(key(&self.id, "hovered"), cx, |_, _| false));
        let mut shape = Shape::new(self.shape).size(self.size).color(container);
        if let Some(icon) = self.icon {
            shape = shape.child(Icon::new(icon).size(self.size * 0.4).color(content));
        }
        if let (Some(other), Some(state)) = (self.other, &hovered) {
            shape = shape
                .breathe_on_hover(key(&self.id, "shape"), other)
                .hovered(*state.read(cx));
        }
        let el = div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.))
            .p(px(24.))
            .text_center()
            .font_family(font_family())
            .child(shape)
            .when_some(self.title, |d, title| {
                d.child(
                    div()
                        .mt(px(4.))
                        .type_emphasized(TITLE_MEDIUM)
                        .text_color(s.on_surface)
                        .child(title),
                )
            })
            .when_some(self.text, |d, text| {
                d.child(
                    div()
                        .max_w(px(360.))
                        .type_style(BODY_MEDIUM)
                        .text_color(s.on_surface_variant)
                        .child(text),
                )
            })
            .when(!self.actions.is_empty(), |d| {
                d.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_center()
                        .gap(px(8.))
                        .mt(px(4.))
                        .children(self.actions),
                )
            });
        match hovered {
            Some(state) => el
                .on_hover(move |on, _, cx| {
                    state.update(cx, |hovered, cx| {
                        *hovered = *on;
                        cx.notify();
                    })
                })
                .into_any_element(),
            None => el.into_any_element(),
        }
    }
}

/// Text with a band of light that sweeps across it ("Thinking…"), for a label that shows work
/// in progress.
///
/// It is continuous: it redraws the view that holds it at [`ShimmerText::fps`] (15 by default;
/// 4 with the window in the background), so give it a view of its own in a large window. With
/// reduced motion it is plain text and requests no frames.
#[derive(IntoElement)]
pub struct ShimmerText {
    id: ElementId,
    text: SharedString,
    fps: f32,
    period: f32,
    color: Option<Hsla>,
    highlight: Option<Hsla>,
}

impl ShimmerText {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            fps: 15.,
            period: 1.8,
            color: None,
            highlight: None,
        }
    }

    /// Frames per second of the sweep (15 by default, from 1 to 60).
    pub fn fps(mut self, fps: f32) -> Self {
        self.fps = fps.clamp(1., 60.);
        self
    }

    /// Seconds for the band to cross the text (1.8 by default).
    pub fn period(mut self, seconds: f32) -> Self {
        self.period = seconds.max(0.2);
        self
    }

    /// Color of the text (`on_surface_variant` by default).
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// Color at the center of the band (`on_surface` by default).
    pub fn highlight(mut self, color: Hsla) -> Self {
        self.highlight = Some(color);
        self
    }
}

/// Weight of the band (0 to 1) on each of `len` characters at `phase` (0 to 1) of the sweep.
/// The band enters before the first character and leaves after the last one.
fn shimmer_weights(len: usize, phase: f32) -> Vec<f32> {
    let band = (len as f32 * 0.18).max(3.);
    let center = -band + (len as f32 + 2. * band) * phase.rem_euclid(1.);
    (0..len)
        .map(|i| {
            let x = (1. - (i as f32 - center).abs() / band).clamp(0., 1.);
            x * x * (3. - 2. * x)
        })
        .collect()
}

/// Byte ranges of `text` that share a weight level, with that level (0 to `LEVELS`), skipping
/// the characters outside the band.
fn shimmer_runs(text: &str, weights: &[f32]) -> Vec<(Range<usize>, usize)> {
    const LEVELS: f32 = 8.;
    let mut runs: Vec<(Range<usize>, usize)> = Vec::new();
    for ((start, ch), weight) in text.char_indices().zip(weights) {
        let level = (weight * LEVELS).round() as usize;
        if level == 0 {
            continue;
        }
        let end = start + ch.len_utf8();
        match runs.last_mut() {
            Some((range, last)) if *last == level && range.end == start => range.end = end,
            _ => runs.push((start..end, level)),
        }
    }
    runs
}

impl RenderOnce for ShimmerText {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let base = self.color.unwrap_or(s.on_surface_variant);
        let light = self.highlight.unwrap_or(s.on_surface);
        let el = div().id(self.id).text_color(base);
        if MotionSettings::get(cx).reduced || self.text.is_empty() {
            return el.child(self.text);
        }
        let t = continuous_at(self.fps, window, cx);
        let weights = shimmer_weights(self.text.chars().count(), t / self.period);
        let highlights: Vec<(Range<usize>, HighlightStyle)> = shimmer_runs(&self.text, &weights)
            .into_iter()
            .map(|(range, level)| {
                let color = mix(light, base, level as f32 / 8.);
                (
                    range,
                    HighlightStyle {
                        color: Some(color),
                        ..Default::default()
                    },
                )
            })
            .collect();
        el.child(StyledText::new(self.text).with_highlights(highlights))
    }
}

/// Dots for the pages of a pager or carousel: the current one stretches into a pill in the
/// primary color, with the spatial spring; the others are small `outline_variant` dots.
///
/// With [`PageIndicator::on_select`] each dot is a button that asks for its page.
#[derive(IntoElement)]
pub struct PageIndicator {
    id: ElementId,
    count: usize,
    current: usize,
    dot: Pixels,
    on_select: Option<ChangeHandler<usize>>,
}

impl PageIndicator {
    /// `count` pages with `current` selected (clamped to the last page).
    pub fn new(id: impl Into<ElementId>, count: usize, current: usize) -> Self {
        Self {
            id: id.into(),
            count,
            current,
            dot: px(8.),
            on_select: None,
        }
    }

    /// Diameter of a dot (8 px by default); the current page is 2.5 times as wide.
    pub fn dot_size(mut self, size: Pixels) -> Self {
        self.dot = size;
        self
    }

    /// Receives the page of a clicked dot.
    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

/// Width of dot `index` when `current` is the selected page.
fn page_dot_width(index: usize, current: usize, dot: f32) -> f32 {
    if index == current { dot * 2.5 } else { dot }
}

impl RenderOnce for PageIndicator {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let mut row = div().flex().flex_none().items_center().gap(px(2.));
        if self.count == 0 {
            return row;
        }
        let current = self.current.min(self.count - 1);
        let dot = f32::from(self.dot);
        for i in 0..self.count {
            let id = ElementId::NamedInteger(format!("{}-dot", self.id).into(), i as u64);
            let width = animate_px(
                key(&id, "width"),
                Spring::SPATIAL,
                px(page_dot_width(i, current, dot)),
                window,
                cx,
            );
            let color = if i == current { s.primary } else { s.outline_variant };
            let color = animate_color(key(&id, "color"), Spring::EFFECTS, color, window, cx);
            let shown = div()
                .w(width.max(px(0.)))
                .h(self.dot)
                .rounded(self.dot / 2.)
                .bg(color);
            // A little room around each dot makes it easier to hit.
            let mut cell = div().id(id).flex().items_center().p(px(3.)).child(shown);
            if let Some(f) = self.on_select.clone() {
                cell = cell.cursor_pointer().on_click(move |_, w, cx| f(i, w, cx));
            }
            row = row.child(cell);
        }
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_change_morphs_from_the_old_status_once() {
        let (state, from) = advance_status(None, Status::Pending);
        assert_eq!(from, Status::Pending);
        let (state, from) = advance_status(Some(state), Status::Running);
        assert_eq!((state, from), ((Status::Running, Status::Pending), Status::Pending));
        let (state, from) = advance_status(Some(state), Status::Done);
        assert_eq!(from, Status::Running);
        // The same status again keeps the shape it came from (the spring finishes on its own).
        let (state2, from) = advance_status(Some(state), Status::Done);
        assert_eq!((state2, from), (state, Status::Running));
    }

    #[test]
    fn each_finished_status_has_its_own_shape_and_icon() {
        let looks = [Status::Done, Status::Failed, Status::Waiting].map(mark_look);
        for (i, a) in looks.iter().enumerate() {
            assert!(a.icon.is_some());
            for b in &looks[i + 1..] {
                assert_ne!(a.shape, b.shape);
            }
        }
        assert_eq!(mark_look(Status::Pending).icon, None);
        assert!(mark_look(Status::Pending).scale < mark_look(Status::Done).scale);
    }

    #[test]
    fn colors_follow_the_status_role() {
        let s = Scheme::baseline_light();
        assert_eq!(mark_colors(&s, Status::Done), (s.primary, s.on_primary));
        assert_eq!(mark_colors(&s, Status::Failed), (s.error, s.on_error));
        assert_eq!(mark_colors(&s, Status::Waiting).0, s.tertiary);
    }

    #[test]
    fn the_shimmer_band_crosses_the_whole_text() {
        let len = 20;
        // Before it enters and after it leaves, nothing is lit.
        assert!(shimmer_weights(len, 0.).iter().all(|w| *w == 0.));
        assert!(shimmer_weights(len, 0.9999).iter().all(|w| *w < 0.01));
        // Halfway, the middle character is fully lit and the ends are not.
        let mid = shimmer_weights(len, 0.5);
        assert_eq!(mid.len(), len);
        let peak = mid.iter().cloned().fold(0., f32::max);
        assert!(peak > 0.95);
        assert_eq!(mid[0], 0.);
        assert_eq!(mid[len - 1], 0.);
    }

    #[test]
    fn shimmer_runs_cover_whole_characters() {
        let text = "Pensándolo…";
        let weights = vec![1.; text.chars().count()];
        let runs = shimmer_runs(text, &weights);
        assert_eq!(runs, vec![(0..text.len(), 8)]);
        let none = shimmer_runs(text, &vec![0.; text.chars().count()]);
        assert!(none.is_empty());
        // Ranges always fall on character boundaries.
        let weights: Vec<f32> = (0..text.chars().count()).map(|i| i as f32 / 10.).collect();
        for (range, _) in shimmer_runs(text, &weights) {
            assert!(text.is_char_boundary(range.start) && text.is_char_boundary(range.end));
        }
    }

    #[test]
    fn only_the_current_page_is_wide() {
        assert_eq!(page_dot_width(2, 2, 8.), 20.);
        assert_eq!(page_dot_width(1, 2, 8.), 8.);
    }
}
