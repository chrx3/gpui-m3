# Integrating gpui-m3 into an existing GPUI app

This guide is for apps that already have their own look (design tokens, a theme switch, maybe
several visual styles) and want to adopt `gpui-m3` for their Material 3 Expressive style.

## 1. Add the dependency and profiles

```toml
[dependencies]
gpui-m3 = { git = "https://github.com/chrx3/gpui-m3" } # or a version from crates.io

[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

`gpui-m3` depends on `gpui = "0.2.2"`. Your app must use the same GPUI (0.2.x from crates.io); two
different GPUI copies in one binary will not work together.

## 2. Initialize once

```rust
gpui_m3::init(cx);
```

This registers the Roboto font, the `TextField` key bindings and a default theme. Call it after
creating the `Application` and before opening windows.

### Assets

Icons are loaded through GPUI's asset source as `m3/icons/<name>.svg` (`gpui_m3::ICON_PREFIX`), so
they never clash with your app's own `icons/`. If your app already has an `AssetSource`, delegate
those paths:

```rust
impl AssetSource for MyAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = gpui_m3::Assets::load_icon(path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        // ...your own assets
    }
    // ...
}
```

## 3. Map your tokens to a `Scheme`

If your app already has colors for its M3 style, keep them as the source of truth and hand them to
`gpui-m3` whenever the theme changes:

```rust
use gpui_m3::{Scheme, Theme};

fn m3_scheme(t: &MyTokens) -> Scheme {
    Scheme {
        primary: t.accent,
        on_primary: t.on_accent,
        primary_container: t.accent_soft,
        on_primary_container: t.on_accent_soft,
        secondary_container: t.sel,
        on_secondary_container: t.text,
        tertiary: t.attention,
        tertiary_container: t.attention,
        on_tertiary_container: t.on_attention,
        surface: t.editor,
        surface_container: t.raised,
        surface_container_high: t.control,
        surface_container_highest: t.control2,
        on_surface: t.text,
        on_surface_variant: t.muted,
        outline: t.faint,
        outline_variant: t.border,
        error: t.bad,
        success: t.ok,
        warning: t.warn,
        shadow: t.shadow,
    }
}

// On startup and every time the user changes theme, mode or accent:
Theme::set_scheme(m3_scheme(&tokens), tokens.dark, cx);
```

Field names follow the M3 color roles. A quick reference:

| M3 role | Typical use |
|---|---|
| `primary` / `on_primary` | filled buttons, selected segments, switch track |
| `primary_container` / `on_primary_container` | FAB, chips, user bubbles, tonal cards |
| `secondary_container` / `on_secondary_container` | tonal buttons, selection indicators |
| `tertiary*` | badges, attention cards |
| `surface` → `surface_container_highest` | background → raised controls |
| `outline` / `outline_variant` | strong / subtle borders |

If your app only has an accent color, use `Theme::set(dark, Some(accent), cx)` instead and let
`gpui-m3` derive the whole scheme.

## 4. Replace components gradually

`gpui-m3` components are ordinary GPUI elements; they can live next to your existing ones. A good
order is: buttons → selection controls → menus → navigation → text fields.

If your app has several visual styles, branch only where the M3 style differs:

```rust
let send = match style {
    Style::Expressive => IconButton::new("send", "up").filled().hover_morph(true).size(px(36.)).into_any_element(),
    _ => my_send_button(cx).into_any_element(),
};
```

Rules of thumb:

- **Ids:** every component needs an id unique within its view. In lists use tuple ids such as
  `("row", index)`.
- **Callbacks:** components take `Fn(&ClickEvent, &mut Window, &mut App)` (or the new value for
  `on_change` / `on_toggle`). Update your view through a weak entity handle and `cx.notify()`.
- **Text fields:** `TextField` is an entity. Create it once, store it, render `field.clone()`, and
  subscribe to `TextFieldEvent::{Changed, Submitted, Cancelled}`.
- **Fonts:** components set `gpui_m3::theme::FONT_FAMILY` on themselves; set it on your root too so
  surrounding text matches.

## 5. Keep it fast

- Animated components request frames only while they move; when everything is still, nothing redraws.
- `LoadingIndicator`, `WavyProgress` and breathing/spinning `Shape`s animate continuously. Each frame
  GPUI re-renders the view that contains them **and every ancestor view**, so wrapping the indicator
  in its own view does not help by itself. What helps is keeping heavy content (message lists, file
  trees) in separate views rendered with `AnyView::from(entity).cached(style)`: cached views that did
  not change are reused while their parent re-renders.
- Respect the user's reduced-motion preference with
  `cx.set_global(MotionSettings { time_scale: 1., reduced: true })`.

## 6. Check it

Run the gallery next to your app (`cargo run --example gallery`) with the same mode and seed color to
compare details.
