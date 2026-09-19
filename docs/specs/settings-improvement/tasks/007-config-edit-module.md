# Task 007: Shared open-in-editor module (`config_edit.rs`)

**Description:** Extract the open-config-in-`$EDITOR` logic from the
tray helper into `crates/flintshot-app/src/config_edit.rs`, updated
for the new model: `ensure_user_config()` runs first (Task 001's
template writer), then `$EDITOR`/`$VISUAL` launches on the path, else
the path is printed. This module is what the settings page's *Edit
config file* link (Task 009) will call.

**Acceptance criteria:**
- [ ] `config_edit::open_config_in_editor() -> eyre::Result<()>`:
  resolves the platform config path, runs `ensure_user_config()`
  (log-on-error, never fatal to the path resolution), then the
  editor spawn / path-print flow (today's tray behavior, doc
  comment carrying the no-controlling-terminal GOTCHA from the spec)
- [ ] `editor_argv` moved here from `tray_helper` (pub(crate) or
  test-scoped as needed) with its three tests
  (`code --wait` splits, plain word, blank → empty)
- [ ] `tray_helper.rs` no longer defines either fn (no dead code
  left behind; the module shrinks to the pump/loop/watch)
- [ ] `mod config_edit;` registered in `main.rs`

**Verification:**
- [ ] `cargo test -p flintshot-app --all-targets` green
- [ ] `cargo build` green

**Dependencies:** Task 1 (`ensure_user_config` exists)

**Files likely touched:**
- `crates/flintshot-app/src/config_edit.rs` (NEW)
- `crates/flintshot-app/src/tray_helper.rs`
- `crates/flintshot-app/src/main.rs`

**Docs:**
- Module doc: why the logic lives outside the helper now (the
  settings page is the caller).

**Estimated scope:** Small: 3 files
