# Changelog

## 0.2.2 — 2026-11-14

Indicador que se desliza, texto que entra y esquinas limpias / A sliding indicator, fading-in text and clean corners

### Español

- **Nuevo:** NavItem::indicator(false): la fila no dibuja su propio indicador de elegida, para que el contenedor pinte uno compartido que se desliza entre filas (como el del riel de navegación)
- **Nuevo:** Markdown::fade_tail: mientras llega en vivo, los trozos recién llegados del último párrafo entran con un fundido; la opacidad la calcula quien llama con la hora de llegada de cada trozo, sin pedir cuadros propios
- **Corregido:** DiffView: el fondo rojo o verde de la primera y la última fila ya no sobresale por las esquinas redondeadas (GPUI recorta en rectángulo); lo mismo con «Mostrar todo» de DiffView y CodeOutput y con el encabezado de las tablas de Markdown

### English

- **New:** NavItem::indicator(false): the row draws no selected indicator of its own, so the container can paint a shared one that slides between rows (like a navigation rail's)
- **New:** Markdown::fade_tail: while streaming, the chunks that just arrived in the last paragraph fade in; the caller computes the opacity from each chunk's arrival time, with no frames of its own
- **Fixed:** DiffView: the red or green background of the first and last rows no longer shows past the rounded corners (GPUI clips to a rectangle); the same for DiffView's and CodeOutput's "Show all" and Markdown tables' header

## 0.2.1 — 2026-11-14

Estados, marcas y salida de herramientas / States, marks and tool output

### Español

- **Nuevo:** StatusMark: una marca de estado (pendiente, corriendo, hecho, fallido, esperando) que cambia de forma una sola vez con el resorte espacial rápido y se queda quieta; mientras corre deja el hueco para el indicador de carga o lo dibuja con spinner(true)
- **Nuevo:** EmptyState: estado vacío con una forma héroe quieta (o que cambia al pasar el mouse con breathe_on_hover), ícono, título, texto y acciones
- **Nuevo:** WavyProgress::still y WavyCircularProgress::still: onda quieta que no pide cuadros continuos, más color y track_color; Ring::color, track_color y warn_at, y LoadingIndicator::color
- **Nuevo:** ShimmerText (texto con brillo a 15 fps, quieto con movimiento reducido), PageIndicator y motion::Enter con shared_axis y stagger
- **Nuevo:** CodeOutput::json (ordena e indenta un JSON), max_chars y show_all_label (también en DiffView) para traducir «Show all»; JsonView, un árbol plegable de un serde_json::Value con la feature json
- **Nuevo:** Botones: Button::busy, Fab::busy, disabled, color y colors, IconButton::icon_swap (el ícono viejo sale girando y el nuevo entra con resorte), ButtonGroupItem::variant y autofocus, y SplitButton::filled, outlined, icon, autofocus y menu
- **Nuevo:** ListItem::selected (contenedor secundario y esquinas con resorte), overline, supporting y hover_actions; Toolbar::dense y ToolbarItem::custom; ExpandableCard::error y container; Chip::danger
- **Nuevo:** Theme::scoped (otro esquema solo para una ventana mientras se dibuja) y MotionSettings::sync (sigue el movimiento reducido del sistema y redibuja solo si cambió)
- **Corregido:** Un botón con autofocus ya no le quita el foco a un campo de texto en el que estás escribiendo, y la selección se ve sobre el código en línea
- **Mejorado:** LoadingIndicator a 15 fps y las animaciones continuas a 4 fps con la ventana atrás: se ven igual y gastan mucha menos batería
- **Nuevo:** Button::full_width: un botón del ancho de su contenedor, alineado con las filas de NavItem (para «Nuevo chat» arriba de una lista)
- **Corregido:** Una onda (ripple) cuyo soltar se perdía (el clic abrió otra ventana o movió el foco) pedía cuadros para siempre y la ventana se redibujaba sin parar (~58 % de un núcleo en reposo). Ahora solo se piden cuadros mientras la onda crece o se desvanece, y pasar el mouse sin botón apretado la suelta
- **Nuevo:** Menu::footer: contenido fijo bajo los ítems. En un menú que abre hacia arriba, un buscador o pestañas ahí no se mueven mientras la lista crece o se achica
- **Corregido:** El carrusel multi-browse terminaba con el último ítem en el puesto chico, donde no se lee; ahora llega hasta dejarlo en el puesto mediano
- **Nuevo:** Ícono `smartphone`

### English

- **New:** StatusMark: a status mark (pending, running, done, failed, waiting) that morphs once with the fast spatial spring and then holds still; while running it leaves room for a loading indicator, or draws one with spinner(true)
- **New:** EmptyState: an empty state with a still hero shape (or one that morphs on hover with breathe_on_hover), an icon, a title, text and actions
- **New:** WavyProgress::still and WavyCircularProgress::still: a still wave that requests no continuous frames, plus color and track_color; Ring::color, track_color and warn_at, and LoadingIndicator::color
- **New:** ShimmerText (shimmering text at 15 fps, still with reduced motion), PageIndicator, and motion::Enter with shared_axis and stagger
- **New:** CodeOutput::json (pretty prints JSON), max_chars and show_all_label (also on DiffView) to translate "Show all"; JsonView, a foldable tree of a serde_json::Value with the json feature
- **New:** Buttons: Button::busy, Fab::busy, disabled, color and colors, IconButton::icon_swap (the old icon spins out and the new one springs in), ButtonGroupItem::variant and autofocus, and SplitButton::filled, outlined, icon, autofocus and menu
- **New:** ListItem::selected (secondary container and corners with springs), overline, supporting and hover_actions; Toolbar::dense and ToolbarItem::custom; ExpandableCard::error and container; Chip::danger
- **New:** Theme::scoped (another scheme for one window while it draws) and MotionSettings::sync (follows the system's reduced motion and redraws only when it changed)
- **Fixed:** A button with autofocus no longer steals the focus from a text field you are typing in, and the selection shows over inline code
- **Improved:** LoadingIndicator at 15 fps and continuous animations at 4 fps with the window in the background: they look the same and cost far less battery
- **New:** Button::full_width: a button as wide as its container, aligned with the NavItem rows (for "New chat" above a list)
- **Fixed:** A ripple whose release got lost (the press opened another window or moved focus) requested frames forever and the window kept redrawing (~58 % of a core at rest). Frames are now requested only while the ripple grows or fades, and moving the pointer over the component with no button held releases it
- **New:** Menu::footer: fixed content below the items. In a menu that opens upward, a filter field or tabs there stay put while the list grows or shrinks
- **Fixed:** The multi-browse carousel ended with the last item in the small slot, where it cannot be read; it now scrolls until the last item sits in the medium slot
- **New:** `smartphone` icon

## 0.2.0 — 2026-11-06

Más componentes, color y elevación / More components, color and elevation

### Español

- **Nuevo:** Ejemplo showcase_music: un reproductor de música Material 3 Expressive hecho con la librería (cargo run --example showcase_music; con SHOWCASE_DEMO=1 corre un recorrido guiado)
- **Nuevo:** Carousel::scroll_to y ReorderList::moved y held mueven un carrusel o una lista por código con la misma animación, y theme_reveal_at inicia el cambio de tema circular desde cualquier punto
- **Corregido:** El carrusel ya no recibe clics en la parte recortada por un contenedor con scroll
- **Nuevo:** Shape::drift: la forma deambula en un lazo lento alrededor de su sitio, pensada para fondos, sin recalcular el layout
- **Mejorado:** Las formas animadas ya no saltan a su pose inicial al perder el foco la ventana: se congelan y siguen desde donde estaban (motion::ambient_clock), y no piden cuadros mientras tanto
- **Nuevo:** Slider con tamaños Expressive (XS a XL, con ícono opcional), versión vertical, modo centrado, marcas de graduación y burbuja con el valor al arrastrar, más RangeSlider con dos manijas; ahora también se manejan con el teclado
- **Nuevo:** StopSlider vertical y con control por flechas, Inicio y Fin
- **Nuevo:** Casillas con estado indeterminado y de error, interruptores con íconos en el pulgar, y botones segmentados de selección múltiple y densos
- **Nuevo:** Campos de texto con estados de Material 3: error con mensaje, texto de apoyo, contador de caracteres con límite opcional, prefijo y sufijo, deshabilitado y solo lectura
- **Nuevo:** El área de texto ahora muestra error, texto de apoyo y contador de caracteres
- **Nuevo:** Selectores de fecha (día o rango) y de hora (reloj de 12 h o 24 h, o dos campos numéricos), en línea o en un diálogo modal, con teclado y nombres traducibles
- **Nuevo:** Carousel Expressive (multi-browse, sin contención, héroe, héroe centrado y pantalla completa) con arrastre, rueda, teclado y ajuste al elemento más cercano, y FabMenu, el FAB que se abre en un menú de acciones con animación escalonada
- **Nuevo:** ButtonGroup: grupos de botones estándar (el botón presionado se ensancha) y conectados (el seleccionado se vuelve píldora), de selección simple o múltiple, en cinco tamaños
- **Nuevo:** Toolbar flotante (horizontal o vertical, estándar o vibrant, con FAB) y Toolbar anclada
- **Nuevo:** TopAppBar pequeña, centrada, mediana y grande, que se colapsa con TopAppBar::collapse y collapse_progress
- **Nuevo:** SearchBar y SearchView: la barra de búsqueda y su vista expandida con resultados
- **Nuevo:** BottomSheet y SideSheet, modales (con scrim, Escape y entrada con resorte) y estándar
- **Nuevo:** Divider con sus márgenes y RichTooltip con título, texto y acciones
- **Nuevo:** Roles de color completos en Scheme: secondary, on_tertiary, on_error, surface_dim, surface_bright, surface_container_lowest y low, inverse_surface, inverse_on_surface, inverse_primary y scrim. Si armas un Scheme a mano, completa con ..Scheme::baseline_dark() y derive_roles()
- **Nuevo:** Las nueve variantes de color dinámico (Expressive, Vibrant, Neutral, Monochrome, Fidelity, Content, Rainbow, Fruit salad) y tres niveles de contraste con Scheme::from_seed_variant
- **Nuevo:** Elevación de M3: seis niveles de sombra con Elevation::shadow
- **Nuevo:** Sección «What's new» en el gallery con las novedades de cada versión, en español e inglés
- **Corregido:** Compila de nuevo con el gpui publicado en crates.io: al arrastrar una selección de texto más allá del borde, la lista rueda con TextSelection::on_edge_scroll o edge_scroll_handle

### English

- **New:** The showcase_music example: a Material 3 Expressive music player built from the library (cargo run --example showcase_music; SHOWCASE_DEMO=1 runs a scripted tour)
- **New:** Carousel::scroll_to and ReorderList::moved and held move a carousel or a list from code with the same animation, and theme_reveal_at starts the circular theme change from any point
- **Fixed:** The carousel no longer takes presses on the part clipped by a scrolling parent
- **New:** Shape::drift: the shape wanders in a slow loop around its place, meant for backdrops, without recalculating the layout
- **Improved:** Animated shapes no longer jump back to their first pose when the window loses focus: they freeze and resume where they were (motion::ambient_clock), and request no frames meanwhile
- **New:** Slider with Expressive sizes (XS to XL, with an optional icon), vertical orientation, a centered mode, tick marks and a value bubble while dragging, plus a two-handle RangeSlider; they now also work with the keyboard
- **New:** StopSlider in a vertical orientation and with arrow, Home and End keys
- **New:** Checkboxes with indeterminate and error states, switches with thumb icons, and multi-select and dense segmented buttons
- **New:** Text fields with Material 3 states: error with message, supporting text, character counter with optional hard limit, prefix and suffix, disabled and read-only
- **New:** The text area now shows error, supporting text and a character counter
- **New:** Date pickers (single day or range) and time pickers (12 h or 24 h dial, or two number fields), docked or modal, with keyboard support and translatable names
- **New:** Expressive Carousel (multi-browse, uncontained, hero, center-aligned hero and full-screen) with drag, wheel, keyboard and snapping, and FabMenu, the FAB that opens into a staggered stack of actions
- **New:** ButtonGroup: standard button groups (the pressed button widens) and connected ones (the selected one turns into a pill), single or multiple selection, in five sizes
- **New:** Floating Toolbar (horizontal or vertical, standard or vibrant, with a FAB) and docked Toolbar
- **New:** TopAppBar small, centered, medium and large, collapsing with TopAppBar::collapse and collapse_progress
- **New:** SearchBar and SearchView: the search bar and its expanded view with results
- **New:** BottomSheet and SideSheet, modal (with scrim, Escape and a spring entrance) and standard
- **New:** Divider with its insets, and RichTooltip with a title, text and actions
- **New:** The full set of color roles in Scheme: secondary, on_tertiary, on_error, surface_dim, surface_bright, surface_container_lowest and low, inverse_surface, inverse_on_surface, inverse_primary and scrim. If you build a Scheme by hand, fill the rest with ..Scheme::baseline_dark() and derive_roles()
- **New:** The nine dynamic color variants (Expressive, Vibrant, Neutral, Monochrome, Fidelity, Content, Rainbow, Fruit salad) and three contrast levels with Scheme::from_seed_variant
- **New:** M3 elevation: six shadow levels with Elevation::shadow
- **New:** A "What's new" section in the gallery with the notes of every version, in Spanish and English
- **Fixed:** Builds again against the gpui published on crates.io: dragging a text selection past the edge scrolls the list through TextSelection::on_edge_scroll or edge_scroll_handle

## 0.1.0 — 2026-10-10

Primera versión / First release

### Español

- **Nuevo:** Botones, botones de ícono, FAB, chips y segmented buttons con el movimiento de Material 3 Expressive
- **Nuevo:** Controles de selección: Switch, Radio, Checkbox, Slider y chips de filtro
- **Nuevo:** Menús, popovers y tarjetas con entrada animada
- **Nuevo:** Las formas de M3 Expressive con morphing, y avatares con foto recortada a la forma
- **Nuevo:** Progreso ondulado, circular y lineal, más el LoadingIndicator
- **Nuevo:** Campos de texto, diálogos, toasts y tooltips
- **Nuevo:** Color dinámico desde cualquier color semilla, o tus propios colores con Theme::set_scheme

### English

- **New:** Buttons, icon buttons, FABs, chips and segmented buttons with Material 3 Expressive motion
- **New:** Selection controls: Switch, Radio, Checkbox, Slider and filter chips
- **New:** Menus, popovers and cards with animated entrances
- **New:** The M3 Expressive shapes with morphing, and avatars with a photo clipped to the shape
- **New:** Wavy, circular and linear progress, plus the LoadingIndicator
- **New:** Text fields, dialogs, toasts and tooltips
- **New:** Dynamic color from any seed color, or your own colors with Theme::set_scheme

### Details

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
- Narrow containers: `SelectCard` clips its title and wraps its hint instead of overflowing a
  shared row (give each card's wrapper `flex_1().min_w_0()`); `SegmentedButtons` shrink and clip
  their labels; `Popover` and `Menu` never exceed the space they sit in; a `Banner` with actions
  drops them to a second line; `ListGroup` rows and `MenuItem` trailing content no longer push
  past the group.
  - `Scheme::error_container` / `on_error_container`; `motion::replay` and `motion::pulse`.
- Added while porting a chat IDE to GPUI, batch 1:
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
- Added while porting a chat IDE to GPUI, batch 2 (code):
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
- Added while porting a chat IDE to GPUI, batch 3 (editor, layout and motion):
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
- Added for Atic Code's split panes: `Button::icon_only` (icon in a round button with the label
  as a tooltip, for narrow containers) and `Button::truncate` (the button shrinks and cuts its
  label with an ellipsis instead of overflowing its row).
- Interaction: hover state layer and ripples clipped to each component's rounded shape.
- `ImageThumb` keeps the image's aspect ratio: `size` caps the longest side and `aspect(w, h)`
  shapes the box from the image's real size, so an attached screenshot shows whole (a wide one
  comes out landscape) instead of being cropped into a square or stretched. Used by the
  composer's attachments and the images of a user message.
- Added for Atic Code's permission cards: `interaction::keyboard_focus` wires the Tab stop and
  the M3 focus ring in one step; `Button` and `ChoiceRow` are keyboard-focusable (Enter / Space
  click them), `Button::autofocus` / `ChoiceRow::autofocus` take the focus the first time the
  element appears, and `Button::tooltip` shows a hover tip (icon-only buttons keep building
  theirs from the label).
- Gallery example to try every component in light and dark, with any seed color and slow motion.
