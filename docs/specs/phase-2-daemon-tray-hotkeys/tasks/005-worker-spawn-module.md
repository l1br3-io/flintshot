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

- [ ] `Spawner` trait: `fn spawn(&self, argv: &[&str]) -> Result<()>`
      (or equivalent) with `CommandSpawner` (real) and `RecordingSpawner`
      (test) implementations.
- [ ] `CommandSpawner` builds the child command from
      `std::env::current_exe()` (not `PATH`); daemon children are
      detached/inherited sensibly for the platform.
- [ ] Dispatch table: `fn dispatch(request, spawner, sink)` maps
      `CaptureRegion`/`CaptureFullscreen` → `spawn(...)` + emit
      `CaptureStarted`; `Quit` → shutdown signal; `Subscribe`/`OpenLast`
      /`PinLast`/`UploadLast` → emit `Error{Unsupported}` (their phases).
- [ ] `RecordingSpawner` asserts: given `CaptureRegion { monitor: Some(1),
      delay_ms: 200 }`, dispatch spawns `fullscreen`/`region --monitor 1
      --delay-ms 200` exactly once and emits `CaptureStarted`.
- [ ] ADR-0019 drafted (`docs/adrs/ADR-0019-*.md`).

## Verification

- [ ] Standing gate passes.
- [ ] `cargo test -p flintshot-app worker::` — dispatch table covered by
      `RecordingSpawner` for every `Request` variant; no real process
      forked in tests.
- [ ] Manual (TEST_MATRIX, optional this task): confirm
      `CommandSpawner` launches a real editor window when invoked from a
      scratch binary / `cargo run -- region`. (Full end-to-end lands in
      Task 006.)

## Dependencies

Task 1 (Request/Event types).

## Files likely touched

- `crates/flintshot-app/src/worker.rs` (new)
- `crates/flintshot-app/src/main.rs` (wire module)
- `docs/adrs/ADR-0019-*.md` (new)

## Size

M
