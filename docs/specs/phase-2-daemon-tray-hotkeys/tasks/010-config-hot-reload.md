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

- [x] On daemon start, a `notify` watcher is attached to the config
      path (or its directory); events are debounced.
      _Watches the parent dir (catches editor temp-file rename saves);
      events coalesced by draining the channel once per loop iteration.
      Runtime-confirmed: "watching config for hot-reload"._
- [x] A `[hotkeys]` change unregisters the previous hotkeys and
      registers the new set, live (no daemon restart).
      _`HotkeyRegistry::refresh` drops the old manager (unregisters on
      drop) and builds a fresh one with the new chords._
- [x] A config write that momentarily yields invalid TOML does not
      deregister working hotkeys: the watcher keeps the last-known-good
      set and logs a warning, retrying on the next change.
      _`run_event_loop` catches `Config::load` failure, warns, and
      skips the refresh (registry untouched)._
- [x] Changes to `[ui]` / `[capture]` are read but do not force a
      hotkey re-registration (avoid needless churn).
      _`refresh` short-circuits via `hotkeys_changed` when the chords
      match; unit-tested._
- [x] Watcher errors (file moved, permissions) are logged; the daemon
      keeps running with the last-known config.
      _Watcher start failure is warn-and-continue (best-effort, like
      the tray)._

## Verification

- [x] Standing gate passes. _(fmt, default + tray clippy, workspace +
      tray tests, doc tests.)_
- [x] `#[ignore]`'d test (real filesystem): write a temp config, start
      the watcher, mutate, assert it fires. (`watcher_fires_on_config_
      write` — passes. The full hotkey re-registration needs a display;
      the pure `hotkeys_changed` comparison + `refresh` rebuild logic is
      unit-tested separately.)
- [ ] Manual (TEST_MATRIX): daemon running, edit `[hotkeys]`, save,
      press the new chord → works without restart. _Watcher + reload
      path runtime-confirmed; the actual key press is interactive._

## Dependencies

Task 9 (hotkey registration to re-drive).

## Files likely touched

- `crates/flintshot-app/src/config_watcher.rs` (new)
- `crates/flintshot-app/src/hotkeys.rs` (add unregister/re-register)
- `crates/flintshot-app/src/daemon.rs` (own the watcher)
- `crates/flintshot-app/src/main.rs` (wire module)

## Size

M
