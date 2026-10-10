//! Theme changes in motion: colors that blend into the new scheme ([`animate_scheme`]) and a
//! circular reveal from the pointer ([`theme_reveal`] with a [`ThemeReveal`] layer).

use std::time::{Duration, Instant};

use gpui::{
    App, Global, Hsla, IntoElement, ParentElement, Pixels, Point, RenderOnce, Styled, Window,
    anchored, deferred, div, point, px,
};

use crate::motion::{MotionSettings, Spring, cubic_bezier, elapsed};
use crate::theme::{Scheme, Theme, alpha};

/// How long [`animate_scheme`] blends the colors (the reference design: 0.45 s).
pub const SCHEME_BLEND: f32 = 0.45;

/// Generation of the running blend, so a newer change cancels an older one.
#[derive(Default)]
struct SchemeBlend(u64);

impl Global for SchemeBlend {}

/// Changes the theme to `scheme`, blending every color from the current scheme over 0.45 s
/// (M3 emphasized easing) instead of switching at once. Use it for mode and accent changes.
/// With reduced motion it switches immediately, like [`Theme::set_scheme`].
pub fn animate_scheme(scheme: Scheme, dark: bool, cx: &mut App) {
    let settings = MotionSettings::get(cx);
    let from = Theme::get(cx).scheme;
    let generation = {
        let blend = cx.default_global::<SchemeBlend>();
        blend.0 += 1;
        blend.0
    };
    if settings.reduced || from == scheme {
        Theme::set_scheme(scheme, dark, cx);
        return;
    }
    let start = Instant::now();
    let ease = cubic_bezier(0.2, 0., 0., 1.);
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
            let done = cx
                .update(|cx| {
                    if cx.global::<SchemeBlend>().0 != generation {
                        return true;
                    }
                    let t = (elapsed(start, cx) / SCHEME_BLEND).min(1.);
                    let current = from.lerp(&scheme, ease(t));
                    cx.set_global(Theme {
                        scheme: current,
                        dark,
                        seed: None,
                    });
                    cx.refresh_windows();
                    t >= 1.
                })
                .unwrap_or(true);
            if done {
                break;
            }
        }
    })
    .detach();
}

/// [`animate_scheme`] for a mode and an optional seed color, like [`Theme::set`] (keeps the seed).
pub fn animate_theme(dark: bool, seed: Option<Hsla>, cx: &mut App) {
    let target = Theme::new(dark, seed);
    animate_scheme(target.scheme, dark, cx);
    // Remember the seed once the blend lands (animate_scheme stores `None`).
    if seed.is_some() {
        let generation = cx.global::<SchemeBlend>().0;
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_secs_f32(SCHEME_BLEND + 0.1))
                .await;
            cx.update(|cx| {
                let current = cx.global::<SchemeBlend>().0;
                let theme = Theme::get(cx);
                if current == generation && theme.scheme == target.scheme {
                    cx.set_global(Theme { seed, ..theme });
                }
            })
            .ok();
        })
        .detach();
    }
}

#[derive(Clone, Copy)]
struct Reveal {
    origin: Point<Pixels>,
    radius: f32,
    color: Hsla,
    scheme: Scheme,
    dark: bool,
    start: Instant,
    /// When the circle covered the window and the new theme was applied.
    swapped: Option<Instant>,
}

#[derive(Default)]
struct RevealState(Option<Reveal>);

impl Global for RevealState {}

/// Seconds the reveal layer takes to fade once the new theme is in place.
const REVEAL_FADE: f32 = 0.2;

/// Circular theme change from the pointer (the reference design's M3 view transition): a circle of the new
/// background grows from where the user clicked with the slow spatial spring; when it covers
/// the window the new scheme is applied and the circle fades away.
///
/// GPUI cannot capture the previous frame, so unlike the web version the old content is covered
/// by the circle instead of being revealed through it. Render a [`ThemeReveal`] once at the root
/// of every window that should show it. With reduced motion the theme changes at once.
pub fn theme_reveal(scheme: Scheme, dark: bool, window: &mut Window, cx: &mut App) {
    if MotionSettings::get(cx).reduced {
        Theme::set_scheme(scheme, dark, cx);
        return;
    }
    let origin = window.mouse_position();
    let size = window.viewport_size();
    let (x, y) = (f32::from(origin.x), f32::from(origin.y));
    let (w, h) = (f32::from(size.width), f32::from(size.height));
    let radius = x.max(w - x).hypot(y.max(h - y));
    cx.set_global(RevealState(Some(Reveal {
        origin,
        radius,
        color: scheme.surface_container,
        scheme,
        dark,
        start: Instant::now(),
        swapped: None,
    })));
    window.refresh();
}

/// The layer that draws [`theme_reveal`]. Render it once at the root of the window, after the
/// content; it draws nothing while no reveal is running.
#[derive(IntoElement, Default)]
pub struct ThemeReveal {
    color: Option<Hsla>,
}

impl ThemeReveal {
    pub fn new() -> Self {
        Self::default()
    }

    /// Color of the growing circle (the new scheme's `surface_container` by default: the
    /// usual window background).
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for ThemeReveal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Some(mut reveal) = cx.try_global::<RevealState>().and_then(|r| r.0) else {
            return div().into_any_element();
        };
        let spring = Spring::SPATIAL_SLOW;
        let t = elapsed(reveal.start, cx);
        let progress = spring.step(t);
        let mut opacity = 1.;
        match reveal.swapped {
            None if progress >= 0.999 => {
                // Outside the frame being drawn, so every window (cached views too) refreshes.
                let (scheme, dark) = (reveal.scheme, reveal.dark);
                cx.defer(move |cx| Theme::set_scheme(scheme, dark, cx));
                reveal.swapped = Some(Instant::now());
                cx.set_global(RevealState(Some(reveal)));
            }
            None => {}
            Some(at) => {
                let fade = elapsed(at, cx) / REVEAL_FADE;
                if fade >= 1. {
                    cx.set_global(RevealState(None));
                    return div().into_any_element();
                }
                opacity = 1. - fade;
            }
        }
        crate::motion::request_frame(window);
        let r = (reveal.radius * progress.max(0.)).max(0.);
        let color = alpha(self.color.unwrap_or(reveal.color), opacity);
        let viewport = window.viewport_size();
        deferred(
            anchored().position(point(px(0.), px(0.))).child(
                div()
                    .w(viewport.width)
                    .h(viewport.height)
                    .overflow_hidden()
                    .child(
                        div()
                            .absolute()
                            .left(reveal.origin.x - px(r))
                            .top(reveal.origin.y - px(r))
                            .size(px(r * 2.))
                            .rounded(px(r))
                            .bg(color),
                    ),
            ),
        )
        .with_priority(2)
        .into_any_element()
    }
}
