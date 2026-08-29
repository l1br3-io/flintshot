# Plan: fix-tray-sni — tray icon registers + menu works on COSMIC

## Overview

The daemon's platform loop never iterates the GLib main context, so
libappindicator's `RegisterStatusNotifierItem` call (and muda's menu
event delivery) never dispatches — the tray objects are exported on
the bus but the icon never registers with the StatusNotifierWatcher.
Fix: a bounded, non-blocking pump of the default GLib main context
inside the existing poll loop, proven headless via glib idle sources,
confirmed live on L5. No new dependencies; backend swap (`ksni`) is
fenced off unless the hypothesis is disproven.

## Architecture Decisions

- **Pump `glib::MainContext::default()` per poll tick, bounded** —
  `iteration(false)` is non-blocking; a hard per-tick iteration cap
  keeps a pathological always-ready source from wedging the daemon
  loop. Caps GLib dispatch latency at `POLL_TIMEOUT` (100 ms),
  imperceptible for tray interactions. ADR-0018's architecture
  (main-thread platform loop + tokio side thread) is untouched.
- **`use gtk::glib`, not a new `glib` dependency** — the `gtk` crate
  re-exports `glib` (already behind the `tray` feature,
  Linux-scoped); zero dep-graph change.
- **Headless test seam: glib idle sources need no display** — the
  pump is unit-testable without GTK init or X11; the
  appindicator-needs-the-loop link is documented and confirmed by
  the L5 pass, not simulated in CI.
- **Correct the false claim in daemon.rs's header** — "raw polling
  suffices on Linux" misled two efforts; the fix rewrites it to say
  what each event source actually needs.

## Task Index

### Phase 1: Fix
- [ ] Task 1: Bounded GLib pump helper + headless unit tests → `tasks/001-glib-pump-helper.md`
- [ ] Task 2: Wire pump into `run_event_loop` + correct module doc → `tasks/002-wire-pump-loop.md`

### Checkpoint: code green
- [ ] `cargo test -p flintshot-app --features tray` green (new pump tests)
- [ ] `cargo check -p flintshot-app --no-default-features` clean
- [ ] Standing gates green

### Phase 2: Docs
- [ ] Task 3: CHANGELOG + TEST_MATRIX updates → `tasks/003-changelog-test-matrix.md`
- [ ] Task 4: ADR-0031 GLib context pumping for the tray → `tasks/004-adr-0031-glib-pump.md`

### Checkpoint: docs recorded
- [ ] TEST_MATRIX finding 3 annotated with the fix commit (L5 pending noted honestly)
- [ ] ADR index updated

### Phase 3: Verify
- [ ] Task 5: L5 runbook for the maintainer + spec closeout → `tasks/005-l5-runbook.md`

### Checkpoint: complete
- [ ] All acceptance criteria met; runbook written
- [ ] Maintainer runs L5 pass → records results in TEST_MATRIX → PR

## Risks and Mitigations

| Risk                                             | Impact | Mitigation                                                                                        |
| ------------------------------------------------ | ------ | ------------------------------------------------------------------------------------------------- |
| Hypothesis wrong (pump doesn't fix registration) | High   | L5 gate before ship; spec fences backend swap behind disproof — stop and re-spec, don't fix-forward |
| Busy GLib source wedges the daemon loop          | Med    | Hard per-tick pump cap + unit test proving termination against a self-re-arming source             |
| `tray`-feature code unlinted/untested in CI      | Med    | Verify with `--features tray` locally; extending the `tray-build` CI job is an explicit ask-first  |
| X11 tray regresses                               | Low    | Pump is additive (context idle there); L5 runbook includes an X11 sanity row                       |

## Open Questions

- Which SNI interface does cosmic-applet-status-area consume (KDE
  vs Ayatana object path)? Noted during the L5 pass; no code change
  expected either way.
- Does the `tray-build` CI job grow `cargo test -p flintshot-app
  --features tray`? Ask-first; default is no CI change.
