# Task 015 — Color & stroke-width pickers + config persistence

> Phase 1.F · size: M · depends on: 001, 008

## Description

Phase 1's polish task. The toolbar gains a color swatch that opens a
color picker (`egui::color_picker`) and a stroke-width slider. Changes
update `state.color` / `state.stroke_width` immediately and are
persisted to config via `Config::save()` (debounced — write at most once
per second, or on editor close).

## Acceptance criteria

- [ ] Color picker swatch shows the current `state.color`.
- [ ] Picking a new color updates `state.color` live; subsequent
      annotations use it.
- [ ] Stroke-width slider covers 1–20 px; updates `state.stroke_width`
      live.
- [ ] On editor close, the latest `color` / `stroke_width` /
      `font_size` are written to `~/.config/flintshot/config.toml`.
- [ ] Restarting `flintshot region` picks up the saved defaults.
- [ ] Config write failures (permission, disk full) are surfaced as a
      non-blocking warning (annotation continues to work).

## Verification

- [ ] Standing gate passes.
- [ ] Manual: change color to blue, change width to 8, draw a rect,
      save, close editor; rerun `flintshot region`, verify the toolbar
      shows blue and width 8.
- [ ] Manual: make config dir read-only (`chmod -R 555
      ~/.config/flintshot`); verify warning on close, no crash.

## Dependencies

Task 1, Task 8.

## Files likely touched

- `crates/flintshot-editor-egui/src/shell.rs` (picker widgets, debounce)
- `crates/flintshot-core/src/config.rs` (no schema change expected;
  confirm serialization roundtrips)

## Size

M
