# Task 005: TraySupervisor — spawn retry, churn backoff, truthful `is_present`

**Description:** The supervisor's `apply(Show|ForceShow)` logs a
spawn failure and gives up (tray silently gone until the next
Hide→Show cycle), `is_present()` unconditionally returns `true` (the
gate cannot see loss), and consecutive fast helper self-exits respawn
at loop cadence (~0.5 s churn). Fix all three: retry failed spawns
with capped exponential backoff, back off across consecutive fast
self-exits (stable uptime resets), and report real child presence.
Spec §Diagnosis A3 / AC 6.

**Acceptance criteria:**
- [x] `is_present()` (tray feature) returns the truthful live-child state; the no-tray placeholder still returns `false`; `service_tray_gate` semantics otherwise unchanged
- [x] Backoff policy as a pure fn (e.g. `fn helper_backoff_delay(consecutive_fast_exits: u32) -> Duration`): exponential, capped (cap ≤ ~8 s), first retry ≥ ~0.5 s; unit-pinned
- [x] Churn state machine on the supervisor: consecutive fast self-exits (uptime below a stable threshold) grow the backoff; uptime ≥ threshold (e.g. ~10 s) resets it; a pending backoff holds an *unscheduled* respawn until its deadline — gate-ordered `Hide`/`Show`/`ForceShow` transitions still apply immediately (the gate owns designed graces)
- [x] Spawn-failure retry: a failed `spawn_helper` schedules a retry through the same backoff path instead of waiting for the next capture's Hide→Show (injectable spawn seam: field holding the spawn fn, default = `spawn_helper`; tests inject fakes)
- [x] RED first: with an injected always-failing spawner, a `Show`-driven supervisor must re-attempt spawn after the computed backoff tick (current code attempts once → RED); with consecutive fast self-exits (`true` binary, real-binary tray-arg pattern), respawn delay grows across generations
- [x] Existing supervisor tests (`stopped_helper_exit_is_not_treated_as_a_click`, `self_exited_helper_arms_the_click_pre_hide`) stay green; every test wait bounded

**Verification:**
- [x] `cargo test -p flintshot-app -q supervisor` + `cargo test -p flintshot-app -q helper` green
- [x] Full gate (four commands, quiet)
- [x] Manual check deferred to the manual matrix / diagnostic log kit (backoff cadence in daemon.log)

**Dependencies:** None (pairs with Task 004 on mechanism A3)

**Files likely touched:**
- `crates/flintshot-app/src/daemon.rs` (supervisor, backoff fn, tests)

**Docs:**
- Comment on the backoff: the churn mechanism (IPC-less daemon →
  connect-fail exits → respawn loop) and why gate-ordered transitions
  bypass it.

**Estimated scope:** Medium: 1 file
