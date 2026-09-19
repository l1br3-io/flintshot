# Task 006: Reconnect policy + daemon-spawn rate limiter (pure fns)

**Description:** The pump's self-healing needs two policies, landed
first as pure, unit-tested functions (spec §Testing Strategy): a
reconnect delay-per-attempt curve (exponential, capped, reset on
connect) and a daemon-spawn rate limiter (a crash-looping daemon must
not storm process spawns — AC 8). No loop wiring in this task.

**Acceptance criteria:**
- [x] RED first: unit tests for both fns fail (they don't exist): `reconnect_delay(0) ≥ base (~250 ms)`, grows monotonically per attempt, capped (≤ ~5 s) and never exceeds the cap for large attempt counts
- [x] `DaemonSpawnLimiter` (or equivalent pure type): `try_spawn(now) -> bool` denies within the min-interval window (~30 s), allows at/after it, and records the grant; denies forever-within-window repeats (rate limit pinned by test — AC 8)
- [x] Reset-on-connect: an explicit reset/constructor path so a successful subscribe restarts the delay curve at the base (unit-pinned)
- [x] All bounds are named consts with why-comments; `Duration` arithmetic saturating (no panics on large attempt counts)
- [x] No behavior change yet: pump loops untouched (compile + existing tests green)

**Verification:**
- [x] `cargo test -p flintshot-app -q reconnect` + `cargo test -p flintshot-app -q limiter` green
- [x] Full gate (four commands, quiet)

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-app/src/app_window.rs` (pure fns + tests near the pump consts)

**Docs:**
- Const comments carry the why (spec boundary: every new wait
  bounded, every retry capped).

**Estimated scope:** Small: 1 file
