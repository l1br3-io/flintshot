# Task 002 — The recapture loop (app crate)

- `cli.rs`: `run_editor_session` extraction; the
  `run_editor_with_config` loop over `recapture_request`;
  `run_overlay_with_config` loops identically;
  `recapture_image` (grace + `capture_with_tray_hidden`);
  `selection_first_for_recapture`.
- `app_window.rs`: `HIDE_GRACE_MS` becomes `pub(crate)` (shared
  bound).

## Verify

Loop-decision unit tests; manual/XV1 chain pass in task 003.
