# Task 015 — Color & stroke-width pickers + config persistence

> Phase 1.F · size: M · depends on: 001, 008

## Description

Phase 1's polish task. The toolbar gains a color swatch that opens a
color picker (`egui::color_picker`) and a stroke-width slider. Changes
update `state.color` / `state.stroke_width` immediately and are
persisted to config via `Config::save()` (debounced — write at most once
per second, or on editor close).

## Acceptance criteria

- [x] Color picker swatch shows the current `state.color`.
- [x] Picking a new color updates `state.color` live; subsequent
      annotations use it.
- [x] Stroke-width slider covers 1–20 px; updates `state.stroke_width`
      live.
- [x] On editor close, the latest `color` / `stroke_width` /
      `font_size` are written to `~/.config/flintshot/config.toml`.
- [x] Restarting `flintshot region` picks up the saved defaults.
- [x] Config write failures (permission, disk full) are surfaced as a
      non-blocking warning (annotation continues to work).

## Verification

- [x] Standing gate passes.
- [ ] Manual: change color to blue, change width to 8, draw a rect,
      save, close editor; rerun `flintshot region`, verify the toolbar
      shows blue and width 8. _(interactive; owed for Checkpoint F)_
- [ ] Manual: make config dir read-only (`chmod -R 555
      ~/.config/flintshot`); verify warning on close, no crash. _(interactive)_
- [x] Config roundtrip test (`save_to_then_load_from_roundtrips_values`)
      already covers the serialization path for color / stroke / font.

## Dependencies

Task 1, Task 8.

## Files likely touched

- `crates/flintshot-editor-egui/src/shell.rs` (picker widgets, debounce)
- `crates/flintshot-core/src/config.rs` (no schema change expected;
  confirm serialization roundtrips)

## Size

M
