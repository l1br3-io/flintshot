# Task 004 — IPC server transport + postcard framing

> Phase 2.C · size: M · depends on: 001, 003

## Description

Build the IPC server transport in `flintshot-app::ipc_server`: an
`interprocess` local socket listener (Unix domain socket on
Linux/macOS, named pipe on Windows) that accepts connections, reads
length-prefixed postcard frames, and handles `Request::Quit` and
`Request::Subscribe`. Subscribers are tracked; the server exposes a
`broadcast(Event)` handle the daemon uses (Task 006) to fan out events.
Transport/encoding stay here; the schema comes from
`flintshot-core::ipc` (Task 001). ADR-0011 covers postcard; if the
length-prefix detail (u32 LE, max frame size) warrants its own record,
split ADR-0021.

## Acceptance criteria

- [x] `ipc_server` listens on `socket_path()` (Task 001); stale socket
      files are removed on start (Unix).
      _`try_overwrite(true)` on `ListenerOptions` reclaims a corpse
      socket; proven at runtime (bound at `/run/user/1000/flintshot.sock`,
      socket file cleaned up on listener drop)._
- [x] Framing: read `u32` little-endian length, then that many bytes,
      `postcard::from_bytes` → `Request`. A length cap rejects
      oversized frames with a logged error (no panic).
      _`MAX_FRAME_LEN` = 1 MiB; `oversized_length_is_rejected` +
      `write_frame_rejects_oversized_payload` unit tests._
- [x] `Request::Subscribe` registers the connection to receive a stream
      of `Event`s; `Request::Quit` triggers a graceful shutdown signal.
      _Subscribe subscribes synchronously (registered the instant the
      frame is read) then spawns a forwarder; Quit calls
      `Notify::notify_one` to stop the accept loop._
- [x] `broadcast(event)` sends an `Event` to all current subscribers;
      a dropped subscriber is reaped without erroring others.
      _`tokio::sync::broadcast`; `broadcast_fans_out_to_all_subscribers`
      + `dropped_subscriber_does_not_break_others` unit tests._
- [x] No `interprocess` / `postcard` types leak into `flintshot-core`
      (this code is all in `flintshot-app`).
      _`no_gui_types_in_core` still green._

## Verification

- [x] Standing gate passes. _(fmt, default + tray clippy, workspace
      tests, doc tests.)_
- [x] Unit test: framing encode/decode roundtrip via in-memory buffers
      (`tokio::io::duplex`); truncated frame (short read) returns an
      error, not a panic. _`framing_roundtrips_a_request` /
      `framing_roundtrips_an_event` / `truncated_frame_returns_error_not_panic`._
- [x] Unit test: broadcast fans out to N mock subscribers; one dropped
      mid-stream doesn't break the others.
- [x] `#[ignore]`'d integration test (needs a real socket, no display):
      spin server on a temp socket, connect a client, send `Subscribe`,
      `broadcast` an `Event`, assert the client decodes it.
      _`integration_subscribe_receives_broadcast_event` — passes when
      run explicitly (multi-thread runtime + broadcast retry to handle
      the inherent subscribe/broadcast race)._

## ADR-0021 (conditional)

Not split: the framing detail (u32 little-endian length, 1 MiB cap) is
the standard length-prefix shape and extends ADR-0011 rather than
diverging from it. Recorded in the module doc instead.

## Dependencies

Task 1 (schema + `socket_path`), Task 3 (runs on the daemon's tokio
side thread).

## Files likely touched

- `crates/flintshot-app/src/ipc_server.rs` (new)
- `crates/flintshot-app/src/main.rs` (wire module)
- `crates/flintshot-app/Cargo.toml` (interprocess + postcard, if not in 002)
- `docs/adrs/ADR-0021-*.md` (conditional — framing detail)

## Size

M
