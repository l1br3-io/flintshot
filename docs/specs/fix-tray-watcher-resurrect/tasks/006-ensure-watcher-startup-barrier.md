# Task 006: Bounded startup barrier (`ensure_watcher`) before `tray::build`

**Description:** Amendment forced by live evidence (2026-08-30,
post-ship L5 session): with the watcher dead at daemon start, the
fire-and-forget startup poke activates it ~1 s in — *during*
libappindicator's init — and one of two observed startups missed
the `NameOwnerChanged` event, leaving the watcher owned but our
item never registered (icon gone until a daemon restart). Fix:
flip spec OQ1's resolution to the blocking variant. `daemon.rs`
calls a new `sni_watchdog::ensure_watcher()` immediately before
`tray::build`: probe `NameHasOwner`; if unowned, poke the standard
then COSMIC name; if any poke succeeded, poll until owned (bounded
`ACTIVATION_WAIT = 2 s`, 50 ms steps). Healthy sessions pay one
round trip; tray-less desktops return as soon as both pokes error
(no pointless wait); failure never blocks startup past the bound.
The watchdog thread is unchanged and still owns mid-session
liveness.

**Acceptance criteria:**

- [x] `ensure_watcher_with(bus, wait)` internal fn over the `SniBus`
      seam: owned → return (no poke); unowned → pokes in name
      order, stop at first success; all pokes failed → return
      immediately (nothing is coming); poke succeeded → poll until
      owned or `wait` elapses
- [x] Public `ensure_watcher()` builds a `SessionBus` (warn +
      return on failure) and uses the consts; called from
      `daemon.rs` before `tray::build` under the same cfg
- [x] Unit tests (fake bus): owned → no pokes; unowned+poke-ok →
      returns once owned (fake flips after N probes); all pokes
      fail → returns without waiting out the bound; never owned →
      returns at the bound; probe error → no panic
- [x] CHANGELOG Fixed entry, ADR-0032 decision, spec OQ1
      annotation, and module docs amended to the barrier model;
      runbook step 3 marked deterministic
- [x] Standing gates green (fmt, clippy default + `--features
      tray`, workspace tests, doc tests)

**Verification:**

- [x] `cargo test -p flintshot-app --features tray --quiet` green
- [x] Live cold-start: kill daemon + watcher, `flintshot daemon`,
      icon appears with no manual poke (runbook step 3, now
      deterministic)

**Dependencies:** Task 2

**Files likely touched:**

- `crates/flintshot-app/src/sni_watchdog.rs`
- `crates/flintshot-app/src/daemon.rs`
- `CHANGELOG.md`, `docs/adrs/ADR-0032-*`, `docs/TEST_MATRIX.md`
- `docs/specs/fix-tray-watcher-resurrect/spec.md`, `todo.md`

**Docs:**

- Amendment history lives in this task file + ADR-0032.

**Estimated scope:** Medium: 5 files
