# Plan: Tray + window lifecycle consistency (self-healing)

## Overview

Fix the three ranked lifecycle faults from the spec's diagnosis: (B)
worker-editor handoffs that leave duplicate windows behind, (A2) the
prepare-timeout double-begin that ghosts the tray for 30 s, (A3)
helper respawn churn / silent tray loss, and (A1) unrecoverable
daemon death. The end state: the app window is the liveness anchor —
on subscription loss it reconnects with backoff, respawns the daemon
rate-limited, and re-announces presence; the tray never stays hidden
past its designed graces; every worker handoff closes behind itself.
Spec: `docs/specs/fix-tray-window-lifecycle/spec.md`.

## Architecture Decisions

- **Mirror, don't generalize (again).** The Settings arm copies the
  ADR-0065 `NavigateHome` choreography verbatim (hook → publish
  `Cancelled` → `ViewportCommand::Close`) instead of extracting a
  shared delegate-and-exit helper — the NavigateHome twin stays
  untouched, and the diff reads as additive.
- **Fallback argv routes through `worker::app_argv`.** The CLI's
  no-daemon spawns reuse the daemon's own argv builder (with its
  trailing `--standalone`) rather than duplicating flag lists — one
  vocabulary for "spawn the app window process".
- **Timeout ≠ unsupported on `PrepareCapture`.** A reply timeout
  means the daemon *did* `begin_capture` (it arms the gate before
  awaiting both handshakes), so the client owes exactly one
  `ReportCaptured` and must NOT run the tray-only fallback (the
  double-begin is the 30 s ghost). Only the explicit `unsupported`
  error reply (a known-old daemon that never began) keeps the legacy
  fallback hide.
- **Backoff as pure functions, loops as thin wiring.** Reconnect
  delay-per-attempt, daemon-spawn rate limit, and helper-churn
  backoff are pure, unit-tested fns; the pump/supervisor loops only
  sequence them. Every wait bounded, every retry capped (spec
  boundary).
- **The window owns daemon recovery.** `degraded_loop` (today's
  terminal cmd-only mode) is replaced by a reconnecting loop: service
  in-process grabs continuously, retry `subscribe_to` with capped
  exponential backoff, respawn the daemon through a rate limiter
  (a crash-looping daemon cannot storm process spawns — AC 8), and on
  success re-spawn the presence announce + re-enter `connected_loop`.
- **Truthful supervisor.** `TraySupervisor::is_present` reports the
  real child state so the tray gate can see loss; spawn failures and
  consecutive fast self-exits back off exponentially (capped), and
  stable uptime resets the backoff. The helper's daemon-watch gets a
  bounded (~2 s) connect-retry window so a helper spawned during
  daemon startup survives the socket race without churn.

## Task Index

### Phase 1: Duplicate windows — handoffs close behind themselves

- [ ] Task 1: Settings arm closes behind the handoff (ADR-0065 mirror) → `tasks/001-settings-arm-closes.md`
- [ ] Task 2: No-daemon fallback spawns route through `worker::app_argv` → `tasks/002-fallback-argv-standalone.md`

### Checkpoint: Handoffs
- [ ] Four CI commands green; worker-editor Settings now publishes + closes in unit tests

### Phase 2: Prompt tray return — no double-begin, no churn

- [ ] Task 3: `Prepared` fallback split — timeout ≠ old-daemon `unsupported` → `tasks/003-prepare-fallback-split.md`
- [ ] Task 4: Tray helper daemon-watch bounded connect retry → `tasks/004-helper-watch-retry.md`
- [ ] Task 5: TraySupervisor — spawn retry, churn backoff, truthful `is_present` → `tasks/005-supervisor-backoff.md`

### Checkpoint: Tray graces
- [ ] Four CI commands green; slow-prepare no longer double-begins (fake-server pin)

### Phase 3: Self-healing — the window as liveness anchor

- [ ] Task 6: Reconnect policy + daemon-spawn rate limiter (pure fns) → `tasks/006-reconnect-policy.md`
- [ ] Task 7: Pump reconnect loop — reconnecting status, respawn, re-announce → `tasks/007-pump-reconnect.md`

### Checkpoint: Anchor
- [ ] Four CI commands green; AC 4/8 pinned by policy unit tests (runtime proof = manual matrix)

### Phase 4: Record

- [ ] Task 8: CHANGELOG `[Unreleased]` + ADR-0066 → `tasks/008-changelog-adr.md`

### Checkpoint: Complete
- [ ] All eight spec acceptance criteria addressed (1–3, 5–8 unit-pinned; 4's runtime behavior = manual matrix — no interactive session in this run)
- [ ] Ready for review / PR

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Reconnect loop starves in-process grabs (select unfairness) | High | Reconnecting loop mirrors `degraded_loop`'s select exactly (commands + grabs first-class); backoff sleeps never hold the select |
| Daemon respawn storm on a crash-looping daemon | High | Pure `DaemonSpawnLimiter` with a min-interval rate limit; AC 8 pinned by unit test |
| Fake-server socket tests unrunnable headless | Med | Existing precedent runs local sockets headless on Linux (`#[ignore]` only if proven flaky); follow `prepare_capture_falls_back_to_tray_on_unsupported` shape |
| Helper backoff vs. tray-gate interplay (respawn held too long) | Med | Backoff only gates *unscheduled* respawns; gate-ordered `Show`/`ForceShow` applies immediately (gate owns designed graces) |
| `is_present` truthfulness changes gate semantics | Med | Spec allows it (truthful presence is the fix); gate tests in `tray_gate.rs` keep passing — `step` already receives presence as an input |
| Clippy pedantic on new hooks/backoff code | Low | CI gate in every task's verify step |

## Open Questions

None — daemon auto-restart, the Settings mirror, and the diagnostic
kit were confirmed by the user (2026-09-20, recorded in spec).
