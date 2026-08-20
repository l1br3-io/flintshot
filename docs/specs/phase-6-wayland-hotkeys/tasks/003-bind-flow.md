# Task 003: Bind flow — portal dialog with config-suggested chords, CLI end-to-end

**Description:** Make `flintshot hotkeys bind` work end-to-end. The
IPC server's `handle_connection` intercepts `Request::BindShortcuts`
(next to its `Subscribe` special-case) and triggers the portal
manager's bind: `bind_shortcuts(&session, &[NewShortcut…], None,
options)` with one `NewShortcut` per capture id, each carrying a
human description ("Capture region" / "Capture fullscreen") and a
**suggested** binding converted from the `[hotkeys]` config chords
by a pure converter fn (`global-hotkey` accelerator syntax → portal
accelerator syntax, e.g. `PrintScreen` → `Print`). Awaiting the
response blocks until the user closes the dialog; the daemon then
logs the *actual* bindings the user chose and updates the id →
Request map (suggestions are not authority). The CLI prints the
outcome. A chord that fails conversion degrades to "no suggestion"
(bind still works), logged at warn.

**Acceptance criteria:**
- [ ] `Request::BindShortcuts` arriving at the daemon opens the
      portal bind dialog (user-initiated only — nothing in daemon
      startup calls bind).
- [ ] Suggested chords come from the loaded `[hotkeys]` config via
      a pure, unit-tested converter; default chords convert to
      valid portal accelerators.
- [ ] After the user binds, the daemon logs each bound shortcut
      (id + effective trigger) at `info` and activations of the new
      bindings dispatch captures without a daemon restart.
- [ ] User cancelling / closing the dialog is a defined outcome:
      logged, id map unchanged, daemon healthy.
- [ ] `flintshot hotkeys bind` prints bound chords on success and
      exits 0; prints the failure reason and exits non-zero
      otherwise.
- [ ] No daemon → Task 001's clear error still holds.
- [ ] A second `hotkeys bind` run replaces the session's bindings
      cleanly (portal treats it as re-bind).

**Verification:**
- [ ] `cargo test --quiet --workspace --all-targets` (converter +
      mapping units)
- [ ] Manual on COSMIC or KDE: `cargo run -- daemon` in one
      terminal; `cargo run -- hotkeys bind` in another → dialog
      shows both shortcuts with suggested chords → bind → press
      keys → captures fire; re-run `bind` to change one → only the
      new one fires
- [ ] Manual cancel path: open dialog, dismiss → daemon still
      serves tray/IPC, log shows cancellation
- [ ] Clippy/fmt gates

**Dependencies:** Tasks 001, 002

**Files likely touched:**
- `crates/flintshot-app/src/hotkeys_portal.rs` (bind method,
  converter, map refresh)
- `crates/flintshot-app/src/ipc_server.rs` (`BindShortcuts`
  interception + reply)
- `crates/flintshot-app/src/cli.rs` (result rendering for `hotkeys
  bind`, if beyond Task 001's stub)

**Docs:**
- Converter gotchas (key-name table) → comment next to the fn.

**Estimated scope:** Medium (3 files)
