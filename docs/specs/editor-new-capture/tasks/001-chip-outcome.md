# Task 001 — Chip + outcome (core, editor-egui)

- `flintshot-core/src/shell.rs`: `NewCaptureMode` + appended
  `EditorOutcome::NewCapture { mode }`; re-export.
- `toolbar.rs`: `ToolbarAction::NewCapture(NewCaptureMode)`;
  `new_capture_chip` popup leading the rail (Region · Ctrl+N /
  Fullscreen); reuses `g_plus`.
- `app.rs`: `handle_new_capture` (publish + Close),
  `new_capture_shortcut` (Ctrl+N, winit-shaped modifiers),
  insertion between the dispatcher chords and the tool keys.
- `overlay.rs`: the publish-and-close twin arm.

## Verify

`cargo test -p flintshot-core -p flintshot-editor-egui` (incl. the
synthetic-context Ctrl+N test).
