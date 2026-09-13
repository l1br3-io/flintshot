# Plan: Remove pin-to-desktop

## Overview

Purge the pin-to-desktop feature (spec:
[`spec.md`](spec.md)) from all four crates plus living docs. Removal
runs **consumer-first**: strip every producer/sender of pin types
before deleting the types themselves, so the workspace compiles and
tests stay green after every task. Nine tasks, four checkpoints.

## Architecture decisions

- **Consumer-first order.** `EditorOutcome::Pinned`,
  `Request::PinLast`, `Event::Pinned`, and `PinConfig` stay in
  `flintshot-core` (unconstructed / unsent, but public) until tasks
  1–6 have removed every reference; task 7 deletes them. Inverse of
  build order — deleting core types first would break three crates at
  once.
- **Postcard re-index accepted.** Removing enum variants re-indexes
  the wire format (spec § Wire-format break). All daemon-spawned
  clients are version-locked; the COSMIC applet is rebuilt in this
  effort. No tombstones.
- **`prepare_gate` simplification, not stub.** `pins_alive` is
  removed from the gate's signature and logic — with no pin sessions
  the condition is vacuous, and the spec forbids stubs.
- **COSMIC before core.** Task 6 strips the applet's
  `PinLast`/`Pinned` surface so task 7's core deletion keeps
  `cargo clippy/test --workspace` green.
- **Docs last.** Task 8 rewrites living docs once the code truth is
  final; task 9 is the `rg` sweep + full CI-parity gate.

## Task index

### Phase 1: Editor + CLI surface
- [ ] Task 1: Remove Pin from editor + overlay toolbars → `tasks/001-editor-pin-buttons.md`
- [ ] Task 2: Remove `flintshot pin` CLI subcommand → `tasks/002-cli-pin-subcommand.md`
- [ ] Task 3: Delete pin window + `EditorOutcome::Pinned` + `run_pin` trait → `tasks/003-pin-window-machinery.md`

### Checkpoint: Editor + CLI
- [ ] Default-member workspace builds + tests green
- [ ] `--help` shows no `pin`; toolbars render without Pin button

### Phase 2: Daemon surface
- [ ] Task 4: Remove daemon `PinLast` dispatch + `PinIdSource` → `tasks/004-daemon-pinlast-dispatch.md`
- [ ] Task 5: Remove pin sessions + `prepare_gate` pins condition → `tasks/005-pin-sessions-prepare-gate.md`

### Checkpoint: Daemon
- [ ] `flintshot-app` tests + clippy green
- [ ] No code path spawns `flintshot pin` or emits `Event::Pinned`

### Phase 3: Applet + core types
- [ ] Task 6: Remove Pin last from COSMIC applet → `tasks/006-cosmic-pin-last.md`
- [ ] Task 7: Delete `PinLast`/`Pinned`/`PinConfig` from `flintshot-core` → `tasks/007-core-pin-types.md`

### Checkpoint: Core types
- [ ] Full workspace (incl. cosmic) builds + tests green
- [ ] `rg` over `crates/` shows no feature pin references

### Phase 4: Docs + closeout
- [ ] Task 8: Living docs, justfile, ADR-0049, CHANGELOG → `tasks/008-docs-tooling.md`
- [ ] Task 9: Final sweep + full CI-parity verification → `tasks/009-final-sweep-verify.md`

### Checkpoint: Complete
- [ ] All spec success criteria met (spec § Success criteria)
- [ ] Ready for review

## Parallelization

Sequential by default. If parallelized: tasks 1, 2, 4, 6 are
independent of each other (different crates/files); task 3 depends on
1–2; task 5 depends on 4 (kills `register_pin`'s only non-test
caller); task 7 depends on all of 1–6; 8–9 are strictly last.

## Risks and mitigations

| Risk                                       | Impact | Mititation                                                                             |
| ------------------------------------------ | ------ | -------------------------------------------------------------------------------------- |
| Shared helpers: `spawn_pin_worker` and `spawn_upload_worker` share a detached-spawn path | Med | Task 1 deletes only the pin wrapper; the shared spawn helper stays for upload           |
| `prepare_gate` signature change ripples into `daemon.rs` call site | Low | Task 5 includes the call site + gate tests; compiler drives completeness              |
| Missed pin reference breaks cosmic build   | Low | Task 7 runs `cargo build -p flintshot-cosmic` + workspace clippy                        |
| Pin temp-file naming reused elsewhere      | Low | `flintshot-pin-<pid>-<n>.png` prefix is pin-only; upload has its own prefix             |

## Open questions

None — scope settled in spec.
