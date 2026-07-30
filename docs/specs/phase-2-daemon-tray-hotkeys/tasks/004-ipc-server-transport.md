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

- [ ] `ipc_server` listens on `socket_path()` (Task 001); stale socket
      files are removed on start (Unix).
- [ ] Framing: read `u32` little-endian length, then that many bytes,
      `postcard::from_bytes` → `Request`. A length cap rejects
      oversized frames with a logged error (no panic).
- [ ] `Request::Subscribe` registers the connection to receive a stream
      of `Event`s; `Request::Quit` triggers a graceful shutdown signal.
- [ ] `broadcast(event)` sends an `Event` to all current subscribers;
      a dropped subscriber is reaped without erroring others.
- [ ] No `interprocess` / `postcard` types leak into `flintshot-core`
      (this code is all in `flintshot-app`).

## Verification

- [ ] Standing gate passes.
- [ ] Unit test: framing encode/decode roundtrip via in-memory buffers
      (`std::io::Cursor` or `Vec<u8>`); truncated frame (short read)
      returns an error, not a panic.
- [ ] Unit test: broadcast fans out to N mock subscribers; one dropped
      mid-stream doesn't break the others.
- [ ] `#[ignore]`'d integration test (needs a real socket, no display):
      spin server on a temp socket, connect a client, send `Subscribe`,
      `broadcast` an `Event`, assert the client decodes it.

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
