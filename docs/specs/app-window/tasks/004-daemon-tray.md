# Task 004 — Daemon dispatch + tray item

## What

- `crates/flintshot-app/src/worker.rs`:
  - `Request::OpenApp` dispatch arm: broadcast
    `Event::AppFocus` to subscribers, then spawn the `app` worker
    (`app_argv()` → `["app"]`). The spawned worker exits fast on
    a held app lock — the broadcast is what focuses an existing
    window; the spawn is what creates a first one.
  - `CommandSpawner::with_bus(bus: SubscriberBus)` — the reaper
    closure broadcasts `Event::SessionEnded` after `child.wait()`
    when the argv is a capture worker. Pure helper
    `is_capture_argv(argv) -> bool` (`region` / `fullscreen`),
    comment tying it to `spawns_capture_worker` (request-side
    twin).
  - Update daemon call sites to construct the bus-carrying
    spawner (daemon.rs, ipc_server.rs where `dispatch` is
    invoked).
- `crates/flintshot-app/src/tray.rs`: *Open app* item at the top
  (`flintshot.app` id) → `TrayAction::Request(Request::OpenApp)`;
  tests for the mapping.
- Keep dispatch unit tests hermetic: `RecordingSpawner` path
  unchanged; the reaper's broadcast is exercised via a real
  short-lived child in a unix-gated test (mirrors daemon.rs
  helper-lifecycle tests).

## Where

- `crates/flintshot-app/src/{worker.rs,daemon.rs,ipc_server.rs,tray.rs}`.

## Verify

- `cargo test -p flintshot-app` (dispatch arm, argv builder,
  capture-argv classification, tray mapping, unix reaper test)
- Manual: tray *Open app* opens/focuses; hotkey capture while
  window hidden doesn't cause early restore.
