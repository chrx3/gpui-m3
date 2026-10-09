# Changelog

## 0.1.0 — unreleased

First release.

- Theme: M3 color roles with the baseline scheme, dynamic color from a seed (tonal spot) and
  custom schemes via `Theme::set_scheme`.
- Motion: analytic springs with the official M3 Expressive values, retargetable mid-flight;
  `animate*`, `animate_on_activate`, `entrance`, `continuous` / `continuous_at`, reduced motion.
- Shapes: the 14 M3 Expressive shapes with morphing; avatars with a stable shape and tone.
- Components: `Button`, `IconButton`, `Fab`, `Switch`, `SegmentedButtons`, `StopSlider`, `Chip`,
  `SplitButton`, `Badge`, `NavItem`, `RailItem`, `IconTabs`, `Menu`, `MenuItem`, `Card`,
  `ListGroup`, `Bubble`, `Shape`, `Avatar`, `LoadingIndicator`, `WavyProgress`, `Ring`,
  `LinearProgress`, `TextField`, `Dialog`, `Toast`, `Tooltip`, `context_menu`.
- Icons are served as `m3/icons/<name>.svg` (`ICON_PREFIX`) so they never clash with the host
  app's own `icons/`.
- Added while porting a full app (Atic Code):
  - Text: multi-line `TextArea`, `CommandPalette`, `TextField` styles, labels, trailing content
    and `restore_on_cancel`.
  - Menus and surfaces: dense, radio, highlighted, static and danger (two-step) `MenuItem`s;
    `Menu` header, back row, direction and content transitions; anchored `Popover`;
    `ExpandableCard`; configurable `Card` with a shape entrance; adjustable `Bubble`.
  - Selection: `Radio`, `Checkbox`, `ChoiceRow`, continuous `Slider`, `MorphDot`; tonal, muted,
    small, mono and input `Chip`s; tonal, strong and popping `Badge`s.
  - Buttons: sublabel, `open`, `Tiny`/`Compact`/`Comfortable` sizes, `Ghost` with confirmation,
    `IconButton::launch`, `FavStar`.
  - Panels: `TreeRow`, `ListItem`, `TabStrip`, `NavItem` emphasis, meta and large rows.
  - Appearance: `SelectCard`, `ColorSwatches`, `HsvPicker`.
  - `Scheme::error_container` / `on_error_container`; `motion::replay` and `motion::pulse`.
- Interaction: hover state layer and ripples clipped to each component's rounded shape.
- Gallery example to try every component in light and dark, with any seed color and slow motion.
