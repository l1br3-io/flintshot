# Plan — Hide the tray during daemon-dispatched captures

> Thin index. Requirements live in [`spec.md`](spec.md). Task files
> under `tasks/` are authored when each task starts.

## Overview

A worker→daemon handshake (`HideTrayForCapture` → confirmed
`Event::TrayHidden` → grab → `ReportCaptured` → restore) so the
daemon's own tray icon never lands in a frozen capture, on every
trigger path, with a depth counter for concurrency, a propagation
grace before the ack, a safety restore for dead workers, and a
pure, unit-tested visibility state machine at the daemon's core.

## Architecture decisions

1. **Schema first, appended**: the two `Request` variants + reply
   `Event` land at the enum tails (postcard variant-index wire
   stability, same note as Phase 6). → Task 001.
2. **The gate is two halves**: a thread-safe `TrayGate` shell
   (atomics: depth, confirmed epoch) shared with the tokio IPC side,
   wrapping a pure `TrayVisibility` state machine owned by the main
   loop — decisions unit-tested headless, GTK side effects applied
   by the loop. → Task 002.
3. **Interception before dispatch** in `handle_connection`, exactly
   like `ReportSaved`/`BindShortcuts`; the worker table gains no-op
   arms + labels. → Task 002.
4. **The guard spans exactly the grab** in `capture_and_dispatch`
   (`capture_with_tray_hidden`), Drop-reported on success and error;
   the editor session never holds the tray hostage. → Task 003.
5. **Docs last, one task**: ADR-0036, CHANGELOG, TEST_MATRIX rows,
   spec closeout. → Task 004.

## Task index

### Stage 1 — schema
- [ ] Task 001: core IPC variants + round-trip tests
  - *Checkpoint: `cargo test -p flintshot-core` green; wire bytes of
    earlier variants unchanged.*

### Stage 2 — daemon side
- [ ] Task 002: `TrayGate`/`TrayVisibility` + daemon loop wiring +
  IPC interception + worker-table arms
  - *Checkpoint: state-machine units green; in-process IPC test gets
    a `TrayHidden` reply; gates green.*

### Stage 3 — worker side
- [ ] Task 003: bounded-wait client helper + RAII guard in
  `capture_and_dispatch`
  - *Checkpoint: no-daemon no-op test; ordering pinned (guard drops
    after the grab call, not after the editor); gates green.*

### Stage 4 — docs
- [ ] Task 004: ADR-0036, CHANGELOG, TEST_MATRIX rows, spec → shipped
  - *Checkpoint: standing gates + docs review.*
