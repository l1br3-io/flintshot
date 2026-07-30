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

- [ ] Daemon's tokio side owns the IPC server; inbound requests flow
      through the Task 005 dispatch table; spawned workers' lifecycle
      is logged (`tracing`).
- [ ] `CaptureRegion` / `CaptureFullscreen` emit `CaptureStarted` to
      subscribers on receipt and `EditorOpened` once the worker child is
      confirmed running.
- [ ] `Request::Quit` triggers a clean daemon shutdown (drops tray,
      unregisters hotkeys registered so far, closes the socket, releases
      the lock).
- [ ] `flintshot daemon --replace` connects to a running daemon, sends
      `Quit`, waits for the socket to disappear (bounded timeout), then
      takes the lock and starts its own loop.
- [ ] `OpenLast` / `PinLast` / `UploadLast` return
      `Event::Error { code: "unsupported", message: "<phase N>" }`
      without spawning anything.

## Verification

- [ ] Standing gate passes.
- [ ] `#[ignore]`'d integration test (xvfb): daemon on a temp socket;
      client sends `CaptureFullscreen`; assert `CaptureStarted` then
      `EditorOpened` arrive on a `Subscribe` stream.
- [ ] `#[ignore]`'d test: two daemons — second with `--replace` succeeds
      and the first exits; without `--replace` the second fails with
      "already running".
- [ ] Manual (TESTMATRIX): `cargo run -- daemon`, then from a script
      connect to the socket, `Subscribe`, request `CaptureFullscreen`,
      watch the editor open and events flow.

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
