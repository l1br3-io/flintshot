# Plan: fix-tray-watcher-resurrect — daemon self-heals a dead SNI host

## Overview

When the session's StatusNotifierWatcher is dead (or starts after the
daemon), Flintshot's tray icon has nowhere to appear and nothing on
our side ever triggers D-Bus activation — the daemon waits passively
forever. Fix: a `sni_watchdog` module in `flintshot-app` that (a)
pokes the watcher's well-known names at daemon startup (activation
trigger, bounded ≤ 2 s) and (b) re-pokes every 30 s whenever
`NameHasOwner` reports the watcher unowned. libappindicator already
re-registers the item the moment a watcher exists (verified live in
the `fix-tray-sni` effort), so the watchdog only owns watcher
liveness — never registration. Decision logic is unit-tested through
a fake-bus seam; the live activation path is honestly manual (L5
runbook), since activation is bus-daemon behavior CI cannot simulate.

## Architecture Decisions

- **Startup poke = the watchdog's first tick, immediately** — the
  thread probes the instant it starts, so `daemon.rs` gets one
  fire-and-forget `spawn` before `tray::build` and no separate
  startup-poke code path. Resolves spec OQ1: libappindicator
  re-registers on watcher appearance (spec assumption 3), so blocking
  daemon startup would add latency for zero correctness.
- **Gate on `NameHasOwner(org.kde.StatusNotifierWatcher)`** — the SNI
  rendezvous name every live host owns (COSMIC's watcher included;
  it's how libappindicator finds it). The call targets
  `org.freedesktop.DBus`, so it never triggers activation (spec
  assumption 4) — safe as the poll gate.
- **Poke order: standard name, then `com.system76.CosmicStatusNotifierWatcher`, stop at first success** — KDE-family
  sessions answer the standard name harmlessly; COSMIC needs its
  vendor name for activation. Poke failures log at `debug`: a
  tray-less desktop is a normal steady state, not a warn-worthy
  condition (spec exit criterion 3).
- **Sync trait seam (`SniBus`), zbus behind it** —
  `watcher_owned() -> Result<bool>` + `poke(name) -> Result<()>` as
  plain sync methods; fakes record calls and make the decision logic
  testable without a bus. The real impl `block_on`s zbus calls on a
  current-thread runtime owned by the watchdog thread — the
  `send_request_blocking` precedent (`ipc_server.rs:511`), not a new
  topology. Poke bounded by a `tokio::time::timeout(2 s)` wrapper so
  the bound holds regardless of zbus proxy default timeouts.
- **Shutdown by channel, not flag polling** — the daemon drops the
  watchdog's mpsc sender at exit; the thread's
  `recv_timeout(30 s)` returns `Disconnected` immediately, so join
  waits at most one in-flight poke (≤ 2 s). Mirrors the daemon's own
  channel-driven shutdown.
- **`zbus` direct dep, mirroring the `gtk` pattern** — workspace
  `zbus = { version = "5", default-features = false, features =
  ["tokio"] }`; optional + `[target.'cfg(target_os = "linux")']`-scoped
  in `flintshot-app`, enabled by the existing `tray` feature exactly
  as `dep:gtk` is today. Unifies with ashpd's zbus 5.18 — a dep edge,
  not a new crate build. (zbus 4.4 via keyring already coexists in
  the lock; unchanged, noted in the ADR.) Ask-first per spec — needs
  approval at plan review.

## Task Index

### Phase 1: Code
- [ ] Task 1: zbus dep edge + `sni_watchdog` decision logic behind the `SniBus` seam, fake-bus unit tests → `tasks/001-sni-bus-decision-logic.md`
- [ ] Task 2: watchdog thread (immediate first tick, 30 s interval, channel shutdown) + `daemon.rs` wiring → `tasks/002-watchdog-thread-wiring.md`

### Checkpoint: code green
- [ ] `cargo test -p flintshot-app --features tray --quiet` green (new seam tests)
- [ ] `cargo check -p flintshot-app --no-default-features` and default (no-tray) build clean — module invisible outside `(tray, linux)`
- [ ] Standing gates green (`fmt`, clippy `--workspace --all-targets -D warnings`, workspace tests, doc tests)

### Phase 2: Docs
- [ ] Task 3: CHANGELOG + TEST_MATRIX rows for the two live scenarios → `tasks/003-changelog-test-matrix.md`
- [ ] Task 4: ADR-0032 poke-to-activate + index → `tasks/004-adr-0032-poke-to-activate.md`

### Checkpoint: docs recorded
- [ ] ADR-0032 in `docs/adrs/` + `docs/adrs/README.md` index updated
- [ ] TEST_MATRIX rows present (L5 pending noted honestly)

### Phase 3: Verify
- [ ] Task 5: L5 runbook in TEST_MATRIX + spec closeout → `tasks/005-l5-runbook.md`

### Checkpoint: complete
- [ ] All acceptance criteria met; runbook written
- [ ] Maintainer runs the L5 pass → records results → PR (merge flips `spec.md` to `shipped`)

## Risks and Mitigations

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| `zbus` direct dep is an ask-first boundary | Med | Flagged here for approval; if declined, the seam trait keeps the decision logic intact for a gio-based impl swap |
| zbus 5 poke-timeout API details differ from expectation | Med | Bound enforced by `tokio::time::timeout(2 s)` wrapper regardless of proxy defaults; exact call shape checked against zbus 5.18 docs during Task 1 (source-driven) |
| Futile activation cycle on deliberately tray-less desktops (spec OQ2) | Low | Only possible where a service file exists; L5 runbook includes the check; back-off guard is fenced off as a follow-up, not this effort |
| Watchdog join delays daemon exit | Low | Channel shutdown wakes the thread immediately; join bounded by one in-flight poke (≤ 2 s) — accepted, documented in module docs |
| Registration (not activation) regresses | Low | Out of scope: libappindicator owns it (spec assumption 3, verified live in fix-tray-sni); would surface in the L5 runbook's baseline step |

## Open Questions

- Approve the `zbus` direct dependency? (spec ask-first; recommended — see Architecture Decisions)
- Should the CI `tray-build` job also run `cargo test -p flintshot-app --features tray`? Default **no** per spec (CI unchanged).
- Spec OQ1 resolved as fire-and-forget via the watchdog's immediate first tick — confirm at review.
