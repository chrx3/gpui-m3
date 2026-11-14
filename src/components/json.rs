//! A foldable view of a JSON value (feature `json`).

use std::collections::HashSet;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, ElementId, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use serde_json::Value;

use super::{Icon, child_id};
use crate::motion::key;
use crate::theme::{MONO_FONT_FAMILY, Scheme, Theme};

/// A JSON value as a tree you can fold: objects and arrays open and close with a click on
/// their row, and long ones show their first [`max_items`](JsonView::max_items) entries and a
/// "N more" row.
///
/// Rows appear without animation (it is data to read, and can be long). The fold state lives
/// in the view under `id`, so keep it stable. Built with the `json` feature.
///
/// ```ignore
/// JsonView::new("tool-input", &value).max_depth(1).max_items(20)
/// ```
#[derive(IntoElement)]
pub struct JsonView {
    id: ElementId,
    value: Rc<Value>,
    max_depth: usize,
    max_items: usize,
    max_height: Pixels,
}

impl JsonView {
    /// A view of a copy of `value`. For a large value drawn every frame, share it with
    /// [`JsonView::shared`] instead.
    pub fn new(id: impl Into<ElementId>, value: &Value) -> Self {
        Self::shared(id, Rc::new(value.clone()))
    }

    /// A view of a shared value (no copy per frame).
    pub fn shared(id: impl Into<ElementId>, value: Rc<Value>) -> Self {
        Self {
            id: id.into(),
            value,
            max_depth: 2,
            max_items: 50,
            max_height: px(420.),
        }
    }

    /// Levels open when it first appears (2 by default): deeper objects and arrays start
    /// folded. 0 folds the root.
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Entries of an object or array shown before a "N more" row that shows the rest (50 by
    /// default).
    pub fn max_items(mut self, items: usize) -> Self {
        self.max_items = items.max(1);
        self
    }

    /// Height after which the view scrolls (420 px by default).
    pub fn max_height(mut self, height: Pixels) -> Self {
        self.max_height = height;
        self
    }
}

/// Which containers the user opened or closed against their default, and which show all their
/// entries. Keyed by path.
#[derive(Default)]
struct FoldState {
    toggled: HashSet<String>,
    expanded: HashSet<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum RowKind {
    /// An object or array: its brackets, how many entries it has and whether it is open.
    Open {
        open: char,
        close: char,
        count: usize,
        is_open: bool,
    },
    /// A string, number, boolean or null, already written as JSON.
    Scalar(String),
    /// The closing bracket of an open container.
    Close(char),
    /// Entries of the container at the row's path that are not shown.
    More(usize),
}

#[derive(Clone, Debug, PartialEq)]
struct JsonRow {
    depth: usize,
    path: String,
    key: Option<String>,
    kind: RowKind,
    comma: bool,
}

/// Path of a child: the parent's path plus the key, as a JSON pointer.
fn child_path(parent: &str, key: &str) -> String {
    format!("{parent}/{}", key.replace('~', "~0").replace('/', "~1"))
}

/// The rows of `value` with the fold state applied.
fn json_rows(
    value: &Value,
    max_depth: usize,
    max_items: usize,
    toggled: &HashSet<String>,
    expanded: &HashSet<String>,
) -> Vec<JsonRow> {
    struct Walk<'a> {
        max_depth: usize,
        max_items: usize,
        toggled: &'a HashSet<String>,
        expanded: &'a HashSet<String>,
        rows: Vec<JsonRow>,
    }
    impl Walk<'_> {
        fn visit(&mut self, value: &Value, key: Option<String>, path: String, depth: usize, comma: bool) {
            let children: Vec<(Option<String>, &Value)> = match value {
                Value::Object(map) => map.iter().map(|(k, v)| (Some(k.clone()), v)).collect(),
                Value::Array(items) => items.iter().map(|v| (None, v)).collect(),
                scalar => {
                    let text = serde_json::to_string(scalar).unwrap_or_default();
                    self.rows.push(JsonRow { depth, path, key, kind: RowKind::Scalar(text), comma });
                    return;
                }
            };
            let (open, close) = if value.is_object() { ('{', '}') } else { ('[', ']') };
            let is_open = !children.is_empty()
                && ((depth < self.max_depth) != self.toggled.contains(&path));
            self.rows.push(JsonRow {
                depth,
                path: path.clone(),
                key,
                kind: RowKind::Open { open, close, count: children.len(), is_open },
                comma,
            });
            if !is_open {
                return;
            }
            let shown = if self.expanded.contains(&path) {
                children.len()
            } else {
                children.len().min(self.max_items)
            };
            let hidden = children.len() - shown;
            for (i, (child_key, child)) in children.into_iter().take(shown).enumerate() {
                let name = child_key.clone().unwrap_or_else(|| i.to_string());
                let last = i + 1 == shown && hidden == 0;
                self.visit(child, child_key, child_path(&path, &name), depth + 1, !last);
            }
            if hidden > 0 {
                self.rows.push(JsonRow {
                    depth: depth + 1,
                    path: path.clone(),
                    key: None,
                    kind: RowKind::More(hidden),
                    comma: false,
                });
            }
            self.rows.push(JsonRow { depth, path, key: None, kind: RowKind::Close(close), comma });
        }
    }
    let mut walk = Walk { max_depth, max_items, toggled, expanded, rows: Vec::new() };
    walk.visit(value, None, String::new(), 0, false);
    walk.rows
}

/// Color of a scalar written as JSON: strings, numbers, and `true`/`false`/`null`.
fn scalar_color(s: &Scheme, text: &str) -> Hsla {
    if text.starts_with('"') {
        s.tertiary
    } else if matches!(text, "true" | "false" | "null") {
        s.secondary
    } else {
        s.primary
    }
}

/// Indent of one level, in pixels.
const INDENT: f32 = 14.;

impl RenderOnce for JsonView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = *Theme::of(cx);
        let state = window.use_keyed_state(key(&self.id, "fold"), cx, |_, _| FoldState::default());
        let rows = {
            let fold = state.read(cx);
            json_rows(&self.value, self.max_depth, self.max_items, &fold.toggled, &fold.expanded)
        };
        let mut list = div().flex().flex_col().min_w_0();
        for (i, row) in rows.into_iter().enumerate() {
            let row_id = ElementId::NamedInteger(format!("{}-row", self.id).into(), i as u64);
            let mut el = div()
                .id(row_id)
                .flex()
                .items_center()
                .gap(px(4.))
                .min_w_0()
                .h(px(18.))
                .pl(px(4. + INDENT * row.depth as f32))
                .pr(px(8.))
                .rounded(px(4.))
                .whitespace_nowrap();
            let toggle = match &row.kind {
                RowKind::Open { count, .. } if *count > 0 => Some(false),
                RowKind::More(_) => Some(true),
                _ => None,
            };
            if let RowKind::Open { count, .. } = &row.kind {
                let is_open = matches!(row.kind, RowKind::Open { is_open: true, .. });
                el = el.child(
                    Icon::new("chevron-right")
                        .size(px(11.))
                        .color(if *count > 0 { s.on_surface_variant } else { s.outline_variant })
                        .rotate(if is_open { std::f32::consts::FRAC_PI_2 } else { 0. }),
                );
            } else if !matches!(row.kind, RowKind::More(_)) {
                el = el.child(div().flex_none().w(px(11.)));
            }
            if let Some(name) = &row.key {
                el = el.child(
                    div()
                        .flex_none()
                        .text_color(s.on_surface)
                        .child(format!("{}:", serde_json::to_string(name).unwrap_or_default())),
                );
            }
            let comma = if row.comma { "," } else { "" };
            el = match &row.kind {
                RowKind::Open { open, is_open: true, .. } => {
                    el.child(div().text_color(s.on_surface_variant).child(SharedString::from(open.to_string())))
                }
                RowKind::Open { open, close, count, is_open: false } => el
                    .child(
                        div()
                            .text_color(s.on_surface_variant)
                            .child(format!("{open}{}{close}{comma}", if *count > 0 { "…" } else { "" })),
                    )
                    .when(*count > 0, |d| {
                        d.child(div().text_color(s.outline).child(if *open == '{' {
                            format!("{count} keys")
                        } else {
                            format!("{count} items")
                        }))
                    }),
                RowKind::Scalar(text) => el.child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_color(scalar_color(&s, text))
                        .child(format!("{text}{comma}")),
                ),
                RowKind::Close(close) => {
                    el.child(div().text_color(s.on_surface_variant).child(format!("{close}{comma}")))
                }
                RowKind::More(n) => el.child(div().text_color(s.primary).child(format!("… {n} more"))),
            };
            if let Some(show_all) = toggle {
                let path = row.path.clone();
                let state = state.clone();
                el = el
                    .cursor_pointer()
                    .hover(|d| d.bg(s.surface_container_high))
                    .on_click(move |_, _, cx| {
                        state.update(cx, |fold, cx| {
                            let set = if show_all { &mut fold.expanded } else { &mut fold.toggled };
                            if !set.remove(&path) {
                                set.insert(path.clone());
                            }
                            cx.notify();
                        })
                    });
            }
            list = list.child(el);
        }
        div()
            .flex()
            .flex_col()
            .min_w_0()
            .rounded(px(12.))
            .bg(s.surface)
            .overflow_hidden()
            .child(
                div()
                    .id(child_id(&self.id, "scroll"))
                    .max_h(self.max_height)
                    .overflow_y_scroll()
                    .py(px(6.))
                    .px(px(4.))
                    .font_family(MONO_FONT_FAMILY)
                    .text_size(px(11.5))
                    .line_height(px(17.))
                    .text_color(s.on_surface)
                    .child(list),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rows(value: &Value, depth: usize, items: usize) -> Vec<JsonRow> {
        json_rows(value, depth, items, &HashSet::new(), &HashSet::new())
    }

    fn texts(rows: &[JsonRow]) -> Vec<String> {
        rows.iter()
            .map(|r| {
                let indent = "  ".repeat(r.depth);
                let key = r.key.as_ref().map(|k| format!("{k}: ")).unwrap_or_default();
                let comma = if r.comma { "," } else { "" };
                let body = match &r.kind {
                    RowKind::Open { open, is_open: true, .. } => open.to_string(),
                    RowKind::Open { open, close, count, .. } => format!("{open}{count}{close}{comma}"),
                    RowKind::Scalar(text) => format!("{text}{comma}"),
                    RowKind::Close(close) => format!("{close}{comma}"),
                    RowKind::More(n) => format!("+{n}"),
                };
                format!("{indent}{key}{body}")
            })
            .collect()
    }

    #[test]
    fn writes_scalars_as_json_with_commas_between_entries() {
        let value = json!({"name": "a\"b", "n": 1.5, "ok": true, "none": null});
        assert_eq!(
            texts(&rows(&value, 2, 50)),
            vec!["{", "  name: \"a\\\"b\",", "  n: 1.5,", "  ok: true,", "  none: null", "}"]
        );
    }

    #[test]
    fn folds_containers_past_max_depth() {
        let value = json!({"a": {"b": {"c": 1}}, "list": [1, 2]});
        assert_eq!(
            texts(&rows(&value, 1, 50)),
            vec!["{", "  a: {1},", "  list: [2]", "}"]
        );
        // Depth 0 folds the root; empty containers are never open.
        assert_eq!(texts(&rows(&value, 0, 50)), vec!["{2}"]);
        assert_eq!(texts(&rows(&json!({"e": {}}), 5, 50)), vec!["{", "  e: {0}", "}"]);
    }

    #[test]
    fn toggling_flips_the_default_of_one_container() {
        let value = json!({"a": {"b": 1}, "c": {"d": 2}});
        let toggled: HashSet<String> = ["/a".to_string()].into();
        let rows = json_rows(&value, 1, 50, &toggled, &HashSet::new());
        assert_eq!(texts(&rows), vec!["{", "  a: {", "    b: 1", "  },", "  c: {1}", "}"]);
        // Toggling the root closes it although it is within the depth.
        let toggled: HashSet<String> = [String::new()].into();
        assert_eq!(texts(&json_rows(&value, 3, 50, &toggled, &HashSet::new())), vec!["{2}"]);
    }

    #[test]
    fn long_containers_show_their_first_items_and_a_more_row() {
        let value = json!([1, 2, 3, 4, 5]);
        assert_eq!(texts(&rows(&value, 1, 2)), vec!["[", "  1,", "  2,", "  +3", "]"]);
        let expanded: HashSet<String> = [String::new()].into();
        let all = json_rows(&value, 1, 2, &HashSet::new(), &expanded);
        assert_eq!(all.len(), 7);
        assert!(!all[5].comma);
    }

    #[test]
    fn paths_escape_slashes_and_tildes() {
        assert_eq!(child_path("/a", "b/c~d"), "/a/b~1c~0d");
    }
}
