# Task 005 — Worker spawn module + dispatch table

> Phase 2.C · size: M · depends on: 001

## Description

Add `flintshot-app::worker`: a `Spawner` trait that the daemon uses to
launch capture+editor work, plus the dispatch table that maps a
`Request` to a spawner call and the `Event`s to emit. The real
`CommandSpawner` runs `std::env::current_exe()` with `region` /
`fullscreen` / `--monitor` / `--delay-ms` args (reusing the Phase 1 CLI
verbatim — the daemon never captures in-process). A `RecordingSpawner`
stand-in makes the dispatch table unit-testable without forking.
ADR-0019 (daemon spawns workers, resolves binary via `current_exe`) is
drafted here.

## Acceptance criteria

- [x] `Spawner` trait: `fn spawn(&self, argv: &[&str]) -> Result<()>`
      (or equivalent) with `CommandSpawner` (real) and `RecordingSpawner`
      (test) implementations.
- [x] `CommandSpawner` builds the child command from
      `std::env::current_exe()` (not `PATH`); daemon children are
      detached/inherited sensibly for the platform.
      _stdin/stdout null'd, stderr inherited; each worker gets a reaper
      thread (`Child::wait`) so finished workers are reaped, not
      zombified._
- [x] Dispatch table: `fn dispatch(request, spawner, sink)` maps
      `CaptureRegion`/`CaptureFullscreen` → `spawn(...)` + emit
      `CaptureStarted`; `Quit` → shutdown signal; `Subscribe`/`OpenLast`
      /`PinLast`/`UploadLast` → emit `Error{Unsupported}` (their phases).
      _`Quit` returns `DispatchOutcome::Shutdown`; the caller performs
      the signal._
- [x] `RecordingSpawner` asserts: given `CaptureRegion { monitor: Some(1),
      delay_ms: 200 }`, dispatch spawns `fullscreen`/`region --monitor 1
      --delay-ms 200` exactly once and emits `CaptureStarted`.
      _`capture_region_spawns_region_subcommand_and_emits_started` +
      `capture_region_omits_flags_at_defaults` + fullscreen variants._
- [x] ADR-0019 drafted (`docs/adrs/ADR-0019-daemon-spawns-worker-processes.md`).

## Verification

- [x] Standing gate passes. _(fmt, default + tray clippy, workspace
      tests, doc tests.)_
- [x] `cargo test -p flintshot-app worker::` — dispatch table covered by
      `RecordingSpawner` for every `Request` variant; no real process
      forked in tests. _(7 worker tests; also a `FailSpawner` proves
      spawn errors propagate without emitting an event.)_
- [ ] Manual (TEST_MATRIX, optional this task): confirm
      `CommandSpawner` launches a real editor window when invoked from a
      scratch binary / `cargo run -- region`. (Full end-to-end lands in
      Task 006.)
      _Deferred to Task 006 / Checkpoint C — the IPC→worker spawn is
      wired (see below) but opening a real editor window is verified at
      the checkpoint._

## Note: dispatch wired into the IPC handler here (advances Task 006)

To avoid 7 forward-looking `allow(dead_code)` markers, `dispatch` is
already wired into `ipc_server::handle_connection`: a non-`Subscribe`
IPC request is routed through the worker dispatch table with
`CommandSpawner` + the connection's bus. So Task 005 delivers a working
"IPC client sends CaptureRegion → daemon spawns a `flintshot region`
worker + broadcasts `CaptureStarted`" path (verified at runtime: daemon
binds, ticks, no panic). Task 006 then narrows to the **`--replace`
takeover** (new daemon sends `Request::Quit` → old daemon fully exits →
new daemon takes the lock) and worker-outcome events (`EditorOpened` /
`Saved`).

## Dependencies

Task 1 (Request/Event types).

## Files likely touched

- `crates/flintshot-app/src/worker.rs` (new)
- `crates/flintshot-app/src/main.rs` (wire module)
- `docs/adrs/ADR-0019-*.md` (new)

## Size

M
