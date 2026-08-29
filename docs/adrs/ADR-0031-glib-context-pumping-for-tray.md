# ADR-0031: GLib context pumping for the Linux tray

**Context.** The L5 launcher pass (2026-08-28, commit `54383b1`)
found the tray icon never appearing on COSMIC: the daemon (tray
build) exported its bus name and `/StatusNotifierItem` +
`/org/ayatana/NotificationItem` objects, but never completed
`RegisterStatusNotifierItem` with the `org.kde.StatusNotifierWatcher`
host — `RegisteredStatusNotifierItems` stayed empty and no icon
rendered (TEST_MATRIX L5, finding 3; fails launcher row 4). Other
appindicator apps on the same watcher registered fine, so the bug
was ours.

Root cause: on Linux, `tray-icon`'s backend is libappindicator and
its menu layer is `muda`, and **both drive from GLib main-context
sources on the process-default context** — the SNI registration
call, watcher owner tracking, and menu-event delivery only dispatch
while that context is iterated. ADR-0018's daemon loop iterates
nothing: it builds the tray once (`gtk::init()` +
`TrayIconBuilder::build()`), then blocks on
`recv_timeout(POLL_TIMEOUT)` forever. Construction exports the
objects; the registration source never runs. ADR-0018's "raw
polling suffices on Linux" claim was true for `global-hotkey`
(crossbeam receiver) and false for the GTK-backed tray half —
`daemon.rs`'s module doc had generalized the true half into a false
whole, misleading two efforts.

Alternatives considered:

- **A dedicated GTK main thread** (`gtk::main()` on its own thread,
  events forwarded over a channel) — a second bridge beside
  ADR-0018's, new thread-lifecycle and shutdown plumbing, all to
  do what a few non-blocking iterations per tick already do.
  Rejected as the larger seam for no behavioural gain.
- **Switch the tray backend to `ksni`** (pure-Rust SNI, no GLib) —
  genuinely appealing for a pure-Rust stack (ADR-0001), but it is
  a rewrite of the tray/muda menu layer, not a fix for a missing
  context iteration, and the fix-tray-sni spec gated it behind
  disproof of the no-pump hypothesis. The hypothesis held, so the
  swap stayed out (spec boundary, not a verdict on `ksni` itself).
- **A direct `glib` crate dependency** — the `gtk` crate already
  behind the `tray` feature re-exports the same `glib`; a separate
  dep line risks version skew inside GTK's own stack for zero
  benefit. Rejected.
- **Unbounded or blocking iteration** (`iteration(true)`, or loop
  until the context is empty with no cap) — a pathological
  always-ready source (or a burst) could then starve or wedge the
  daemon loop that ADR-0018 keeps latency-bounded. Rejected.

**Decision.** The daemon's platform loop pumps the process-default
GLib main context itself, bounded per tick.

- `flintshot-app::daemon` gains `pump_glib_context()`: up to
  `MAX_GLIB_PUMP_ITERATIONS` (64) non-blocking
  `glib::MainContext::default().iteration(false)` calls, stopping
  early once nothing is pending. `iteration(false)` never blocks,
  and the cap bounds the worst case — leftover work spreads over
  later ticks instead of piling up inside one.
- Wired as the first step of every `run_event_loop` iteration,
  before draining the tray menu receiver — muda's menu events and
  appindicator's registration/retry sources all progress only after
  it. GLib dispatch latency is therefore capped at `POLL_TIMEOUT`
  (100 ms), imperceptible for tray-paced interaction.
- `glib` is reached via the `gtk` crate's re-export; **no new
  dependency**. The pump is `#[cfg(all(feature = "tray",
  target_os = "linux"))]` so non-tray and non-Linux builds never
  see it.
- ADR-0018's architecture is untouched: same main-thread platform
  loop, same tokio side thread, same mpsc bridge. This is the
  smallest seam that fixes registration *and* menu delivery, and
  the `daemon.rs` module doc now states the per-source truth
  (crossbeam receiver for hotkeys, GLib iteration for the tray)
  instead of the disproven blanket claim.
- The cost — one 64-iteration bounded drain per 100 ms tick on the
  main thread, plus coupling the loop to GLib scheduling — is
  accepted; with no tray (or no Linux) the cfg gate removes it
  entirely.

**Consequences.**

- SNI registration and tray menu delivery dispatch on every
  desktop with a `StatusNotifierWatcher`, including the
  daemon-started-before-watcher race (appindicator's retry /
  `NameOwnerChanged` handling is GLib-source-driven too). Fix
  landed `5c78497` + `8ac5c4f` 2026-08-29; the automated evidence
  is headless, the on-metal L5 re-verification stays a manual pass
  (TEST_MATRIX finding 3 annotation — honest, not simulated).
- The unit tests prove the seam without a display: an idle source
  on the default context never fires on an unpumped spin, fires
  after `pump_glib_context`, fires when driven through the real
  `run_event_loop`, and a self-re-arming source stops at exactly
  the 64-iteration cap. Remove the pump and the tests go red —
  the regression seam the spec asked for. The default-context
  tests serialize on a module mutex because cargo runs `#[test]`s
  on parallel threads and the context is process-global.
- CI's default jobs never compile the `tray` feature; the pump
  tests live in the opt-in tray build path (same honesty as
  ADR-0030's at-spi limitation — the real-path proof is the L5
  run, recorded in TEST_MATRIX).
- A GLib source that legitimately needs more than 64 dispatches
  per tick sees its work spread across ticks, not dropped; nothing
  in the current tray stack is known to need that headroom.
- macOS remains future work: `tray-icon` there needs the main
  NSRunLoop pumped (ADR-0018 already flagged it); the cfg gate
  keeps this decision Linux-scoped and reversible if the platform
  loop is ever reshaped.
