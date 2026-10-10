//! Drag to reorder: rows you can pick up with the pointer and drop elsewhere in their group,
//! with an insertion line and the other rows sliding into place.

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Bounds, ElementId, FontWeight, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, RenderOnce, SharedString, Styled, Window,
    canvas, div, px,
};

use super::{RoundedExt, all, shadow_pop};
use crate::motion::{MotionSettings, Spring, elapsed, key};
use crate::theme::{FONT_FAMILY, Theme};

/// A finished drag: the row at `from` now goes to index `to` (both count rows only, not
/// headers). Apply it with [`apply_reorder`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReorderEvent {
    pub from: usize,
    pub to: usize,
}

/// Moves `items[from]` to index `to`, as reported by a [`ReorderEvent`].
pub fn apply_reorder<T>(items: &mut Vec<T>, event: ReorderEvent) {
    if event.from < items.len() && event.from != event.to {
        let item = items.remove(event.from);
        items.insert(event.to.min(items.len()), item);
    }
}

/// Final index of the row at `from` dropped before (`after = false`) or after the row at `over`.
pub(crate) fn drop_index(from: usize, over: usize, after: bool) -> usize {
    let over_in_rest = if over > from { over - 1 } else { over };
    over_in_rest + after as usize
}

/// Pointer distance before a press becomes a drag (so clicks still work).
const DRAG_THRESHOLD: f32 = 5.;

struct Drag {
    from: usize,
    y0: Pixels,
    moved: bool,
    /// Row under the pointer and whether the drop goes after it.
    target: Option<(usize, bool)>,
}

struct Slide {
    offset: f32,
    start: Instant,
}

#[derive(Default)]
struct ReorderState {
    drag: Option<Drag>,
    /// Layout bounds of each row in the last frame (without the slide offset), by key.
    bounds: HashMap<SharedString, Bounds<Pixels>>,
    slides: HashMap<SharedString, Slide>,
}

enum Entry {
    Header(AnyElement),
    Row {
        key: SharedString,
        group: usize,
        content: AnyElement,
    },
}

type ReorderHandler = Rc<dyn Fn(ReorderEvent, &mut Window, &mut App)>;

/// A list whose rows can be reordered by dragging, within their group (favorites stay with
/// favorites). Each row needs a stable key; the order is yours: apply the [`ReorderEvent`] from
/// [`ReorderList::on_reorder`] to your data (see [`apply_reorder`]) and render the new order.
///
/// While dragging, the picked row lifts onto a tonal pill with a shadow and a 2 px line in the
/// primary color marks where it will land; on drop, the rows slide to their new places with the
/// spatial spring. A press that moves less than 5 px is still a click.
#[derive(IntoElement)]
pub struct ReorderList {
    id: ElementId,
    entries: Vec<Entry>,
    gap: Pixels,
    grab: Option<Pixels>,
    on_reorder: Option<ReorderHandler>,
}

impl ReorderList {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            entries: Vec::new(),
            gap: px(2.),
            grab: None,
            on_reorder: None,
        }
    }

    /// A draggable row. `key` must be stable across renders (a path, an id); rows only move
    /// within the same `group`.
    pub fn row(
        mut self,
        key: impl Into<SharedString>,
        group: usize,
        content: impl IntoElement,
    ) -> Self {
        self.entries.push(Entry::Row {
            key: key.into(),
            group,
            content: content.into_any_element(),
        });
        self
    }

    /// A static group title ("Favorites", "Projects"); it does not count as a row.
    pub fn header(mut self, title: impl Into<SharedString>) -> Self {
        self.entries.push(Entry::Header(
            ReorderHeader(title.into()).into_any_element(),
        ));
        self
    }

    /// Any static element between rows (it does not count as a row).
    pub fn item(mut self, element: impl IntoElement) -> Self {
        self.entries.push(Entry::Header(element.into_any_element()));
        self
    }

    /// Space between rows (2 px by default).
    pub fn gap(mut self, gap: Pixels) -> Self {
        self.gap = gap;
        self
    }

    /// Only the top `height` of each row starts a drag (a project header with its
    /// conversations below): the rest of the row keeps its own clicks and drags.
    pub fn grab_height(mut self, height: Pixels) -> Self {
        self.grab = Some(height);
        self
    }

    pub fn on_reorder(mut self, f: impl Fn(ReorderEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_reorder = Some(Rc::new(f));
        self
    }
}

#[derive(IntoElement)]
struct ReorderHeader(SharedString);

impl RenderOnce for ReorderHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .pt(px(8.))
            .pb(px(4.))
            .px(px(12.))
            .text_size(px(12.))
            .font_weight(FontWeight(700.))
            .text_color(Theme::of(cx).primary)
            .child(self.0)
    }
}

impl RenderOnce for ReorderList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let state =
            window.use_keyed_state(key(&self.id, "reorder"), cx, |_, _| ReorderState::default());
        let reduced = MotionSettings::get(cx).reduced;
        // Rows as (key, group), in order, for the pointer listeners.
        let rows: Rc<Vec<(SharedString, usize)>> = Rc::new(
            self.entries
                .iter()
                .filter_map(|e| match e {
                    Entry::Row { key, group, .. } => Some((key.clone(), *group)),
                    Entry::Header(_) => None,
                })
                .collect(),
        );
        let (dragging_row, target) = {
            let st = state.read(cx);
            match &st.drag {
                Some(d) if d.moved => (Some(d.from), d.target),
                _ => (None, None),
            }
        };

        // Slide offsets after a drop.
        let mut moving = false;
        let offsets: HashMap<SharedString, f32> = {
            let st = state.read(cx);
            st.slides
                .iter()
                .filter_map(|(k, slide)| {
                    let t = elapsed(slide.start, cx);
                    let spring = Spring::SPATIAL;
                    if reduced || t >= spring.settle_time().as_secs_f32() {
                        return None;
                    }
                    moving = true;
                    Some((k.clone(), slide.offset * (1. - spring.step(t))))
                })
                .collect()
        };
        if moving {
            window.request_animation_frame();
        } else if !state.read(cx).slides.is_empty() {
            state.update(cx, |st, _| st.slides.clear());
        }

        let mut list = div()
            .relative()
            .flex()
            .flex_col()
            .gap(self.gap)
            .font_family(FONT_FAMILY)
            .when(dragging_row.is_some(), |d| d.cursor_grabbing());
        let mut row_ix = 0;
        for entry in self.entries {
            match entry {
                Entry::Header(el) => list = list.child(el),
                Entry::Row {
                    key: k, content, ..
                } => {
                    let ix = row_ix;
                    row_ix += 1;
                    let offset = offsets.get(&k).copied().unwrap_or(0.);
                    let lifted = dragging_row == Some(ix);
                    let record = {
                        let state = state.clone();
                        let k = k.clone();
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, _, cx| {
                                let mut logical = bounds;
                                logical.origin.y -= px(offset);
                                state.update(cx, |st, _| {
                                    st.bounds.insert(k, logical);
                                });
                            },
                        )
                        .absolute()
                        .size_full()
                    };
                    let line = |top: bool| {
                        div()
                            .absolute()
                            .left(px(8.))
                            .right(px(8.))
                            .h(px(2.))
                            .rounded(px(1.))
                            .bg(s.primary)
                            .when(top, |d| d.top(px(-2.)))
                            .when(!top, |d| d.bottom(px(-2.)))
                    };
                    let row = div()
                        .relative()
                        .top(px(offset))
                        .when(lifted, |d| {
                            d.bg(s.secondary_container)
                                .corners(all(px(16.)))
                                .shadow(shadow_pop(&s))
                        })
                        .child(content)
                        .child(record)
                        .when(target == Some((ix, false)), |d| d.child(line(true)))
                        .when(target == Some((ix, true)), |d| d.child(line(false)));
                    list = list.child(row);
                }
            }
        }

        let on_reorder = self.on_reorder;
        let grab = self.grab;
        let listener = {
            let state = state.clone();
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let st = state.clone();
                    let rows_down = rows.clone();
                    window.on_mouse_event(move |e: &MouseDownEvent, phase, _, cx| {
                        if !phase.bubble() || e.button != MouseButton::Left {
                            return;
                        }
                        let hit = {
                            let s = st.read(cx);
                            rows_down.iter().position(|(k, _)| {
                                s.bounds.get(k).is_some_and(|b| {
                                    b.contains(&e.position)
                                        && grab.is_none_or(|h| e.position.y < b.origin.y + h)
                                })
                            })
                        };
                        if let Some(from) = hit {
                            st.update(cx, |s, _| {
                                s.drag = Some(Drag {
                                    from,
                                    y0: e.position.y,
                                    moved: false,
                                    target: None,
                                })
                            });
                        }
                    });
                    let st = state.clone();
                    let rows_move = rows.clone();
                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                        if !phase.bubble() {
                            return;
                        }
                        let Some((from, y0, moved)) =
                            st.read(cx).drag.as_ref().map(|d| (d.from, d.y0, d.moved))
                        else {
                            return;
                        };
                        if e.pressed_button != Some(MouseButton::Left) {
                            st.update(cx, |s, cx| {
                                s.drag = None;
                                cx.notify();
                            });
                            return;
                        }
                        if !moved && (e.position.y - y0).abs() < px(DRAG_THRESHOLD) {
                            return;
                        }
                        let group = rows_move[from].1;
                        let target = {
                            let s = st.read(cx);
                            rows_move.iter().enumerate().find_map(|(i, (k, g))| {
                                let b = s.bounds.get(k)?;
                                let inside = e.position.y >= b.top() && e.position.y < b.bottom();
                                (inside && i != from && *g == group)
                                    .then(|| (i, e.position.y > b.center().y))
                            })
                        };
                        st.update(cx, |s, cx| {
                            if let Some(d) = s.drag.as_mut() {
                                d.moved = true;
                                d.target = target;
                            }
                            cx.notify();
                        });
                    });
                    let st = state.clone();
                    let rows_up = rows.clone();
                    let on_reorder = on_reorder.clone();
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                        if !phase.bubble() {
                            return;
                        }
                        let Some(drag) = st.update(cx, |s, cx| {
                            cx.notify();
                            s.drag.take()
                        }) else {
                            return;
                        };
                        if !drag.moved {
                            return;
                        }
                        // The release ends a drag: it must not click the row under it.
                        cx.stop_propagation();
                        let Some((over, after)) = drag.target else {
                            return;
                        };
                        let to = drop_index(drag.from, over, after);
                        if to == drag.from {
                            return;
                        }
                        st.update(cx, |s, _| {
                            start_slides(s, &rows_up, drag.from, to);
                        });
                        if let Some(f) = &on_reorder {
                            f(
                                ReorderEvent {
                                    from: drag.from,
                                    to,
                                },
                                window,
                                cx,
                            );
                        }
                    });
                },
            )
            .absolute()
            .size_full()
        };
        list.child(listener)
    }
}

/// Predicts where every row lands after moving `from` to `to` and starts sliding each one
/// from its old place.
fn start_slides(st: &mut ReorderState, rows: &[(SharedString, usize)], from: usize, to: usize) {
    let tops: Vec<Option<Bounds<Pixels>>> = rows
        .iter()
        .map(|(k, _)| st.bounds.get(k).copied())
        .collect();
    let Some(Some(first)) = tops.first().copied() else {
        return;
    };
    let mut order: Vec<usize> = (0..rows.len()).collect();
    let moved = order.remove(from);
    order.insert(to.min(order.len()), moved);
    // Rows keep their heights and the spacing between consecutive rows (headers included)
    // stays with the slot, so the new top of slot `i` is the old top of slot `i` shifted by
    // the difference in heights of the rows above it.
    let mut y = f32::from(first.top());
    let now = Instant::now();
    for (slot, &row) in order.iter().enumerate() {
        let Some(old) = tops[row] else {
            return;
        };
        if slot > 0
            && let (Some(Some(prev_slot)), Some(Some(this_slot))) =
                (tops.get(slot - 1).copied(), tops.get(slot).copied())
        {
            // The gap that followed the previous slot (it includes any header in between).
            let gap = f32::from(this_slot.top()) - f32::from(prev_slot.bottom());
            y += gap;
        }
        let offset = f32::from(old.top()) - y;
        if offset.abs() > 0.5 {
            st.slides
                .insert(rows[row].0.clone(), Slide { offset, start: now });
        }
        y += f32::from(old.size.height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_index_accounts_for_the_removed_row() {
        // Drop row 0 after row 2: it ends at index 2.
        assert_eq!(drop_index(0, 2, true), 2);
        // Drop row 3 before row 1: index 1.
        assert_eq!(drop_index(3, 1, false), 1);
        // Drop row 1 before row 2: no change.
        assert_eq!(drop_index(1, 2, false), 1);
        let mut v = vec!["a", "b", "c", "d"];
        apply_reorder(
            &mut v,
            ReorderEvent {
                from: 0,
                to: drop_index(0, 2, true),
            },
        );
        assert_eq!(v, ["b", "c", "a", "d"]);
        apply_reorder(
            &mut v,
            ReorderEvent {
                from: 3,
                to: drop_index(3, 0, false),
            },
        );
        assert_eq!(v, ["d", "b", "c", "a"]);
    }

    #[test]
    fn slides_start_from_the_old_places() {
        let mut st = ReorderState::default();
        let rows: Vec<(SharedString, usize)> = ["a", "b", "c"]
            .iter()
            .map(|k| (SharedString::from(*k), 0))
            .collect();
        for (i, (k, _)) in rows.iter().enumerate() {
            let top = px(i as f32 * 32.);
            st.bounds.insert(
                k.clone(),
                Bounds::new(gpui::point(px(0.), top), gpui::size(px(100.), px(30.))),
            );
        }
        // "a" goes to the end: it slides up from 0 to 64 (offset -64); b and c move up 32.
        start_slides(&mut st, &rows, 0, 2);
        assert_eq!(st.slides["a"].offset, -64.);
        assert_eq!(st.slides["b"].offset, 32.);
        assert_eq!(st.slides["c"].offset, 32.);
    }
}
