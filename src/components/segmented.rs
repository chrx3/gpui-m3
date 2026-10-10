use std::rc::Rc;

use gpui::{
    App, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use super::{ChangeHandler, Icon, RoundedExt, corners};
use crate::interaction::{interactive, use_interaction};
use crate::motion::{Spring, animate, animate_color, key};
use crate::theme::{FONT_FAMILY, Theme};

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
#[derive(IntoElement)]
pub struct SegmentedButtons {
    id: ElementId,
    segments: Vec<Segment>,
    selected: usize,
    on_change: Option<ChangeHandler<usize>>,
}

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
            on_change: None,
        }
    }

    pub fn on_change(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
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
            .font_family(FONT_FAMILY);
        for (i, segment) in self.segments.into_iter().enumerate() {
            let id = ElementId::NamedInteger(format!("{}-seg", self.id).into(), i as u64);
            let state = use_interaction(&id, window, cx);
            let pressed = state.read(cx).pressed;
            let on = i == self.selected;
            let (first, last) = (i == 0, i + 1 == count);
            let outer = 16.;
            let inner = if pressed { 4. } else { 6. };
            let start = if first { outer } else { inner };
            let end = if last { outer } else { inner };
            let target = if on && !pressed {
                [outer; 4]
            } else {
                [start, end, end, start]
            };
            let r = animate(key(&id, "radius"), Spring::SPATIAL_FAST, target, window, cx);
            let (bg, fg) = if on {
                (s.primary, s.on_primary)
            } else {
                (s.surface_container_highest, s.on_surface_variant)
            };
            let bg = animate_color(key(&id, "bg"), Spring::EFFECTS, bg, window, cx);
            let fg = animate_color(key(&id, "fg"), Spring::EFFECTS, fg, window, cx);

            let mut item = div()
                .id(id.clone())
                .flex()
                .items_center()
                .justify_center()
                .gap(px(6.))
                // In a narrow container the segments shrink and clip their labels.
                .min_w_0()
                .overflow_hidden()
                .h(px(32.))
                .px(px(if segment.label.is_some() { 14. } else { 10. }))
                .corners(corners(r))
                .bg(bg)
                .text_color(fg)
                .text_size(px(12.5))
                .font_weight(FontWeight(if on { 600. } else { 500. }));
            if let Some(icon) = segment.icon {
                item = item.child(Icon::new(icon).size(px(16.)).color(fg));
            }
            if let Some(label) = segment.label {
                item = item.child(div().min_w_0().truncate().child(label));
            }
            let mut item = interactive(item, &state, true, false);
            if let Some(f) = self.on_change.clone() {
                item = item.on_click(move |_, w, cx| f(i, w, cx));
            }
            row = row.child(item);
        }
        row
    }
}
