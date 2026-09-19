# Task 010: Wayland-gated *Rebind shortcuts…* row

**Description:** The Shortcuts section gains a *Rebind shortcuts…*
action row, rendered only when the bridge reports a Wayland session
(the tray item's gate, moved into Settings). The click drives the
same portal path the tray item used: `ipc_client::bind_shortcuts_to`
on a plain thread, outcomes logged.

**Acceptance criteria:**
- [ ] `HomeBridge::session_is_wayland(&mut self) -> bool`
  (default: `false`) and
  `HomeBridge::request_rebind(&mut self)` (default: no-op)
- [ ] `SettingsAction::RebindShortcuts` joins the drained action
  queue; the window router services it through the bridge
- [ ] The row renders iff `session_is_wayland()` (the setting is
  snapshot into `SettingsApp` at construction, like
  `display_server`); X11 renders no dead row — pinned headlessly
  for both gate values
- [ ] App bridge: `session_is_wayland` delegates to
  `crate::platform::session_is_wayland()`; `request_rebind` spawns
  the bind thread (the tray helper's old arm, verbatim: outcome
  logs on success / no-daemon / failure)
- [ ] Headless click test: the row reports through the queue when
  present

**Verification:**
- [ ] `cargo test --workspace --all-targets` green
- [ ] Manual on a Wayland session: row present, dialog opens

**Dependencies:** Task 9 (the action-queue plumbing exists)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs`
- `crates/flintshot-editor-egui/src/settings.rs`
- `crates/flintshot-editor-egui/src/window.rs`
- `crates/flintshot-app/src/app_window.rs`

**Docs:**
- Why-comment at the gate (portal bind dialogs are Wayland-only;
  X11 re-registers from `[hotkeys]`).

**Estimated scope:** Small: 4 files (touch-sized edits each)
