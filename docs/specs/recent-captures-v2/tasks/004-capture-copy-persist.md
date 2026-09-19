# Task 004: Capture-path clipboard copy persists

**Description:** `after_grab = clipboard` on the native capture path
(`cli.rs` `PostGrabAction::Clipboard` arm) gains the same persistence:
after the clipboard write succeeds and `save-clipboard-copies` is on,
save the grab via core's `clipboard_copy_path` + `save_image` into the
resolved dir and record it into recents at the configured limit. Best
effort — a persist failure logs and never fails the capture. The helper
lives in `capture_flow.rs` (core-based, no editor-egui dependency:
workers may build without the `egui` feature).

**Acceptance criteria:**
- [ ] The persistence helper (unit-tested with a tempdir + injected config): on-flag → file written + recents recorded; off-flag → nothing written
- [ ] A persist failure returns `None`/logs without erroring the capture
- [ ] The Clipboard arm calls it after the copy, before the editor shell

**Verification:**
- [ ] `cargo test -p flintshot-app --features egui` and without features (helper must compile feature-lean)
- [ ] fmt + clippy gates; workspace tests

**Dependencies:** Tasks 001, 002

**Files likely touched:**
- `crates/flintshot-app/src/capture_flow.rs`
- `crates/flintshot-app/src/cli.rs`

**Docs:**
- None

**Estimated scope:** Small
