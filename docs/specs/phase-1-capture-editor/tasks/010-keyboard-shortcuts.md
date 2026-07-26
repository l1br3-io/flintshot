# Task 010 — Keyboard shortcuts

> Phase 1.D · size: S · depends on: 009

## Description

Wire the Phase 1 keyboard surface. All shortcuts operate via the `Key`
enum from `flintshot-core::geom`, so the egui shell translates
`egui::Key` → `Key` and dispatches.

## Acceptance criteria

- [x] `Esc` cancels: drops pending, returns `EditorOutcome::Cancelled`.
      _(Routed through `EditorState::dispatch_input`; the shell's
      `logic()` sees `DispatchOutcome::Cancel`, publishes
      `Ok(Cancelled)`, and sends `ViewportCommand::Close`.)_
- [x] `Ctrl+S` triggers Save (same path as Task 9's button). _(Dispatcher
      returns `DispatchOutcome::Save`; shell calls the existing
      `handle_save` which runs `save_to_disk` + publish + close on
      success.)_
- [x] `Ctrl+C` triggers Copy (same path as Task 9's button). _(Same
      shape as Ctrl+S, calls `handle_copy` → `copy_to_clipboard`.)_
- [x] `Ctrl+Z` undoes last commit. _(Moved into the dispatcher;
      previously wired directly in `EditorApp::logic` in Task 8.)_
- [x] `Ctrl+Y` (and `Ctrl+Shift+Z`) redoes last undo. _(Both chords map
      to `state.redo()`. Tested independently in
      `dispatch_ctrl_y_redoes` and `dispatch_ctrl_shift_z_redoes`.)_
- [x] Shortcuts are documented in `docs/TEST_MATRIX.md` Phase 1 table
      for manual verification. _(Rows 58–61 were already present from
      the initial TEST_MATRIX layout; Task 10's implementation now
      makes them fillable. The L2 / Linux + Wayland + XWayland row is
      the next maintainer-verification target — see Checkpoint D
      carry-forward.)_

## Verification

- [x] Standing gate passes. _(`fmt --check`, `clippy --workspace
      --all-targets -- -D warnings`, `clippy --no-default-features -p
      flintshot-app --all-targets -- -D warnings`, `test --workspace
      --all-targets`, doc tests — all green. Test count: 74 pass (was
      57; +13 dispatcher tests + 4 Modifiers tests), 1 ignored (capture
      smoke), 0 fail.)_
- [ ] Manual: each shortcut tested end-to-end on L1 or M1. _(The
      dispatcher logic is unit-tested; the egui-side translation
      (`translate_modifiers`, key-event capture in `logic()`) needs the
      maintainer's interactive run. Scheduled for the Checkpoint D
      sign-off — same `cargo run -- region` pass that closes Task 9's
      Save / Copy interactive debt.)_
- [x] Unit test in egui crate: feed synthetic `Key` events through the
      shell's dispatcher; assert state machine transitions. _(Lives in
      `flintshot-core::editor::tests` against the dispatcher itself
      rather than in `flintshot-editor-egui` — the shell's translation
      layer is too thin to unit-test without a synthetic
      `egui::Context`, and the dispatcher is where every contract lives
      anyway. Thirteen new tests cover every shortcut + the pointer
      flow + edge cases like "stray drag before PointerDown is a
      no-op" and "Ctrl+Alt+S ignores Alt spoiling the chord".)_

## Dependencies

Task 9.

## Files likely touched

- `crates/flintshot-editor-egui/src/shell.rs`

## Size

S
