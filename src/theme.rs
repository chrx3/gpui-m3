//! Material 3 Expressive color, typography and corner scale.
//!
//! The baseline schemes use the same tokens as the reference design (the M3 reference purple).
//! [`Scheme::from_seed`] generates a dynamic (tonal spot) scheme from any color, and
//! [`Scheme::from_seed_variant`] any of the nine Material variants at any contrast.

use std::str::FromStr;

use gpui::{App, BoxShadow, Global, Hsla, Pixels, Rgba, hsla, point, px, rgb};
use material_colors::{
    color::Argb,
    hct::Hct,
    scheme::variant::{
        SchemeContent, SchemeExpressive, SchemeFidelity, SchemeFruitSalad, SchemeMonochrome,
        SchemeNeutral, SchemeRainbow, SchemeTonalSpot, SchemeVibrant,
    },
};

/// Default font family, registered by [`crate::init`] (Inter, static files).
pub const FONT_FAMILY: &str = "Inter";

/// Roboto, also registered by [`crate::init`].
pub const ROBOTO_FONT_FAMILY: &str = "Roboto";

static CURRENT_FONT: std::sync::RwLock<&'static str> = std::sync::RwLock::new(FONT_FAMILY);

/// The font family components draw with: [`FONT_FAMILY`] unless the app chose another
/// one with [`set_font_family`].
pub fn font_family() -> &'static str {
    *CURRENT_FONT.read().unwrap_or_else(|e| e.into_inner())
}

/// Switches the family every component uses from the next frame on (a bundled one or
/// any installed family name). Refresh the windows after calling it.
pub fn set_font_family(family: &'static str) {
    *CURRENT_FONT.write().unwrap_or_else(|e| e.into_inner()) = family;
}

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
    /// The secondary accent: less prominent than `primary`.
    pub secondary: Hsla,
    /// Content on `secondary`.
    pub on_secondary: Hsla,
    /// (--secondary)
    pub secondary_container: Hsla,
    /// (--on-secondary)
    pub on_secondary_container: Hsla,
    /// (--tertiary)
    pub tertiary: Hsla,
    /// Content on `tertiary`.
    pub on_tertiary: Hsla,
    /// (--tertiary-soft)
    pub tertiary_container: Hsla,
    /// (--on-tertiary-soft)
    pub on_tertiary_container: Hsla,
    /// (--pane)
    pub surface: Hsla,
    /// The darkest surface: behind everything, never brighter than `surface`.
    pub surface_dim: Hsla,
    /// The brightest surface: never dimmer than `surface`.
    pub surface_bright: Hsla,
    /// Sunk below the other containers (a field inside a card).
    pub surface_container_lowest: Hsla,
    /// Slightly above `surface`.
    pub surface_container_low: Hsla,
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
    /// Inverse of `surface`: snackbars and tooltips.
    pub inverse_surface: Hsla,
    /// Content on `inverse_surface`.
    pub inverse_on_surface: Hsla,
    /// An action on `inverse_surface` (a snackbar's button).
    pub inverse_primary: Hsla,
    /// Dims what is behind a modal.
    pub scrim: Hsla,
    /// (--bad)
    pub error: Hsla,
    /// Content on `error`.
    pub on_error: Hsla,
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
            secondary: rgb(0x625b71).into(),
            on_secondary: rgb(0xffffff).into(),
            secondary_container: rgb(0xe8def8).into(),
            on_secondary_container: rgb(0x4a4458).into(),
            tertiary: rgb(0x7d5260).into(),
            on_tertiary: rgb(0xffffff).into(),
            tertiary_container: rgb(0xffd8e4).into(),
            on_tertiary_container: rgb(0x633b48).into(),
            surface: rgb(0xfef7ff).into(),
            surface_dim: rgb(0xded8e1).into(),
            surface_bright: rgb(0xfef7ff).into(),
            surface_container_lowest: rgb(0xffffff).into(),
            surface_container_low: rgb(0xf7f2fa).into(),
            surface_container: rgb(0xf3edf7).into(),
            surface_container_high: rgb(0xece6f0).into(),
            surface_container_highest: rgb(0xe6e0e9).into(),
            on_surface: rgb(0x1d1b20).into(),
            on_surface_variant: rgb(0x49454f).into(),
            outline: rgb(0x79747e).into(),
            outline_variant: rgb(0xcac4d0).into(),
            inverse_surface: rgb(0x322f35).into(),
            inverse_on_surface: rgb(0xf5eff7).into(),
            inverse_primary: rgb(0xd0bcff).into(),
            scrim: rgb(0x000000).into(),
            error: rgb(0xb3261e).into(),
            on_error: rgb(0xffffff).into(),
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
            secondary: rgb(0xccc2dc).into(),
            on_secondary: rgb(0x332d41).into(),
            secondary_container: rgb(0x4a4458).into(),
            on_secondary_container: rgb(0xe8def8).into(),
            tertiary: rgb(0xefb8c8).into(),
            on_tertiary: rgb(0x492532).into(),
            tertiary_container: rgb(0x633b48).into(),
            on_tertiary_container: rgb(0xffd8e4).into(),
            surface: rgb(0x141218).into(),
            surface_dim: rgb(0x141218).into(),
            surface_bright: rgb(0x3b383e).into(),
            surface_container_lowest: rgb(0x0f0d13).into(),
            surface_container_low: rgb(0x1d1b20).into(),
            surface_container: rgb(0x211f26).into(),
            surface_container_high: rgb(0x2b2930).into(),
            surface_container_highest: rgb(0x36343b).into(),
            on_surface: rgb(0xe6e0e9).into(),
            on_surface_variant: rgb(0xcac4d0).into(),
            outline: rgb(0x938f99).into(),
            outline_variant: rgb(0x49454f).into(),
            inverse_surface: rgb(0xe6e0e9).into(),
            inverse_on_surface: rgb(0x322f35).into(),
            inverse_primary: rgb(0x6750a4).into(),
            scrim: rgb(0x000000).into(),
            error: rgb(0xf2b8b5).into(),
            on_error: rgb(0x601410).into(),
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
            secondary: m(self.secondary, other.secondary),
            on_secondary: m(self.on_secondary, other.on_secondary),
            secondary_container: m(self.secondary_container, other.secondary_container),
            on_secondary_container: m(self.on_secondary_container, other.on_secondary_container),
            tertiary: m(self.tertiary, other.tertiary),
            on_tertiary: m(self.on_tertiary, other.on_tertiary),
            tertiary_container: m(self.tertiary_container, other.tertiary_container),
            on_tertiary_container: m(self.on_tertiary_container, other.on_tertiary_container),
            surface: m(self.surface, other.surface),
            surface_dim: m(self.surface_dim, other.surface_dim),
            surface_bright: m(self.surface_bright, other.surface_bright),
            surface_container_lowest: m(
                self.surface_container_lowest,
                other.surface_container_lowest,
            ),
            surface_container_low: m(self.surface_container_low, other.surface_container_low),
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
            inverse_surface: m(self.inverse_surface, other.inverse_surface),
            inverse_on_surface: m(self.inverse_on_surface, other.inverse_on_surface),
            inverse_primary: m(self.inverse_primary, other.inverse_primary),
            scrim: m(self.scrim, other.scrim),
            error: m(self.error, other.error),
            on_error: m(self.on_error, other.on_error),
            error_container: m(self.error_container, other.error_container),
            on_error_container: m(self.on_error_container, other.on_error_container),
            success: m(self.success, other.success),
            warning: m(self.warning, other.warning),
            shadow: m(self.shadow, other.shadow),
        }
    }

    /// Dynamic "tonal spot" scheme (2021 spec) from a seed color.
    pub fn from_seed(seed: Hsla, dark: bool) -> Self {
        Self::from_seed_variant(seed, dark, SchemeVariant::TonalSpot, 0.)
    }

    /// A dynamic scheme from a seed color in one of the nine Material variants. `contrast`
    /// goes from -1 (least) through 0 (the standard design) to 1 (most, for readability).
    pub fn from_seed_variant(seed: Hsla, dark: bool, variant: SchemeVariant, contrast: f32) -> Self {
        let rgba = Rgba::from(seed);
        let argb = Argb::new(
            255,
            (rgba.r * 255.).round() as u8,
            (rgba.g * 255.).round() as u8,
            (rgba.b * 255.).round() as u8,
        );
        let source = Hct::new(argb);
        let contrast = Some(f64::from(contrast.clamp(-1., 1.)));
        let s = match variant {
            SchemeVariant::TonalSpot => SchemeTonalSpot::new(source, dark, contrast).scheme,
            SchemeVariant::Expressive => SchemeExpressive::new(source, dark, contrast).scheme,
            SchemeVariant::Vibrant => SchemeVibrant::new(source, dark, contrast).scheme,
            SchemeVariant::Neutral => SchemeNeutral::new(source, dark, contrast).scheme,
            SchemeVariant::Monochrome => SchemeMonochrome::new(source, dark, contrast).scheme,
            SchemeVariant::Fidelity => SchemeFidelity::new(source, dark, contrast).scheme,
            SchemeVariant::Content => SchemeContent::new(source, dark, contrast).scheme,
            SchemeVariant::Rainbow => SchemeRainbow::new(source, dark, contrast).scheme,
            SchemeVariant::FruitSalad => SchemeFruitSalad::new(source, dark, contrast).scheme,
        };
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
            secondary: c(s.secondary()),
            on_secondary: c(s.on_secondary()),
            secondary_container: c(s.secondary_container()),
            on_secondary_container: c(s.on_secondary_container()),
            tertiary: c(s.tertiary()),
            on_tertiary: c(s.on_tertiary()),
            tertiary_container: c(s.tertiary_container()),
            on_tertiary_container: c(s.on_tertiary_container()),
            surface: c(s.surface()),
            surface_dim: c(s.surface_dim()),
            surface_bright: c(s.surface_bright()),
            surface_container_lowest: c(s.surface_container_lowest()),
            surface_container_low: c(s.surface_container_low()),
            surface_container: c(s.surface_container()),
            surface_container_high: c(s.surface_container_high()),
            surface_container_highest: c(s.surface_container_highest()),
            on_surface: c(s.on_surface()),
            on_surface_variant: c(s.on_surface_variant()),
            outline: c(s.outline()),
            outline_variant: c(s.outline_variant()),
            inverse_surface: c(s.inverse_surface()),
            inverse_on_surface: c(s.inverse_on_surface()),
            inverse_primary: c(s.inverse_primary()),
            scrim: c(s.scrim()),
            error: c(s.error()),
            on_error: c(s.on_error()),
            error_container: c(s.error_container()),
            on_error_container: c(s.on_error_container()),
            ..base
        }
    }

    /// Fills the roles a hand-made scheme has no color for from the ones it has: the secondary
    /// and inverse roles, the `on_*` of each accent, the dim, bright, low and lowest surfaces
    /// and the scrim. They are approximations that follow the scheme's own polarity; a
    /// generated scheme ([`Scheme::from_seed_variant`]) already has the exact ones.
    ///
    /// ```ignore
    /// Scheme { primary, on_primary, /* your tokens */ ..Scheme::baseline_dark() }.derive_roles()
    /// ```
    pub fn derive_roles(self) -> Self {
        let light = self.surface.l > 0.5;
        let white = hsla(0., 0., 1., 1.);
        let black = hsla(0., 0., 0., 1.);
        // `amount` of `from`, the rest `to`.
        let blend = |from: Hsla, to: Hsla, amount: f32| mix(from, to, amount);
        Self {
            secondary: blend(self.secondary_container, self.on_secondary_container, 0.2),
            on_secondary: self.on_primary,
            on_tertiary: self.on_primary,
            on_error: self.on_primary,
            surface_dim: if light {
                blend(self.surface_container_highest, black, 0.96)
            } else {
                self.surface
            },
            surface_bright: if light {
                self.surface
            } else {
                blend(self.surface_container_highest, white, 0.94)
            },
            surface_container_lowest: if light {
                blend(self.surface, white, 0.5)
            } else {
                blend(self.surface, black, 0.65)
            },
            surface_container_low: blend(self.surface, self.surface_container, 0.5),
            inverse_surface: self.on_surface,
            inverse_on_surface: if light {
                self.surface_container
            } else {
                self.surface_container_highest
            },
            inverse_primary: self.primary_container,
            scrim: black,
            ..self
        }
    }
}

/// The nine dynamic color variants of Material 3, for [`Scheme::from_seed_variant`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SchemeVariant {
    /// The default: calm, mid chroma.
    #[default]
    TonalSpot,
    /// Playful: the accents' hue is shifted away from the seed's.
    Expressive,
    /// Very colorful, detached from the seed.
    Vibrant,
    /// Almost gray, with a hint of the seed.
    Neutral,
    /// Pure grays.
    Monochrome,
    /// Keeps the seed color as the primary.
    Fidelity,
    /// Like fidelity, with a tertiary analogous to the seed.
    Content,
    /// A rainbow of accents around the seed.
    Rainbow,
    /// Bolder, with an offset hue.
    FruitSalad,
}

impl SchemeVariant {
    /// Every variant, in the order the gallery lists them.
    pub const ALL: [SchemeVariant; 9] = [
        SchemeVariant::TonalSpot,
        SchemeVariant::Expressive,
        SchemeVariant::Vibrant,
        SchemeVariant::Neutral,
        SchemeVariant::Monochrome,
        SchemeVariant::Fidelity,
        SchemeVariant::Content,
        SchemeVariant::Rainbow,
        SchemeVariant::FruitSalad,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SchemeVariant::TonalSpot => "Tonal spot",
            SchemeVariant::Expressive => "Expressive",
            SchemeVariant::Vibrant => "Vibrant",
            SchemeVariant::Neutral => "Neutral",
            SchemeVariant::Monochrome => "Monochrome",
            SchemeVariant::Fidelity => "Fidelity",
            SchemeVariant::Content => "Content",
            SchemeVariant::Rainbow => "Rainbow",
            SchemeVariant::FruitSalad => "Fruit salad",
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

/// The M3 elevation levels. Higher levels cast a larger, softer shadow.
///
/// ```ignore
/// div().bg(s.surface_container_low).shadow(Elevation::Level2.shadow(&s))
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Elevation {
    /// Flat (0 dp): no shadow.
    #[default]
    Level0,
    /// 1 dp: a resting card.
    Level1,
    /// 3 dp: a raised button or a menu bar.
    Level2,
    /// 6 dp: a FAB, a menu.
    Level3,
    /// 8 dp: a pressed or dragged card.
    Level4,
    /// 12 dp: a dialog.
    Level5,
}

impl Elevation {
    pub const ALL: [Elevation; 6] = [
        Elevation::Level0,
        Elevation::Level1,
        Elevation::Level2,
        Elevation::Level3,
        Elevation::Level4,
        Elevation::Level5,
    ];

    /// The level's distance from the surface, in dp (taken as px).
    pub fn dp(self) -> f32 {
        match self {
            Elevation::Level0 => 0.,
            Elevation::Level1 => 1.,
            Elevation::Level2 => 3.,
            Elevation::Level3 => 6.,
            Elevation::Level4 => 8.,
            Elevation::Level5 => 12.,
        }
    }

    /// The shadow of this level: a key light that falls straight down and a tighter
    /// ambient one, in the scheme's shadow color. Level 0 has none.
    pub fn shadow(self, scheme: &Scheme) -> Vec<BoxShadow> {
        let dp = self.dp();
        if dp == 0. {
            return Vec::new();
        }
        vec![
            BoxShadow {
                color: scheme.shadow,
                offset: point(px(0.), px(dp * 0.5)),
                blur_radius: px(dp * 1.5),
                spread_radius: px(0.),
            },
            BoxShadow {
                color: alpha(scheme.shadow, 0.7),
                offset: point(px(0.), px(1.)),
                blur_radius: px(2. + dp * 0.5),
                spread_radius: px(0.),
            },
        ]
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

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: &str = "#0b57d0";

    fn close(a: Hsla, b: Hsla) -> bool {
        (a.h - b.h).abs() < 1e-3 && (a.s - b.s).abs() < 1e-3 && (a.l - b.l).abs() < 1e-3
    }

    #[test]
    fn containers_climb_away_from_the_surface() {
        for dark in [false, true] {
            for variant in SchemeVariant::ALL {
                let s = Scheme::from_seed_variant(hex(SEED), dark, variant, 0.);
                let steps = [
                    s.surface_container_lowest.l,
                    s.surface_container_low.l,
                    s.surface_container.l,
                    s.surface_container_high.l,
                    s.surface_container_highest.l,
                ];
                for pair in steps.windows(2) {
                    let ordered = if dark { pair[0] <= pair[1] } else { pair[0] >= pair[1] };
                    assert!(ordered, "{variant:?} dark={dark}: {steps:?}");
                }
            }
        }
    }

    #[test]
    fn variants_are_different_palettes() {
        let primary = |variant| Scheme::from_seed_variant(hex(SEED), false, variant, 0.).primary;
        let tonal = primary(SchemeVariant::TonalSpot);
        assert!(!close(tonal, primary(SchemeVariant::Vibrant)));
        assert!(!close(tonal, primary(SchemeVariant::Fidelity)));
        assert!(primary(SchemeVariant::Monochrome).s < 0.01, "monochrome is gray");
        assert!(close(tonal, Scheme::from_seed(hex(SEED), false).primary));
    }

    #[test]
    fn more_contrast_separates_the_accent_from_its_content() {
        let gap = |contrast| {
            let s = Scheme::from_seed_variant(hex(SEED), false, SchemeVariant::TonalSpot, contrast);
            (s.on_primary.l - s.primary.l).abs() + (s.on_surface_variant.l - s.surface.l).abs()
        };
        assert!(gap(1.) > gap(0.), "{} vs {}", gap(1.), gap(0.));
        assert!(gap(0.) > gap(-1.), "{} vs {}", gap(0.), gap(-1.));
    }

    #[test]
    fn derived_roles_follow_the_polarity_and_keep_the_given_ones() {
        for (base, dark) in [(Scheme::baseline_light(), false), (Scheme::baseline_dark(), true)] {
            let s = base.derive_roles();
            assert_eq!(s.primary, base.primary);
            assert_eq!(s.scrim, hsla(0., 0., 0., 1.));
            assert_eq!(s.inverse_surface, base.on_surface);
            if dark {
                assert!(s.surface_dim.l <= s.surface.l && s.surface_bright.l >= s.surface.l);
                assert!(s.surface_container_lowest.l <= s.surface.l);
            } else {
                assert!(s.surface_dim.l <= s.surface.l && s.surface_bright.l >= s.surface.l);
                assert!(s.surface_container_lowest.l >= s.surface.l);
            }
        }
    }

    #[test]
    fn each_level_casts_a_larger_shadow() {
        let s = Scheme::baseline_light();
        assert!(Elevation::Level0.shadow(&s).is_empty());
        let reach = |level: Elevation| {
            let shadows = level.shadow(&s);
            assert_eq!(shadows.len(), 2);
            f32::from(shadows[0].blur_radius) + f32::from(shadows[0].offset.y)
        };
        for pair in Elevation::ALL[1..].windows(2) {
            assert!(reach(pair[0]) < reach(pair[1]), "{:?}", pair);
        }
    }

    #[test]
    fn lerp_reaches_the_new_roles() {
        let (light, dark) = (Scheme::baseline_light(), Scheme::baseline_dark());
        let end = light.lerp(&dark, 1.);
        assert!(close(end.inverse_surface, dark.inverse_surface));
        assert!(close(end.surface_container_low, dark.surface_container_low));
        assert!(close(end.on_error, dark.on_error));
    }
}
