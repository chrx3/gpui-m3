//! Material 3 Expressive color, typography and corner scale.
//!
//! The baseline schemes use the same tokens as the reference design (the M3 reference purple).
//! [`Scheme::from_seed`] generates a dynamic (tonal spot) scheme from any color.

use std::str::FromStr;

use gpui::{App, Global, Hsla, Pixels, Rgba, px, rgb};
use material_colors::{color::Argb, hct::Hct, scheme::variant::SchemeTonalSpot};

/// Font family registered by [`crate::init`].
pub const FONT_FAMILY: &str = "Roboto";

/// Monospace family for shortcuts and code (the platform's own; none is bundled).
#[cfg(target_os = "windows")]
pub const MONO_FONT_FAMILY: &str = "Consolas";
#[cfg(target_os = "macos")]
pub const MONO_FONT_FAMILY: &str = "Menlo";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub const MONO_FONT_FAMILY: &str = "DejaVu Sans Mono";

/// M3 corner scale (0 · 4 · 8 · 12 · 16 · 20 · 28 · 32 · 48 · full).
pub mod corner {
    use gpui::{Pixels, px};

    pub const NONE: Pixels = px(0.);
    pub const XS: Pixels = px(4.);
    pub const S: Pixels = px(8.);
    pub const M: Pixels = px(12.);
    pub const L: Pixels = px(16.);
    pub const LI: Pixels = px(20.);
    pub const XL: Pixels = px(28.);
    pub const XLI: Pixels = px(32.);
    pub const XXL: Pixels = px(48.);
    /// Fully round: GPUI clamps a radius to half the shorter side.
    pub const FULL: Pixels = px(9999.);
}

/// M3 color roles. Names follow the spec; the reference design's CSS token is in parentheses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scheme {
    /// (--accent)
    pub primary: Hsla,
    /// (--on-accent)
    pub on_primary: Hsla,
    /// (--accent-soft)
    pub primary_container: Hsla,
    /// (--on-accent-soft)
    pub on_primary_container: Hsla,
    /// (--secondary)
    pub secondary_container: Hsla,
    /// (--on-secondary)
    pub on_secondary_container: Hsla,
    /// (--tertiary)
    pub tertiary: Hsla,
    /// (--tertiary-soft)
    pub tertiary_container: Hsla,
    /// (--on-tertiary-soft)
    pub on_tertiary_container: Hsla,
    /// (--pane)
    pub surface: Hsla,
    /// (--bg, --raised)
    pub surface_container: Hsla,
    /// (--control, --hover)
    pub surface_container_high: Hsla,
    /// (--control-2)
    pub surface_container_highest: Hsla,
    /// (--text)
    pub on_surface: Hsla,
    /// (--muted)
    pub on_surface_variant: Hsla,
    /// (--outline, --faint)
    pub outline: Hsla,
    /// (--border)
    pub outline_variant: Hsla,
    /// (--bad)
    pub error: Hsla,
    pub error_container: Hsla,
    pub on_error_container: Hsla,
    /// (--ok)
    pub success: Hsla,
    /// (--warn)
    pub warning: Hsla,
    /// Shadow color for menus and FABs.
    pub shadow: Hsla,
}

impl Scheme {
    pub fn baseline_light() -> Self {
        Self {
            primary: rgb(0x6750a4).into(),
            on_primary: rgb(0xffffff).into(),
            primary_container: rgb(0xeaddff).into(),
            on_primary_container: rgb(0x4f378b).into(),
            secondary_container: rgb(0xe8def8).into(),
            on_secondary_container: rgb(0x4a4458).into(),
            tertiary: rgb(0x7d5260).into(),
            tertiary_container: rgb(0xffd8e4).into(),
            on_tertiary_container: rgb(0x633b48).into(),
            surface: rgb(0xfef7ff).into(),
            surface_container: rgb(0xf3edf7).into(),
            surface_container_high: rgb(0xece6f0).into(),
            surface_container_highest: rgb(0xe6e0e9).into(),
            on_surface: rgb(0x1d1b20).into(),
            on_surface_variant: rgb(0x49454f).into(),
            outline: rgb(0x79747e).into(),
            outline_variant: rgb(0xcac4d0).into(),
            error: rgb(0xb3261e).into(),
            error_container: rgb(0xf9dedc).into(),
            on_error_container: rgb(0x410e0b).into(),
            success: rgb(0x146c2e).into(),
            warning: rgb(0x8b5000).into(),
            shadow: Hsla {
                h: 0.,
                s: 0.,
                l: 0.,
                a: 0.12,
            },
        }
    }

    pub fn baseline_dark() -> Self {
        Self {
            primary: rgb(0xd0bcff).into(),
            on_primary: rgb(0x381e72).into(),
            primary_container: rgb(0x4f378b).into(),
            on_primary_container: rgb(0xeaddff).into(),
            secondary_container: rgb(0x4a4458).into(),
            on_secondary_container: rgb(0xe8def8).into(),
            tertiary: rgb(0xefb8c8).into(),
            tertiary_container: rgb(0x633b48).into(),
            on_tertiary_container: rgb(0xffd8e4).into(),
            surface: rgb(0x141218).into(),
            surface_container: rgb(0x211f26).into(),
            surface_container_high: rgb(0x2b2930).into(),
            surface_container_highest: rgb(0x36343b).into(),
            on_surface: rgb(0xe6e0e9).into(),
            on_surface_variant: rgb(0xcac4d0).into(),
            outline: rgb(0x938f99).into(),
            outline_variant: rgb(0x49454f).into(),
            error: rgb(0xf2b8b5).into(),
            error_container: rgb(0x8c1d18).into(),
            on_error_container: rgb(0xf9dedc).into(),
            success: rgb(0xa8dab5).into(),
            warning: rgb(0xffb868).into(),
            shadow: Hsla {
                h: 0.,
                s: 0.,
                l: 0.,
                a: 0.4,
            },
        }
    }

    /// Every role mixed between `self` (at `t = 0`) and `other` (at `t = 1`), in sRGB.
    pub fn lerp(&self, other: &Scheme, t: f32) -> Scheme {
        let m = |a: Hsla, b: Hsla| mix(b, a, t.clamp(0., 1.));
        Scheme {
            primary: m(self.primary, other.primary),
            on_primary: m(self.on_primary, other.on_primary),
            primary_container: m(self.primary_container, other.primary_container),
            on_primary_container: m(self.on_primary_container, other.on_primary_container),
            secondary_container: m(self.secondary_container, other.secondary_container),
            on_secondary_container: m(self.on_secondary_container, other.on_secondary_container),
            tertiary: m(self.tertiary, other.tertiary),
            tertiary_container: m(self.tertiary_container, other.tertiary_container),
            on_tertiary_container: m(self.on_tertiary_container, other.on_tertiary_container),
            surface: m(self.surface, other.surface),
            surface_container: m(self.surface_container, other.surface_container),
            surface_container_high: m(self.surface_container_high, other.surface_container_high),
            surface_container_highest: m(
                self.surface_container_highest,
                other.surface_container_highest,
            ),
            on_surface: m(self.on_surface, other.on_surface),
            on_surface_variant: m(self.on_surface_variant, other.on_surface_variant),
            outline: m(self.outline, other.outline),
            outline_variant: m(self.outline_variant, other.outline_variant),
            error: m(self.error, other.error),
            error_container: m(self.error_container, other.error_container),
            on_error_container: m(self.on_error_container, other.on_error_container),
            success: m(self.success, other.success),
            warning: m(self.warning, other.warning),
            shadow: m(self.shadow, other.shadow),
        }
    }

    /// Dynamic "tonal spot" scheme (2021 spec) from a seed color.
    pub fn from_seed(seed: Hsla, dark: bool) -> Self {
        let rgba = Rgba::from(seed);
        let argb = Argb::new(
            255,
            (rgba.r * 255.).round() as u8,
            (rgba.g * 255.).round() as u8,
            (rgba.b * 255.).round() as u8,
        );
        let s = SchemeTonalSpot::new(Hct::new(argb), dark, None).scheme;
        let base = if dark {
            Self::baseline_dark()
        } else {
            Self::baseline_light()
        };
        let c = |a: Argb| -> Hsla { rgb(u32::from_str_radix(&a.to_hex(), 16).unwrap_or(0)).into() };
        Self {
            primary: c(s.primary()),
            on_primary: c(s.on_primary()),
            primary_container: c(s.primary_container()),
            on_primary_container: c(s.on_primary_container()),
            secondary_container: c(s.secondary_container()),
            on_secondary_container: c(s.on_secondary_container()),
            tertiary: c(s.tertiary()),
            tertiary_container: c(s.tertiary_container()),
            on_tertiary_container: c(s.on_tertiary_container()),
            surface: c(s.surface()),
            surface_container: c(s.surface_container()),
            surface_container_high: c(s.surface_container_high()),
            surface_container_highest: c(s.surface_container_highest()),
            on_surface: c(s.on_surface()),
            on_surface_variant: c(s.on_surface_variant()),
            outline: c(s.outline()),
            outline_variant: c(s.outline_variant()),
            error: c(s.error()),
            error_container: c(s.error_container()),
            on_error_container: c(s.on_error_container()),
            ..base
        }
    }
}

/// The app's active theme. Read it with [`Theme::of`] and change it with [`Theme::set`].
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub scheme: Scheme,
    pub dark: bool,
    pub seed: Option<Hsla>,
}

impl Global for Theme {}

impl Theme {
    pub fn new(dark: bool, seed: Option<Hsla>) -> Self {
        let scheme = match seed {
            Some(seed) => Scheme::from_seed(seed, dark),
            None if dark => Scheme::baseline_dark(),
            None => Scheme::baseline_light(),
        };
        Self { scheme, dark, seed }
    }

    pub fn of(cx: &App) -> &Scheme {
        &cx.global::<Theme>().scheme
    }

    pub fn get(cx: &App) -> Theme {
        *cx.global::<Theme>()
    }

    /// Uses a custom scheme (e.g. your app's existing tokens) instead of generating one.
    pub fn set_scheme(scheme: Scheme, dark: bool, cx: &mut App) {
        cx.set_global(Theme {
            scheme,
            dark,
            seed: None,
        });
        cx.refresh_windows();
    }

    /// Changes the theme and redraws every window.
    pub fn set(dark: bool, seed: Option<Hsla>, cx: &mut App) {
        cx.set_global(Theme::new(dark, seed));
        cx.refresh_windows();
    }
}

/// Parses a "#rrggbb" color.
pub fn hex(value: &str) -> Hsla {
    Argb::from_str(value)
        .map(|a| rgb(((a.red as u32) << 16) | ((a.green as u32) << 8) | a.blue as u32).into())
        .unwrap_or_default()
}

/// Mixes two colors in sRGB (like `color-mix(in srgb, a t%, b)` with `t` in 0–1).
pub fn mix(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let (a, b) = (Rgba::from(a), Rgba::from(b));
    Rgba {
        r: a.r * t + b.r * (1. - t),
        g: a.g * t + b.g * (1. - t),
        b: a.b * t + b.b * (1. - t),
        a: a.a * t + b.a * (1. - t),
    }
    .into()
}

/// The same color with a different opacity.
pub fn alpha(color: Hsla, a: f32) -> Hsla {
    Hsla {
        a: color.a * a,
        ..color
    }
}

/// Text sizes used by the reference design with M3 Expressive.
pub mod text {
    use super::*;

    pub const BODY: Pixels = px(13.5);
    pub const LABEL: Pixels = px(12.5);
    pub const TITLE: Pixels = px(16.);
    pub const HEADLINE: Pixels = px(22.);
    pub const DISPLAY: Pixels = px(32.);
}

/// The M3 type scale (`TypeScaleTokens`), with the Expressive emphasized weights.
/// Sizes are the spec's sp taken as px.
pub mod type_scale {
    use gpui::{FontWeight, Pixels, Styled, px};

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct TypeStyle {
        pub size: Pixels,
        pub line_height: Pixels,
        pub weight: FontWeight,
        /// The weight of the emphasized variant.
        pub emphasized: FontWeight,
    }

    const fn style(size: f32, line: f32, weight: f32, emphasized: f32) -> TypeStyle {
        TypeStyle { size: px(size), line_height: px(line), weight: FontWeight(weight), emphasized: FontWeight(emphasized) }
    }

    pub const DISPLAY_LARGE: TypeStyle = style(57., 64., 400., 500.);
    pub const DISPLAY_MEDIUM: TypeStyle = style(45., 52., 400., 500.);
    pub const DISPLAY_SMALL: TypeStyle = style(36., 44., 400., 500.);
    pub const HEADLINE_LARGE: TypeStyle = style(32., 40., 400., 500.);
    pub const HEADLINE_MEDIUM: TypeStyle = style(28., 36., 400., 500.);
    pub const HEADLINE_SMALL: TypeStyle = style(24., 32., 400., 500.);
    pub const TITLE_LARGE: TypeStyle = style(22., 28., 400., 500.);
    pub const TITLE_MEDIUM: TypeStyle = style(16., 24., 500., 700.);
    pub const TITLE_SMALL: TypeStyle = style(14., 20., 500., 700.);
    pub const BODY_LARGE: TypeStyle = style(16., 24., 400., 500.);
    pub const BODY_MEDIUM: TypeStyle = style(14., 20., 400., 500.);
    pub const BODY_SMALL: TypeStyle = style(12., 16., 400., 500.);
    pub const LABEL_LARGE: TypeStyle = style(14., 20., 500., 700.);
    pub const LABEL_MEDIUM: TypeStyle = style(12., 16., 500., 700.);
    pub const LABEL_SMALL: TypeStyle = style(11., 16., 500., 700.);

    /// `.type_style(BODY_MEDIUM)` or `.type_emphasized(TITLE_MEDIUM)`.
    pub trait TypeScaleExt: Styled + Sized {
        fn type_style(self, style: TypeStyle) -> Self {
            self.text_size(style.size).line_height(style.line_height).font_weight(style.weight)
        }

        fn type_emphasized(self, style: TypeStyle) -> Self {
            self.text_size(style.size).line_height(style.line_height).font_weight(style.emphasized)
        }
    }

    impl<E: Styled> TypeScaleExt for E {}
}
