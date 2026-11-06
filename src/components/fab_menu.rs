//! FAB menu (M3 Expressive): a FAB that opens into a close button and a stack of labelled actions.
//!
//! The FAB turns from a rounded square into a circle and its plus into an x; the actions rise
//! from it one by one with the spatial spring and leave in the opposite order.

use std::{cell::Cell, f32::consts::FRAC_PI_4, rc::Rc, time::Instant};

use gpui::prelude::FluentBuilder as _;

use gpui::{
    App, Bounds, ClickEvent, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement,
    KeyDownEvent, MouseDownEvent, ParentElement, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, deferred, div, px,
};

use super::{ChangeHandler, ClickHandler, Fab, Icon, RoundedExt, all, child_id};
use crate::interaction::{interactive, keyboard_focus, state_layer, use_interaction};
use crate::motion::{MotionSettings, Spring, animate_color, animate_f32, key, request_frame};
use crate::theme::{Elevation, Scheme, Theme, font_family};

/// Side of the FAB and height of an action.
const SIZE: f32 = 56.;

/// Space between the actions and between them and the FAB.
const GAP: f32 = 16.;

/// Time between one action starting to appear and the next (seconds), and the same leaving.
const STAGGER_IN: f32 = 0.045;
const STAGGER_OUT: f32 = 0.03;

/// Color family of a [`FabMenu`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FabMenuColor {
    #[default]
    Primary,
    Secondary,
    Tertiary,
}

/// The colors of a family: the container of the closed FAB and the actions, and the solid color
/// of the close button.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Palette {
    container: Hsla,
    on_container: Hsla,
    solid: Hsla,
    on_solid: Hsla,
}

fn palette(s: &Scheme, color: FabMenuColor) -> Palette {
    match color {
        FabMenuColor::Primary => Palette {
            container: s.primary_container,
            on_container: s.on_primary_container,
            solid: s.primary,
            on_solid: s.on_primary,
        },
        FabMenuColor::Secondary => Palette {
            container: s.secondary_container,
            on_container: s.on_secondary_container,
            solid: s.secondary,
            on_solid: s.on_secondary,
        },
        FabMenuColor::Tertiary => Palette {
            container: s.tertiary_container,
            on_container: s.on_tertiary_container,
            solid: s.tertiary,
            on_solid: s.on_tertiary,
        },
    }
}

/// One action of a [`FabMenu`].
struct FabMenuItem {
    icon: SharedString,
    label: SharedString,
    on_click: ClickHandler,
}

/// The FAB menu: the button that opens it and, while open, the actions above it.
///
/// The menu is controlled: it shows [`open`](FabMenu::open) and asks you to change it through
/// [`on_toggle`](FabMenu::on_toggle) (the button) or [`on_dismiss`](FabMenu::on_dismiss).
/// It takes the room of a 56 px FAB only; the actions are drawn above it, over what is
/// there, so put it where they have room (a corner of the screen).
///
/// Escape closes it while it has the focus, which it takes when it opens. A click anywhere
/// else also calls `on_dismiss`; without one, both call `on_toggle(false)`. Choosing an action
/// runs its handler and then asks to close.
#[derive(IntoElement)]
pub struct FabMenu {
    id: ElementId,
    icon: SharedString,
    color: FabMenuColor,
    open: bool,
    align_start: bool,
    items: Vec<FabMenuItem>,
    on_toggle: Option<ChangeHandler<bool>>,
    on_dismiss: Option<ClickHandler>,
}

impl FabMenu {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            icon: "plus".into(),
            color: FabMenuColor::default(),
            open: false,
            align_start: false,
            items: Vec::new(),
            on_toggle: None,
            on_dismiss: None,
        }
    }

    /// The action at the top comes first; the last one is closest to the FAB.
    pub fn item(
        mut self,
        icon: impl Into<SharedString>,
        label: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.items.push(FabMenuItem {
            icon: icon.into(),
            label: label.into(),
            on_click: Rc::new(on_click),
        });
        self
    }

    /// Icon of the closed FAB, a plus by default. The open one is always an x.
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = icon.into();
        self
    }

    pub fn color(mut self, color: FabMenuColor) -> Self {
        self.color = color;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Lines the actions up with the left edge of the FAB (for a FAB at the left of the screen).
    pub fn align_start(mut self) -> Self {
        self.align_start = true;
        self
    }

    /// Receives the state the FAB asks for when it is pressed.
    pub fn on_toggle(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }

    /// Called for Escape and for a click outside the menu while it is open. Put your own
    /// outside-click catcher in front of the page if you need something else.
    pub fn on_dismiss(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }
}

/// When the menu last changed, to time the stagger.
struct Timing {
    open: bool,
    changed: Option<Instant>,
}

/// Seconds after the change at which action `k` (0 is the one next to the FAB) of `count` starts
/// moving: the nearest one leads when opening, the farthest one when closing.
fn start_delay(opening: bool, k: usize, count: usize) -> f32 {
    if opening {
        k as f32 * STAGGER_IN
    } else {
        (count - 1 - k) as f32 * STAGGER_OUT
    }
}

/// How far an action is still from its resting place, in px, at `progress` (0 hidden, 1 shown).
fn rise(progress: f32, k: usize) -> f32 {
    (1. - progress) * (k as f32 + 1.) * (SIZE + GAP)
}

fn item_key(id: &ElementId, index: usize) -> ElementId {
    ElementId::NamedChild(Box::new(id.clone()), format!("item-{index}").into())
}

impl RenderOnce for FabMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let pal = palette(&s, self.color);
        let open = self.open;
        let count = self.items.len();
        let motion = MotionSettings::get(cx);
        let handle = window
            .use_keyed_state(key(&self.id, "focus"), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();

        let timing = window.use_keyed_state(key(&self.id, "timing"), cx, |_, _| Timing {
            open,
            changed: None,
        });
        let now = Instant::now();
        let opened = timing.update(cx, |t, _| {
            let opened = open && !t.open;
            if t.open != open {
                t.open = open;
                t.changed = Some(now);
            }
            opened
        });
        if opened {
            // Escape reaches the menu through the focus.
            window.focus(&handle);
        }
        let since = timing.read(cx).changed.map_or(f32::MAX, |t| {
            now.duration_since(t).as_secs_f32() / motion.time_scale.max(0.01)
        });

        let turn = animate_f32(
            key(&self.id, "turn"),
            Spring::SPATIAL,
            if open { FRAC_PI_4 } else { 0. },
            window,
            cx,
        );
        let container = animate_color(
            key(&self.id, "fab-container"),
            Spring::EFFECTS,
            if open { pal.solid } else { pal.container },
            window,
            cx,
        );
        let content = animate_color(
            key(&self.id, "fab-content"),
            Spring::EFFECTS,
            if open { pal.on_solid } else { pal.on_container },
            window,
            cx,
        );
        let toggle = self.on_toggle.clone();
        let fab = Fab::new(child_id(&self.id, "fab"), self.icon)
            .menu_look(container, content, open, turn)
            .on_click(move |_, window, cx| {
                if let Some(f) = &toggle {
                    f(!open, window, cx);
                }
            });

        // Each action springs to shown or hidden once its turn comes; one that is hidden and
        // done is not drawn.
        let mut slots = Vec::with_capacity(count);
        let mut pending = false;
        let mut any_drawn = false;
        let dismiss_after = self.on_toggle.clone();
        for (index, item) in self.items.into_iter().enumerate() {
            let k = count - 1 - index;
            let delay = if motion.reduced {
                0.
            } else {
                start_delay(open, k, count)
            };
            let due = since >= delay;
            pending |= !due;
            let shown = if due { open } else { !open };
            let progress = animate_f32(
                key(&item_key(&self.id, index), "progress"),
                Spring::SPATIAL,
                if shown { 1. } else { 0. },
                window,
                cx,
            );
            let draw = shown || progress > 0.004;
            any_drawn |= draw;
            let slot = div()
                .flex_none()
                .h(px(SIZE))
                .flex()
                .flex_col()
                .justify_center();
            if !draw {
                slots.push(slot);
                continue;
            }
            let id = item_key(&self.id, index);
            let state = use_interaction(&id, window, cx);
            let radius = px(SIZE / 2.);
            let layer = state_layer(&id, &state, all(radius), pal.on_container, window, cx);
            let on_click = item.on_click;
            let close = dismiss_after.clone();
            let pill = div()
                .id(id.clone())
                .relative()
                .top(px(rise(progress, k)))
                .opacity((progress * 1.6).clamp(0., 1.))
                .flex()
                .flex_none()
                .items_center()
                .gap(px(8.))
                .h(px(SIZE))
                .pl(px(20.))
                .pr(px(24.))
                .corners(all(radius))
                .bg(pal.container)
                .text_color(pal.on_container)
                .shadow(Elevation::Level2.shadow(&s))
                .child(layer)
                .child(Icon::new(item.icon).size(px(24.)).color(pal.on_container))
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(FontWeight(500.))
                        .whitespace_nowrap()
                        .child(item.label),
                );
            let pill = interactive(pill, &state, shown, true);
            let pill = keyboard_focus(pill, &id, all(radius), false, window, cx).on_click(
                move |e, window, cx| {
                    if !shown {
                        return;
                    }
                    on_click(e, window, cx);
                    if let Some(f) = &close {
                        f(false, window, cx);
                    }
                },
            );
            slots.push(slot.child(pill));
        }
        if pending && !motion.reduced {
            request_frame(window);
        }

        let column_bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let menu = any_drawn.then(|| {
            let column = div()
                .absolute()
                .bottom(px(SIZE + GAP))
                .flex()
                .flex_col()
                .gap(px(GAP))
                .font_family(font_family())
                .when(self.align_start, |d| d.left_0().items_start())
                .when(!self.align_start, |d| d.right_0().items_end())
                .children(slots)
                .child({
                    let column_bounds = column_bounds.clone();
                    canvas(
                        move |bounds, _, _| column_bounds.set(bounds),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full()
                });
            deferred(column).with_priority(1)
        });

        // Escape and clicks elsewhere close it.
        let dismiss = {
            let on_dismiss = self.on_dismiss.clone();
            let on_toggle = self.on_toggle.clone();
            Rc::new(move |window: &mut Window, cx: &mut App| {
                if let Some(f) = &on_dismiss {
                    f(&ClickEvent::default(), window, cx);
                } else if let Some(f) = &on_toggle {
                    f(false, window, cx);
                }
            })
        };
        let outside = {
            let dismiss = dismiss.clone();
            canvas(
                |bounds, _, _| bounds,
                move |fab_bounds, _, window, _| {
                    if !open {
                        return;
                    }
                    let dismiss = dismiss.clone();
                    window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                        if phase.capture()
                            && !fab_bounds.contains(&e.position)
                            && !column_bounds.get().contains(&e.position)
                        {
                            dismiss(window, cx);
                        }
                    });
                },
            )
            .absolute()
            .size_full()
        };
        div()
            .id(self.id.clone())
            .track_focus(&handle)
            .relative()
            .flex_none()
            .size(px(SIZE))
            .on_key_down(move |e: &KeyDownEvent, window, cx| {
                if open && e.keystroke.key == "escape" {
                    cx.stop_propagation();
                    dismiss(window, cx);
                }
            })
            .child(outside)
            .child(fab)
            .children(menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_open_from_the_fab_up_and_close_from_the_top_down() {
        let opening: Vec<f32> = (0..4).map(|k| start_delay(true, k, 4)).collect();
        assert_eq!(opening[0], 0.);
        assert!(opening.windows(2).all(|w| w[1] > w[0]));
        // Closing starts with the farthest action (the largest k) and ends with the nearest.
        let closing: Vec<f32> = (0..4).map(|k| start_delay(false, k, 4)).collect();
        assert_eq!(closing[3], 0.);
        assert!(closing.windows(2).all(|w| w[1] < w[0]));
    }

    #[test]
    fn actions_start_at_the_fab_and_end_in_place() {
        for k in 0..4 {
            assert_eq!(rise(1., k), 0.);
            // Hidden, an action sits over the FAB: 72 px per place above it.
            assert_eq!(rise(0., k), (k as f32 + 1.) * (SIZE + GAP));
        }
        // Farther actions travel farther.
        assert!(rise(0.5, 2) > rise(0.5, 1));
    }

    #[test]
    fn each_family_has_a_container_and_a_solid_close_button() {
        for scheme in [Scheme::baseline_light(), Scheme::baseline_dark()] {
            for color in [
                FabMenuColor::Primary,
                FabMenuColor::Secondary,
                FabMenuColor::Tertiary,
            ] {
                let p = palette(&scheme, color);
                // The open FAB stands out from the actions around it.
                assert_ne!(p.container, p.solid);
                assert_ne!(p.on_container, p.on_solid);
            }
        }
        let s = Scheme::baseline_light();
        assert_eq!(
            palette(&s, FabMenuColor::Secondary).container,
            s.secondary_container
        );
        assert_eq!(palette(&s, FabMenuColor::Tertiary).solid, s.tertiary);
    }
}
