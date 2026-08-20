# Task 004: Tray "Bind shortcuts…" item + `ShortcutsChanged` refresh + Wayland hot-reload notice

**Description:** Integration polish — three small behaviors that make
the feature whole: (1) a tray menu item "Bind shortcuts…" (Linux
portal contexts only — gate on `session_is_wayland()` so X11 trays
don't grow a dead item) whose click triggers the same portal bind
path as the IPC request, crossing the main → tokio thread boundary
via a sync-callable `tokio::sync::Notify` (or equivalent); (2)
`receive_shortcuts_changed()` consumption — when the user re-binds
from the compositor's own settings UI, refresh the id → Request map
and log the new set; (3) config hot-reload semantics on Wayland: a
`[hotkeys]` change must **not** attempt `HotkeyRegistry::refresh`
on the portal path (re-binding needs the user dialog) — log a
notice pointing at `flintshot hotkeys bind` instead, while existing
portal bindings keep firing.

**Acceptance criteria:**
- [ ] Tray menu (when built with `--features tray` on a Wayland
      session) shows "Bind shortcuts…"; clicking it opens the bind
      dialog; on non-Wayland the item is absent.
- [ ] `TrayAction` grows the bind variant; unknown/other-platform
      behavior of `action_for` unchanged (existing tray tests
      pass).
- [ ] `ShortcutsChanged` → id map refreshed + `info` log; verified
      by re-binding via compositor settings while the daemon runs.
- [ ] Editing `[hotkeys]` while the daemon runs on Wayland: notice
      logged ("suggestions changed; re-run `flintshot hotkeys
      bind`"), no crash, previously bound shortcuts still fire, no
      dialog spawns.
- [ ] Same edit on X11: existing `HotkeyRegistry::refresh` behavior
      unchanged (re-registers).
- [ ] Tray Quit / Ctrl-C still shut down cleanly with the portal
      task + notify wired in.

**Verification:**
- [ ] `cargo test --quiet --workspace --all-targets` (tray
      `action_for` unit tests, `TrayAction` mapping)
- [ ] Manual on COSMIC or KDE with `--features tray`: click path,
      compositor-settings re-bind path, config-edit path (three
      flows from the spec's exit criteria)
- [ ] Manual on X11 with `--features tray`: menu has no bind item;
      `[hotkeys]` edit re-registers as before
- [ ] Clippy/fmt gates

**Dependencies:** Tasks 002, 003

**Files likely touched:**
- `crates/flintshot-app/src/tray.rs` (item + `TrayAction` variant)
- `crates/flintshot-app/src/daemon.rs` (tray click → notify; Wayland
  reload branch)
- `crates/flintshot-app/src/hotkeys_portal.rs` (changed-signal
  consumption)

**Docs:**
- The Wayland reload asymmetry (registry refresh = X11-only) gets a
  comment in `daemon.rs` next to the branch, since it reads like a
  bug otherwise.

**Estimated scope:** Medium (3 files)
