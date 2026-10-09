use gpui::{
    App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px,
    size,
};
use gpui_m3::{Button, Fab, IconButton, Theme};

struct Smoke;

impl Render for Smoke {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        div()
            .size_full()
            .bg(s.surface)
            .text_color(s.on_surface)
            .font_family(gpui_m3::theme::FONT_FAMILY)
            .flex()
            .flex_col()
            .gap_4()
            .p_8()
            .child(
                div()
                    .text_size(px(32.))
                    .font_weight(gpui::FontWeight(300.))
                    .child("Roboto 300"),
            )
            .child(
                div()
                    .text_size(px(32.))
                    .font_weight(gpui::FontWeight(700.))
                    .child("Roboto 700"),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(Button::new("a", "Filled").filled())
                    .child(Button::new("b", "Tonal").icon("plus"))
                    .child(Button::new("c", "Outlined").outlined())
                    .child(
                        IconButton::new("d", "up")
                            .filled()
                            .hover_morph(true)
                            .size(px(36.)),
                    )
                    .child(Fab::new("e", "pen"))
                    .child(Fab::new("f", "plus").label("New chat")),
            )
    }
}

fn main() {
    Application::new()
        .with_assets(gpui_m3::Assets)
        .run(|cx: &mut App| {
            gpui_m3::init(cx);
            let bounds = Bounds::centered(None, size(px(900.), px(500.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Smoke),
            )
            .unwrap();
            cx.activate(true);
        });
}
