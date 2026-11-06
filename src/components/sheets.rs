//! Sheets: bottom and side sheets, modal (over a scrim) or standard (inline).

use std::cell::Cell;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, Hsla, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, RenderOnce, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window, anchored, canvas, deferred, div, point, px,
};

use super::overlays::{ground, over};
use super::{ClickHandler, Divider, IconButton, child_id};
use crate::motion::{Spring, entrance, key};
use crate::theme::type_scale::{TITLE_LARGE, TypeScaleExt};
use crate::theme::{Elevation, Scheme, Theme, alpha, font_family};

/// Scrim opacity of the modal sheets.
const SCRIM_OPACITY: f32 = 0.32;
/// Gap left above a modal bottom sheet.
const TOP_MARGIN: f32 = 72.;
/// Extra surface behind the edge of a modal sheet, so the spring's overshoot never shows a gap.
const SKIRT: f32 = 32.;
const SIDE_SHEET_MIN_WIDTH: f32 = 256.;
const SIDE_SHEET_MAX_WIDTH: f32 = 400.;

/// Width of a side sheet kept inside the 256–400 dp range of the spec.
fn side_sheet_width(width: Pixels) -> Pixels {
    px(f32::from(width).clamp(SIDE_SHEET_MIN_WIDTH, SIDE_SHEET_MAX_WIDTH))
}

/// How far a modal sheet is from its resting place. `entered` is the entrance spring (0 → 1,
/// it may overshoot) and `shown` the exit progress (1 shown, 0 gone); `travel` is the distance
/// to a position fully out of view.
fn slide_offset(entered: f32, shown: f32, travel: f32) -> f32 {
    travel * ((1. - entered) + (1. - shown))
}

fn is_escape(key: &str) -> bool {
    key == "escape"
}

/// Surface of a sheet over a translucent scheme (Liquid Glass) gets an opaque ground.
fn sheet_background(s: &Scheme) -> Hsla {
    if s.surface.a < 1. {
        over(s.surface_container_low, ground(s))
    } else {
        s.surface_container_low
    }
}

/// The window-sized layer under a modal sheet: the scrim, dismissal by click or Escape, and the
/// keyboard focus that makes Escape reach it. Lay out its child with the usual flex methods.
fn scrim_layer(
    id: &ElementId,
    s: &Scheme,
    shown: f32,
    on_dismiss: Option<ClickHandler>,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let fade = entrance(key(id, "scrim"), Spring::EFFECTS, 0., window, cx).clamp(0., 1.) * shown;
    let focus = window.use_keyed_state(key(id, "focus"), cx, |_, cx| cx.focus_handle());
    let handle = focus.read(cx).clone();
    let asked = window.use_keyed_state(key(id, "autofocus"), cx, |_, _| false);
    if !*asked.read(cx) {
        asked.update(cx, |asked, _| *asked = true);
        window.focus(&handle);
    }
    let viewport = window.viewport_size();
    let veil = if s.surface.a < 1. {
        // Over a blurred window a plain scrim barely hides it: a frosted veil instead.
        alpha(ground(s), 0.7 * fade)
    } else {
        alpha(s.scrim, SCRIM_OPACITY * fade)
    };
    let scrim = div()
        .id(id.clone())
        .track_focus(&handle)
        .w(viewport.width)
        .h(viewport.height)
        .bg(veil)
        .font_family(font_family())
        .occlude();
    // A sheet that is leaving ignores dismissals.
    match on_dismiss.filter(|_| shown >= 1.) {
        Some(f) => {
            let on_key = f.clone();
            scrim
                .on_click(move |e, w, cx| f(e, w, cx))
                .on_key_down(move |event, w, cx| {
                    if is_escape(event.keystroke.key.as_str()) {
                        cx.stop_propagation();
                        on_key(&ClickEvent::default(), w, cx);
                    }
                })
        }
        None => scrim,
    }
}

/// Puts a modal layer above everything else, anchored to the window's corner.
fn overlay(layer: impl IntoElement) -> impl IntoElement {
    deferred(anchored().position(point(px(0.), px(0.))).child(layer)).with_priority(1)
}

/// Clicks inside a sheet must not reach the scrim.
fn shield<E: InteractiveElement + StatefulInteractiveElement>(sheet: E) -> E {
    sheet
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| cx.stop_propagation())
}

/// Bottom sheet, modal or standard.
///
/// A modal sheet is a scrim over the whole window and a sheet at the bottom that slides up with
/// the spatial spring. Render it anywhere while it is open: it covers the window, and Escape or a
/// click on the scrim call [`BottomSheet::on_dismiss`]. To animate its exit, keep building it
/// while [`crate::motion::presence`] returns `Some` and pass the progress to
/// [`BottomSheet::exit`]; a `Presence` around it cannot do this, because the sheet is drawn
/// deferred.
///
/// A standard sheet is the same surface drawn in place, without scrim, for the host to position.
#[derive(IntoElement)]
pub struct BottomSheet {
    id: ElementId,
    modal: bool,
    max_width: Pixels,
    handle: bool,
    padding: Pixels,
    children: Vec<AnyElement>,
    on_dismiss: Option<ClickHandler>,
    shown: f32,
}

impl BottomSheet {
    fn with_modal(id: impl Into<ElementId>, modal: bool) -> Self {
        Self {
            id: id.into(),
            modal,
            max_width: px(640.),
            handle: true,
            padding: px(24.),
            children: Vec::new(),
            on_dismiss: None,
            shown: 1.,
        }
    }

    /// A modal sheet over a scrim.
    pub fn modal(id: impl Into<ElementId>) -> Self {
        Self::with_modal(id, true)
    }

    /// A standard sheet, drawn in place.
    pub fn standard(id: impl Into<ElementId>) -> Self {
        Self::with_modal(id, false)
    }

    /// Maximum width (640 dp by default); on narrower windows the sheet fills the width.
    pub fn max_width(mut self, max_width: Pixels) -> Self {
        self.max_width = max_width;
        self
    }

    /// The drag handle on top (on by default). It is a visual cue: the sheet is not draggable.
    pub fn drag_handle(mut self, handle: bool) -> Self {
        self.handle = handle;
        self
    }

    /// Padding around the content (24 px by default; no top padding under the handle).
    pub fn padding(mut self, padding: Pixels) -> Self {
        self.padding = padding;
        self
    }

    /// Draws a modal sheet while it leaves: `progress` goes from 1 (shown) to 0 (gone), as
    /// [`crate::motion::presence`] returns it. The scrim fades and the sheet slides down.
    pub fn exit(mut self, progress: f32) -> Self {
        self.shown = progress.clamp(0., 1.);
        self
    }

    /// Called when the scrim is clicked or Escape is pressed (modal sheets only).
    pub fn on_dismiss(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }
}

impl ParentElement for BottomSheet {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for BottomSheet {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let bg = sheet_background(&s);
        let viewport = window.viewport_size();
        let handle = self.handle;
        let padding = self.padding;
        let mut sheet = div()
            .id(child_id(&self.id, "sheet"))
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .max_w(self.max_width)
            .rounded_tl(px(28.))
            .rounded_tr(px(28.))
            .bg(bg)
            .shadow(Elevation::Level1.shadow(&s))
            .text_color(s.on_surface)
            .when(handle, |d| {
                d.child(
                    div().flex().flex_none().justify_center().py(px(22.)).child(
                        div()
                            .w(px(32.))
                            .h(px(4.))
                            .rounded(px(2.))
                            .bg(alpha(s.on_surface_variant, 0.4)),
                    ),
                )
            })
            .child(
                div()
                    .id(child_id(&self.id, "body"))
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(padding)
                    .pb(padding)
                    .when(!handle, |d| d.pt(padding))
                    .children(self.children),
            );
        if !self.modal {
            return sheet.into_any_element();
        }

        sheet = shield(sheet.max_h(viewport.height - px(TOP_MARGIN)).child(
            div().absolute().left_0().right_0().bottom(px(-SKIRT)).h(px(SKIRT)).bg(bg),
        ));
        // The height is measured while painting, to know how far to slide.
        let measured = window
            .use_keyed_state(key(&self.id, "height"), cx, |_, _| Rc::new(Cell::new(0f32)))
            .read(cx)
            .clone();
        let travel = match measured.get() {
            height if height > 0. => height,
            _ => f32::from(viewport.height),
        };
        let entered = entrance(key(&self.id, "slide"), Spring::SPATIAL, 0., window, cx);
        let shown = self.shown;
        let sheet = sheet
            .top(px(slide_offset(entered, shown, travel)))
            .when(shown < 1., |d| d.opacity(shown))
            .child(
                canvas(move |bounds, _, _| measured.set(f32::from(bounds.size.height)), |_, _, _, _| {})
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            );
        overlay(
            scrim_layer(&self.id, &s, shown, self.on_dismiss, window, cx)
                .flex()
                .flex_col()
                .justify_end()
                .items_center()
                .child(sheet),
        )
        .into_any_element()
    }
}

/// The edge a [`SideSheet`] is attached to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SheetSide {
    Left,
    #[default]
    Right,
}

/// Side sheet, modal or standard, on the left or right edge.
///
/// It has a header (an optional back button, the title and an optional close button), the
/// content, which scrolls, and an actions row at the bottom. The modal sheet slides in from its
/// edge over a scrim, like [`BottomSheet::modal`] (and leaves the same way through
/// [`SideSheet::exit`]); the standard one is drawn in place and fills the height of its parent.
#[derive(IntoElement)]
pub struct SideSheet {
    id: ElementId,
    modal: bool,
    side: SheetSide,
    width: Pixels,
    title: Option<SharedString>,
    on_back: Option<ClickHandler>,
    on_close: Option<ClickHandler>,
    on_dismiss: Option<ClickHandler>,
    children: Vec<AnyElement>,
    actions: Vec<AnyElement>,
    shown: f32,
}

impl SideSheet {
    fn with_modal(id: impl Into<ElementId>, modal: bool) -> Self {
        Self {
            id: id.into(),
            modal,
            side: SheetSide::default(),
            width: px(360.),
            title: None,
            on_back: None,
            on_close: None,
            on_dismiss: None,
            children: Vec::new(),
            actions: Vec::new(),
            shown: 1.,
        }
    }

    /// A modal sheet over a scrim.
    pub fn modal(id: impl Into<ElementId>) -> Self {
        Self::with_modal(id, true)
    }

    /// A standard sheet, drawn in place.
    pub fn standard(id: impl Into<ElementId>) -> Self {
        Self::with_modal(id, false)
    }

    /// The edge it is attached to (right by default).
    pub fn side(mut self, side: SheetSide) -> Self {
        self.side = side;
        self
    }

    /// Width, kept between 256 and 400 dp (360 by default).
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = side_sheet_width(width);
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Shows a back button before the title.
    pub fn on_back(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_back = Some(Rc::new(f));
        self
    }

    /// Shows a close button after the title.
    pub fn on_close(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }

    /// Called when the scrim is clicked or Escape is pressed (modal sheets only).
    pub fn on_dismiss(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }

    /// A button in the actions row at the bottom (end-aligned).
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// Draws a modal sheet while it leaves: `progress` goes from 1 (shown) to 0 (gone), as
    /// [`crate::motion::presence`] returns it. The scrim fades and the sheet slides out.
    pub fn exit(mut self, progress: f32) -> Self {
        self.shown = progress.clamp(0., 1.);
        self
    }
}

impl ParentElement for SideSheet {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for SideSheet {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let bg = sheet_background(&s);
        let viewport = window.viewport_size();
        let right = self.side == SheetSide::Right;
        // Rounded on the side that faces the content.
        let radius = px(if self.modal { 28. } else { 16. });

        let has_header = self.title.is_some() || self.on_back.is_some() || self.on_close.is_some();
        let mut header = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .min_h(px(64.))
            .pl(px(if self.on_back.is_some() { 12. } else { 24. }))
            .pr(px(if self.on_close.is_some() { 12. } else { 24. }));
        if let Some(f) = self.on_back {
            header = header.child(
                IconButton::new(child_id(&self.id, "back"), "arrow-left")
                    .size(px(40.))
                    .on_click(move |e, w, cx| f(e, w, cx)),
            );
        }
        header = header.child(
            div()
                .flex_1()
                .min_w_0()
                .type_style(TITLE_LARGE)
                .text_color(s.on_surface_variant)
                .children(self.title),
        );
        if let Some(f) = self.on_close {
            header = header.child(
                IconButton::new(child_id(&self.id, "close"), "x")
                    .size(px(40.))
                    .on_click(move |e, w, cx| f(e, w, cx)),
            );
        }

        let mut sheet = div()
            .id(child_id(&self.id, "sheet"))
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .h_full()
            .w(self.width)
            .max_w(viewport.width)
            .bg(bg)
            .text_color(s.on_surface)
            .when(self.modal, |d| d.shadow(Elevation::Level1.shadow(&s)))
            .when(right, |d| d.rounded_tl(radius).rounded_bl(radius))
            .when(!right, |d| d.rounded_tr(radius).rounded_br(radius))
            .when(has_header, |d| d.child(header))
            .child(
                div()
                    .id(child_id(&self.id, "body"))
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(24.))
                    .children(self.children),
            );
        if !self.actions.is_empty() {
            sheet = sheet.child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .child(Divider::horizontal())
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.))
                            .px(px(24.))
                            .py(px(16.))
                            .children(self.actions),
                    ),
            );
        }
        if !self.modal {
            return sheet.into_any_element();
        }

        // The skirt sits behind the screen edge, so the overshoot of the spring shows no gap.
        let skirt = div().absolute().top_0().bottom_0().w(px(SKIRT)).bg(bg);
        let skirt = if right { skirt.right(px(-SKIRT)) } else { skirt.left(px(-SKIRT)) };
        let entered = entrance(key(&self.id, "slide"), Spring::SPATIAL, 0., window, cx);
        let shown = self.shown;
        let offset = slide_offset(entered, shown, f32::from(self.width));
        let sheet = shield(sheet.child(skirt))
            .left(px(if right { offset } else { -offset }))
            .when(shown < 1., |d| d.opacity(shown));
        let scrim = scrim_layer(&self.id, &s, shown, self.on_dismiss, window, cx).flex();
        overlay(if right { scrim.justify_end() } else { scrim.justify_start() }.child(sheet))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_sheet_width_stays_in_range() {
        assert_eq!(side_sheet_width(px(100.)), px(256.));
        assert_eq!(side_sheet_width(px(320.)), px(320.));
        assert_eq!(side_sheet_width(px(900.)), px(400.));
    }

    #[test]
    fn slide_offset_is_zero_at_rest() {
        assert_eq!(slide_offset(1., 1., 300.), 0.);
        assert_eq!(slide_offset(0., 1., 300.), 300.);
        assert_eq!(slide_offset(1., 0., 300.), 300.);
        // The spring's overshoot moves the sheet a little past its resting place.
        assert!(slide_offset(1.02, 1., 300.) < 0.);
        // Halfway through the entrance and the exit.
        assert_eq!(slide_offset(0.5, 0.5, 100.), 100.);
    }

    #[test]
    fn only_escape_dismisses() {
        assert!(is_escape("escape"));
        assert!(!is_escape("enter"));
        assert!(!is_escape("e"));
    }
}
