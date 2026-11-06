//! "Text field states" section: filled and outlined fields in every state.
//!
//! `M3_PAGE=0|1|2` shows one part only (states, counters and affixes, live validation) and
//! `M3_FOCUS=filled|outlined` focuses that "Focused" field on start (both for screenshots).

use gpui::{
    AnyElement, AppContext as _, Context, Entity, Hsla, IntoElement, ParentElement, Render,
    SharedString, Styled, Window, div, px,
};
use gpui_m3::{Button, Icon, TextArea, TextField, TextFieldEvent, TextFieldStyle, Theme};

use super::{column, demo, page, row};

const FIELD_WIDTH: f32 = 300.;

type Cell = (&'static str, Entity<TextField>);

fn field(
    cx: &mut Context<FieldDemos>,
    style: TextFieldStyle,
    text: &'static str,
    build: impl FnOnce(TextField) -> TextField,
) -> Entity<TextField> {
    cx.new(|cx| {
        let mut field = build(TextField::new(cx).style(style).label("Label"));
        if !text.is_empty() {
            field.set_text(text, cx);
        }
        field
    })
}

fn clear_button(_: &mut Window, cx: &mut gpui::App) -> AnyElement {
    Icon::new("x")
        .size(px(16.))
        .color(Theme::of(cx).on_surface_variant)
        .into_any_element()
}

/// The first part of the page: one field per state.
fn states(cx: &mut Context<FieldDemos>, style: TextFieldStyle) -> Vec<Cell> {
    let help = "Supporting text";
    let plain = |f: TextField| f.placeholder("Placeholder").supporting_text(help);
    vec![
        ("Default", field(cx, style, "", plain)),
        ("Focused", field(cx, style, "", plain)),
        ("Filled", field(cx, style, "Ada Lovelace", plain)),
        (
            "Error",
            field(cx, style, "ada@", |f| {
                plain(f)
                    .label("Email")
                    .error(Some("Enter a valid email".into()))
            }),
        ),
        (
            "Disabled",
            field(cx, style, "Ada Lovelace", |f| plain(f).disabled(true)),
        ),
        (
            "Read-only",
            field(cx, style, "Ada Lovelace", |f| plain(f).read_only(true)),
        ),
    ]
}

/// The second part: counters, limits, prefix, suffix and icons.
fn extras(cx: &mut Context<FieldDemos>, style: TextFieldStyle) -> Vec<Cell> {
    vec![
        (
            "Counter",
            field(cx, style, "Hello there", |f| {
                f.supporting_text("Up to 20 characters").max_length(20)
            }),
        ),
        (
            "Over the limit (flags)",
            field(cx, style, "This is too long", |f| {
                f.supporting_text("Up to 10 characters").max_length(10)
            }),
        ),
        (
            "Hard limit (blocks)",
            field(cx, style, "Try typing", |f| {
                f.supporting_text("Stops at 10 characters")
                    .max_length(10)
                    .hard_limit(true)
            }),
        ),
        (
            "Prefix",
            field(cx, style, "", |f| {
                f.label("Amount")
                    .placeholder("0.00")
                    .prefix_text("$")
                    .supporting_text("Before taxes")
            }),
        ),
        (
            "Suffix",
            field(cx, style, "", |f| {
                f.label("Weight")
                    .placeholder("0")
                    .suffix_text("kg")
                    .supporting_text("Without the box")
            }),
        ),
        (
            "Prefix, suffix and counter",
            field(cx, style, "12.50", |f| {
                f.label("Price")
                    .prefix_text("$")
                    .suffix_text("USD")
                    .max_length(8)
            }),
        ),
        (
            "Icons",
            field(cx, style, "", |f| {
                f.icon("search")
                    .placeholder("Search")
                    .trailing(clear_button)
                    .supporting_text("Leading and trailing icons")
            }),
        ),
        (
            "Icons and error",
            field(cx, style, "ada", |f| {
                f.icon("at")
                    .label("Email")
                    .trailing(clear_button)
                    .supporting_text("Leading and trailing icons")
                    .error(Some("Not an email".into()))
            }),
        ),
    ]
}

pub struct FieldDemos {
    part: Option<usize>,
    filled: Vec<Cell>,
    outlined: Vec<Cell>,
    extra_filled: Vec<Cell>,
    extra_outlined: Vec<Cell>,
    email: Entity<TextField>,
    email_filled: Entity<TextField>,
    area: Entity<TextArea>,
    toggled_error: bool,
    focus_pending: Option<String>,
}

impl FieldDemos {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let email = |cx: &mut Context<Self>, style| {
            let field = field(cx, style, "", |f| {
                f.label("Email")
                    .placeholder("name@example.com")
                    .supporting_text("We never share it")
            });
            cx.subscribe(&field, |_: &mut Self, field, event: &TextFieldEvent, cx| {
                if let TextFieldEvent::Changed(text) = event {
                    let error = (!text.is_empty() && !text.contains('@'))
                        .then(|| SharedString::from("Missing the @"));
                    field.update(cx, |f, cx| f.set_error(error, cx));
                }
            })
            .detach();
            field
        };
        Self {
            part: std::env::var("M3_PAGE").ok().and_then(|v| v.parse().ok()),
            filled: states(cx, TextFieldStyle::Filled),
            outlined: states(cx, TextFieldStyle::Outlined),
            extra_filled: extras(cx, TextFieldStyle::Filled),
            extra_outlined: extras(cx, TextFieldStyle::Outlined),
            email: email(cx, TextFieldStyle::Outlined),
            email_filled: email(cx, TextFieldStyle::Filled),
            area: cx.new(|cx| {
                let mut area = TextArea::new(cx)
                    .placeholder("Tell us more")
                    .min_rows(3)
                    .supporting_text("Visible to the team")
                    .max_length(60);
                area.set_text(
                    "This note is already longer than the sixty characters it may have",
                    cx,
                );
                area
            }),
            toggled_error: false,
            focus_pending: std::env::var("M3_FOCUS").ok(),
        }
    }
}

/// One row per state: the caption, then the filled and the outlined version.
fn grid(filled: &[Cell], outlined: &[Cell], caption: Hsla) -> gpui::Div {
    let mut grid = column().gap(px(16.));
    for ((name, f), (_, o)) in filled.iter().zip(outlined) {
        grid = grid.child(
            column()
                .gap(px(6.))
                .child(div().text_size(px(11.)).text_color(caption).child(*name))
                .child(
                    row()
                        .items_start()
                        .gap(px(24.))
                        .child(div().w(px(FIELD_WIDTH)).child(f.clone()))
                        .child(div().w(px(FIELD_WIDTH)).child(o.clone())),
                ),
        );
    }
    grid
}

impl Render for FieldDemos {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        if let Some(which) = self.focus_pending.take() {
            let cells = if which == "outlined" {
                &self.outlined
            } else {
                &self.filled
            };
            cells[1].1.read(cx).focus(window);
        }
        let show = |part: usize| self.part.is_none_or(|p| p == part);
        let caption = s.on_surface_variant;
        let mut out = page();

        if show(0) {
            out = out.child(demo(
                "States",
                "Filled (left) and outlined (right): default, focused, filled, error, disabled \
                 and read-only.",
                grid(&self.filled, &self.outlined, caption),
            ));
        }
        if show(1) {
            out = out.child(demo(
                "Counters, prefix, suffix and icons",
                "The counter flags past the limit; the hard limit stops typing. The supporting \
                 row keeps its height.",
                grid(&self.extra_filled, &self.extra_outlined, caption),
            ));
        }
        if show(2) {
            let this = cx.entity().downgrade();
            let toggle = Button::new("field-toggle-error", "Toggle error")
                .tonal()
                .on_click(move |_, _, cx| {
                    this.update(cx, |d, cx| {
                        d.toggled_error = !d.toggled_error;
                        let message = d
                            .toggled_error
                            .then(|| SharedString::from("This field is required"));
                        for f in [&d.email_filled, &d.email] {
                            f.update(cx, |f, cx| f.set_error(message.clone(), cx));
                        }
                        cx.notify();
                    })
                    .ok();
                });
            let invalid = self.area.read(cx).is_invalid();
            out = out
                .child(demo(
                    "Live validation",
                    "Type without an @ to get the error, or toggle it. The text below does not move.",
                    row()
                        .items_start()
                        .gap(px(24.))
                        .child(
                            column()
                                .w(px(FIELD_WIDTH))
                                .child(self.email_filled.clone())
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(s.on_surface_variant)
                                        .child("Content below the field"),
                                ),
                        )
                        .child(
                            column()
                                .w(px(FIELD_WIDTH))
                                .child(self.email.clone())
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(s.on_surface_variant)
                                        .child("Content below the field"),
                                )
                                .child(row().child(toggle)),
                        ),
                ))
                .child(demo(
                    "Text area",
                    "Counter and supporting line under the text; the host paints the box.",
                    div()
                        .w(px(FIELD_WIDTH))
                        .p(px(12.))
                        .rounded(px(12.))
                        .border_1()
                        .border_color(if invalid { s.error } else { s.outline_variant })
                        .child(self.area.clone()),
                ));
        }
        out
    }
}
