//! Rows for side panels: file trees, change lists and tab strips.

use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AnyView, App, ClickEvent, ElementId, Entity, FontWeight, Hsla, InteractiveElement,
    IntoElement, MouseButton, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px,
};

use super::{ChangeHandler, ClickHandler, Icon, RoundedExt, Tooltip, all, child_id};
use crate::interaction::{interactive, state_layer, use_interaction};
use crate::motion::{Spring, animate_color, animate_f32, key};
use crate::theme::{FONT_FAMILY, MONO_FONT_FAMILY, Theme, alpha};

/// Indentation per tree level.
const TREE_INDENT: f32 = 14.;

/// File tree row: a 30 px pill indented 14 px per level, with indent guides, a chevron that
/// turns for folders and a colored icon.
#[derive(IntoElement)]
pub struct TreeRow {
    id: ElementId,
    label: SharedString,
    depth: usize,
    expanded: Option<bool>,
    icon: Option<SharedString>,
    icon_color: Option<Hsla>,
    root: bool,
    dimmed: bool,
    selected: bool,
    guides: bool,
    tooltip: Option<SharedString>,
    trailing: Option<AnyElement>,
    on_click: Option<ClickHandler>,
}

impl TreeRow {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            depth: 0,
            expanded: None,
            icon: None,
            icon_color: None,
            root: false,
            dimmed: false,
            selected: false,
            guides: true,
            tooltip: None,
            trailing: None,
            on_click: None,
        }
    }

    pub fn depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    /// Folder state: `Some(open)` shows a chevron; `None` (files) leaves its space empty.
    pub fn expanded(mut self, expanded: Option<bool>) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Icon color (file type colors); `on_surface_variant` by default.
    pub fn icon_color(mut self, color: Hsla) -> Self {
        self.icon_color = Some(color);
        self
    }

    /// Workspace root row: 34 px and bold.
    pub fn root(mut self, root: bool) -> Self {
        self.root = root;
        self
    }

    /// Hidden files: icon and label at 55 % opacity.
    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Vertical indent guides (on by default).
    pub fn guides(mut self, guides: bool) -> Self {
        self.guides = guides;
        self
    }

    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Content at the end of the row (e.g. the folder of a search result).
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing = Some(el.into_any_element());
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for TreeRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let state = use_interaction(&self.id, window, cx);
        let hovered = state.read(cx).hovered;
        let height = if self.root { 34. } else { 30. };
        let (bg, fg) = if self.selected {
            (s.secondary_container, s.on_secondary_container)
        } else if hovered {
            (s.surface_container_high, s.on_surface)
        } else {
            (alpha(s.surface_container_high, 0.), s.on_surface)
        };
        let bg = animate_color(key(&self.id, "bg"), Spring::EFFECTS, bg, window, cx);
        let rotation = animate_f32(
            key(&self.id, "chevron"),
            Spring::SPATIAL_FAST,
            if self.expanded == Some(true) {
                FRAC_PI_2
            } else {
                0.
            },
            window,
            cx,
        );
        let opacity = if self.dimmed { 0.55 } else { 1. };

        let mut row = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(height))
            .pl(px(6. + self.depth as f32 * TREE_INDENT))
            .pr(px(8.))
            .rounded(px(height / 2.))
            .bg(bg)
            .text_color(fg)
            .font_family(FONT_FAMILY)
            .text_size(px(13.))
            .whitespace_nowrap()
            .when(self.root, |d| d.font_weight(FontWeight(700.)));
        if self.guides {
            for level in 0..self.depth {
                row = row.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(13. + level as f32 * TREE_INDENT))
                        .w(px(1.))
                        .bg(s.outline_variant),
                );
            }
        }
        row = row.child(match self.expanded {
            Some(_) => div().flex_none().child(
                Icon::new("chevron-right")
                    .size(px(12.))
                    .color(s.outline)
                    .rotate(rotation),
            ),
            None => div().flex_none().w(px(12.)),
        });
        if let Some(icon) = self.icon {
            let color = if self.selected {
                fg
            } else {
                self.icon_color.unwrap_or(s.on_surface_variant)
            };
            row = row.child(
                div()
                    .flex_none()
                    .opacity(opacity)
                    .child(Icon::new(icon).size(px(14.)).color(color)),
            );
        }
        row = row
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .opacity(opacity)
                    .child(self.label),
            )
            .children(self.trailing);
        if let Some(text) = self.tooltip {
            row = row.tooltip(Tooltip::text(text));
        }
        let mut row = interactive(row, &state, true, false);
        if let Some(f) = self.on_click {
            row = row.on_click(move |e, w, cx| f(e, w, cx));
        }
        row
    }
}

/// Two-line list row (the change list): a square badge whose corners open on hover, a title,
/// a subtitle and trailing content; the row's corners tighten from 20 to 12 px on hover.
#[derive(IntoElement)]
pub struct ListItem {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    badge: Option<(SharedString, Hsla, Hsla)>,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
    tooltip: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl ListItem {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            badge: None,
            leading: None,
            trailing: None,
            tooltip: None,
            on_click: None,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// 26 px square badge with a short monospace label (a git status letter): text color and
    /// background.
    pub fn badge(mut self, label: impl Into<SharedString>, color: Hsla, background: Hsla) -> Self {
        self.badge = Some((label.into(), color, background));
        self
    }

    /// Leading element instead of the badge.
    pub fn leading(mut self, el: impl IntoElement) -> Self {
        self.leading = Some(el.into_any_element());
        self
    }

    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing = Some(el.into_any_element());
        self
    }

    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ListItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let state = use_interaction(&self.id, window, cx);
        let (hovered, pressed) = {
            let st = state.read(cx);
            (st.hovered, st.pressed)
        };
        let active = hovered || pressed;
        let [radius, badge_radius] = crate::motion::animate(
            key(&self.id, "shape"),
            Spring::SPATIAL_FAST,
            if active { [12., 13.] } else { [20., 9.] },
            window,
            cx,
        );
        let bg = animate_color(
            key(&self.id, "bg"),
            Spring::EFFECTS,
            if active {
                s.surface_container_highest
            } else {
                alpha(s.surface_container_highest, 0.)
            },
            window,
            cx,
        );
        let layer = state_layer(&self.id, &state, all(px(radius)), s.on_surface, window, cx);
        let mut row = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(10.))
            .min_h(px(40.))
            .px(px(8.))
            .py(px(4.))
            .rounded(px(radius))
            .bg(bg)
            .text_color(s.on_surface)
            .font_family(FONT_FAMILY)
            .text_size(px(13.))
            .child(layer);
        if let Some(leading) = self.leading {
            row = row.child(leading);
        } else if let Some((label, color, background)) = self.badge {
            row = row.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(26.))
                    .rounded(px(badge_radius))
                    .bg(background)
                    .text_color(color)
                    .font_family(MONO_FONT_FAMILY)
                    .text_size(px(11.))
                    .font_weight(FontWeight(700.))
                    .child(label),
            );
        }
        let mut text = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .line_height(gpui::relative(1.3))
            .child(
                div()
                    .font_weight(FontWeight(500.))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(self.title),
            );
        if let Some(subtitle) = self.subtitle {
            text = text.child(
                div()
                    .text_size(px(11.5))
                    .text_color(s.outline)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(subtitle),
            );
        }
        row = row.child(text).children(self.trailing);
        if let Some(text) = self.tooltip {
            row = row.tooltip(Tooltip::text(text));
        }
        let mut row = interactive(row, &state, true, true);
        if let Some(f) = self.on_click {
            row = row.on_click(move |e, w, cx| f(e, w, cx));
        }
        row
    }
}

/// One tab of a [`TabStrip`].
pub struct Tab {
    key: SharedString,
    label: SharedString,
    icon: Option<SharedString>,
    tooltip: Option<SharedString>,
    dirty: bool,
    closable: bool,
}

impl Tab {
    /// `key` identifies the tab in the callbacks (a path, a terminal id…).
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            icon: None,
            tooltip: None,
            dirty: false,
            closable: true,
        }
    }

    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Unsaved changes: a dot that turns into the close button on hover.
    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }
}

/// Horizontal strip of closable tabs (editor files, terminals) with horizontal scrolling,
/// unsaved dots, double-click to rename and optional leading/trailing buttons.
///
/// Renaming is yours: on [`TabStrip::on_rename`] show a field with [`TabStrip::editing`]
/// (for instance a [`super::TextField`] with the bare style) and commit it yourself.
#[derive(IntoElement)]
pub struct TabStrip {
    id: ElementId,
    tabs: Vec<Tab>,
    selected: Option<SharedString>,
    editing: Option<(SharedString, AnyView)>,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
    on_select: Option<ChangeHandler<SharedString>>,
    on_close: Option<ChangeHandler<SharedString>>,
    on_rename: Option<ChangeHandler<SharedString>>,
}

impl TabStrip {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            tabs: Vec::new(),
            selected: None,
            editing: None,
            leading: None,
            trailing: None,
            on_select: None,
            on_close: None,
            on_rename: None,
        }
    }

    pub fn tab(mut self, tab: Tab) -> Self {
        self.tabs.push(tab);
        self
    }

    pub fn tabs(mut self, tabs: impl IntoIterator<Item = Tab>) -> Self {
        self.tabs.extend(tabs);
        self
    }

    pub fn selected(mut self, key: impl Into<SharedString>) -> Self {
        self.selected = Some(key.into());
        self
    }

    /// Shows `field` in place of the label of the tab `key`.
    pub fn editing<V: gpui::Render>(
        mut self,
        key: impl Into<SharedString>,
        field: Entity<V>,
    ) -> Self {
        self.editing = Some((key.into(), field.into()));
        self
    }

    /// Element before the tabs (e.g. a back button).
    pub fn leading(mut self, el: impl IntoElement) -> Self {
        self.leading = Some(el.into_any_element());
        self
    }

    /// Element after the tabs (e.g. a "+" button).
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing = Some(el.into_any_element());
        self
    }

    pub fn on_select(mut self, f: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    pub fn on_close(mut self, f: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }

    /// Called on a double click on a tab.
    pub fn on_rename(mut self, f: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_rename = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for TabStrip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let mut list = div()
            .id(child_id(&self.id, "list"))
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(px(2.))
            .overflow_x_scroll();
        for (i, tab) in self.tabs.into_iter().enumerate() {
            let id = ElementId::NamedInteger(format!("{}-tab", self.id).into(), i as u64);
            let state = use_interaction(&id, window, cx);
            let hovered = state.read(cx).hovered;
            let selected = self.selected.as_ref() == Some(&tab.key);
            let radius = px(animate_f32(
                key(&id, "radius"),
                Spring::SPATIAL_FAST,
                if selected { 12. } else { 8. },
                window,
                cx,
            ));
            let (bg, fg) = if selected {
                (s.secondary_container, s.on_secondary_container)
            } else if hovered {
                (s.surface_container_high, s.on_surface)
            } else {
                (alpha(s.surface_container_high, 0.), s.on_surface_variant)
            };
            let bg = animate_color(key(&id, "bg"), Spring::EFFECTS, bg, window, cx);
            let editing = self
                .editing
                .as_ref()
                .filter(|(key, _)| *key == tab.key)
                .map(|(_, view)| view.clone());

            let mut el = div()
                .id(id.clone())
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(2.))
                .h(px(26.))
                .pl(px(10.))
                .pr(px(4.))
                .corners(all(radius))
                .bg(bg)
                .text_color(fg)
                .text_size(px(12.5))
                .whitespace_nowrap()
                .when(selected, |d| d.font_weight(FontWeight(700.)));
            if let Some(icon) = tab.icon {
                el = el.child(
                    div()
                        .mr(px(4.))
                        .child(Icon::new(icon).size(px(13.)).color(fg)),
                );
            }
            el = match editing {
                Some(view) => el.child(div().w(px(120.)).child(view)),
                None => el.child(tab.label),
            };
            if tab.closable {
                let close_id = child_id(&id, "close");
                let close_state = use_interaction(&close_id, window, cx);
                let close_hovered = close_state.read(cx).hovered;
                let show_x = hovered || selected || close_hovered;
                let content = if tab.dirty && !close_hovered {
                    div()
                        .size(px(7.))
                        .rounded(px(3.5))
                        .bg(s.on_surface_variant)
                        .into_any_element()
                } else if show_x {
                    Icon::new("x").size(px(12.)).color(fg).into_any_element()
                } else {
                    div().into_any_element()
                };
                let close = div()
                    .id(close_id)
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(18.))
                    .rounded(px(5.))
                    .when(close_hovered, |d| d.bg(alpha(fg, 0.12)))
                    .child(content)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
                let mut close = interactive(close, &close_state, true, false);
                if let Some(f) = self.on_close.clone() {
                    let key = tab.key.clone();
                    close = close.on_click(move |_, w, cx| {
                        cx.stop_propagation();
                        f(key.clone(), w, cx)
                    });
                }
                el = el.child(close);
            } else {
                el = el.pr(px(10.));
            }
            if let Some(text) = tab.tooltip {
                el = el.tooltip(Tooltip::text(text));
            }
            let mut el = interactive(el, &state, true, false);
            let (select, rename) = (self.on_select.clone(), self.on_rename.clone());
            let key = tab.key.clone();
            el = el.on_click(move |e, w, cx| {
                if e.click_count() >= 2 {
                    if let Some(f) = &rename {
                        f(key.clone(), w, cx);
                    }
                } else if let Some(f) = &select {
                    f(key.clone(), w, cx);
                }
            });
            list = list.child(el);
        }
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(38.))
            .px(px(8.))
            .border_b_1()
            .border_color(s.outline_variant)
            .font_family(FONT_FAMILY)
            .children(self.leading)
            .child(list)
            .children(self.trailing)
    }
}
