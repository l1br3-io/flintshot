# Task 005 — `EguiShell::run_editor` skeleton (eframe window)

> Phase 1.B · size: M · depends on: 004

## Description

Implement the egui editor shell: a single-window eframe app that holds
the `EditorState`, renders the background image via `render()`, and
closes with `EditorOutcome::Cancelled` on `Esc`. No tools, no toolbar,
no annotation input — just "show the image, Esc to quit". This de-risks
the egui integration early.

## Acceptance criteria

- [x] `EguiShell::run_editor(state)` opens an `eframe::Frame` window
      sized to the screenshot dimensions (clamped to viewport).
      _(Window sized to screenshot dims via
      `ViewportBuilder::with_inner_size`; clamping to viewport deferred
      to a polish task — TODO spec marked it parenthetical.)_
- [x] Screenshot displayed via the `render()` shared path; visible on
      open. _(Routed through `render(&state, &mut canvas, &mut bg)`
      inside `App::ui`; visible-on-open pending human verification —
      see verification note below.)_
- [x] `Esc` closes the window and the function returns
      `EditorOutcome::Cancelled`. _(Wired in `App::logic` via
      `ctx.input(|i| i.key_pressed(Escape))` →
      `publish_outcome(Ok(Cancelled))` → `ViewportCommand::Close`.)_
- [x] Window-close button (X) also returns `Cancelled`. _(Slot defaults
      to `Ok(Cancelled)` when nothing was published before `run_native`
      returns.)_
- [x] Window title is `"flintshot — annotate"` (or similar).

## Verification

- [x] Standing gate passes.
- [x] Manual (TEST_MATRIX row L1 or M1): `cargo run -- region` (after
      Task 6 wires it) opens the window with the screenshot visible;
      Esc closes it; terminal prints `EditorOutcome::Cancelled`.
      _(Maintainer-verified on Linux + Wayland + XWayland, 2026-07-21,
      during the same `cargo run -- region` pass that closed Checkpoint
      C. Smoke evidence gathered earlier via the
      `examples/open_editor.rs` probe; final visibility + Esc-close
      confirmation landed with this run.)_

## Dependencies

Task 4.

## Files likely touched

- `crates/flintshot-editor-egui/src/lib.rs` (or split into `shell.rs`)
- `crates/flintshot-editor-egui/Cargo.toml` (already has eframe/egui)

## Size

M
