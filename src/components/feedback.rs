//! Inline feedback: labeled dividers in a conversation and dismissible banners.

use std::f32::consts::TAU;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, Hsla, IntoElement, ParentElement, Pixels,
    RenderOnce, SharedString, Styled, Window, canvas, div, fill, point, px,
};

use super::{ClickHandler, Icon, IconButton, Shape, paint_polyline};
use crate::motion::{Spring, continuous_at, entrance, key};
use crate::shapes::{ShapeName, Tone};
use crate::theme::{FONT_FAMILY, Theme, alpha};

/// Look of a [`DividerLabel`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DividerStyle {
    /// Thin lines in the outline variant color around faint text («Context compacted»).
    #[default]
    Plain,
    /// Wavy lines in the primary color around a tonal pill («Switched to Opus»).
    Pill,
}

/// A horizontal separator with a label in the middle, for events inside a conversation: a
/// compaction, a model switch. Its lines grow out from the label when it appears.
#[derive(IntoElement)]
pub struct DividerLabel {
    id: ElementId,
    label: SharedString,
    icon: Option<SharedString>,
    style: DividerStyle,
    pending: bool,
}

impl DividerLabel {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            style: DividerStyle::Plain,
            pending: false,
        }
    }

    /// Wavy lines and a tonal pill (the reference design's model mark).
    pub fn pill(mut self) -> Self {
        self.style = DividerStyle::Pill;
        self
    }

    pub fn style(mut self, style: DividerStyle) -> Self {
        self.style = style;
        self
    }

    /// Icon before the label (12 px).
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Not applied yet (a model chosen for the next message): an outlined pill and, with
    /// [`DividerLabel::pill`], waves that flow.
    pub fn pending(mut self, pending: bool) -> Self {
        self.pending = pending;
        self
    }
}

/// Which side of the label a divider line is on (it grows away from the label).
#[derive(Clone, Copy, PartialEq)]
enum Side {
    Left,
    Right,
}

fn divider_line(
    side: Side,
    progress: f32,
    wave: Option<f32>,
    color: Hsla,
    dashed: bool,
) -> impl IntoElement {
    let height = if wave.is_some() { 8. } else { 1. };
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let x0 = f32::from(bounds.origin.x);
            let w = f32::from(bounds.size.width);
            let len = (w * progress.clamp(0., 1.2)).min(w);
            if len < 0.5 {
                return;
            }
            let (from, to) = match side {
                Side::Left => (x0 + w - len, x0 + w),
                Side::Right => (x0, x0 + len),
            };
            let cy = f32::from(bounds.origin.y) + f32::from(bounds.size.height) / 2.;
            match wave {
                Some(phase) => {
                    // "M0 4 Q5 0.5 10 4 T20 4": 20 px wavelength, 1.75 px amplitude, 1.6 px stroke.
                    let steps = (len / 1.5).ceil().max(2.) as usize;
                    let points: Vec<_> = (0..=steps)
                        .map(|i| {
                            let x = from + (to - from) * i as f32 / steps as f32;
                            let y = cy - 1.75 * (((x - x0) - phase) / 20. * TAU).sin();
                            point(px(x), px(y))
                        })
                        .collect();
                    paint_polyline(&points, 1.6, color, window);
                }
                None if dashed => {
                    let mut x = from;
                    while x < to {
                        let end = (x + 4.).min(to);
                        window.paint_quad(fill(
                            gpui::Bounds::from_corners(
                                point(px(x), px(cy - 0.5)),
                                point(px(end), px(cy + 0.5)),
                            ),
                            color,
                        ));
                        x += 8.;
                    }
                }
                None => window.paint_quad(fill(
                    gpui::Bounds::from_corners(
                        point(px(from), px(cy - 0.5)),
                        point(px(to), px(cy + 0.5)),
                    ),
                    color,
                )),
            }
        },
    )
    .flex_1()
    .min_w(px(12.))
    .h(px(height))
}

impl RenderOnce for DividerLabel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        // Lines: scaleX(0) → 1 from the label with the slow spatial spring (m3-line).
        let lines = entrance(key(&self.id, "lines"), Spring::SPATIAL_SLOW, 0., window, cx);
        let pill = self.style == DividerStyle::Pill;
        let wave = (pill).then(|| {
            if self.pending {
                // model-wave: the mask moves one wavelength every 1.4 s.
                (continuous_at(30., window, cx) / 1.4).fract() * 20.
            } else {
                0.
            }
        });
        let line_color = match (pill, self.pending) {
            (true, false) => alpha(s.primary, 0.5),
            (true, true) => alpha(s.outline, 0.5),
            (false, _) => s.outline_variant,
        };
        let dashed = !pill && self.pending;

        let icon_color = |c: Hsla| {
            self.icon
                .clone()
                .map(|i| Icon::new(i).size(px(12.)).color(c))
        };
        let label = if pill {
            // m3-chip-in: fades in and its corners open from 6 px to a full pill.
            let p = entrance(key(&self.id, "pill"), Spring::SPATIAL_FAST, 0., window, cx);
            let radius = 6. + (12. - 6.) * p.clamp(0., 1.2);
            let (bg, fg) = if self.pending {
                (alpha(s.primary_container, 0.), s.on_surface_variant)
            } else {
                (s.primary_container, s.on_primary_container)
            };
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(6.))
                .pl(px(8.))
                .pr(px(12.))
                .py(px(4.))
                .rounded(px(radius))
                .bg(bg)
                .when(self.pending, |d| d.border_1().border_color(s.outline))
                .text_color(fg)
                .text_size(px(12.))
                .font_weight(FontWeight(700.))
                .whitespace_nowrap()
                .opacity(p.clamp(0., 1.))
                .children(icon_color(fg))
                .child(self.label)
        } else {
            let fg = if self.pending {
                s.on_surface_variant
            } else {
                s.outline
            };
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(6.))
                .text_color(fg)
                .text_size(px(11.5))
                .whitespace_nowrap()
                .children(icon_color(fg))
                .child(self.label)
        };
        div()
            .flex()
            .items_center()
            .w_full()
            .gap(px(if pill { 8. } else { 10. }))
            .my(px(if pill { 10. } else { 4. }))
            .font_family(FONT_FAMILY)
            .child(divider_line(Side::Left, lines, wave, line_color, dashed))
            .child(label)
            .child(divider_line(Side::Right, lines, wave, line_color, dashed))
    }
}

/// Look of a [`Banner`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerStyle {
    /// Tonal container with 24 px corners and a shape mark (the "Artifact published" offer).
    Tonal(Tone),
    /// Quiet notice on `surface_container_high` (a long conversation).
    Subtle,
    /// Notice tinted with the warning color (the context is almost full).
    Warning,
}

/// A dismissible card above the composer: a mark, a title and text, actions and a close button.
/// Use it for offers ("open the artifact?") and notices ("this conversation is long").
///
/// It enters with a spring (tonal: rises 18 px while its corners close from 56 to 24 px; notices:
/// rise 18 px). Wrap it in [`super::Presence`] to animate its exit.
#[derive(IntoElement)]
pub struct Banner {
    id: ElementId,
    style: BannerStyle,
    icon: Option<SharedString>,
    shape: Option<ShapeName>,
    title: Option<SharedString>,
    text: Option<SharedString>,
    actions: Vec<AnyElement>,
    dismiss_label: SharedString,
    on_dismiss: Option<ClickHandler>,
}

impl Banner {
    /// A tonal banner in the tertiary container.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: BannerStyle::Tonal(Tone::Tertiary),
            icon: None,
            shape: None,
            title: None,
            text: None,
            actions: Vec::new(),
            dismiss_label: "Not now".into(),
            on_dismiss: None,
        }
    }

    pub fn style(mut self, style: BannerStyle) -> Self {
        self.style = style;
        self
    }

    pub fn tonal(self, tone: Tone) -> Self {
        self.style(BannerStyle::Tonal(tone))
    }

    pub fn subtle(self) -> Self {
        self.style(BannerStyle::Subtle)
    }

    pub fn warning(self) -> Self {
        self.style(BannerStyle::Warning)
    }

    /// Leading icon. In a tonal banner it sits inside the shape mark and pops in.
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Shape of a tonal banner's mark (Cookie9 by default).
    pub fn shape(mut self, shape: ShapeName) -> Self {
        self.shape = Some(shape);
        self
    }

    /// Bold first line, truncated.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Body text (under the title, or alone in a notice).
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// An action at the end (a [`super::Button`] or [`super::Chip`]).
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// Shows a close button that calls `f`.
    pub fn on_dismiss(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }

    /// Tooltip of the close button ("Not now" by default).
    pub fn dismiss_label(mut self, label: impl Into<SharedString>) -> Self {
        self.dismiss_label = label.into();
        self
    }
}

impl RenderOnce for Banner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let tonal = matches!(self.style, BannerStyle::Tonal(_));
        let (bg, fg, icon_color) = match self.style {
            BannerStyle::Tonal(tone) => {
                let (bg, fg) = tone.colors(&s);
                (bg, fg, fg)
            }
            BannerStyle::Subtle => (
                s.surface_container_high,
                s.on_surface_variant,
                s.on_surface_variant,
            ),
            BannerStyle::Warning => (alpha(s.warning, 0.14), s.on_surface, s.warning),
        };
        let p = entrance(key(&self.id, "enter"), Spring::SPATIAL, 0., window, cx);
        let (radius, pad): (f32, [f32; 4]) = if tonal {
            // m3-dialog-in: corners close from 56 px.
            (24. + (56. - 24.) * (1. - p).max(0.), [10., 10., 10., 12.])
        } else {
            (14., [6., 6., 6., 12.])
        };

        let mark = self.icon.clone().map(|icon| {
            if let BannerStyle::Tonal(tone) = self.style {
                // m3-icon-pop, 80 ms after the card: grows from 20 % while turning from -60°.
                let pop = entrance(
                    key(&self.id, "mark"),
                    Spring::SPATIAL_FAST,
                    0.08,
                    window,
                    cx,
                );
                let (mark_bg, mark_fg) = match tone {
                    Tone::Primary => (s.primary, s.on_primary),
                    Tone::Secondary => (s.on_secondary_container, s.secondary_container),
                    Tone::Tertiary => (s.tertiary, s.on_primary),
                };
                Shape::new(self.shape.unwrap_or(ShapeName::Cookie9))
                    .size(px(34.))
                    .color(mark_bg)
                    .child(
                        Icon::new(icon)
                            .size(px(16.))
                            .color(alpha(mark_fg, p.clamp(0., 1.)))
                            .scale(0.2 + 0.8 * pop.max(0.))
                            .rotate((1. - pop) * -std::f32::consts::FRAC_PI_3),
                    )
                    .into_any_element()
            } else {
                Icon::new(icon)
                    .size(px(15.))
                    .color(icon_color)
                    .into_any_element()
            }
        });

        let text = if tonal {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .line_height(gpui::relative(1.3))
                .children(self.title.map(|title| {
                    div()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(14.))
                        .font_weight(FontWeight(700.))
                        .child(title)
                }))
                .children(
                    self.text
                        .map(|text| div().text_size(px(11.5)).opacity(0.8).child(text)),
                )
        } else {
            div()
                .flex_1()
                .min_w_0()
                .line_height(gpui::relative(1.4))
                .children(self.title.map(|title| {
                    div()
                        .font_weight(FontWeight(600.))
                        .text_color(s.on_surface)
                        .child(title)
                }))
                .children(self.text)
        };

        let close = self.on_dismiss.map(|f| {
            IconButton::new(key(&self.id, "close"), "x")
                .size(px(26.))
                .tooltip(self.dismiss_label.clone())
                .on_click(move |e, w, cx| f(e, w, cx))
        });

        let rise: Pixels = px((1. - p) * 18.);
        // With actions, a narrow banner drops them to a second line instead of squeezing the
        // text down to nothing.
        let has_actions = !self.actions.is_empty();
        div()
            .relative()
            .top(rise)
            .opacity(p.clamp(0., 1.))
            .flex()
            .when(has_actions, |d| d.flex_wrap())
            .items_center()
            .gap(px(if tonal { 12. } else { 6. }))
            .pt(px(pad[0]))
            .pr(px(pad[1]))
            .pb(px(pad[2]))
            .pl(px(pad[3]))
            .rounded(px(radius))
            .bg(bg)
            .text_color(fg)
            .font_family(FONT_FAMILY)
            .text_size(px(12.5))
            .children(mark)
            .child(text.when(has_actions, |d| d.min_w(px(140.))))
            .children(self.actions.into_iter().map(|a| div().flex_none().child(a)))
            .children(close)
    }
}
