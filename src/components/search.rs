//! Search: the docked search bar and the search view it expands into.
//!
//! [`Search`] is the model they share: it wraps a [`TextField`] and holds whether the search is
//! expanded. [`SearchBar`] (collapsed) and [`SearchView`] (expanded, with the results) are drawn
//! from it; the field is drawn by whichever of the two is showing.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, ElementId, Entity, EventEmitter, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Window, canvas, deferred, div, px,
};

use super::{Icon, IconButton, TextField, TextFieldEvent, all, child_id};
use crate::interaction::{focus_ring, interactive, state_layer, use_interaction};
use crate::motion::{self, Spring, entrance, key};
use crate::theme::{Elevation, Theme, font_family};

/// Height of the search bar and of the input row of the view.
const BAR_HEIGHT: f32 = 56.;
/// Corners of the bar and of the open view.
const RADIUS: f32 = 28.;

/// Events emitted by a [`Search`]. Subscribe with `cx.subscribe(&search, ...)` and call
/// `cx.notify()` in the handler: the bar, the view and the clear button are drawn by your view
/// from the state of the search.
#[derive(Clone, Debug, PartialEq)]
pub enum SearchEvent {
    /// The search expanded (the bar was pressed, or [`Search::open`]).
    Opened,
    /// The text changed (typing, paste, [`Search::set_text`], clear).
    Changed(SharedString),
    /// Enter was pressed.
    Submitted(SharedString),
    /// The clear button (or [`Search::clear`]) emptied the text.
    Cleared,
    /// The search collapsed (the back arrow, Escape, or [`Search::close`]).
    Closed,
}

/// A text field styled for a [`Search`]: no container (the bar and the view draw it), 16 px text
/// and a "Search" placeholder.
///
/// ```ignore
/// let field = cx.new(|cx| search_field(cx).placeholder("Search chats"));
/// let search = cx.new(|cx| Search::new(field, cx));
/// ```
pub fn search_field(cx: &mut Context<TextField>) -> TextField {
    TextField::new(cx)
        .bare()
        .text_size(px(16.))
        .placeholder("Search")
}

/// State shared by a [`SearchBar`] and a [`SearchView`]: the text field and whether the search is
/// expanded. Create it once and keep the entity in your view.
pub struct Search {
    field: Entity<TextField>,
    expanded: bool,
    /// Escape or the back arrow were used: the field gives up the focus when the bar is drawn.
    blur_pending: bool,
}

impl EventEmitter<SearchEvent> for Search {}

impl Search {
    /// Wraps `field`, which should come from [`search_field`] (a bare text field).
    pub fn new(field: Entity<TextField>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(
            &field,
            |this: &mut Self, _, event: &TextFieldEvent, cx| match forwarded(event) {
                Some(event) => cx.emit(event),
                None => this.close(cx),
            },
        )
        .detach();
        Self {
            field,
            expanded: false,
            blur_pending: false,
        }
    }

    /// Starts expanded (a view that is open from the beginning).
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn field(&self) -> &Entity<TextField> {
        &self.field
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn text(&self, cx: &App) -> SharedString {
        self.field.read(cx).text().clone()
    }

    /// Expands the search and moves the focus to the field.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.blur_pending = false;
        self.field.read(cx).focus(window);
        if !self.expanded {
            self.expanded = true;
            cx.emit(SearchEvent::Opened);
            cx.notify();
        }
    }

    /// Collapses the search. The text stays; the bar shows it.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.expanded {
            self.expanded = false;
            self.blur_pending = true;
            cx.emit(SearchEvent::Closed);
            cx.notify();
        }
    }

    /// Replaces the text (a chosen suggestion). Emits [`SearchEvent::Changed`].
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text = text.into();
        self.field.update(cx, |field, cx| field.set_text(text, cx));
    }

    /// Empties the text and keeps the focus in the field.
    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.field.update(cx, |field, cx| field.set_text("", cx));
        self.field.read(cx).focus(window);
        cx.emit(SearchEvent::Cleared);
        cx.notify();
    }
}

/// The search event a text field event becomes. Escape becomes none: it closes the search.
fn forwarded(event: &TextFieldEvent) -> Option<SearchEvent> {
    match event {
        TextFieldEvent::Changed(text) => Some(SearchEvent::Changed(text.clone())),
        TextFieldEvent::Submitted(text) => Some(SearchEvent::Submitted(text.clone())),
        TextFieldEvent::Cancelled => None,
    }
}

/// Height the results are clipped to while the view opens, `None` once it has settled.
fn reveal_height(natural: f32, reveal: f32) -> Option<f32> {
    (reveal < 1.).then(|| natural * reveal.max(0.))
}

/// Look of the input row.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RowKind {
    /// Inside the bar: a search icon at the start.
    Docked,
    /// Inside the view, or alone on a page: a back arrow at the start.
    Active,
}

/// Wraps an action of the bar so that pressing it does not press the bar itself.
fn own_press(el: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(el)
}

/// The 56 px row shared by the bar and the view: leading icon or back arrow, the field, a clear
/// button while there is text and the host's trailing slot. With `leaving` the field is replaced
/// by its text, because the field is already drawn by the bar.
fn input_row(
    id: &ElementId,
    search: &Entity<Search>,
    kind: RowKind,
    trailing: Option<AnyElement>,
    leaving: bool,
    cx: &App,
) -> gpui::Div {
    let s = *Theme::of(cx);
    let field = search.read(cx).field.clone();
    let text = field.read(cx).text().clone();

    let leading = match kind {
        RowKind::Docked => div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(40.))
            .child(Icon::new("search").size(px(22.)).color(s.on_surface))
            .into_any_element(),
        RowKind::Active => {
            let search = search.clone();
            IconButton::new(child_id(id, "back"), "arrow-left")
                .size(px(40.))
                .tooltip("Back")
                .on_click(move |_, _, cx| search.update(cx, |search, cx| search.close(cx)))
                .into_any_element()
        }
    };
    let input = if leaving {
        div()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_size(px(16.))
            .child(text.clone())
            .into_any_element()
    } else {
        div().flex_1().min_w_0().child(field).into_any_element()
    };
    let clear = (!text.is_empty()).then(|| {
        let search = search.clone();
        own_press(
            IconButton::new(child_id(id, "clear"), "x")
                .size(px(40.))
                .tooltip("Clear")
                .on_click(move |_, window, cx| {
                    search.update(cx, |search, cx| search.clear(window, cx))
                }),
        )
    });
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .w_full()
        .h(px(BAR_HEIGHT))
        .px(px(8.))
        .font_family(font_family())
        .text_color(s.on_surface)
        .child(leading)
        .child(input)
        .children(clear)
        .children(trailing.map(own_press))
}

/// Docked search bar: a 56 px pill with a search icon, the field, a clear button while there is
/// text and an optional trailing slot (an avatar, a microphone).
///
/// Pressing it expands the [`Search`] (see [`SearchBar::open_on_press`]); while the search is
/// expanded the bar draws nothing but keeps its 56 px, so a [`SearchView`] can open over it.
/// With [`SearchBar::active`] it is the flat input row instead (a back arrow at the start, no
/// container), for a search that takes over a page.
#[derive(IntoElement)]
pub struct SearchBar {
    id: ElementId,
    search: Entity<Search>,
    trailing: Option<AnyElement>,
    active: bool,
    open_on_press: bool,
}

impl SearchBar {
    pub fn new(id: impl Into<ElementId>, search: &Entity<Search>) -> Self {
        Self {
            id: id.into(),
            search: search.clone(),
            trailing: None,
            active: false,
            open_on_press: true,
        }
    }

    /// Element at the end of the bar, after the clear button (an [`super::Avatar`] or an
    /// [`IconButton`]). Pressing it does not open the search.
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing = Some(el.into_any_element());
        self
    }

    /// Draws the active row: no container, a back arrow instead of the search icon.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Whether pressing the bar expands the search (on by default). Off, it only focuses the
    /// field, for a bar that filters a list in place.
    pub fn open_on_press(mut self, open: bool) -> Self {
        self.open_on_press = open;
        self
    }
}

impl RenderOnce for SearchBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        if self.active {
            return input_row(
                &self.id,
                &self.search,
                RowKind::Active,
                self.trailing,
                false,
                cx,
            )
            .into_any_element();
        }
        let (expanded, field) = {
            let search = self.search.read(cx);
            (search.expanded, search.field.clone())
        };
        if expanded {
            return div().w_full().h(px(BAR_HEIGHT)).into_any_element();
        }
        let focused = field.read(cx).is_focused(window);
        let blur = self
            .search
            .update(cx, |search, _| std::mem::take(&mut search.blur_pending));
        if blur && focused {
            window.blur();
        }

        let state = use_interaction(&self.id, window, cx);
        let radii = all(px(RADIUS));
        let layer = state_layer(&self.id, &state, radii, s.on_surface, window, cx);
        let ring = focus_ring(&self.id, focused, radii, window, cx);
        let row = input_row(
            &self.id,
            &self.search,
            RowKind::Docked,
            self.trailing,
            false,
            cx,
        );
        let bar = div()
            .id(self.id.clone())
            .relative()
            .w_full()
            .h(px(BAR_HEIGHT))
            .rounded(px(RADIUS))
            .bg(s.surface_container_high)
            .child(layer)
            .child(row)
            .children(ring);
        let search = self.search;
        let open = self.open_on_press;
        interactive(bar, &state, true, false)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if open {
                    search.update(cx, |search, cx| search.open(window, cx));
                } else {
                    field.read(cx).focus(window);
                }
            })
            .into_any_element()
    }
}

/// The expanded search: a container with 28 px corners holding the input row, a divider and the
/// results, which scroll when they pass [`SearchView::max_height`]. It opens with a spring and
/// fades out when the [`Search`] collapses.
///
/// The results are the children, usually [`super::ListItem`]s; keep building them while the view
/// leaves. Render it next to a [`SearchBar`] with [`SearchView::overlay`] to open over the content
/// below the bar, or in the page's flow without it.
#[derive(IntoElement)]
pub struct SearchView {
    id: ElementId,
    search: Entity<Search>,
    trailing: Option<AnyElement>,
    empty_text: Option<SharedString>,
    max_height: Pixels,
    overlay: bool,
    children: Vec<AnyElement>,
}

impl SearchView {
    pub fn new(id: impl Into<ElementId>, search: &Entity<Search>) -> Self {
        Self {
            id: id.into(),
            search: search.clone(),
            trailing: None,
            empty_text: None,
            max_height: px(360.),
            overlay: false,
            children: Vec::new(),
        }
    }

    /// Element at the end of the input row, the same as the bar's.
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing = Some(el.into_any_element());
        self
    }

    /// Text shown when there are no results ("No results").
    pub fn empty_text(mut self, text: impl Into<SharedString>) -> Self {
        self.empty_text = Some(text.into());
        self
    }

    /// Height of the results before they scroll (360 px by default).
    pub fn max_height(mut self, height: Pixels) -> Self {
        self.max_height = height;
        self
    }

    /// Draws the view over what comes after it, from the top of its parent, which should be a
    /// 56 px slot holding the bar. Without it the view takes its place in the layout.
    pub fn overlay(mut self, overlay: bool) -> Self {
        self.overlay = overlay;
        self
    }
}

impl ParentElement for SearchView {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for SearchView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let expanded = self.search.read(cx).expanded;
        let Some(presence) = motion::presence(key(&self.id, "presence"), expanded, window, cx)
        else {
            return div().into_any_element();
        };
        let leaving = !expanded;
        // Both restart each time the view appears, because their state lives only while it is drawn.
        let reveal = entrance(
            key(&self.id, "reveal"),
            Spring::SPATIAL_FAST,
            0.,
            window,
            cx,
        )
        .clamp(0., 1.);
        let fade = entrance(key(&self.id, "fade"), Spring::EFFECTS, 0., window, cx).clamp(0., 1.);

        let row = input_row(
            &self.id,
            &self.search,
            RowKind::Active,
            self.trailing,
            leaving,
            cx,
        );

        let results = if self.children.is_empty() {
            self.empty_text.map(|text| {
                div()
                    .p(px(16.))
                    .text_center()
                    .text_size(px(14.))
                    .text_color(s.on_surface_variant)
                    .child(text)
                    .into_any_element()
            })
        } else {
            Some(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .p(px(8.))
                    .children(self.children)
                    .into_any_element(),
            )
        };
        let results = results.map(|results| {
            // The natural height, measured each frame, is what the opening reveals.
            let measured = window.use_keyed_state(key(&self.id, "results"), cx, |_, _| px(0.));
            let height = *measured.read(cx);
            let measure = canvas(
                move |bounds, _, cx| measured.update(cx, |h, _| *h = bounds.size.height),
                |_, _, _, _| {},
            )
            .absolute()
            .size_full();
            let natural = div()
                .relative()
                .flex()
                .flex_col()
                .flex_none()
                .child(div().h(px(1.)).bg(s.outline_variant))
                .child(
                    div()
                        .id(key(&self.id, "scroll"))
                        .max_h(self.max_height)
                        .overflow_y_scroll()
                        .opacity(if leaving { 1. } else { fade })
                        .child(results),
                )
                .child(measure);
            match reveal_height(f32::from(height), reveal) {
                None => natural,
                Some(clip) => div()
                    .h(px(clip))
                    .flex_none()
                    .overflow_hidden()
                    .child(natural),
            }
        });

        let panel = div()
            .id(key(&self.id, "panel"))
            .flex()
            .flex_col()
            .w_full()
            .rounded(px(RADIUS))
            .overflow_hidden()
            .bg(s.surface_container_high)
            .shadow(Elevation::Level3.shadow(&s))
            .text_color(s.on_surface)
            .font_family(font_family())
            .when(leaving, |d| d.opacity(presence.clamp(0., 1.)))
            .occlude()
            .child(row)
            .children(results);
        if self.overlay {
            deferred(div().absolute().top_0().left_0().right_0().child(panel))
                .with_priority(1)
                .into_any_element()
        } else {
            panel.into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_events_become_search_events_and_escape_closes() {
        let text: SharedString = "rust".into();
        assert_eq!(
            forwarded(&TextFieldEvent::Changed(text.clone())),
            Some(SearchEvent::Changed(text.clone()))
        );
        assert_eq!(
            forwarded(&TextFieldEvent::Submitted(text.clone())),
            Some(SearchEvent::Submitted(text))
        );
        assert_eq!(forwarded(&TextFieldEvent::Cancelled), None);
    }

    #[test]
    fn results_are_clipped_only_while_opening() {
        assert_eq!(reveal_height(300., 0.), Some(0.));
        assert_eq!(reveal_height(300., 0.25), Some(75.));
        assert_eq!(reveal_height(300., 1.), None);
        // A spring can dip below zero before it starts.
        assert_eq!(reveal_height(300., -0.1), Some(0.));
    }
}
