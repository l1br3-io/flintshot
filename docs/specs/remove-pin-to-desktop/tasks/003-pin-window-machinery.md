# Task 003: Delete pin window + `EditorOutcome::Pinned` + `run_pin` trait

**Description:** Remove the pin window machinery end-to-end: the
`PinApp` module, `EguiShell::run_pin` / `run_pin_blocking` /
`run_pin_window` / `with_pin_config`, the `EditorShell::run_pin`
trait method and `EditorOutcome::Pinned` variant in core, plus every
exhaustive-match arm and test that names the variant. After this
task, only the IPC pair (`PinLast`/`Pinned`) and `PinConfig` remain.

**Acceptance criteria:**
- [ ] `crates/flintshot-editor-egui/src/pin.rs` deleted; `mod pin`
      gone from the crate root and crate docs
- [ ] `EguiShell::run_pin` impl, `run_pin_blocking`,
      `run_pin_window`, `with_pin_config` + the
      `with_selection_first_composes_with_pin_config` test removed
      from editor-egui `shell.rs`
- [ ] `EditorShell::run_pin` trait method removed from core `shell.rs`
- [ ] `EditorOutcome::Pinned` variant deleted from core `shell.rs`;
      exhaustive matches/tests updated (`cli.rs` status arm +
      `session_next` test, editor-egui `shell.rs` variant test)
- [ ] No `PinConfig` import left in editor-egui

**Verification:**
- [ ] `cargo test --workspace --all-targets` green (default members)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
      (cosmic still compiles: it only uses the not-yet-removed IPC
      variants)
- [ ] `rg -n "pin.rs|PinApp|run_pin|with_pin_config|EditorOutcome::Pinned" crates/` empty

**Dependencies:** Tasks 001, 002

**Files likely touched:**
- `crates/flintshot-editor-egui/src/pin.rs` (delete)
- `crates/flintshot-editor-egui/src/shell.rs`
- `crates/flintshot-editor-egui/src/lib.rs`
- `crates/flintshot-core/src/shell.rs`
- `crates/flintshot-app/src/cli.rs` (match arm + test)

**Docs:**
- None expected.

**Estimated scope:** Medium (5 files, deletion-heavy)
