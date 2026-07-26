# SPEC — Phase 1: Capture + Editor Window

> Current phase spec. For strategy, see [`tasks/plan.md`](tasks/plan.md);
> for the per-task checklist, see [`tasks/todo.md`](tasks/todo.md). For
> product context, see [`docs/VISION.md`](docs/VISION.md); for sequencing,
> see [`docs/ROADMAP.md`](docs/ROADMAP.md).

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
- [ ] All annotation drawing routed through the `Canvas` trait from day
      one — no direct `egui::Painter` calls for annotations
- [ ] Tools: **Select (region crop), Rectangle, Arrow, Text, Blur**
- [ ] Color and stroke-width picker, persisted in config
- [ ] Outputs: **Save** (`rfd` dialog), **Copy to clipboard** (`arboard`)
- [ ] Undo/redo with snapshot history
- [ ] Keyboard: `Esc` cancel, `Ctrl+S` save, `Ctrl+C` copy,
      `Ctrl+Z`/`Ctrl+Y` undo/redo
- [x] Initial config file at platform-appropriate location (`dirs` crate)
- [ ] `cosmic-text` for text annotation rendering
- [ ] HiDPI-aware coordinates

## Exit criteria

A user can run `flintshot region`, drag-select a region, draw a red
rectangle with an arrow pointing at it, add a text label, save the
result as a PNG, and have it land in their clipboard too. The
abstraction holds: no egui types leak into `flintshot-core`.
