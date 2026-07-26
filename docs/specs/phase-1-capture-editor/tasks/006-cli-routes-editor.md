# Task 006 — CLI routes `region` / `fullscreen` through `EguiShell`

> Phase 1.B · size: S · depends on: 003, 005

## Description

Replace Task 3's `--out PATH` default behavior with: capture →
`EguiShell::run_editor(state)` → print the outcome. Keep `--out PATH` as
an optional non-editor path (useful for headless captures / scripting).
Wire the active shell via the `egui` Cargo feature flag — non-`egui`
builds fall back to `--out` only.

## Acceptance criteria

- [x] `flintshot region` opens the editor (no `--out`).
- [x] `flintshot region --out PATH` skips the editor and writes the PNG
      directly (Task 3's path preserved).
- [x] When the editor returns `EditorOutcome::Saved(p)`, the CLI logs
      the path.
- [x] When the editor returns `Cancelled`, the CLI exits cleanly with
      no output file.
- [x] `cfg(not(feature = "egui"))` build still works: `region` without
      `--out` errors clearly ("editor not built; pass --out").

## Verification

- [x] Standing gate passes (including a `--no-default-features` build).
      _(Both `cargo clippy --workspace --all-targets -- -D warnings` and
      `cargo clippy --no-default-features -p flintshot-app --all-targets
      -- -D warnings` clean. Tests: 33 pass, 1 ignored [capture smoke],
      0 fail.)_
- [x] Manual: `cargo run -- region` opens editor, Esc cancels, no file
      written, clean exit code 0. _(Maintainer-verified on Linux +
      Wayland + XWayland, 2026-07-21, during the same `cargo run --
      region` pass that closed Checkpoint C.)_
- [x] Manual: `cargo run --no-default-features -- region --out /tmp/x.png`
      still works. _(Wrote a valid 2560×1440 RGBA PNG.)_

## Dependencies

Task 3, Task 5.

## Files likely touched

- `crates/flintshot-app/src/cli.rs`

## Size

S
