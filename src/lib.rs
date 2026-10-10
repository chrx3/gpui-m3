//! Material 3 Expressive components for GPUI, based on the design of the reference design app.
//!
//! ```ignore
//! Application::new().with_assets(gpui_m3::Assets).run(|cx| {
//!     gpui_m3::init(cx);
//!     // ...
//! });
//! ```

mod assets;
mod cache;
pub mod components;
pub mod interaction;
pub mod motion;
pub mod shapes;
pub mod syntax;
pub mod theme;

use std::borrow::Cow;

use gpui::App;

pub use assets::{Assets, ICON_PREFIX};
pub use components::*;
pub use motion::{MotionSettings, Spring};
pub use shapes::{Morph, ShapeName, Tone};
pub use syntax::{SyntaxHighlighter, SyntaxPalette, TokenKind};
pub use theme::{Scheme, Theme, corner};

/// Registers the typeface, the base (light) theme and the motion settings.
pub fn init(cx: &mut App) {
    let fonts: Vec<Cow<'static, [u8]>> =
        vec![Cow::Borrowed(include_bytes!("../assets/fonts/Roboto.ttf"))];
    if let Err(err) = cx.text_system().add_fonts(fonts) {
        eprintln!("gpui_m3: could not load Roboto: {err}");
    }
    if !cx.has_global::<Theme>() {
        cx.set_global(Theme::new(false, None));
    }
    components::bind_text_field_keys(cx);
    components::bind_text_area_keys(cx);
    components::bind_command_palette_keys(cx);
    // After the text inputs, so open suggestion lists take the arrows, Enter, Tab and Escape.
    components::bind_suggestion_keys(cx);
    if !cx.has_global::<MotionSettings>() {
        cx.set_global(MotionSettings::default());
    }
}
