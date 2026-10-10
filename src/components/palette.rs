//! Command palette: a filter field over a keyboard-navigable list of commands.

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyBinding, MouseButton, ParentElement, Render, ScrollHandle,
    SharedString, StatefulInteractiveElement, Styled, Window, actions, anchored, deferred, div, px,
};

use super::{Icon, MenuItem, TextField, TextFieldEvent, pop_in, shadow_pop};
use crate::motion::{Spring, entrance, key};
use crate::theme::{FONT_FAMILY, Theme, alpha};

actions!(m3_command_palette, [SelectPrevious, SelectNext]);

const KEY_CONTEXT: &str = "M3CommandPalette";

pub fn bind_command_palette_keys(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("up", SelectPrevious, ctx),
        KeyBinding::new("down", SelectNext, ctx),
    ]);
}

/// One command of a [`CommandPalette`].
#[derive(Clone, Debug)]
pub struct Command {
    pub icon: SharedString,
    pub label: SharedString,
    /// Shortcut or kind shown at the end ("Ctrl+N", "Conversation").
    pub hint: Option<SharedString>,
}

impl Command {
    pub fn new(icon: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            icon: icon.into(),
            label: label.into(),
            hint: None,
        }
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

#[derive(Clone, Debug)]
pub enum CommandPaletteEvent {
    /// A command was chosen (Enter or click): its index in the list given to
    /// [`CommandPalette::set_commands`]. Close the palette before running it.
    Run(usize),
    /// Escape or a click outside.
    Dismissed,
}

/// Command palette (Ctrl/Cmd+K): a modal surface near the top of the window with a filter
/// field and the matching commands. ↑/↓ or the pointer move the selection, Enter or a click
/// runs it, Escape closes.
///
/// It is an entity: create it once, give it commands, call [`CommandPalette::reset`] and
/// [`CommandPalette::focus`] when opening it, render it while open and subscribe to
/// [`CommandPaletteEvent`].
pub struct CommandPalette {
    field: Entity<TextField>,
    commands: Vec<Command>,
    matches: Vec<usize>,
    selected: usize,
    empty_text: SharedString,
    opened: u64,
    scroll: ScrollHandle,
}

impl EventEmitter<CommandPaletteEvent> for CommandPalette {}

impl Focusable for CommandPalette {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}

impl CommandPalette {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| {
            TextField::new(cx)
                .bare()
                .height(px(48.))
                .text_size(px(15.))
                .placeholder("Search for a command…")
        });
        cx.subscribe(
            &field,
            |this: &mut Self, _, event: &TextFieldEvent, cx| match event {
                TextFieldEvent::Changed(_) => this.refilter(cx),
                TextFieldEvent::Submitted(_) => {
                    if let Some(&index) = this.matches.get(this.selected) {
                        cx.emit(CommandPaletteEvent::Run(index));
                    }
                }
                TextFieldEvent::Cancelled => cx.emit(CommandPaletteEvent::Dismissed),
            },
        )
        .detach();
        Self {
            field,
            commands: Vec::new(),
            matches: Vec::new(),
            selected: 0,
            empty_text: "No results".into(),
            opened: 0,
            scroll: ScrollHandle::new(),
        }
    }

    /// Placeholder of the filter field ("Search for a command…" by default).
    pub fn placeholder(self, placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        let placeholder = placeholder.into();
        self.field
            .update(cx, |field, cx| field.set_placeholder(placeholder, cx));
        self
    }

    /// Text shown when nothing matches ("No results" by default).
    pub fn empty_text(mut self, text: impl Into<SharedString>) -> Self {
        self.empty_text = text.into();
        self
    }

    pub fn set_commands(&mut self, commands: Vec<Command>, cx: &mut Context<Self>) {
        self.commands = commands;
        self.refilter(cx);
    }

    /// Clears the query and selects the first command (call it when opening the palette).
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.field.update(cx, |field, cx| field.set_text("", cx));
        self.selected = 0;
        self.opened += 1;
        self.refilter(cx);
    }

    pub fn focus(&self, window: &mut Window, cx: &App) {
        self.field.read(cx).focus(window);
    }

    fn refilter(&mut self, cx: &mut Context<Self>) {
        let query = self.field.read(cx).text().to_lowercase();
        self.matches = self
            .commands
            .iter()
            .enumerate()
            .filter(|(_, c)| query.is_empty() || c.label.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        self.selected = self.selected.min(self.matches.len().saturating_sub(1));
        cx.notify();
    }

    /// Index (in the filtered list) of the highlighted command.
    pub fn selected(&self) -> usize {
        self.selected
    }

    fn select_previous(&mut self, _: &SelectPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.selected = self.selected.saturating_sub(1);
        self.scroll.scroll_to_item(self.selected);
        cx.notify();
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected + 1 < self.matches.len() {
            self.selected += 1;
        }
        self.scroll.scroll_to_item(self.selected);
        cx.notify();
    }

    /// The pointer moved over a row: it becomes the selection (as in the reference design's lists).
    fn hover_row(&mut self, row: usize, cx: &mut Context<Self>) {
        if self.selected != row && row < self.matches.len() {
            self.selected = row;
            cx.notify();
        }
    }
}

impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let id = gpui::ElementId::from(("m3-command-palette", self.opened));
        let fade = entrance(key(&id, "scrim"), Spring::EFFECTS, 0., window, cx).clamp(0., 1.);

        let mut list = div()
            .id("m3-palette-list")
            .flex()
            .flex_col()
            .max_h(px(380.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .py(px(6.))
            .px(px(4.));
        if self.matches.is_empty() {
            list = list.child(
                div()
                    .p(px(16.))
                    .text_center()
                    .text_color(s.on_surface_variant)
                    .child(self.empty_text.clone()),
            );
        }
        let query = self.field.read(cx).text().clone();
        for (row, &index) in self.matches.iter().enumerate() {
            let command = &self.commands[index];
            let mut item = MenuItem::new(("m3-palette-item", index), command.label.clone())
                .icon(command.icon.clone())
                .highlighted(row == self.selected)
                .hover_fill(false)
                .label_highlights(super::match_ranges(&command.label, &query))
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.emit(CommandPaletteEvent::Run(index));
                }));
            if let Some(hint) = &command.hint {
                item = item.shortcut(hint.clone());
            }
            list = list.child(
                div()
                    .id(("m3-palette-row", index))
                    .on_mouse_move(cx.listener(move |this, _, _, cx| this.hover_row(row, cx)))
                    .child(item),
            );
        }

        let surface = div()
            .id("m3-palette-surface")
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::select_next))
            .flex()
            .flex_col()
            .w(px(600.))
            .max_w_full()
            .rounded(px(16.))
            .overflow_hidden()
            .bg(s.surface_container)
            .shadow(shadow_pop(&s))
            .text_color(s.on_surface)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(16.))
                    .border_b_1()
                    .border_color(s.outline_variant)
                    .child(Icon::new("search").size(px(16.)).color(s.outline))
                    .child(div().flex_1().child(self.field.clone())),
            )
            .child(list);
        let surface = pop_in(div().child(surface), &id, 0., window, cx);

        let viewport = window.viewport_size();
        deferred(
            anchored().position(gpui::point(px(0.), px(0.))).child(
                div()
                    .id(id)
                    .w(viewport.width)
                    .h(viewport.height)
                    .flex()
                    .flex_col()
                    .items_center()
                    .pt(viewport.height * 0.12)
                    .px(px(16.))
                    .bg(alpha(gpui::black(), 0.18 * fade))
                    .font_family(FONT_FAMILY)
                    .text_size(px(13.5))
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _, _, cx| cx.emit(CommandPaletteEvent::Dismissed)),
                    )
                    .child(surface),
            ),
        )
        .with_priority(1)
    }
}
