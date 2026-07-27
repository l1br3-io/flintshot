---
status: active
started: 2026-07-20
---

# Spec — Phase 1: Capture + Editor Window

> Current effort spec (in flight). For the plan index, see
> [`plan.md`](plan.md); for the flat status checklist, see
> [`todo.md`](todo.md); per-task detail lives in [`tasks/`](tasks/).
> For product context, see [`../../VISION.md`](../../VISION.md); for
> sequencing, see [`../../ROADMAP.md`](../../ROADMAP.md).

## Goal

`flintshot region` opens an editor window over a screenshot and lets the
user annotate, save, or copy.

## Scope (in / out)

**In:**

- Single-monitor capture via `xcap`
- Editor window via `eframe` (implements `EditorShell`)
- Tools: Select (region crop), Rect, Arrow, Text, Blur
- Color & stroke-width pickers, persisted to TOML config
- Outputs: Save (`rfd`), Copy to clipboard (`arboard`)
- Undo/redo via snapshot history
- Keyboard shortcuts: `Esc`, `Ctrl+S`, `Ctrl+C`, `Ctrl+Z`, `Ctrl+Y`
- `cosmic-text` for text shaping
- HiDPI-aware coordinates (physical px in core, logical px at the shell)

**Out (deferred):**

- Multi-monitor capture (Phase 2)
- Daemon, tray, hotkeys, IPC (Phase 2)
- Ellipse / Line / Marker / Pencil / Number tools (Phase 2)
- Magnifier (Phase 2)
- Pin-to-desktop (Phase 3)
- Upload (Phase 4)
- Live overlay (Phase 5)

## Deliverables

- [x] `xcap` integration for single-monitor capture
- [x] `flintshot-editor-egui`: eframe window implementing `EditorShell`;
      screenshot uploaded via `Canvas::upload_image`
- [x] All annotation drawing routed through the `Canvas` trait from day
      one — no direct `egui::Painter` calls for annotations _(shared
      `flintshot-core::render::render()` walks `EditorState`; egui only
      implements primitives. Locked by ADR-0012.)_
- [x] Tools: **Select (region crop), Rectangle, Arrow, Text, Blur**
- [x] Color and stroke-width picker, persisted in config
- [x] Outputs: **Save** (`rfd` dialog), **Copy to clipboard** (`arboard`)
- [x] Undo/redo with snapshot history
- [x] Keyboard: `Esc` cancel, `Ctrl+S` save, `Ctrl+C` copy,
      `Ctrl+Z`/`Ctrl+Y` undo/redo
- [x] Initial config file at platform-appropriate location (`dirs` crate)
- [x] Text annotation rendering _(via `ab_glyph` + `fontdb`, not
      `cosmic-text`; see [ADR-0017](../../adrs/ADR-0017-ab-glyph-over-cosmic-text-phase-1.md).
      `cosmic-text` is the planned Phase 2 upgrade.)_
- [x] HiDPI-aware coordinates _(core in physical px; shell converts at
      the event boundary via `pixels_per_point`.)_

## Exit criteria

A user can run `flintshot region`, drag-select a region, draw a red
rectangle with an arrow pointing at it, add a text label, save the
result as a PNG, and have it land in their clipboard too. The
abstraction holds: no egui types leak into `flintshot-core`.
