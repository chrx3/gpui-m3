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
- Added for the reference design port, batch 1:
  - `DividerLabel`: a labeled separator (compaction; model switch with wavy lines and a tonal
    pill, outlined and flowing while pending); its lines grow out from the label.
  - `Banner`: tonal offer card with a shape mark and pill actions, or a subtle/warning notice,
    with a close button and spring entrance.
  - `Presence` / `Exit` and `motion::presence`: keep content on screen while it animates out;
    `motion::play_on_rise` for one-shot effects on a state edge.
  - `DropZone`: a "Drop to attach" layer over an area that accepts `ExternalPaths`.
  - `ImageThumb`: rounded image thumbnail with a remove button on hover.
  - `Avatar::image` / `Avatar::photo` and `Shape::image`: photos clipped to an M3 shape with
    anti-aliased edges (masked in the background and cached); `Shape::breathe_on_hover` and
    `Avatar::breathe_on_hover` morph toward another shape once per hover.
  - `SuggestionList` with `SuggestionNav`, `match_ranges`, `rank_mentions` and the
    `Suggestion*` actions; `TextArea::set_suggesting` / `TextField::set_suggesting` route
    ↑ ↓ Enter Tab Esc to an open list.
  - `ExpandableCard`: the header is a Tab stop, toggles with Enter or Space and shows a focus
    ring (`interaction::use_focus`, `focusable`, `focus_ring`).
  - `CommandPalette`: the pointer moves the selection, the list scrolls to it and the matching
    text is highlighted; `MenuItem::hover_fill` and `MenuItem::label_highlights`.
- Interaction: hover state layer and ripples clipped to each component's rounded shape.
- Gallery example to try every component in light and dark, with any seed color and slow motion.
