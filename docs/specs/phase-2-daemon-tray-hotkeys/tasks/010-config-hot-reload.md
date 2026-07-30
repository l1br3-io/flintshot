# Task 010 — Config hot-reload via `notify`

> Phase 2.E · size: M · depends on: 009

## Description

Add `flintshot-app::config_watcher`: watch `config.toml` with `notify`
(debounced), re-read on change, and when the `[hotkeys]` section
differs from the currently-registered set, unregister the old hotkeys
and register the new ones — without restarting the daemon. This is the
spec's exit-criterion step 4. A mid-write fire must not break things:
debounce + tolerate a transient parse failure by keeping the
last-known-good hotkeys and logging a warning.

## Acceptance criteria

- [ ] On daemon start, a `notify` watcher is attached to the config
      path (or its directory); events are debounced.
- [ ] A `[hotkeys]` change unregisters the previous hotkeys and
      registers the new set, live (no daemon restart).
- [ ] A config write that momentarily yields invalid TOML does not
      deregister working hotkeys: the watcher keeps the last-known-good
      set and logs a warning, retrying on the next change.
- [ ] Changes to `[ui]` / `[capture]` are read but do not force a
      hotkey re-registration (avoid needless churn).
- [ ] Watcher errors (file moved, permissions) are logged; the daemon
      keeps running with the last-known config.

## Verification

- [ ] Standing gate passes.
- [ ] `#[ignore]`'d test (real filesystem): write a temp config, start
      the watcher, mutate `[hotkeys]`, assert the registered set
      updates; write invalid TOML, assert no deregistration + a warning
      logged.
- [ ] Manual (TEST_MATRIX): daemon running, edit
      `~/.config/flintshot/config.toml`'s `[hotkeys]`, save, press the
      new chord → it works without restarting the daemon.

## Dependencies

Task 9 (hotkey registration to re-drive).

## Files likely touched

- `crates/flintshot-app/src/config_watcher.rs` (new)
- `crates/flintshot-app/src/hotkeys.rs` (add unregister/re-register)
- `crates/flintshot-app/src/daemon.rs` (own the watcher)
- `crates/flintshot-app/src/main.rs` (wire module)

## Size

M
