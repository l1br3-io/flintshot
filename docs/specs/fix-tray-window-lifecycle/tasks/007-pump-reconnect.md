# Task 007: Pump reconnect loop — reconnecting status, respawn, re-announce

**Description:** Wire Task 006's policies into the pump: on
subscription loss (or initial connect failure) the window becomes the
liveness anchor — it shows a *reconnecting* status (not today's
terminal "restart `flintshot app`" banner), keeps servicing commands
and in-process grabs (the `degraded_loop` select), retries
`subscribe_to` on the backoff curve, respawns the daemon through the
rate limiter, and on success re-spawns the presence announce, pushes
"connected", and re-enters `connected_loop`. Spec §Diagnosis A1/B2 /
AC 4, 8. `degraded_loop` is superseded (renamed/absorbed) — there is
no terminal degraded state anymore.

**Acceptance criteria:**
- [ ] `HomeCore::disconnected()` pushes a reconnecting status (e.g. "daemon connection lost — reconnecting…") + force-show, replacing the restart banner; any test pinning the old text updated; no other `disconnected()` semantics change (in-flight reset, force-show)
- [ ] The reconnect loop (both entries: initial `ensure_subscribed` failure and `LoopExit::Disconnected`): services `cmd_rx` (start grabs) + `grabs_rx` (route deliveries) exactly like today's degraded loop, and between attempts sleeps `reconnect_delay(attempt)` — never holding a select arm hostage
- [ ] Daemon respawn flows through `DaemonSpawnLimiter::try_spawn` (first attempt may spawn immediately when none succeeded recently; then rate-limited); spawn failures logged, loop continues
- [ ] On successful subscribe: reset the delay curve, re-spawn `announce_app_window` (presence re-registered — the restarted daemon's fresh registry learns the window; this is what prevents the AC 4 worker-editor duplicate), push "connected" status, re-enter `connected_loop`; a later disconnect re-enters the reconnect loop (idempotent cycle)
- [ ] Window close during reconnect exits the pump cleanly (`cmd_rx` None → return; `mark_pump_dead` as today)
- [ ] RED first: status-text test (old banner text asserted gone, reconnecting text asserted) fails on current code; policy wiring pinned by asserting the loop consults `reconnect_delay`/`try_spawn` (unit-test the seam: e.g. drive the loop body's per-attempt decision as a pure fn if the loop itself stays integration-shaped)
- [ ] Module doc updated: "no auto-reconnect in this effort" → the anchor stance (matches ADR-0066, Task 008)
- [ ] Existing app_window tests stay green (epoch/pump/Shared tests; update only where they pinned the old banner or degraded stance)

**Verification:**
- [ ] `cargo test -p flintshot-app -q app_window` green
- [ ] Full gate (four commands, quiet)
- [ ] Manual check deferred to the manual matrix (AC 4: `pkill -f 'flintshot daemon'` → reconnecting status, daemon respawns, tray returns, next capture routes in-window)

**Dependencies:** Task 006 (the policies)

**Files likely touched:**
- `crates/flintshot-app/src/app_window.rs` (pump_main, reconnect loop, `HomeCore::disconnected`, module doc, tests)

**Docs:**
- Loop comments carry the anchor rationale; the ADR lands in Task 008.

**Estimated scope:** Medium: 1 file
