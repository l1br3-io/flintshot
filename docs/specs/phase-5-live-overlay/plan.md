# Plan: Phase 5 — Live overlay

> Thin index. Requirements live in [`spec.md`](spec.md). Task files
> under `tasks/` are authored when each task starts (per-task:
> description, acceptance criteria, verification commands).

## Overview

Freeze-then-annotate: capture via the existing `xcap` path, then show
the frozen frame fullscreen (borderless eframe viewport — the Phase 3
pin window is the precedent), dim everything outside the user's
region, and drive selection + all annotation through the unchanged
`EditorState` dispatcher. Outputs reuse the Phase 1–4 pipelines;
Enter copies, Esc cancels. Wayland falls back to the editor window
honestly.

## Architecture decisions

1. **`OverlayApp` is a new eframe app in
   `flintshot-editor-egui::overlay`, not a mode on `EditorApp`.** The
   editor window's chrome (top toolbar panel, window-resize-on-crop)
   is structurally wrong for a fullscreen overlay; sharing
   `EditorState` + `render()` + the `toolbar` chip components is the
   reuse that pays. → Task 002.
2. **Selection chrome (dim mask, 8 handles, move, W×H badge) is
   overlay-owned.** The dispatcher already owns selection *geometry*;
   handles are hit-testing + cursor-shape concerns at the shell, with
   the handle/hit math as pure functions (unit-tested) — candidate
   for `core::geom` only if the COSMIC crate ever needs it. → Task
   003.
3. **Mode resolution order: CLI flag > config > platform fallback.**
   `capture_and_dispatch` in `cli.rs` resolves it once and logs the
   source of the decision. → Tasks 001, 007.
4. **Floating toolbar = compact rail + accept row, anchored to the
   selection**, flipping above/below and clamping to the viewport.
   Reuses `toolbar`'s `ToolSpec`/glyph chips and action handlers;
   new layout only. → Task 004.
5. **No second dispatcher.** Overlay pointer/key events translate to
   `InputEvent`s exactly like `EditorApp::logic` does (the translation
   fns are factored out for reuse where they're literal duplicates).
   → Task 002.
6. **Enter = Copy**; Esc = Cancel; Save/Pin/Upload buttons reuse
   `compose_final[_region]` + the temp-PNG/worker paths verbatim
   (X11 clipboard-owner rule: Copy keeps the process alive — same
   policy as the editor window's "Copied" state, adapted to overlay
   close semantics). → Task 006.

## Task index

### Stage 1 — Plumbing (core + CLI)
- [ ] Task 001: `interaction_mode` config + `--overlay`/`--editor` flags + resolution helper
  - *Checkpoint: config round-trip test; flags parse; resolution unit tests; all gates green.*

### Stage 2 — Overlay shell MVP
- [ ] Task 002: `OverlayApp` fullscreen viewport + frozen-capture render + dispatcher wiring + Esc/Enter
  - *Checkpoint: overlay shows frozen screen, drag draws a selection, Esc exits clean — X11.*
- [ ] Task 003: selection chrome — dim mask, 8 resize handles, move, W×H badge, magnifier on hover
- [ ] Task 004: floating toolbar anchored to selection (tool rail + outputs + accept)

### Stage 3 — Annotation + outputs
- [ ] Task 005: all ten tools inside the selection (clip rendering to region; verify text/number/blur/marker parity with editor window)
- [ ] Task 006: output flows — Enter=Copy, Save dialog, Pin/Upload workers, outcome publication
  - *Checkpoint: full Flameshot-equivalent flow works end-to-end on X11.*

### Stage 4 — Platform matrix + polish
- [ ] Task 007: Wayland detect + fallback (+ force-flag semantics), macOS/Windows verification pass
- [ ] Task 008: idle-repaint discipline (on-demand egui repaint), perf check, rough edges
- [ ] Task 009: docs — ADR-0028, CHANGELOG, TEST_MATRIX rows, spec flip to active→shipped on completion
