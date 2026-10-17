//! M3 Expressive motion: physics-based springs (not fixed-duration curves).
//!
//! The values are the official ones from `androidx.compose.material3` (`MotionScheme.expressive()`):
//! spatial springs for position, size and shape (with bounce) and effects springs for color and opacity (no bounce).
//! When the target changes mid-flight the spring keeps its velocity, as in Compose.
//!
//! GPUI does not support curves that overshoot 1 in `with_animation`, so components
//! animate with [`animate`], which keeps the spring state between frames.

use std::{
    cell::RefCell,
    collections::HashSet,
    time::{Duration, Instant},
};

use gpui::{App, ElementId, EntityId, Global, Hsla, Pixels, Rgba, Window, px};

/// Unit-mass spring: stiffness and damping ratio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub stiffness: f32,
    pub damping_ratio: f32,
}

impl Spring {
    pub const SPATIAL_FAST: Spring = Spring {
        stiffness: 800.,
        damping_ratio: 0.6,
    };
    pub const SPATIAL: Spring = Spring {
        stiffness: 380.,
        damping_ratio: 0.8,
    };
    pub const SPATIAL_SLOW: Spring = Spring {
        stiffness: 200.,
        damping_ratio: 0.8,
    };
    pub const EFFECTS_FAST: Spring = Spring {
        stiffness: 3800.,
        damping_ratio: 1.,
    };
    pub const EFFECTS: Spring = Spring {
        stiffness: 1600.,
        damping_ratio: 1.,
    };
    pub const EFFECTS_SLOW: Spring = Spring {
        stiffness: 800.,
        damping_ratio: 1.,
    };

    /// Displacement and velocity after `t` seconds, starting from `x0` (relative to the target) with velocity `v0`.
    pub fn solve(self, x0: f32, v0: f32, t: f32) -> (f32, f32) {
        let w0 = self.stiffness.sqrt();
        let z = self.damping_ratio;
        if z < 1. {
            let wd = w0 * (1. - z * z).sqrt();
            let decay = (-z * w0 * t).exp();
            let a = x0;
            let b = (v0 + z * w0 * x0) / wd;
            let (sin, cos) = (wd * t).sin_cos();
            let x = decay * (a * cos + b * sin);
            let v = decay * ((b * wd - z * w0 * a) * cos - (a * wd + z * w0 * b) * sin);
            (x, v)
        } else {
            let decay = (-w0 * t).exp();
            let b = v0 + w0 * x0;
            ((x0 + b * t) * decay, (v0 - w0 * b * t) * decay)
        }
    }

    /// Step response (0 → 1) after `t` seconds. May exceed 1 (bounce).
    pub fn step(self, t: f32) -> f32 {
        1. - self.solve(1., 0., t).0
    }

    /// Approximate time to settle (step response within 0.1 %).
    pub fn settle_time(self) -> Duration {
        let w0 = self.stiffness.sqrt();
        let z = self.damping_ratio;
        let seconds = if z < 1. {
            // The oscillation envelope starts at 1/sqrt(1 - z²), not at 1.
            (1000. / (1. - z * z).sqrt()).ln() / (z * w0)
        } else {
            // Critically damped: (1 + w0·t)·e^(-w0·t) drops below 0.1% at w0·t ≈ 9.23.
            9.25 / w0
        };
        Duration::from_secs_f32(seconds)
    }
}

/// Global motion settings. `time_scale` > 1 slows everything down (useful for reviewing animations).
#[derive(Clone, Copy, Debug)]
pub struct MotionSettings {
    pub time_scale: f32,
    pub reduced: bool,
}

impl Default for MotionSettings {
    fn default() -> Self {
        Self {
            time_scale: 1.,
            reduced: false,
        }
    }
}

impl Global for MotionSettings {}

impl MotionSettings {
    pub fn get(cx: &App) -> MotionSettings {
        cx.try_global::<MotionSettings>()
            .copied()
            .unwrap_or_default()
    }
}

/// Seconds elapsed since `start`, in animation time (applying `time_scale`).
pub fn elapsed(start: Instant, cx: &App) -> f32 {
    start.elapsed().as_secs_f32() / MotionSettings::get(cx).time_scale.max(0.01)
}

/// Shared clock for continuous animations (loading indicators, breathing shapes).
pub fn clock(cx: &App) -> f32 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    elapsed(*START.get_or_init(Instant::now), cx)
}

/// Who asks for frames: each call site of the motion helpers that keeps the window animating,
/// counted until [`take_frame_requests`] reads them. Lets an app find what keeps it redrawing.
/// Only with the `frame-stats` feature: otherwise nothing is counted.
#[cfg(feature = "frame-stats")]
static FRAME_REQUESTS: std::sync::Mutex<Vec<(&'static std::panic::Location<'static>, u32)>> =
    std::sync::Mutex::new(Vec::new());

#[track_caller]
pub(crate) fn request_frame(window: &mut Window) {
    count_frame_request();
    window.request_animation_frame();
}

/// Counts a frame request from the caller's call site (see [`take_frame_requests`]). The
/// timed redraws of [`continuous_at`] count too: they are frames as much as an animation
/// frame, and without them a window kept busy by spinners showed no culprit.
#[track_caller]
fn count_frame_request() {
    #[cfg(feature = "frame-stats")]
    {
        let at = std::panic::Location::caller();
        if let Ok(mut list) = FRAME_REQUESTS.lock() {
            match list.iter_mut().find(|(loc, _)| std::ptr::eq(*loc, at)) {
                Some((_, n)) => *n += 1,
                None => list.push((at, 1)),
            }
        }
    }
}

/// Shared grid for timed redraws, in ticks per second. Every [`tick_delay`] lands on it, so a
/// spinner at 30 fps, a caret at 4 fps and an app's coalesced updates fire at the same
/// instants and GPUI draws them in one frame instead of interleaving them (30 + 33 updates
/// per second used to be ~50 frames).
const TICK_GRID: f64 = 60.;

/// Start of the tick grid (not scaled by the motion settings: it paces timers, not values).
fn tick_origin() -> Instant {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *START.get_or_init(Instant::now)
}

/// Time until the next tick of a `fps` ticker on the shared grid. Use it to pace redraws that
/// do not need every frame (streamed text, clocks): `fps` ticks per second at most, in step
/// with the motion helpers' own timed redraws.
pub fn tick_delay(fps: f32) -> Duration {
    let now = tick_origin().elapsed().as_secs_f64();
    Duration::from_secs_f64(delay_to_tick(now, fps as f64))
}

/// Seconds from `now` (seconds since the grid origin) to the next multiple of `1 / fps`,
/// moved forward onto the [`TICK_GRID`] when that multiple is not on it. Never zero, so a
/// redraw that lands right on a tick waits for the next one instead of firing twice.
fn delay_to_tick(now: f64, fps: f64) -> f64 {
    let period = 1. / fps.clamp(0.1, TICK_GRID);
    let mut next = ((now / period).floor() + 1.) * period;
    if next - now < 1e-4 {
        // `now` is on a tick (float error put it just before): the next one.
        next += period;
    }
    // The epsilon keeps a tick that is already on the grid (k/30 → 2k/60) from rounding a
    // whole grid step up because of float error.
    let next = ((next * TICK_GRID) - 1e-6).ceil() / TICK_GRID;
    (next - now).max(1e-4)
}

/// The frame requests since the last call, as `file:line` and count, most frequent first.
/// Always empty without the `frame-stats` feature.
pub fn take_frame_requests() -> Vec<(String, u32)> {
    #[cfg(feature = "frame-stats")]
    {
        let Ok(mut list) = FRAME_REQUESTS.lock() else {
            return Vec::new();
        };
        let mut out: Vec<(String, u32)> = list
            .drain(..)
            .map(|(at, n)| (format!("{}:{}", at.file(), at.line()), n))
            .collect();
        out.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        out
    }
    #[cfg(not(feature = "frame-stats"))]
    Vec::new()
}

/// Clock for a continuous animation: requests the next frame and returns the current time.
/// With reduced motion it always returns 0 and requests no frames (the animation stays still).
#[track_caller]
pub fn continuous(window: &mut Window, cx: &App) -> f32 {
    if MotionSettings::get(cx).reduced {
        return 0.;
    }
    request_frame(window);
    clock(cx)
}

/// Frame rate cap for [`continuous_at`] while its window is not the active one.
const BACKGROUND_FPS: f32 = 12.;

thread_local! {
    /// Views that already have a redraw scheduled by [`continuous_at`] (avoids piling up timers).
    static PENDING: RefCell<HashSet<EntityId>> = RefCell::new(HashSet::new());
}

/// Like [`continuous`], but at `fps` frames per second. For slow ambient motion
/// (shapes that breathe or rotate slowly) 30 fps looks the same and costs half as much.
#[track_caller]
pub fn continuous_at(fps: f32, window: &mut Window, cx: &mut App) -> f32 {
    if MotionSettings::get(cx).reduced {
        return 0.;
    }
    // In a background window it keeps moving (it may still be on screen), but slower:
    // every tick redraws the whole view, not just the 16 px spinner.
    let fps = if window.is_window_active() { fps } else { fps.min(BACKGROUND_FPS) };
    if fps >= 60. {
        request_frame(window);
        return clock(cx);
    }
    let view = window.current_view();
    if PENDING.with(|p| p.borrow_mut().insert(view)) {
        count_frame_request();
        // On the shared grid: other timed redraws of this window land on the same frame.
        let timer = cx.background_executor().timer(tick_delay(fps.max(1.)));
        cx.spawn(async move |cx| {
            timer.await;
            PENDING.with(|p| p.borrow_mut().remove(&view));
            cx.update(|cx| cx.notify(view)).ok();
        })
        .detach();
    }
    clock(cx)
}

/// State of an `N`-channel value that follows its target with a spring.
#[derive(Clone, Debug)]
pub struct SpringValue<const N: usize> {
    spring: Spring,
    target: [f32; N],
    offset: [f32; N],
    velocity: [f32; N],
    start: Instant,
    settled: bool,
}

impl<const N: usize> SpringValue<N> {
    pub fn new(spring: Spring, value: [f32; N]) -> Self {
        Self {
            spring,
            target: value,
            offset: [0.; N],
            velocity: [0.; N],
            start: Instant::now(),
            settled: true,
        }
    }

    /// Current value and velocity.
    fn sample(&self, t: f32) -> ([f32; N], [f32; N]) {
        let mut value = self.target;
        let mut velocity = [0.; N];
        if !self.settled {
            for i in 0..N {
                let (x, v) = self.spring.solve(self.offset[i], self.velocity[i], t);
                value[i] = self.target[i] + x;
                velocity[i] = v;
            }
        }
        (value, velocity)
    }

    /// Advances to the current instant; returns the value and whether it is still moving.
    pub fn update(
        &mut self,
        target: [f32; N],
        spring: Spring,
        t: f32,
        now: Instant,
    ) -> ([f32; N], bool) {
        if target != self.target || spring != self.spring {
            let (value, velocity) = self.sample(t);
            for i in 0..N {
                self.offset[i] = value[i] - target[i];
                self.velocity[i] = velocity[i];
            }
            self.target = target;
            self.spring = spring;
            self.start = now;
            self.settled = false;
            return (value, true);
        }
        if self.settled {
            return (self.target, false);
        }
        let (value, velocity) = self.sample(t);
        let resting =
            (0..N).all(|i| (value[i] - self.target[i]).abs() < 0.002 && velocity[i].abs() < 0.05);
        if resting {
            self.settled = true;
            return (self.target, false);
        }
        (value, true)
    }

    /// Launches the value from `from` toward the current target (for entrance animations).
    pub fn kick(&mut self, from: [f32; N], now: Instant) {
        for (offset, (from, target)) in self.offset.iter_mut().zip(from.iter().zip(&self.target)) {
            *offset = from - target;
        }
        self.velocity = [0.; N];
        self.start = now;
        self.settled = false;
    }
}

/// Animates `target` with a spring and returns this frame's value.
///
/// `id` identifies the value across frames (must be unique within the view). The first frame
/// returns the target directly; use [`animate_from`] to animate the entrance.
#[track_caller]
pub fn animate<const N: usize>(
    id: impl Into<ElementId>,
    spring: Spring,
    target: [f32; N],
    window: &mut Window,
    cx: &mut App,
) -> [f32; N] {
    animate_inner(id.into(), spring, target, None, window, cx)
}

/// Like [`animate`], but the first frame starts from `from` (spring entrance).
#[track_caller]
pub fn animate_from<const N: usize>(
    id: impl Into<ElementId>,
    spring: Spring,
    from: [f32; N],
    target: [f32; N],
    window: &mut Window,
    cx: &mut App,
) -> [f32; N] {
    animate_inner(id.into(), spring, target, Some(from), window, cx)
}

#[track_caller]
fn animate_inner<const N: usize>(
    id: ElementId,
    spring: Spring,
    target: [f32; N],
    from: Option<[f32; N]>,
    window: &mut Window,
    cx: &mut App,
) -> [f32; N] {
    let settings = MotionSettings::get(cx);
    #[cfg(feature = "frame-stats")]
    let label = id.clone();
    let (value, moving) = element_state(id, window, |state: Option<SpringValue<N>>, _| {
        let mut s = state.unwrap_or_else(|| {
            let mut value = SpringValue::new(spring, target);
            if let Some(from) = from {
                value.kick(from, Instant::now());
            }
            value
        });
        if settings.reduced {
            return ((target, false), SpringValue::new(spring, target));
        }
        // Shortcut: a value at rest with the same target needs no computation.
        if s.settled && s.target == target && s.spring == spring {
            return ((target, false), s);
        }
        let now = Instant::now();
        let t = now.duration_since(s.start).as_secs_f32() / settings.time_scale.max(0.01);
        // Retargeted again right after starting: two live elements share this id and keep
        // flipping each other's target every frame, so the window never stops redrawing.
        #[cfg(feature = "frame-stats")]
        if (target != s.target || spring != s.spring) && t < 0.004 {
            log::warn!("spring {label:?} retargeted twice within a frame (duplicate element id)");
        }
        (s.update(target, spring, t, now), s)
    });
    if moving {
        request_frame(window);
    }
    value
}

/// Plain state tied to `id` that lives as long as the element is drawn in consecutive frames.
/// Lighter than `use_keyed_state` (creates no entity or observer).
fn element_state<S: 'static, R>(
    id: ElementId,
    window: &mut Window,
    f: impl FnOnce(Option<S>, &mut Window) -> (R, S),
) -> R {
    window.with_global_id(id, |global_id, window| {
        window.with_element_state(global_id, f)
    })
}

struct Triggered<const N: usize> {
    value: SpringValue<N>,
    active: bool,
}

/// Each time `active` goes from `false` to `true`, the value jumps to `from` and springs back
/// to `rest` (e.g. the icon that "pops" when a tab is selected).
#[track_caller]
pub fn animate_on_activate<const N: usize>(
    id: impl Into<ElementId>,
    spring: Spring,
    active: bool,
    from: [f32; N],
    rest: [f32; N],
    window: &mut Window,
    cx: &mut App,
) -> [f32; N] {
    let settings = MotionSettings::get(cx);
    let (value, moving) = element_state(id.into(), window, |state: Option<Triggered<N>>, _| {
        let mut s = state.unwrap_or_else(|| {
            let mut value = SpringValue::new(spring, rest);
            // Anything already active when it appears also animates its entrance.
            if active {
                value.kick(from, Instant::now());
            }
            Triggered { value, active }
        });
        if settings.reduced {
            return ((rest, false), s);
        }
        let now = Instant::now();
        if active && !s.active {
            s.value.kick(from, now);
        }
        s.active = active;
        if s.value.settled && s.value.target == rest {
            return ((rest, false), s);
        }
        let t = now.duration_since(s.value.start).as_secs_f32() / settings.time_scale.max(0.01);
        (s.value.update(rest, spring, t, now), s)
    });
    if moving {
        request_frame(window);
    }
    value
}

/// Entrance progress (0 → 1, with the spring's bounce) since the element appeared,
/// after `delay` seconds. Useful for staggering lists and menus.
#[track_caller]
pub fn entrance(
    id: impl Into<ElementId>,
    spring: Spring,
    delay: f32,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    let settings = MotionSettings::get(cx);
    let start = element_state(id.into(), window, |start: Option<Instant>, _| {
        let start = start.unwrap_or_else(Instant::now);
        (start, start)
    });
    if settings.reduced {
        return 1.;
    }
    let t = elapsed(start, cx) - delay;
    if t < spring.settle_time().as_secs_f32() {
        request_frame(window);
        spring.step(t.max(0.))
    } else {
        1.
    }
}

/// Shortcut for animating a single number.
#[track_caller]
pub fn animate_f32(
    id: impl Into<ElementId>,
    spring: Spring,
    target: f32,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    animate(id, spring, [target], window, cx)[0]
}

/// Shortcut for animating a length in pixels.
#[track_caller]
pub fn animate_px(
    id: impl Into<ElementId>,
    spring: Spring,
    target: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> Pixels {
    px(animate(id, spring, [f32::from(target)], window, cx)[0])
}

/// Shortcut for animating a color (interpolated in sRGB; no bounce when using an effects spring).
#[track_caller]
pub fn animate_color(
    id: impl Into<ElementId>,
    spring: Spring,
    target: Hsla,
    window: &mut Window,
    cx: &mut App,
) -> Hsla {
    let c = Rgba::from(target);
    let [r, g, b, a] = animate(id, spring, [c.r, c.g, c.b, c.a], window, cx);
    Rgba {
        r: r.clamp(0., 1.),
        g: g.clamp(0., 1.),
        b: b.clamp(0., 1.),
        a: a.clamp(0., 1.),
    }
    .into()
}

/// Cubic Bézier curve (like CSS `cubic-bezier()`).
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> impl Fn(f32) -> f32 + Copy {
    move |x: f32| {
        if x <= 0. {
            return 0.;
        }
        if x >= 1. {
            return 1.;
        }
        let bez = |t: f32, a: f32, b: f32| {
            3. * a * t * (1. - t).powi(2) + 3. * b * t * t * (1. - t) + t.powi(3)
        };
        let (mut lo, mut hi) = (0f32, 1f32);
        let mut t = x;
        for _ in 0..24 {
            let v = bez(t, x1, x2);
            if (v - x).abs() < 1e-5 {
                break;
            }
            if v < x {
                lo = t;
            } else {
                hi = t;
            }
            t = (lo + hi) / 2.;
        }
        bez(t, y1, y2)
    }
}

/// Stable child key for a component's animated values.
/// Opacity for pulsing text ("Working…"): 1 → 0.45 → 1 every `period` seconds, at 30 fps.
/// With reduced motion it stays at 1.
#[track_caller]
pub fn pulse(period: f32, window: &mut Window, cx: &mut App) -> f32 {
    if MotionSettings::get(cx).reduced {
        return 1.;
    }
    let t = continuous_at(30., window, cx);
    0.725 + 0.275 * (t / period.max(0.1) * std::f32::consts::TAU).cos()
}

/// Time-based replay: returns the seconds elapsed since `generation` last changed, or `None`
/// once `duration` has passed (and on the first frame, so nothing plays on mount). Use it for
/// one-shot effects such as the send button's launch or a tab's content fading in.
#[track_caller]
pub fn replay(
    id: impl Into<ElementId>,
    generation: u64,
    duration: f32,
    window: &mut Window,
    cx: &mut App,
) -> Option<f32> {
    let settings = MotionSettings::get(cx);
    let start = element_state(
        id.into(),
        window,
        |state: Option<(u64, Option<Instant>)>, _| {
            let state = match state {
                Some((g, start)) if g == generation => (g, start),
                Some(_) => (generation, Some(Instant::now())),
                None => (generation, None),
            };
            (state.1, state)
        },
    );
    if settings.reduced {
        return None;
    }
    let t = elapsed(start?, cx);
    (t < duration).then(|| {
        request_frame(window);
        t
    })
}

/// Seconds since `active` last went from `false` to `true`, while fewer than `duration` have
/// passed; `None` otherwise (and when it starts active, so nothing plays on mount). Use it for
/// one-shot effects tied to a state edge, such as a shape that breathes once when hovered.
#[track_caller]
pub fn play_on_rise(
    id: impl Into<ElementId>,
    active: bool,
    duration: f32,
    window: &mut Window,
    cx: &mut App,
) -> Option<f32> {
    let settings = MotionSettings::get(cx);
    let start = element_state(
        id.into(),
        window,
        |state: Option<(bool, Option<Instant>)>, _| {
            let state = match state {
                Some((false, _)) if active => (true, Some(Instant::now())),
                Some((_, start)) => (active, start),
                None => (active, None),
            };
            (state.1, state)
        },
    );
    if settings.reduced {
        return None;
    }
    let t = elapsed(start?, cx);
    (t < duration).then(|| {
        request_frame(window);
        t
    })
}

/// Keeps something on screen while it animates out.
///
/// Returns `Some(progress)` while the content should be drawn: `1.` while `show` is true, then
/// falling to `0.` with the fast effects spring once `show` turns false (exits are shorter than
/// entrances). Returns `None` when the exit has finished or when it was never shown. Call it
/// every frame, shown or not, with the same `id`:
///
/// ```ignore
/// div().when_some(motion::presence("offer", self.offer.is_some(), window, cx), |d, p| {
///     d.child(banner(&self.last_offer).opacity(p))
/// })
/// ```
///
/// Keep the data of the leaving content (the last offer, the last menu) until it returns `None`.
#[track_caller]
pub fn presence(
    id: impl Into<ElementId>,
    show: bool,
    window: &mut Window,
    cx: &mut App,
) -> Option<f32> {
    let settings = MotionSettings::get(cx);
    let spring = Spring::EFFECTS_FAST;
    let (value, moving) = element_state(id.into(), window, |state: Option<Presence>, _| {
        let mut s = state.unwrap_or(Presence {
            value: SpringValue::new(spring, [if show { 1. } else { 0. }]),
            mounted: show,
        });
        let now = Instant::now();
        if show {
            s.mounted = true;
            s.value = SpringValue::new(spring, [1.]);
            return ((Some(1.), false), s);
        }
        if !s.mounted || settings.reduced {
            s.mounted = false;
            return ((None, false), s);
        }
        if s.value.target != [0.] {
            s.value.update([0.], spring, 0., now);
        }
        let t = now.duration_since(s.value.start).as_secs_f32() / settings.time_scale.max(0.01);
        let ([value], moving) = s.value.update([0.], spring, t, now);
        if !moving || value <= 0.01 {
            s.mounted = false;
            return ((None, false), s);
        }
        ((Some(value.clamp(0., 1.)), true), s)
    });
    if moving {
        request_frame(window);
    }
    value
}

struct Presence {
    value: SpringValue<1>,
    mounted: bool,
}

/// What an expanding section should draw this frame. See [`expand`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Expand {
    /// Closed and at rest: draw nothing.
    Hidden,
    /// Open and at rest: draw the content at its natural height (it may keep growing).
    Natural,
    /// Opening or closing: draw the content clipped to this height.
    Clipped(Pixels),
}

struct ExpandState {
    open: bool,
    value: Option<SpringValue<1>>,
}

/// Height animation for content that opens and closes (an expandable card's body).
///
/// `measured` is the content's natural height, measured in the previous frame (0 if unknown).
/// When `open` changes, the height springs between 0 and `measured`; at rest it returns
/// [`Expand::Natural`] or [`Expand::Hidden`], so open content can change size freely. Nothing
/// animates on the first frame (content that starts open is just shown).
#[track_caller]
pub fn expand(
    id: impl Into<ElementId>,
    open: bool,
    measured: Pixels,
    spring: Spring,
    window: &mut Window,
    cx: &mut App,
) -> Expand {
    let settings = MotionSettings::get(cx);
    let measured = f32::from(measured).max(0.);
    let (result, moving) = element_state(id.into(), window, |state: Option<ExpandState>, _| {
        let mut s = state.unwrap_or(ExpandState { open, value: None });
        let rest = if open {
            Expand::Natural
        } else {
            Expand::Hidden
        };
        if settings.reduced {
            s.open = open;
            s.value = None;
            return ((rest, false), s);
        }
        let now = Instant::now();
        if s.open != open {
            s.open = open;
            if s.value.is_none() {
                let from = if open { 0. } else { measured };
                s.value = Some(SpringValue::new(spring, [from]));
            }
        }
        let Some(value) = s.value.as_mut() else {
            return ((rest, false), s);
        };
        // Opening content that was never measured: lay it out clipped to 0 for one frame.
        if open && measured <= 0. {
            return ((Expand::Clipped(px(0.)), true), s);
        }
        let target = if open { measured } else { 0. };
        let t = now.duration_since(value.start).as_secs_f32() / settings.time_scale.max(0.01);
        let ([h], moving) = value.update([target], spring, t, now);
        if !moving {
            s.value = None;
            return ((rest, false), s);
        }
        ((Expand::Clipped(px(h.max(0.))), true), s)
    });
    if moving {
        request_frame(window);
    }
    result
}

pub fn key(id: &ElementId, name: &'static str) -> ElementId {
    ElementId::NamedChild(Box::new(id.clone()), name.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_land_on_a_shared_grid() {
        let at = |now: f64, fps: f64| now + delay_to_tick(now, fps);
        let on_grid = |t: f64| ((t * 60.) - (t * 60.).round()).abs() < 1e-6;
        // 30 fps: the next multiple of 1/30, always ahead of `now`.
        assert!((at(0.010, 30.) - 1. / 30.).abs() < 1e-9);
        assert!((at(1. / 30., 30.) - 2. / 30.).abs() < 1e-9);
        // A caret at 4 fps and a spinner at 30 fps tick on the same 60 Hz grid.
        assert!((at(0.20, 4.) - 0.25).abs() < 1e-9);
        for i in 0..200 {
            let now = i as f64 * 0.0137;
            assert!(on_grid(at(now, 30.)) && on_grid(at(now, 4.)) && on_grid(at(now, 12.)));
        }
        // A rate that is not on the grid moves forward onto it.
        let t = at(0.0, 24.);
        assert!(t >= 1. / 24. && on_grid(t));
        // Never fires twice on the same tick, never waits more than a period plus a grid step.
        for i in 0..1000 {
            let now = i as f64 * 0.00731;
            let delay = delay_to_tick(now, 30.);
            assert!(delay > 0. && delay <= 1. / 30. + 1. / 60. + 1e-3);
        }
    }

    #[test]
    fn springs_settle_on_target() {
        for spring in [
            Spring::SPATIAL_FAST,
            Spring::SPATIAL,
            Spring::SPATIAL_SLOW,
            Spring::EFFECTS_FAST,
            Spring::EFFECTS,
            Spring::EFFECTS_SLOW,
        ] {
            let end = spring.step(spring.settle_time().as_secs_f32());
            assert!((end - 1.).abs() < 0.002, "{spring:?} ends at {end}");
        }
    }

    #[test]
    fn spatial_springs_bounce_and_effects_do_not() {
        let peak = |s: Spring| {
            (0..400)
                .map(|i| s.step(i as f32 / 400.))
                .fold(0f32, f32::max)
        };
        // Damping 0.6 overshoots about 9.5%, like the reference CSS curve (1.093).
        assert!((peak(Spring::SPATIAL_FAST) - 1.095).abs() < 0.005);
        assert!(peak(Spring::EFFECTS) <= 1.0001);
    }

    #[test]
    fn retargeting_from_rest_starts_from_the_old_value() {
        let now = Instant::now();
        let mut value = SpringValue::new(Spring::SPATIAL, [120.]);
        let (start, moving) = value.update([0.], Spring::SPATIAL, 0., now);
        assert!(moving);
        assert!((start[0] - 120.).abs() < 1e-4);
        let (later, _) = value.update([0.], Spring::SPATIAL, 0.1, now);
        assert!(later[0] < 120. && later[0] > 0.);
    }

    #[test]
    fn retargeting_keeps_position() {
        let now = Instant::now();
        let mut value = SpringValue::new(Spring::SPATIAL, [0.]);
        value.update([10.], Spring::SPATIAL, 0., now);
        let (mid, _) = value.update([10.], Spring::SPATIAL, 0.1, now);
        // Changing the target mid-flight continues from where the value is, without a jump.
        let (after, moving) = value.update([0.], Spring::SPATIAL, 0.1, now);
        assert!(moving);
        assert!((after[0] - mid[0]).abs() < 1e-4);
    }
}
