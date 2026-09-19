# Task 009: Config-file links + read-only defaults view

**Description:** The settings page gains a *Config file* section
with the VSCode pair: **Edit config file** (ensures the file exists,
opens it in `$EDITOR`) and **View default config** (expands an
inline, read-only, monospace rendering of the embedded default
TOML). Actions drain through new default-bodied `HomeBridge`
methods; the app crate's bridge implements them over Task 007's
module and Task 001's accessor. Load the
`.opencode/skills/frontend-design/SKILL.md` guide before the visual
pass.

**Acceptance criteria:**
- [ ] New `HomeBridge` methods (default-bodied, test-double safe):
  `open_config_file(&mut self)` and
  `default_config_toml(&mut self) -> String` (default:
  `flintshot_core::default_config_toml().to_owned()`)
- [ ] `SettingsApp` holds a drained action queue:
  `SettingsAction::{EditConfigFile}` collected by clicks,
  `drain_action()` consumed by the window router, which services it
  via the bridge (spawn the editor off the UI thread — a plain
  thread, mirroring the tray helper's bind-dialog precedent)
- [ ] The *Config file* section renders both links with consistent
  link affordances per the theme tokens (frontend-design pass:
  spacing, hierarchy, monospace card styling for the defaults)
- [ ] The defaults view: collapsed by default; expanding renders
  the embedded TOML in a monospace card that is visibly read-only
  (no editable widget registers; pin headlessly — e.g. no
  `TextEdit` response id exists for it) and scrolls within the
  page's ScrollArea
- [ ] Headless click tests: both links register + report (the edit
  link enqueues the action; the defaults link toggles the view and
  the TOML text renders)
- [ ] App bridge: `Bridge::open_config_file` calls
  `crate::config_edit::open_config_in_editor()` (errors logged, not
  fatal); `default_config_toml` returns the core asset

**Verification:**
- [ ] `cargo test --workspace --all-targets` green
- [ ] `cargo run -- app --route settings`: both links work
  (manual; `$EDITOR` set and unset)

**Dependencies:** Tasks 1, 7, 8

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs` (bridge trait)
- `crates/flintshot-editor-egui/src/settings.rs`
- `crates/flintshot-editor-egui/src/window.rs` (drain + service)
- `crates/flintshot-app/src/app_window.rs` (bridge impl)

**Docs:**
- None beyond code docs (the asset carries the user-facing docs).

**Estimated scope:** Medium: 4 files
