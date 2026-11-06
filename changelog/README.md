# Release notes

`changelog.json` is the single source of the notes of every version, newest first. It feeds the
gallery's **What's new** section, `CHANGELOG.md` and the GitHub release notes.

```json
{ "version": "0.2.0", "date": "2026-11-06",
  "title": { "es": "…", "en": "…" },
  "items": [
    { "kind": "new", "es": "…", "en": "…",
      "image": "changelog/0.2.0/button-group-dark.png", "section": 0 }
  ] }
```

- `kind` is `new`, `improved` or `fixed`. Spanish (`es`) is written first, then English (`en`).
- `image` is optional: a path from the repository root. `section` is optional too: the gallery
  section that shows the component (a "See in the gallery" button jumps to it).
- Keep a version's longer, hand-written account in `changelog/<version>/notes.md`; it goes under
  that version in `CHANGELOG.md`.

## Adding a version

1. Add the entry at the top of `changelog.json` (it can be written while the version is in
   progress).
2. Take its pictures in light and dark: `.\scripts\capture.ps1 -Version 0.2.0 -Section 0 -Name buttons`
   (Windows; build the gallery first with `cargo build --example gallery`).
3. `node scripts/changelog.mjs md` rewrites `CHANGELOG.md`; `node scripts/changelog.mjs notes 0.2.0`
   prints the GitHub release notes; `check` fails if `CHANGELOG.md` is out of date.
4. `cargo test --example gallery changelog` checks the file, its images and its sections.
