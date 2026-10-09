//! Measures the cost of a frame: `bench_frame shape` animates a shape (path); `bench_frame quad` animates a rectangle.
use gpui::{
    App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px,
    size,
};
use gpui_m3::{Shape, ShapeName, Theme};

struct Bench(bool);

impl Render for Bench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);
        let t = gpui_m3::motion::continuous(window, cx);
        let el = if self.0 {
            Shape::new(ShapeName::Cookie9)
                .size(px(30.))
                .color(s.primary)
                .spin(4.)
                .into_any_element()
        } else {
            div()
                .size(px(30.))
                .ml(px(20. + 10. * (t * 3.).sin()))
                .bg(s.primary)
                .into_any_element()
        };
        div().size_full().bg(s.surface).p_8().child(el)
    }
}

fn main() {
    let shape = std::env::args().nth(1).as_deref() == Some("shape");
    Application::new()
        .with_assets(gpui_m3::Assets)
        .run(move |cx: &mut App| {
            gpui_m3::init(cx);
            let bounds = Bounds::centered(None, size(px(1280.), px(860.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Bench(shape)),
            )
            .unwrap();
        });
}
