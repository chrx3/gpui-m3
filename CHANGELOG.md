# Changelog

## 0.1.0 — unreleased

First release.

- Theme: M3 color roles with the baseline scheme, dynamic color from a seed (tonal spot) and
  custom schemes via `Theme::set_scheme`.
- Motion: analytic springs with the official M3 Expressive values, retargetable mid-flight;
  `animate*`, `animate_on_activate`, `entrance`, `continuous` / `continuous_at`, reduced motion.
- Shapes: the 14 M3 Expressive shapes with morphing, plus three soft organic ones (`Blob`,
  `Blob2`, `Pebble`) for decorations without corners or lobes; avatars with a stable shape and tone.
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
- Added for the reference design port, batch 2 (code):
  - `Diff` (a real line diff with `similar`: old/new numbers, `hunks`, `rows` with folding,
    `from_unified` for `git diff` output), `diff_stats`, `DiffView` (folded runs as
    "⋯ N unchanged lines", "Show all" past 400 rows, header with `+N −M`, new-file style) and
    `DiffBar`.
  - `CodeOutput`: clipped monospace output with "Show all (N more lines)", a prompt and an
    error style.
  - `syntax`: `SyntaxHighlighter` (syntect, line by line, cloneable state), `highlight`,
    `highlight_cached` (resumes a growing block), `language` labels and `SyntaxPalette`.
  - `CodeBlock`: language name, Copy → "Copied" for 1.4 s, highlighting, scrolling both ways.
  - `Markdown`: GFM via `pulldown-cmark` (headings, nested and numbered lists, tasks, quotes,
    rules, emphasis, strikethrough, inline code, tables, links and bare URLs, images, code
    blocks), cached by text for streaming; `on_link`, `on_path` (`looks_like_path`,
    `split_path_line`) and `on_image`; `parse_markdown` for tests and custom renderers.
- Added for the reference design port, batch 3 (editor, layout and motion):
  - `CodeEditor` over a `CodeBuffer`: line numbers, a virtualized list, selection with mouse
    and keys, clipboard, undo/redo, Tab / Shift+Tab, auto-indent, bracket pairs, `Save` on
    Ctrl+S, `dirty` and the selection as context. Colors come from a `LineHighlighter`;
    `SyntaxLines` plugs `SyntaxHighlighter` in, keeping parser checkpoints every 16 lines.
  - `ResizeHandle` and `Splitter`: draggable dividers with limits.
  - `ReorderList` with `ReorderEvent` / `apply_reorder`: drag to reorder, with groups and an
    insertion line.
  - `ImageCropper`: drag, wheel or slider zoom, a shaped mask, Enter / Esc, and the cropped image.
  - `theme_reveal` (circular reveal from the click), `animate_theme` / `animate_scheme`
    (`Scheme::lerp` over about 0.45 s).
  - `ExpandableCard` animates its height with a spring; `Icon::pop`, `Avatar::hover_spin`,
    menu items staggered one by one, and `Dialog` / `Toast` entrances with scale and radius.
  - Two-line `MenuItem` with actions under the text.
  - `Dialog::exit(progress)`: fades and sinks a dialog while it leaves (`motion::presence`),
    because a dialog is drawn deferred and a `Presence` around it cannot fade it.
- Added for the Atic Code settings:
  - `Kbd`, `KeyCombo`, `KeyNames` and `parse_keys`: key caps in a tonal container; a text such as
    `ctrl-shift-k` becomes one aligned row in a fixed modifier order, with configurable words
    (`KeyNames::spanish()`: Mayús, Supr, Av Pág…), muted keys, a hover layer when clickable and a
    pulsing "capturing" pill with its own prompt.
  - `SettingRow`: icon, title, description and a trailing control (switch, key combo, button),
    optionally clickable as a whole; meant for `ListGroup::custom_row`.
- Interaction: hover state layer and ripples clipped to each component's rounded shape.
- Gallery example to try every component in light and dark, with any seed color and slow motion.
