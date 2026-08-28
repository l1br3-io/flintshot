# Plan: Fix delegated capture (fork loop + at-spi window crash)

## Overview

Two independent bugs keep every daemon-mediated capture from ever
opening an editor: (1) daemon-spawned workers re-delegate back to the
daemon (fork loop, ~470 procs in 8 s), and (2) window-opening workers
panic on at-spi desktops because zbus 5 (via accesskit) demands an
ambient tokio reactor the workers don't have. Both fixes are local to
`flintshot-app`; each gets an automated regression test. Docs
(CHANGELOG, TEST_MATRIX, ADR) follow the code.

## Architecture Decisions

- **Fork-loop fix: `--standalone` in worker argv** (spec Decisions).
  `region_argv` / `fullscreen_argv` append `--standalone`;
  `delegate_to_daemon` already short-circuits on it. Argv content is
  the existing `RecordingSpawner` test seam. `pin` / `open` / `upload`
  argv stay unguarded — those `run_*` functions never delegate.
- **Ambient runtime wraps only the `EguiShell` window calls** (resolves
  spec open question 1). New small module in `flintshot-app`
  (`ambient.rs`) with `with_ambient_runtime(f)`: builds a
  multi-thread `Runtime`, `enter()`s it, runs `f`, drops the guard.
  Called inside `run_editor_with_config`, `run_overlay_with_config`,
  and `run_pin_window` around the `EguiShell` call only. This is the
  narrowest seam covering every window path (editor via capture *and*
  `open`, overlay, pin) and — critically — the guard is dropped
  before `report_editor_outcome` → `ipc_client` fires its own one-shot
  `block_on` runtimes, so no runtime nesting can occur.
- **`upload` needs no guard** (resolves spec open question 2):
  `run_upload_core` never reaches eframe and already creates its own
  runtime for `block_on`. No change.
- **ADR-0030** records the ambient-runtime decision (zbus 5 via
  accesskit, runtime-per-worker trade-off, why not zbus 4 / async-io).

## Task Index

### Phase 1: Code fixes (independent, test-pinned)

- [ ] Task 1: Worker argv `--standalone` guard + dispatch tests → `tasks/001-worker-argv-standalone.md`
- [ ] Task 2: Ambient tokio runtime guard for window workers → `tasks/002-ambient-runtime-guard.md`

### Checkpoint: Code fixes

- [ ] `cargo test -p flintshot-app` green (new argv + guard tests)
- [ ] Full standing gates green (fmt / clippy / workspace tests / doc tests)
- [ ] `flintshot-core` and `flintshot-editor-egui` untouched

### Phase 2: Documentation

- [ ] Task 3: CHANGELOG + TEST_MATRIX updates → `tasks/003-changelog-test-matrix.md`
- [ ] Task 4: ADR-0030 ambient runtime for window workers → `tasks/004-adr-0030-ambient-runtime.md`

### Checkpoint: Complete

- [ ] All acceptance criteria in task files met
- [ ] Manual L5 pass (at-spi session) left for the maintainer — this
  environment has no desktop session; TEST_MATRIX records the fix as
  landed with automated coverage and the manual re-run as pending
- [ ] Ready for review / PR

## Risks and Mitigations

| Risk                                              | Impact | Mitigation                                                                                                     |
| :--- | :--- | :--- |
| zbus reactor presence untestable in CI (no at-spi) | Med    | Guard unit test proxies the exact condition via `tokio::runtime::Handle::try_current()`                         |
| Runtime nesting panic (ambient guard vs ipc_client one-shot runtimes) | Med | Guard scope limited to the `EguiShell` call; drops before any report path runs; noted in ADR-0030 |
| Manual L5 verification unavailable here           | Low    | Recorded honestly in TEST_MATRIX as pending; exit criteria 1–3 checked by the maintainer before merge |

## Open Questions

- None remaining from the spec (both resolved in Architecture
  Decisions above).
