# Task 005: Tray menu swap (Settings replaces Open config / Bind shortcuts)

**Description:** The tray menu becomes *Open app window / Capture /
Settings / Quit*. The *Bind shortcuts…* item (Wayland-gated) and
*Open config* item are removed along with their `TrayAction`
variants and the helper's handling arms; the rebind feature remains
reachable via `flintshot hotkeys bind` (untouched) and returns to
the GUI in Task 010's settings row.

**Acceptance criteria:**
- [ ] `const ID_SETTINGS: &str = "flintshot.settings"` added;
  menu built as Open app window / Capture / Settings / Quit with
  separators matching the existing rhythm (no Wayland-conditional
  item anymore, so `build`'s `wayland` parameter goes away — check
  callers)
- [ ] `action_for(flintshot.settings)` →
  `Some(TrayAction::Request(Request::OpenSettings))`
- [ ] `TrayAction::{BindShortcuts, OpenConfig}` variants removed;
  `action_for(flintshot.open-config)` and
  `action_for(flintshot.bind-shortcuts)` resolve to `None`, pinned
  in a `legacy_config_ids_are_gone` test (never reused)
- [ ] `tray_helper::event_loop` drops the `BindShortcuts` and
  `OpenConfig` arms; `exits_after_action(Request::OpenSettings) ==
  false` pinned (the helper stays alive, like `OpenApp`)
- [ ] The helper's `open_config_file` + `editor_argv` survive this
  task untouched (they move in Task 007) — only the dispatch arms
  and menu items change here

**Verification:**
- [ ] `cargo test -p flintshot-app --all-targets` green (tray
  feature)
- [ ] `cargo build --features tray` green

**Dependencies:** Task 2 (Request::OpenSettings exists)

**Files likely touched:**
- `crates/flintshot-app/src/tray.rs`
- `crates/flintshot-app/src/tray_helper.rs`

**Docs:**
- Module doc comment refresh (tray.rs's menu inventory paragraph).

**Estimated scope:** Small: 2 files
