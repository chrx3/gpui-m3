//! Expandable cards: a clickable header that shows or hides a body.

use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, canvas, div, px,
};

use super::{ChangeHandler, Icon, RoundedExt, all, child_id, corners};
use crate::interaction::{
    focus_ring, focusable, interactive, state_layer, use_focus, use_interaction,
};
use crate::motion::{self, Expand, Spring, animate_f32, entrance, key};
use crate::shapes::Tone;
use crate::theme::{FONT_FAMILY, Theme};

/// Card with a clickable header and a collapsible body (tool cards, change groups). The open
/// state is yours: pass it with [`ExpandableCard::open`] and update it in `on_toggle`.
///
/// [`ExpandableCard::plain`] drops the background and moves the chevron before the header, for
/// lighter disclosures such as a model's reasoning.
///
/// The header is a Tab stop: Enter or Space toggles it, and keyboard focus shows a focus ring
/// (hidden after a click, like CSS `:focus-visible`).
#[derive(IntoElement)]
pub struct ExpandableCard {
    id: ElementId,
    open: bool,
    fixed: bool,
    plain: bool,
    tone: Option<Tone>,
    header: Option<AnyElement>,
    preview: Option<AnyElement>,
    under: Option<AnyElement>,
    children: Vec<AnyElement>,
    on_toggle: Option<ChangeHandler<bool>>,
}

impl ExpandableCard {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            open: false,
            fixed: false,
            plain: false,
            tone: None,
            header: None,
            preview: None,
            under: None,
            children: Vec::new(),
            on_toggle: None,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Always open, without chevron or toggling (e.g. a task list).
    pub fn fixed(mut self, fixed: bool) -> Self {
        self.fixed = fixed;
        self
    }

    /// No background; the chevron goes first and the body hangs from a 2 px guide line.
    pub fn plain(mut self, plain: bool) -> Self {
        self.plain = plain;
        self
    }

    /// Tonal container instead of `surface_container_high` (subagent, plan and task cards).
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    /// Header content (status, verb, target, badges…). The chevron is added for you.
    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }

    /// Shown after the header only while closed (e.g. the last line of a streaming reasoning),
    /// truncated to the remaining width.
    pub fn preview(mut self, preview: impl IntoElement) -> Self {
        self.preview = Some(preview.into_any_element());
        self
    }

    /// Content between the header and the body that stays visible when closed.
    pub fn under(mut self, under: impl IntoElement) -> Self {
        self.under = Some(under.into_any_element());
        self
    }

    /// Receives the new open state.
    pub fn on_toggle(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }
}

impl ParentElement for ExpandableCard {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ExpandableCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let expandable = !self.fixed && !self.children.is_empty();
        let open = (self.open || self.fixed) && !self.children.is_empty();
        let head_id = child_id(&self.id, "head");
        let state = use_interaction(&head_id, window, cx);
        let pressed = state.read(cx).pressed && expandable;
        let (bg, fg) = match self.tone {
            Some(tone) => tone.colors(&s),
            None => (s.surface_container_high, s.on_surface),
        };
        let card_radius = if self.plain { 0. } else { 16. };
        // Only a closed card tightens its corners on press (an open one keeps its shape).
        let radius = px(animate_f32(
            key(&self.id, "radius"),
            Spring::SPATIAL_FAST,
            if pressed && !open { 8. } else { card_radius },
            window,
            cx,
        ));
        let rotation = animate_f32(
            key(&self.id, "chevron"),
            Spring::SPATIAL_FAST,
            if open { FRAC_PI_2 } else { 0. },
            window,
            cx,
        );
        let chevron_color = if self.plain {
            s.on_surface_variant
        } else {
            s.outline
        };
        let chevron = expandable.then(|| {
            Icon::new("chevron-right")
                .size(px(if self.plain { 12. } else { 13. }))
                .color(chevron_color)
                .rotate(rotation)
        });

        // Open: the header's hover layer keeps square bottom corners against the body.
        let r = f32::from(radius);
        let head_radius = if open {
            corners([r, r, 0., 0.])
        } else {
            all(radius)
        };
        let layer = state_layer(&head_id, &state, head_radius, fg, window, cx);
        let head_id_ring = head_id.clone();
        // The plain variant has no shape of its own: its ring gets small corners.
        let ring_radius = if self.plain { all(px(6.)) } else { head_radius };
        let preview = self.preview.filter(|_| !open).map(|preview| {
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .italic()
                .text_color(s.outline)
                .child(preview)
        });
        let mut head = div().id(head_id).relative().flex().items_center().min_w_0();
        head = if self.plain {
            head.gap(px(6.))
                .py(px(2.))
                .children(chevron)
                .child(div().flex_none().children(self.header))
                .children(preview)
        } else {
            head.gap(px(7.))
                .min_h(px(30.))
                .px(px(10.))
                .py(px(4.))
                .corners(head_radius)
                .child(layer)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .flex_1()
                        .min_w_0()
                        .children(self.header)
                        .children(preview),
                )
                .children(chevron)
        };
        let mut head = interactive(head, &state, expandable, !self.plain);
        if let (Some(f), true) = (self.on_toggle, expandable) {
            let open = self.open;
            let (focus, handle, ring_visible) = use_focus(&head_id_ring, window, cx);
            let ring = focus_ring(&head_id_ring, ring_visible, ring_radius, window, cx);
            // GPUI turns Enter and Space on a focused clickable element into a click.
            head = focusable(head, &focus, &handle)
                .on_click(move |_, w, cx| f(!open, w, cx))
                .children(ring);
        }

        // The body's natural height, measured each frame it is drawn, drives the spring that
        // opens and closes it.
        let measured = window.use_keyed_state(key(&self.id, "body-height"), cx, |_, _| px(0.));
        let expand = if expandable || open {
            motion::expand(
                key(&self.id, "expand"),
                open,
                *measured.read(cx),
                Spring::SPATIAL,
                window,
                cx,
            )
        } else {
            Expand::Hidden
        };
        let body = (expand != Expand::Hidden).then(|| {
            // Fades in each time the body appears (its entrance state lives only while shown).
            let p = entrance(key(&self.id, "body"), Spring::EFFECTS, 0., window, cx);
            let p = if open { p } else { 1. };
            let measure = canvas(
                move |bounds, _, cx| {
                    measured.update(cx, |h, _| *h = bounds.size.height);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full();
            let body = div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .opacity(p.clamp(0., 1.))
                .children(self.children);
            let body = if self.plain {
                body.mt(px(4.))
                    .ml(px(6.))
                    .pl(px(12.))
                    .border_l_2()
                    .border_color(s.outline_variant)
            } else {
                body.pt(px(2.)).px(px(10.)).pb(px(10.))
            };
            // Margins included: the measured box is exactly what the clip must reveal.
            let body = div()
                .relative()
                .flex()
                .flex_col()
                .flex_none()
                .child(body)
                .child(measure);
            match expand {
                Expand::Clipped(h) => div().h(h).flex_none().overflow_hidden().child(body),
                _ => body,
            }
        });

        div()
            .flex()
            .flex_col()
            .min_w_0()
            .font_family(FONT_FAMILY)
            .text_size(px(12.5))
            .text_color(if self.plain { s.on_surface_variant } else { fg })
            .when(!self.plain, |d| d.corners(all(radius)).bg(bg))
            .child(head)
            .children(self.under)
            .children(body)
    }
}
