# Task 006: Native path — grab-only guard scope

**Description:** extract `native_grab_prepared` (twin of the
fallback's `capture_prepared`) so the `CaptureGuard` releases right
after the grab; the editor session never holds the tray.

**Done:** `cli.rs` restructured; the grab/quiet-exit paths compile
unchanged around the new scope.

## Verification

- [x] `cargo check --workspace --all-targets`
- [ ] Live: tray returns while the editor is open (TEST_MATRIX row)
