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

/// Clock for a continuous animation: requests the next frame and returns the current time.
/// With reduced motion it always returns 0 and requests no frames (the animation stays still).
pub fn continuous(window: &mut Window, cx: &App) -> f32 {
    if MotionSettings::get(cx).reduced {
        return 0.;
    }
    window.request_animation_frame();
    clock(cx)
}

thread_local! {
    /// Views that already have a redraw scheduled by [`continuous_at`] (avoids piling up timers).
    static PENDING: RefCell<HashSet<EntityId>> = RefCell::new(HashSet::new());
}

/// Like [`continuous`], but at `fps` frames per second. For slow ambient motion
/// (shapes that breathe or rotate slowly) 30 fps looks the same and costs half as much.
pub fn continuous_at(fps: f32, window: &mut Window, cx: &mut App) -> f32 {
    if MotionSettings::get(cx).reduced {
        return 0.;
    }
    if fps >= 60. {
        window.request_animation_frame();
        return clock(cx);
    }
    let view = window.current_view();
    if PENDING.with(|p| p.borrow_mut().insert(view)) {
        let timer = cx
            .background_executor()
            .timer(Duration::from_secs_f32(1. / fps.max(1.)));
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

fn animate_inner<const N: usize>(
    id: ElementId,
    spring: Spring,
    target: [f32; N],
    from: Option<[f32; N]>,
    window: &mut Window,
    cx: &mut App,
) -> [f32; N] {
    let settings = MotionSettings::get(cx);
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
        (s.update(target, spring, t, now), s)
    });
    if moving {
        window.request_animation_frame();
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
        window.request_animation_frame();
    }
    value
}

/// Entrance progress (0 → 1, with the spring's bounce) since the element appeared,
/// after `delay` seconds. Useful for staggering lists and menus.
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
        window.request_animation_frame();
        spring.step(t.max(0.))
    } else {
        1.
    }
}

/// Shortcut for animating a single number.
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
pub fn key(id: &ElementId, name: &'static str) -> ElementId {
    ElementId::NamedChild(Box::new(id.clone()), name.into())
}

#[cfg(test)]
mod tests {
    use super::*;

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
