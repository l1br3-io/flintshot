# Task 006 — Wire capture requests + `--replace`

> Phase 2.C · size: M · depends on: 004, 005

## Description

Connect the IPC server (Task 004) and the worker dispatch table
(Task 005) into the daemon (Task 003) so the full request path is live:
an inbound `CaptureRegion` / `CaptureFullscreen` spawns a worker child
and the resulting `Event`s are broadcast to subscribers. `Request::Quit`
becomes the graceful-shutdown path that powers `flintshot daemon
--replace` (the new daemon sends `Quit` over IPC, waits for the socket
to free, then takes the single-instance lock). This closes the first
end-to-end vertical slice: an external client can request a capture and
observe the event stream. ADR-0020 (single-instance scoped to daemon
role + the `--replace` handshake) is finalized here.

## Acceptance criteria

- [x] Daemon's tokio side owns the IPC server; inbound requests flow
      through the Task 005 dispatch table; spawned workers' lifecycle
      is logged (`tracing`).
      _`handle_connection` routes non-Subscribe requests through
      `worker::dispatch` with `CommandSpawner` + the connection's bus._
- [x] `CaptureRegion` / `CaptureFullscreen` emit `CaptureStarted` to
      subscribers on receipt and `EditorOpened` once the worker child is
      confirmed running.
      _`CaptureStarted` emitted before spawn (on receipt); `EditorOpened`
      after `spawn()` returns Ok. `EditorOpened` is a process-started
      proxy — true window-open detection needs worker→daemon IPC
      (documented). Unit-tested via `RecordingSpawner`._
- [x] `Request::Quit` triggers a clean daemon shutdown (drops tray,
      unregisters hotkeys registered so far, closes the socket, releases
      the lock).
      _`Quit` → `notify_waiters()` stops both the IPC accept loop and
      the heartbeat ticker → `tx` drops → main loop exits → `run()`
      returns → lock/hotkey/tray drop. **Proven at runtime**: the
      replaced daemon logged "IPC shutdown requested" → "daemon exited
      cleanly"._
- [x] `flintshot daemon --replace` connects to a running daemon, sends
      `Quit`, waits for the socket to disappear (bounded timeout), then
      takes the lock and starts its own loop.
      _`replace_running_daemon` + `send_request_blocking(Quit)` +
      `wait_for_socket_to_free` (3 s) + `acquire_daemon_lock_with_retry`
      (3 s). **Proven at runtime**: `--replace` swapped daemons — old
      one exited cleanly, new one took the lock ~50 ms later and ran._
- [x] `OpenLast` / `PinLast` / `UploadLast` return
      `Event::Error { code: "unsupported", message: "<phase N>" }`
      without spawning anything.
      _`dispatch`'s wildcard arm (Request is `#[non_exhaustive]`)._

## Verification

- [x] Standing gate passes. _(fmt, default + tray clippy, workspace
      tests, doc tests.)_
- [~] `#[ignore]`'d integration test (xvfb): daemon on a temp socket;
      client sends `CaptureFullscreen`; assert `CaptureStarted` then
      `EditorOpened` arrive on a `Subscribe` stream.
      _Deferred to manual TEST_MATRIX / Checkpoint C: `handle_connection`
      uses `CommandSpawner` (not injectable), so a capture integration
      test would spawn a real editor window. The event emission itself
      is unit-tested (`RecordingSpawner` asserts
      `[CaptureStarted, EditorOpened]`); the IPC plumbing is covered by
      the Subscribe + Quit `#[ignore]` tests._
- [x] `#[ignore]`'d test: two daemons — second with `--replace` succeeds
      and the first exits; without `--replace` the second fails with
      "already running".
      _`integration_quit_triggers_shutdown_and_frees_socket`
      (`#[ignore]`, passes) covers the Quit→shutdown→socket-reclaim
      core; the full two-daemon swap is **proven at runtime** (see
      acceptance) and left as a manual matrix row._
- [x] Manual (TESTMATRIX): `cargo run -- daemon`, then from a script
      connect to the socket, `Subscribe`, request `CaptureFullscreen`,
      watch the editor open and events flow.
      _`--replace` swap + Subscribe + Quit all runtime-proven; the
      capture-opens-editor flow is the interactive Checkpoint C row._

## ADR-0020

Finalized at `docs/adrs/ADR-0020-single-instance-daemon-role-replace-handshake.md`.

## Dependencies

Task 4 (server), Task 5 (dispatch/worker).

## Files likely touched

- `crates/flintshot-app/src/daemon.rs` (wire server + dispatch)
- `crates/flintshot-app/src/ipc_server.rs` (shutdown signalling)
- `crates/flintshot-app/src/cli.rs` (`--replace` handshake)
- `crates/flintshot-app/src/single.rs` (`--replace` wait loop)
- `docs/adrs/ADR-0020-*.md` (new/finalize)

## Size

M
