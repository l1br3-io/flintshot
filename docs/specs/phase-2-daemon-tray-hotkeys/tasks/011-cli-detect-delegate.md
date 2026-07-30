# Task 011 — CLI capture subcommands detect-and-delegate

> Phase 2.E · size: M · depends on: 001, 006

## Description

Add `flintshot-app::ipc_client` and teach `flintshot region` /
`fullscreen` to detect a running daemon: try to connect to the IPC
socket; on success, send the `Request` and relay/log the returned
`Event`s; on connect failure, fall back to the existing standalone
Phase 1 capture+editor path, unchanged. This centralises the event
stream (a `flintshot region` issued while a daemon runs is visible to
subscribers) without breaking headless/scripting use. Resolves spec
Open Question 2 (confirmed default: detect-and-delegate).

## Acceptance criteria

- [ ] `ipc_client::send(request)` connects to `socket_path()`, sends the
      request, returns the `Event` stream (or outcome); a missing socket
      / refused connection returns a distinct `NoDaemon` error the CLI
      treats as "fall back to standalone".
- [ ] `run_region` / `run_fullscreen`: if a daemon answers, the request
      is delegated and the CLI exits once the editor opens (or the
      outcome is logged); if no daemon, the Phase 1 in-process capture
      + editor path runs exactly as before.
- [ ] A `--standalone` (or `--no-daemon`) flag forces the in-process
      path even when a daemon is running (escape hatch for debugging).
- [ ] Delegated requests observe the same `Event` sequence a tray/IPC
      client would (`CaptureStarted` → `EditorOpened`).

## Verification

- [ ] Standing gate passes.
- [ ] Unit test: `ipc_client` returns `NoDaemon` when the socket file
      is absent (pure, no display).
- [ ] Manual: with a daemon running, `flintshot region` opens the
      editor and the daemon's subscribers see the event; kill the
      daemon, `flintshot region` still opens the editor standalone.
- [ ] Manual: `flintshot region --standalone` with a daemon running
      runs the in-process path (events not centralised).

## Dependencies

Task 1 (Request/Event + socket_path), Task 6 (daemon actually answers).

## Files likely touched

- `crates/flintshot-app/src/ipc_client.rs` (new)
- `crates/flintshot-app/src/cli.rs` (detect-and-delegate in
  `run_region`/`run_fullscreen`; `--standalone` flag)
- `crates/flintshot-app/src/main.rs` (wire module)

## Size

M
