use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use super::{ChangeHandler, Icon, RoundedExt, corners};
use crate::interaction::{interactive, keyboard_focus, use_interaction};
use crate::motion::{Spring, animate, animate_color, animate_f32, key};
use crate::theme::{font_family, Theme};

/// A group option: text, icon or both.
#[derive(Clone)]
pub struct Segment {
    pub label: Option<SharedString>,
    pub icon: Option<SharedString>,
}

impl Segment {
    pub fn label(label: impl Into<SharedString>) -> Self {
        Self {
            label: Some(label.into()),
            icon: None,
        }
    }

    pub fn icon(icon: impl Into<SharedString>) -> Self {
        Self {
            label: None,
            icon: Some(icon.into()),
        }
    }

    pub fn with_icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// Connected button group (M3 Expressive): the ends are round, the inner buttons
/// nearly square, and the selected one springs into a pill.
///
/// [`SegmentedButtons::multi`] lets several segments be selected at once; the ones without an
/// icon show a check when selected.
///
/// With [`SegmentedButtons::sliding`], the options sit on a single track and the
/// selected pill travels between them instead: its leading edge springs ahead
/// and the trailing one follows, so it stretches like a liquid on the way.
#[derive(IntoElement)]
pub struct SegmentedButtons {
    id: ElementId,
    segments: Vec<Segment>,
    selected: usize,
    /// Multiple selection: one flag per segment (`selected` is then unused).
    multi: Option<Vec<bool>>,
    on_change: Option<ChangeHandler<usize>>,
    on_change_many: Option<ChangeHandler<Vec<bool>>>,
    sliding: bool,
    dense: bool,
}

/// The x and width of each option, relative to the track.
type SlotSpans = Rc<[(f32, f32)]>;

/// Where each option of a sliding group was last frame (x and width, relative
/// to the track), and which way the pill last went.
#[derive(Default)]
struct Slots {
    spans: Cell<Option<SlotSpans>>,
    target: Cell<Option<(f32, f32)>>,
    forward: Cell<bool>,
}

/// Track padding around the options of a sliding group.
const TRACK_PAD: f32 = 3.;

impl SegmentedButtons {
    pub fn new(
        id: impl Into<ElementId>,
        segments: impl IntoIterator<Item = Segment>,
        selected: usize,
    ) -> Self {
        Self {
            id: id.into(),
            segments: segments.into_iter().collect(),
            selected,
            multi: None,
            on_change: None,
            on_change_many: None,
            sliding: false,
            dense: false,
        }
    }

    /// Multiple selection: `selected` has one flag per segment (missing ones count as off).
    /// Selected segments without an icon show a check. Not combined with [`Self::sliding`].
    pub fn multi(
        id: impl Into<ElementId>,
        segments: impl IntoIterator<Item = Segment>,
        selected: impl IntoIterator<Item = bool>,
    ) -> Self {
        let mut group = Self::new(id, segments, 0);
        let mut flags: Vec<bool> = selected.into_iter().collect();
        flags.resize(group.segments.len(), false);
        group.multi = Some(flags);
        group
    }

    /// Smaller height (24 px instead of 32 px).
    pub fn dense(mut self, dense: bool) -> Self {
        self.dense = dense;
        self
    }

    /// One track with a pill that slides to the selected option.
    pub fn sliding(mut self, sliding: bool) -> Self {
        self.sliding = sliding;
        self
    }

    pub fn on_change(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    /// Receives the new flags with the clicked segment toggled (for [`Self::multi`]).
    pub fn on_change_many(
        mut self,
        f: impl Fn(Vec<bool>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change_many = Some(Rc::new(f));
        self
    }
}

/// The flags with segment `index` flipped.
fn toggled(selected: &[bool], index: usize) -> Vec<bool> {
    let mut next = selected.to_vec();
    if let Some(flag) = next.get_mut(index) {
        *flag = !*flag;
    }
    next
}

impl RenderOnce for SegmentedButtons {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let count = self.segments.len();
        let mut row = div()
            .flex()
            .flex_shrink()
            .min_w_0()
            .max_w_full()
            .gap(px(2.))
            .font_family(font_family());
        let height = if self.dense { 24. } else { 32. };
        let slots = (self.sliding && self.multi.is_none()).then(|| {
            window
                .use_keyed_state(key(&self.id, "slots"), cx, |_, _| Rc::new(Slots::default()))
                .read(cx)
                .clone()
        });
        let pill = slots.as_ref().and_then(|slots| {
            let spans = slots.spans.take();
            slots.spans.set(spans.clone());
            let &(x, w) = spans?.get(self.selected)?;
            Some((x, x + w))
        });
        if let Some(slots) = &slots {
            row = row
                .relative()
                .p(px(TRACK_PAD))
                .rounded(px(height / 2. + TRACK_PAD))
                .bg(s.surface_container_highest);
            if let Some((left, right)) = pill {
                // The edge that leads springs fast; the other one trails.
                if let Some((was_left, _)) = slots.target.get().filter(|t| *t != (left, right)) {
                    slots.forward.set(left > was_left);
                }
                slots.target.set(Some((left, right)));
                let (lead, trail) = (Spring::SPATIAL_FAST, Spring::SPATIAL);
                let (left_spring, right_spring) = if slots.forward.get() { (trail, lead) } else { (lead, trail) };
                let [l] = animate(key(&self.id, "pill-left"), left_spring, [left], window, cx);
                let [r] = animate(key(&self.id, "pill-right"), right_spring, [right], window, cx);
                // Stretched, it thins a little: a liquid keeps its volume.
                let stretch = ((r - l) - (right - left)).abs();
                let inset = (stretch * 0.08).min(3.);
                row = row.child(
                    div()
                        .absolute()
                        .left(px(l))
                        .w(px((r - l).max(0.)))
                        .top(px(TRACK_PAD + inset))
                        .h(px(height - 2. * inset))
                        .rounded(px(height / 2.))
                        .bg(s.primary),
                );
            }
            let slots = slots.clone();
            let skip = usize::from(pill.is_some());
            row = row.on_children_prepainted(move |bounds, _, _| {
                // The pill, when drawn, is the first child; the first option
                // starts right after the track padding.
                let options = bounds.get(skip..).unwrap_or_default();
                let Some(first) = options.first() else {
                    return;
                };
                let origin = first.origin.x - px(TRACK_PAD);
                let spans: Rc<[(f32, f32)]> =
                    options.iter().map(|b| (f32::from(b.origin.x - origin), f32::from(b.size.width))).collect();
                slots.spans.set(Some(spans));
            });
        }
        for (i, segment) in self.segments.into_iter().enumerate() {
            let id = ElementId::NamedInteger(format!("{}-seg", self.id).into(), i as u64);
            let state = use_interaction(&id, window, cx);
            let pressed = state.read(cx).pressed;
            let on = match &self.multi {
                Some(flags) => flags.get(i).copied().unwrap_or(false),
                None => i == self.selected,
            };
            let (first, last) = (i == 0, i + 1 == count);
            let outer = height / 2.;
            let inner = if pressed { 4. } else { 6. };
            let start = if first { outer } else { inner };
            let end = if last { outer } else { inner };
            let target = if slots.is_some() || (on && !pressed) {
                [outer; 4]
            } else {
                [start, end, end, start]
            };
            let r = animate(key(&id, "radius"), Spring::SPATIAL_FAST, target, window, cx);
            let (bg, fg) = match (on, slots.is_some()) {
                // Sliding: the pill under the track paints the selected one.
                (true, true) if pill.is_some() => (s.primary.opacity(0.), s.on_primary),
                (false, true) => (s.surface_container_highest.opacity(0.), s.on_surface_variant),
                (true, _) => (s.primary, s.on_primary),
                (false, false) => (s.surface_container_highest, s.on_surface_variant),
            };
            let bg = animate_color(key(&id, "bg"), Spring::EFFECTS, bg, window, cx);
            let fg = animate_color(key(&id, "fg"), Spring::EFFECTS, fg, window, cx);

            let icon_size = if self.dense { 14. } else { 16. };
            let mut item = div()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(6.))
                // In a narrow container the segments shrink and clip their labels.
                .min_w_0()
                .overflow_hidden()
                .h(px(height))
                .px(px(match (segment.label.is_some(), self.dense) {
                    (true, false) => 14.,
                    (true, true) => 12.,
                    (false, _) => 10.,
                }))
                .corners(corners(r))
                .bg(bg)
                .text_color(fg)
                .text_size(px(if self.dense { 12. } else { 12.5 }))
                .font_weight(FontWeight(if on { 600. } else { 500. }));
            if self.multi.is_some() && segment.icon.is_none() {
                // The check grows in from nothing and takes its gap with it.
                let check = animate_f32(
                    key(&id, "check"),
                    Spring::SPATIAL_FAST,
                    if on { 1. } else { 0. },
                    window,
                    cx,
                )
                .max(0.);
                if check > 0.001 {
                    item = item.gap(px(0.)).child(
                        div()
                            .flex_none()
                            .overflow_hidden()
                            .w(px((icon_size + 6.) * check))
                            .child(Icon::new("check").size(px(icon_size)).color(fg)),
                    );
                }
            }
            if let Some(icon) = segment.icon {
                item = item.child(Icon::new(icon).size(px(icon_size)).color(fg));
            }
            if let Some(label) = segment.label {
                item = item.child(div().min_w_0().truncate().child(label));
            }
            // The wrapper keeps the focus ring from being clipped with the segment.
            let item = div().id(id.clone()).flex().min_w_0().child(item);
            let mut item = interactive(item, &state, true, false);
            item = keyboard_focus(item, &id, corners(r), false, window, cx);
            if let Some(flags) = &self.multi {
                if let Some(f) = self.on_change_many.clone() {
                    let next = toggled(flags, i);
                    item = item.on_click(move |_, w, cx| f(next.clone(), w, cx));
                }
            } else if let Some(f) = self.on_change.clone() {
                item = item.on_click(move |_, w, cx| f(i, w, cx));
            }
            row = row.child(item);
        }
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_flips_only_the_clicked_segment() {
        assert_eq!(toggled(&[true, false, true], 1), [true, true, true]);
        assert_eq!(toggled(&[true, false, true], 0), [false, false, true]);
    }

    #[test]
    fn toggling_out_of_range_changes_nothing() {
        assert_eq!(toggled(&[true, false], 5), [true, false]);
    }

    #[test]
    fn multi_pads_missing_flags_and_drops_extra_ones() {
        let segments = || ["a", "b", "c"].map(Segment::label);
        let short = SegmentedButtons::multi("g", segments(), [true]);
        assert_eq!(short.multi, Some(vec![true, false, false]));
        let long = SegmentedButtons::multi("g", segments(), [true, true, true, true]);
        assert_eq!(long.multi, Some(vec![true, true, true]));
    }
}
