# HANDOFF · gpui-m3 — 2026-11-13

- Repo: `chrx3/gpui-m3` (crate `gpui-m3`, Material 3 Expressive para GPUI) · Mac: `~/dev/gpui-m3`
- Base: `main` (= `origin/main`, `846e2b8` «Add full-width buttons and markdown tweaks»). Última versión publicada: **0.2.0** (crates.io + release GitHub `v0.2.0`, tag en `e7ea9c6`).
- WIP: `wip/migracion-mac` (1 commit `7afd686`, 8 archivos, +119/−17).

## En qué estaba
Cambios pedidos por **atic-play** (chat 2026-11-12, subagente «gpui-m3: foco y animación»), sin commit en `main`:
- `IconButton`, `ListItem` y `RailItem` toman foco con Tab, muestran anillo de foco y se activan con Enter/Espacio si son clickeables.
- `BottomSheet::spring(...)` para elegir el resorte de entrada (por defecto `Spring::SPATIAL`).
- `WavyProgress::amplitude` anima la onda entre 0 (recta) y 3 px (`DEFAULT_AMPLITUDE`), para pasar a recta al pausar.
- Botón de ícono se encoge a 0.96 al presionar (antes 0.94); `IconButton` y `Fab` comparten `PRESS_SCALE`.
- Galería: demo de hoja con resorte más elástico y variables `M3_OPEN` / `M3_PART` para abrir una demo directo (para capturas).
- `CHANGELOG.md` ya tiene las entradas en ES/EN bajo la sección sin versión.

## Estado
- **Hecho:** código + changelog. atic-play compila contra esto (vía `[patch]` local) y pasaron sus tests en Windows.
- **Sin verificar:** tests/clippy de gpui-m3 por sí solo tras estos cambios (por confirmar); foco con Tab y hover con teclado/mouse reales; nada probado en macOS ni Linux (la CI de GitHub está desactivada).
- **Pendiente:** publicar versión nueva (¿0.2.1?) para que atic-play deje de depender de la copia local.

## Archivos clave
- `src/components/{button,lists,nav,sheets,visual}.rs`
- `examples/gallery.rs`, `examples/gallery/sheets.rs`
- `CHANGELOG.md`

## Próximos pasos
1. En el Mac: `git fetch`, revisar `git diff main..origin/wip/migracion-mac`.
2. `cargo test` y `cargo run --example gallery` en Mac (Tab, Enter/Espacio, `M3_OPEN=bottom-sheet-bouncy`).
3. Integrar a `main` (ver abajo) y subir.
4. Decidir versión (0.2.1 o 0.3.0 si se considera cambio de API; son solo métodos nuevos) y publicar en crates.io + release GitHub. Usar un token nuevo de crates.io como variable de entorno, sin `cargo login`.
5. En atic-play: quitar el `[patch]` local y fijar la versión nueva (ver `atic-play.md`).
6. Revisar que Atic (`apps/atic`) siga compilando con el cambio de `PRESS_SCALE` y el foco nuevo (la pill usa gpui-m3 por ruta relativa: se lleva estos cambios apenas estén en el checkout).

## Decisiones y contexto
- Atic usa gpui-m3 por ruta relativa (`../gpui-m3` junto a `atic`); atic-play y teams-rs por git (`https://github.com/chrx3/gpui-m3`). Lo que se suba a `main` llega a todos.
- El proyecto del video de la 0.2.0 vive **fuera del repo** en Windows (`C:\Users\Christian\gpui-m3-video`, con grabaciones, música y guion): no está en GitHub; copiarlo a mano si se quiere conservar. El MP4 final está en el release `v0.2.0`.
- En un chat se pegó un token de crates.io: revocarlo en crates.io (Account Settings → API Tokens) si no se hizo.
- Pendiente opcional del chat del catálogo: botón en la galería para abrir el ejemplo del reproductor de música (`examples/`).

## Notas para macOS
- Nada del wip es específico de Windows. Hasta ahora solo se probó en Windows: primera prueba real en Mac (Metal, foco de teclado, fuentes).
- En Mac, GPUI compila shaders en runtime en la pill de Atic (`runtime_shaders`); revisar si la galería necesita lo mismo con Xcode 26 (por confirmar).

## Cómo integrar
```bash
git checkout main && git pull
git merge --ff-only origin/wip/migracion-mac   # es un solo commit encima de main
cargo test && cargo clippy --all-targets
git push origin main && git push origin --delete wip/migracion-mac
```
Conviene reescribir el mensaje del commit (hoy es «Save work in progress…»), p. ej. «Add keyboard focus, sheet spring and wave amplitude».
