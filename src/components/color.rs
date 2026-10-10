//! Appearance pickers: selectable preview cards, accent swatches and a minimal HSV picker.

use std::f32::consts::{FRAC_PI_4, TAU};
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Bounds, ClickEvent, ElementId, FontWeight, Hsla, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement,
    PathBuilder, Pixels, Point, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, canvas, div, hsla, linear_color_stop, linear_gradient, point, px,
};

use super::{ChangeHandler, ClickHandler, Icon, RoundedExt, all};
use crate::interaction::{interactive, rounded_rect, state_layer, use_interaction};
use crate::motion::{Spring, animate, animate_f32, key};
use crate::theme::{FONT_FAMILY, Theme, alpha};

/// Selectable card with a preview, a title and a hint (the style picker). Selected cards get
/// a 2 px primary border. Side by side in a row, wrap each in `div().flex_1().min_w_0()`: the
/// cards then share the width, truncate the title and wrap the hint.
#[derive(IntoElement)]
pub struct SelectCard {
    id: ElementId,
    preview: Option<AnyElement>,
    title: SharedString,
    hint: Option<SharedString>,
    selected: bool,
    on_click: Option<ClickHandler>,
}

impl SelectCard {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            preview: None,
            title: title.into(),
            hint: None,
            selected: false,
            on_click: None,
        }
    }

    /// Picture of the option (about 52 px tall).
    pub fn preview(mut self, preview: impl IntoElement) -> Self {
        self.preview = Some(preview.into_any_element());
        self
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for SelectCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let state = use_interaction(&self.id, window, cx);
        let (hovered, pressed) = {
            let st = state.read(cx);
            (st.hovered, st.pressed)
        };
        let radius = px(animate_f32(
            key(&self.id, "radius"),
            Spring::SPATIAL_FAST,
            if pressed { 12. } else { 18. },
            window,
            cx,
        ));
        let layer = state_layer(&self.id, &state, all(radius), s.on_surface, window, cx);
        let mut card = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_col()
            .gap(px(3.))
            .p(px(8.))
            // Cards share a row: let them shrink below their content and clip what is left.
            .min_w_0()
            .overflow_hidden()
            .corners(all(radius))
            .when(hovered && !self.selected, |d| {
                d.bg(s.surface_container_high)
            })
            .border(px(if self.selected { 2. } else { 1. }))
            .border_color(if self.selected {
                s.primary
            } else {
                s.outline_variant
            })
            .font_family(FONT_FAMILY)
            .text_color(s.on_surface)
            .child(layer);
        if let Some(preview) = self.preview {
            card = card.child(div().mb(px(4.)).min_w_0().overflow_hidden().child(preview));
        }
        card = card.child(
            div()
                .min_w_0()
                .truncate()
                .text_size(px(12.5))
                .font_weight(FontWeight(600.))
                .child(self.title),
        );
        if let Some(hint) = self.hint {
            card = card.child(
                div()
                    .min_w_0()
                    .text_size(px(11.5))
                    .text_color(s.on_surface_variant)
                    .child(hint),
            );
        }
        let mut card = interactive(card, &state, true, true);
        if let Some(f) = self.on_click {
            card = card.on_click(move |e, w, cx| f(e, w, cx));
        }
        card
    }
}

/// Row of 30 px accent swatches. The chosen one turns into a rounded diamond with a double
/// ring; an optional last swatch ("Choose another color") shows a color wheel and reports a
/// click so you can open an [`HsvPicker`].
#[derive(IntoElement)]
pub struct ColorSwatches {
    id: ElementId,
    presets: Vec<Hsla>,
    current: Option<Hsla>,
    custom: Option<ClickHandler>,
    on_change: Option<ChangeHandler<Hsla>>,
}

impl ColorSwatches {
    pub fn new(id: impl Into<ElementId>, presets: impl IntoIterator<Item = Hsla>) -> Self {
        Self {
            id: id.into(),
            presets: presets.into_iter().collect(),
            current: None,
            custom: None,
            on_change: None,
        }
    }

    /// The current color. When it is not one of the presets, the custom swatch shows it selected.
    pub fn current(mut self, color: Hsla) -> Self {
        self.current = Some(color);
        self
    }

    /// Adds the custom swatch; `f` runs when it is clicked.
    pub fn custom(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.custom = Some(Rc::new(f));
        self
    }

    pub fn on_change(mut self, f: impl Fn(Hsla, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

fn same_color(a: Hsla, b: Hsla) -> bool {
    let (a, b) = (gpui::Rgba::from(a), gpui::Rgba::from(b));
    (a.r - b.r).abs() < 0.004 && (a.g - b.g).abs() < 0.004 && (a.b - b.b).abs() < 0.004
}

/// Paints a rounded square of `size` around `center`, turned by `angle`.
fn paint_turned_square(
    center: Point<Pixels>,
    size: f32,
    radius: f32,
    angle: f32,
    color: Hsla,
    window: &mut Window,
) {
    if size <= 0. {
        return;
    }
    let half = px(size / 2.);
    let bounds = Bounds::from_corners(
        point(center.x - half, center.y - half),
        point(center.x + half, center.y + half),
    );
    let (sin, cos) = angle.sin_cos();
    let points: Vec<Point<Pixels>> = rounded_rect(bounds, all(px(radius.min(size / 2.))))
        .into_iter()
        .map(|p| {
            let (x, y) = (f32::from(p.x - center.x), f32::from(p.y - center.y));
            point(
                center.x + px(x * cos - y * sin),
                center.y + px(x * sin + y * cos),
            )
        })
        .collect();
    let mut builder = PathBuilder::fill();
    builder.add_polygon(&points, true);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// Paints a hue wheel (twelve slices) inside a circle.
fn paint_wheel(center: Point<Pixels>, radius: f32, window: &mut Window) {
    const SLICES: usize = 12;
    for i in 0..SLICES {
        let (a0, a1) = (
            i as f32 / SLICES as f32 * TAU,
            (i + 1) as f32 / SLICES as f32 * TAU,
        );
        let mut points = vec![center];
        for step in 0..=4 {
            let a = a0 + (a1 - a0) * step as f32 / 4. - TAU / 4.;
            points.push(point(
                center.x + px(radius * a.cos()),
                center.y + px(radius * a.sin()),
            ));
        }
        let mut builder = PathBuilder::fill();
        builder.add_polygon(&points, true);
        if let Ok(path) = builder.build() {
            window.paint_path(path, hsla(i as f32 / SLICES as f32, 0.85, 0.55, 1.));
        }
    }
}

#[derive(IntoElement)]
struct Swatch {
    id: ElementId,
    color: Option<Hsla>,
    selected: bool,
    on_click: Option<ClickHandler>,
}

impl RenderOnce for Swatch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let state = use_interaction(&self.id, window, cx);
        let (hovered, pressed) = {
            let st = state.read(cx);
            (st.hovered, st.pressed)
        };
        let [selected, scale] = animate(
            key(&self.id, "shape"),
            Spring::SPATIAL_FAST,
            [
                if self.selected { 1. } else { 0. },
                if pressed {
                    0.85
                } else if hovered {
                    1.08
                } else {
                    1.
                },
            ],
            window,
            cx,
        );
        let raised = s.surface_container;
        let color = self.color;
        let el = div()
            .id(self.id.clone())
            .flex_none()
            .size(px(30.))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let center = bounds.center();
                        let k = selected.clamp(0., 1.2);
                        let size = 30. * scale * (1. - 0.08 * k);
                        let radius = 15. - 5. * k;
                        let angle = FRAC_PI_4 * k;
                        let tint = color.unwrap_or(hsla(0., 0., 0.5, 1.));
                        if k > 0.01 {
                            paint_turned_square(
                                center,
                                size + 8.,
                                radius + 4.,
                                angle,
                                alpha(tint, k.min(1.)),
                                window,
                            );
                            paint_turned_square(
                                center,
                                size + 4.,
                                radius + 2.,
                                angle,
                                raised,
                                window,
                            );
                        }
                        match color {
                            Some(color) => {
                                paint_turned_square(center, size, radius, angle, color, window)
                            }
                            None => paint_wheel(center, size / 2., window),
                        }
                    },
                )
                .size_full(),
            )
            .when(self.color.is_none(), |d| {
                d.relative().child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new("plus").size(px(13.)).color(gpui::white())),
                )
            });
        let mut el = interactive(el, &state, true, false);
        if let Some(f) = self.on_click {
            el = el.on_click(move |e, w, cx| f(e, w, cx));
        }
        el
    }
}

impl RenderOnce for ColorSwatches {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let current = self.current;
        let is_preset = current.is_some_and(|c| self.presets.iter().any(|p| same_color(*p, c)));
        let mut row = div().flex().flex_wrap().gap(px(8.)).p(px(4.));
        for (i, color) in self.presets.into_iter().enumerate() {
            let on_click = self.on_change.clone().map(|f| -> ClickHandler {
                Rc::new(move |_: &ClickEvent, w: &mut Window, cx: &mut App| f(color, w, cx))
            });
            row = row.child(Swatch {
                id: ElementId::NamedInteger(format!("{}-swatch", self.id).into(), i as u64),
                color: Some(color),
                selected: current.is_some_and(|c| same_color(c, color)),
                on_click,
            });
        }
        if let Some(custom) = self.custom {
            row = row.child(Swatch {
                id: super::child_id(&self.id, "custom"),
                color: None,
                selected: current.is_some() && !is_preset,
                on_click: Some(custom),
            });
        }
        row
    }
}

/// Minimal HSV color picker: a saturation/value square and a hue bar. Drag on either.
#[derive(IntoElement)]
pub struct HsvPicker {
    id: ElementId,
    color: Hsla,
    width: Pixels,
    on_change: Option<ChangeHandler<Hsla>>,
}

impl HsvPicker {
    pub fn new(id: impl Into<ElementId>, color: Hsla) -> Self {
        Self {
            id: id.into(),
            color,
            width: px(220.),
            on_change: None,
        }
    }

    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    pub fn on_change(mut self, f: impl Fn(Hsla, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

/// HSL (GPUI) → HSV, all in 0..1.
fn to_hsv(c: Hsla) -> (f32, f32, f32) {
    let v = c.l + c.s * c.l.min(1. - c.l);
    let s = if v <= 0. { 0. } else { 2. * (1. - c.l / v) };
    (c.h, s, v)
}

/// HSV → HSL (GPUI), all in 0..1.
pub fn hsv(h: f32, s: f32, v: f32) -> Hsla {
    let l = v * (1. - s / 2.);
    let sl = if l <= 0. || l >= 1. {
        0.
    } else {
        (v - l) / l.min(1. - l)
    };
    hsla(h.rem_euclid(1.), sl.clamp(0., 1.), l.clamp(0., 1.), 1.)
}

/// Which part of the picker a drag started on.
#[derive(Clone, Copy, PartialEq)]
enum HsvDrag {
    None,
    Square,
    Hue,
}

impl RenderOnce for HsvPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        // The hue is kept here so it survives at zero saturation or value.
        let hue_state = window.use_keyed_state(key(&self.id, "hue"), cx, |_, _| None::<f32>);
        let drag = window.use_keyed_state(key(&self.id, "drag"), cx, |_, _| HsvDrag::None);
        let (h, sat, val) = to_hsv(self.color);
        let h = if sat > 0.01 && val > 0.01 {
            h
        } else {
            hue_state.read(cx).unwrap_or(h)
        };
        let width = f32::from(self.width);
        let square_h = width * 0.62;
        let pure = hsv(h, 1., 1.);

        let square = div()
            .relative()
            .w(self.width)
            .h(px(square_h))
            .rounded(px(12.))
            .overflow_hidden()
            .bg(pure)
            .child(div().absolute().size_full().bg(linear_gradient(
                90.,
                linear_color_stop(gpui::white(), 0.),
                linear_color_stop(alpha(gpui::white(), 0.), 1.),
            )))
            .child(div().absolute().size_full().bg(linear_gradient(
                180.,
                linear_color_stop(alpha(gpui::black(), 0.), 0.),
                linear_color_stop(gpui::black(), 1.),
            )))
            .child(
                div()
                    .absolute()
                    .left(px(sat * width - 7.))
                    .top(px((1. - val) * square_h - 7.))
                    .size(px(14.))
                    .rounded(px(7.))
                    .border_2()
                    .border_color(gpui::white())
                    .bg(self.color),
            );
        let mut hue_bar = div()
            .relative()
            .flex()
            .w(self.width)
            .h(px(14.))
            .rounded(px(7.))
            .overflow_hidden();
        for i in 0..6 {
            hue_bar = hue_bar.child(div().flex_1().h_full().bg(linear_gradient(
                90.,
                linear_color_stop(hsv(i as f32 / 6., 1., 1.), 0.),
                linear_color_stop(hsv((i + 1) as f32 / 6., 1., 1.), 1.),
            )));
        }
        let hue_bar = hue_bar.child(
            div()
                .absolute()
                .left(px(h * width - 3.))
                .top_0()
                .w(px(6.))
                .h_full()
                .rounded(px(3.))
                .border_2()
                .border_color(gpui::white()),
        );

        let mut picker = div()
            .relative()
            .flex()
            .flex_col()
            .gap(px(10.))
            .w(self.width)
            .child(square)
            .child(hue_bar);
        if let Some(f) = self.on_change {
            picker = picker.child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let square = Bounds {
                            origin: bounds.origin,
                            size: gpui::size(px(width), px(square_h)),
                        };
                        let hue = Bounds {
                            origin: point(bounds.origin.x, bounds.origin.y + px(square_h + 10.)),
                            size: gpui::size(px(width), px(14.)),
                        };
                        let pick =
                            move |part: HsvDrag, p: Point<Pixels>, hue_now: f32| -> (f32, Hsla) {
                                let x = (f32::from(p.x - bounds.origin.x) / width).clamp(0., 1.);
                                match part {
                                    HsvDrag::Hue => (x, hsv(x, sat.max(0.01), val.max(0.01))),
                                    _ => {
                                        let y = (f32::from(p.y - square.origin.y) / square_h)
                                            .clamp(0., 1.);
                                        (hue_now, hsv(hue_now, x, 1. - y))
                                    }
                                }
                            };
                        let (down, moving) = (f.clone(), f.clone());
                        let (drag_down, drag_move, drag_up) =
                            (drag.clone(), drag.clone(), drag.clone());
                        let (hue_down, hue_move) = (hue_state.clone(), hue_state.clone());
                        window.on_mouse_event(move |e: &MouseDownEvent, phase, w, cx| {
                            if !phase.bubble() || e.button != MouseButton::Left {
                                return;
                            }
                            let part = if square.contains(&e.position) {
                                HsvDrag::Square
                            } else if hue.contains(&e.position) {
                                HsvDrag::Hue
                            } else {
                                return;
                            };
                            drag_down.update(cx, |d, _| *d = part);
                            let (new_hue, color) = pick(part, e.position, h);
                            hue_down.update(cx, |stored, _| *stored = Some(new_hue));
                            down(color, w, cx);
                        });
                        window.on_mouse_event(move |e: &MouseMoveEvent, phase, w, cx| {
                            let part = *drag_move.read(cx);
                            if !phase.bubble()
                                || part == HsvDrag::None
                                || e.pressed_button.is_none()
                            {
                                return;
                            }
                            let hue_now = hue_move.read(cx).unwrap_or(h);
                            let (new_hue, color) = pick(part, e.position, hue_now);
                            hue_move.update(cx, |stored, _| *stored = Some(new_hue));
                            moving(color, w, cx);
                        });
                        window.on_mouse_event(move |_: &MouseUpEvent, _, _, cx| {
                            drag_up.update(cx, |d, _| *d = HsvDrag::None);
                        });
                    },
                )
                .absolute()
                .size_full(),
            );
        }
        picker.text_color(s.on_surface)
    }
}
