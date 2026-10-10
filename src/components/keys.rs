//! Keyboard keys: [`Kbd`] (one key cap), [`KeyCombo`] (a combination such as Ctrl + Shift + K,
//! optionally clickable, with a "capturing" state) and [`KeyNames`] (the words used for the
//! modifiers and named keys, so an app can show them in its own language).

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use super::{ClickHandler, Tooltip, all};
use crate::interaction::{interactive, state_layer, use_interaction};
use crate::theme::{FONT_FAMILY, Theme};

/// The words shown for the modifiers and named keys.
///
/// Tokens: `ctrl`, `alt`, `shift`, `meta`, `enter`, `escape`, `space`, `backspace`, `delete`,
/// `tab`, `up`, `down`, `left`, `right`, `pageup`, `pagedown`, `home`, `end`, `insert`. Any other
/// key (a letter, `f5`, `,`) is shown in upper case.
#[derive(Clone, Debug)]
pub struct KeyNames {
    names: HashMap<&'static str, SharedString>,
}

/// English names; arrows are symbols.
const ENGLISH: [(&str, &str); 19] = [
    ("ctrl", "Ctrl"),
    ("alt", "Alt"),
    ("shift", "Shift"),
    ("meta", "Win"),
    ("enter", "Enter"),
    ("escape", "Esc"),
    ("space", "Space"),
    ("backspace", "Backspace"),
    ("delete", "Del"),
    ("tab", "Tab"),
    ("up", "↑"),
    ("down", "↓"),
    ("left", "←"),
    ("right", "→"),
    ("pageup", "PgUp"),
    ("pagedown", "PgDn"),
    ("home", "Home"),
    ("end", "End"),
    ("insert", "Ins"),
];

/// Spanish names that differ from the English ones.
const SPANISH: [(&str, &str); 8] = [
    ("shift", "Mayús"),
    ("space", "Espacio"),
    ("backspace", "Retroceso"),
    ("delete", "Supr"),
    ("pageup", "Re Pág"),
    ("pagedown", "Av Pág"),
    ("home", "Inicio"),
    ("end", "Fin"),
];

impl Default for KeyNames {
    fn default() -> Self {
        Self::english()
    }
}

impl KeyNames {
    pub fn english() -> Self {
        Self {
            names: ENGLISH
                .iter()
                .map(|(k, v)| (*k, SharedString::from(*v)))
                .collect(),
        }
    }

    /// Spanish: Mayús, Espacio, Retroceso, Supr, Re Pág, Av Pág, Inicio, Fin.
    pub fn spanish() -> Self {
        Self::english().with_all(SPANISH)
    }

    /// Replaces the word of one token (`"ctrl"` → `"Control"`); unknown tokens are ignored.
    pub fn with(mut self, token: &'static str, name: impl Into<SharedString>) -> Self {
        if self.names.contains_key(token) {
            self.names.insert(token, name.into());
        }
        self
    }

    fn with_all<'a>(self, pairs: impl IntoIterator<Item = (&'static str, &'a str)>) -> Self {
        pairs.into_iter().fold(self, |names, (token, name)| {
            names.with(token, name.to_string())
        })
    }

    /// The word of a token, or the key in upper case when it has none.
    pub fn name(&self, token: &str) -> SharedString {
        self.names
            .get(token)
            .cloned()
            .unwrap_or_else(|| token.to_uppercase().into())
    }
}

/// Modifier prefixes and the token each stands for, in the order they are shown.
const MODIFIERS: [&str; 4] = ["ctrl", "alt", "shift", "meta"];

fn modifier_token(prefix: &str) -> Option<&'static str> {
    Some(match prefix {
        "ctrl" | "control" => "ctrl",
        "alt" | "option" => "alt",
        "shift" => "shift",
        "cmd" | "win" | "super" | "meta" => "meta",
        _ => return None,
    })
}

/// Parses a key text (`ctrl-shift-k`, `escape escape`) into steps of key words: modifiers first
/// in a fixed order (Ctrl, Alt, Shift, Win), then the key. Steps are separated by spaces. A `-`
/// right after a modifier is the key itself (`ctrl--` is Ctrl and «-»).
pub fn parse_keys(spec: &str, names: &KeyNames) -> Vec<Vec<SharedString>> {
    spec.split_whitespace()
        .map(|step| {
            let mut mods = [false; 4];
            let mut rest = step;
            while let Some((head, tail)) = rest.split_once('-') {
                match modifier_token(&head.to_lowercase()) {
                    Some(token) if !tail.is_empty() => {
                        if let Some(slot) = MODIFIERS.iter().position(|m| *m == token) {
                            mods[slot] = true;
                        }
                        rest = tail;
                    }
                    _ => break,
                }
            }
            let mut keys: Vec<SharedString> = MODIFIERS
                .iter()
                .zip(mods)
                .filter(|(_, on)| *on)
                .map(|(token, _)| names.name(token))
                .collect();
            keys.push(names.name(&rest.to_lowercase()));
            keys
        })
        .collect()
}

/// A single key cap: a tonal container with a label-sized caption.
#[derive(IntoElement)]
pub struct Kbd {
    label: SharedString,
    small: bool,
    muted: bool,
    active: bool,
}

impl Kbd {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            small: false,
            muted: false,
            active: false,
        }
    }

    /// 22 px instead of 26 px.
    pub fn small(mut self, small: bool) -> Self {
        self.small = small;
        self
    }

    /// Quieter colors, for keys that cannot be changed.
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// Highlighted in the secondary container (a key that is being pressed).
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

impl RenderOnce for Kbd {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let height = if self.small { 22. } else { 26. };
        let (bg, fg) = if self.active {
            (s.secondary_container, s.on_secondary_container)
        } else if self.muted {
            (s.surface_container_high, s.on_surface_variant)
        } else {
            (s.surface_container_highest, s.on_surface)
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(height))
            .min_w(px(height))
            .px(px(if self.small { 6. } else { 8. }))
            .rounded(px(if self.small { 7. } else { 9. }))
            .bg(bg)
            .text_color(fg)
            .font_family(FONT_FAMILY)
            .text_size(px(if self.small { 11. } else { 12. }))
            .font_weight(FontWeight(600.))
            .whitespace_nowrap()
            .child(self.label)
    }
}

/// A key combination in one row of [`Kbd`]s: `KeyCombo::new("save", "ctrl-s")`.
///
/// - Several steps (`"escape escape"`) are shown one after the other with a wider gap.
/// - `on_click` makes it a button (hover layer, pointer); with `capturing(true)` it turns into a
///   pulsing pill with a prompt ("Press the new keys…") while the app waits for a key.
/// - `names` changes the language of the key words (see [`KeyNames`]).
#[derive(IntoElement)]
pub struct KeyCombo {
    id: ElementId,
    spec: SharedString,
    names: Rc<KeyNames>,
    small: bool,
    muted: bool,
    capturing: bool,
    prompt: SharedString,
    tooltip: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl KeyCombo {
    pub fn new(id: impl Into<ElementId>, spec: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            spec: spec.into(),
            names: Rc::new(KeyNames::english()),
            small: false,
            muted: false,
            capturing: false,
            prompt: "Press the new keys…".into(),
            tooltip: None,
            on_click: None,
        }
    }

    pub fn names(mut self, names: KeyNames) -> Self {
        self.names = Rc::new(names);
        self
    }

    pub fn small(mut self, small: bool) -> Self {
        self.small = small;
        self
    }

    /// Quieter keys, for shortcuts that cannot be changed.
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// Waiting for the user to press a new combination.
    pub fn capturing(mut self, capturing: bool) -> Self {
        self.capturing = capturing;
        self
    }

    /// Text of the capturing pill.
    pub fn prompt(mut self, prompt: impl Into<SharedString>) -> Self {
        self.prompt = prompt.into();
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

impl RenderOnce for KeyCombo {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let height = if self.small { 22. } else { 26. };
        let content: AnyElement = if self.capturing {
            let opacity = crate::motion::pulse(1.4, window, cx);
            div()
                .flex()
                .items_center()
                .h(px(height + 4.))
                .px(px(14.))
                .rounded(px((height + 4.) / 2.))
                .bg(s.primary_container)
                .text_color(s.on_primary_container)
                .font_family(FONT_FAMILY)
                .text_size(px(12.5))
                .font_weight(FontWeight(600.))
                .whitespace_nowrap()
                .opacity(opacity)
                .child(self.prompt.clone())
                .into_any_element()
        } else {
            let steps = parse_keys(&self.spec, &self.names);
            let mut row = div().flex().flex_none().items_center().gap(px(10.));
            for step in steps {
                let mut keys = div().flex().flex_none().items_center().gap(px(4.));
                for key in step {
                    keys = keys.child(Kbd::new(key).small(self.small).muted(self.muted));
                }
                row = row.child(keys);
            }
            row.into_any_element()
        };
        let Some(on_click) = self.on_click else {
            return div().flex().flex_none().child(content).into_any_element();
        };
        let state = use_interaction(&self.id, window, cx);
        let layer = state_layer(&self.id, &state, all(px(14.)), s.on_surface, window, cx);
        let mut button = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .p(px(3.))
            .rounded(px(14.))
            .child(layer)
            .child(content);
        if let Some(text) = self.tooltip {
            button = button.tooltip(Tooltip::text(text));
        }
        interactive(button, &state, true, true)
            .on_click(move |e, w, cx| on_click(e, w, cx))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(spec: &str, names: &KeyNames) -> Vec<Vec<String>> {
        parse_keys(spec, names)
            .into_iter()
            .map(|step| step.into_iter().map(|k| k.to_string()).collect())
            .collect()
    }

    #[test]
    fn parses_modifiers_in_a_fixed_order() {
        let en = KeyNames::english();
        assert_eq!(words("ctrl-shift-k", &en), [["Ctrl", "Shift", "K"]]);
        assert_eq!(words("shift-ctrl-K", &en), [["Ctrl", "Shift", "K"]]);
        assert_eq!(words("cmd-alt-x", &en), [["Alt", "Win", "X"]]);
        assert_eq!(words("control-option-f5", &en), [["Ctrl", "Alt", "F5"]]);
    }

    #[test]
    fn a_dash_after_a_modifier_is_the_key() {
        let en = KeyNames::english();
        assert_eq!(words("ctrl--", &en), [["Ctrl", "-"]]);
        assert_eq!(words("ctrl-,", &en), [["Ctrl", ","]]);
        assert_eq!(words("-", &en), [["-"]]);
    }

    #[test]
    fn named_keys_and_sequences() {
        let en = KeyNames::english();
        assert_eq!(words("escape escape", &en), [["Esc"], ["Esc"]]);
        assert_eq!(words("alt-enter", &en), [["Alt", "Enter"]]);
        assert_eq!(words("up", &en), [["↑"]]);
        assert!(parse_keys("", &en).is_empty());
    }

    #[test]
    fn names_are_configurable() {
        let es = KeyNames::spanish();
        assert_eq!(words("ctrl-shift-k", &es), [["Ctrl", "Mayús", "K"]]);
        assert_eq!(words("pagedown", &es), [["Av Pág"]]);
        let custom = KeyNames::spanish()
            .with("ctrl", "Control")
            .with("nope", "x");
        assert_eq!(words("ctrl-space", &custom), [["Control", "Espacio"]]);
    }
}
