---
status: shipped
started: 2026-08-30
shipped: 2026-08-30
---

# Spec — Fix tray watcher resurrect: daemon self-heals a dead SNI host

> Bug-fix effort. Root cause observed live on L5 (COSMIC, Wayland,
> 2026-08-30): the session's StatusNotifierWatcher service
> (`cosmic-applet-status-area --status-notifier-watcher`) had exited.
> Flintshot's daemon was healthy and its tray item registers
> correctly the moment a watcher exists (the `fix-tray-sni` GLib pump
> works — verified: item `tray_app_49901_1` appeared within a second
> of the watcher being D-Bus-activated). The gap: when no watcher
> exists, Flintshot waits passively forever. Nothing on our side ever
> triggers D-Bus activation, so the icon has nowhere to appear and
> relaunching the daemon doesn't help.

## Objective

Make the daemon resurrect (or surface) the session's SNI watcher
instead of waiting passively:

1. **Startup poke.** Before building the tray icon, the daemon makes
   a best-effort, bounded D-Bus call that triggers activation of the
   session's StatusNotifierWatcher if it isn't running.
2. **Keepalive watchdog.** A background probe re-pokes when the
   watcher disappears mid-session — the exact live scenario: watcher
   died, daemon kept running, icon vanished until manual
   resurrection.

**Target users:** anyone whose desktop environment's tray host can
exit or start late — observed on COSMIC (epoch D-Bus-activatable
status-area); late-starting trays on other DEs benefit from the same
poke.

**Success looks like:** on a session where the watcher was killed
(or never started), the daemon (re)activates it within one watchdog
interval and the Flintshot tray icon (re)appears with no user action
beyond having the daemon running.

## Assumptions

1. A method call to an activatable well-known name is the only
   client-side trigger for D-Bus activation; watching
   `NameOwnerChanged` (what libappindicator does) never activates
   anything. Verified live: `busctl --user call
   com.system76.CosmicStatusNotifierWatcher … Peer Ping` started the
   service.
2. The standard name `org.kde.StatusNotifierWatcher` is **not**
   activatable on COSMIC (no owner, no service file — observed);
   COSMIC's activatable name is
   `com.system76.CosmicStatusNotifierWatcher`. On KDE/GNOME-with-
   extension the watcher is owned by the shell, so a poke is a
   no-op round-trip.
3. libappindicator already re-registers when a watcher appears
   (observed today: registration landed ~1 s after activation), so
   Flintshot never needs to manage registration itself — only
   watcher liveness.
4. `NameHasOwner` calls go to `org.freedesktop.DBus` (the bus
   daemon) and therefore never trigger activation — safe as the
   watchdog's gate.
5. The COSMIC watcher service, once activated, stays resident
   (observed: still running after activation). Worst case on a
   desktop whose tray was deliberately removed, the watchdog keeps
   one headless process alive — items register, no icon renders;
   acceptable and noted as a Known caveat in TEST_MATRIX.
6. zbus 5 (tokio) is already compiled into the tree via ashpd; a
   direct `zbus` dependency adds a dep *edge*, not a new crate
   build.

## Scope (in / out)

**In:**

- New `sni_watchdog` module in `flintshot-app`, gated
  `(feature = "tray", target_os = "linux")`.
- One-shot poke helper: bounded blocking call, tries
  `org.kde.StatusNotifierWatcher` first, then
  `com.system76.CosmicStatusNotifierWatcher`; every failure is
  non-fatal and logged.
- Watchdog thread: every 30 s, `NameHasOwner(org.kde.StatusNotifierWatcher)`
  → if unowned, run the poke. Shuts down with the daemon.
- Wiring: startup poke in `daemon.rs` before `tray::build`; thread
  join on daemon exit.
- Probe injection: the D-Bus-touching layer behind a small trait or
  closure seam so decision logic is unit-testable with fakes.
- TEST_MATRIX: live-session reproduction + fix verification (kill
  watcher → icon returns within ~interval; restart daemon with
  watcher dead → icon appears).
- CHANGELOG under `## [Unreleased]`.
- ADR: poke-to-activate decision (names, interval, why not
  NameOwnerChanged-driven, zbus-direct-dep rationale).

**Out:**

- Any change to tray menu contents or `tray::build` itself.
- Replacing libappindicator's SNI registration with our own client.
- Hotkey/portal, IPC, worker, editor paths.
- Windows/macOS (no SNI there).
- Config knobs for the interval (fixed 30 s; no surface).

## Deliverables

1. `sni_watchdog` module: poke + gated periodic probe, DI seam,
   unit tests with fake probes.
2. `daemon.rs` wiring (startup poke + watchdog thread + join).
3. `zbus` added as a direct workspace-consistent dep of
   `flintshot-app` (ask-first item — flagged below).
4. TEST_MATRIX + CHANGELOG + ADR-00NN.

## Exit criteria

1. On L5 with the watcher killed (`systemctl --user stop` or kill of
   `cosmic-applet-status-area`): the Flintshot icon reappears within
   ~30 s with the daemon left running.
2. On L5 with the watcher dead and a fresh `flintshot daemon`: the
   icon appears at startup (no manual busctl).
3. On a watcher-healthy session (watcher owned at daemon start):
   behavior unchanged — one no-op `NameHasOwner` probe per interval,
   no extra processes spawned, no log spam above `debug`.
4. New unit tests green under `--features tray`; standing gates
   green (`fmt`, clippy `--workspace --all-targets -D warnings`,
   workspace tests, doc tests).
5. TEST_MATRIX rows recorded with commit hash; launcher-desktop-entry
   task 007's row-4 re-check can cite this fix.

## Technologies

- zbus 5 (tokio backend), same version ashpd already resolves —
  **new direct dependency in `flintshot-app`** (ask-first boundary).
- One dedicated `std::thread` for the watchdog; blocking poke runs
  on a short-lived tokio runtime (same pattern as ADR-0030's
  ambient-runtime rationale, but `block_on`-shaped since the poke is
  fire-and-collect, not ambient-handle-dependent).
- No GUI-toolkit types near `flintshot-core` (untouched).

## Commands

```sh
cargo test -p flintshot-app --features tray   # watchdog unit tests
cargo test --workspace --all-targets          # full regression
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
# manual, L5 (COSMIC):
cargo build --features tray && ./target/debug/flintshot daemon &
pkill -f cosmic-applet-status-area            # kill the tray host
# → icon returns within ~30 s (watchdog), or restart daemon → immediate
busctl --user get-property com.system76.CosmicStatusNotifierWatcher \
  /StatusNotifierWatcher org.kde.StatusNotifierWatcher \
  RegisteredStatusNotifierItems                # should list tray_app_<pid>_1
```

## Project structure

| Change                    | Location                                            |
| ------------------------- | --------------------------------------------------- |
| Watchdog module + tests   | `crates/flintshot-app/src/sni_watchdog.rs` (new)    |
| Wiring (startup + join)   | `crates/flintshot-app/src/daemon.rs`                |
| Dependency edge           | `crates/flintshot-app/Cargo.toml`                   |
| Docs                      | `CHANGELOG.md`, `docs/TEST_MATRIX.md`, `docs/adrs/` |

## Code style

Standing repo conventions (AGENTS.md): `#![forbid(unsafe_code)]`,
no `unwrap()`/`expect()` outside tests, comments only for
non-obvious "why", `color_eyre` in the bin crate. The watchdog's
"why it exists" doc comment cites today's live session (watcher
exited; passive waiting = invisible icon).

## Testing strategy

- **Unit (DI seam):** decision logic — "owned → no poke", "unowned →
  poke standard name then COSMIC name", "probe error → log, no
  panic", interval math — tested against fake probes; no real bus
  needed.
- **Unit (real bus, skipped when unavailable):** a poke against a
  zbus in-process peer is *not* representative (activation is
  bus-daemon behavior), so no such test is pretended; the live path
  is covered by the manual matrix instead. Stated honestly.
- **Manual (L5):** exit criteria 1–3 above, recorded in TEST_MATRIX
  with commit hash and session name.
- CI: `tray-build` job gains no new scope (it compiles the tray
  path; watchdog tests run locally with `--features tray`).

## Boundaries

- **Always:** keep every poke best-effort and bounded (≤ 2 s);
  never let tray-watchdog failure affect daemon startup; gate all
  new code behind `(feature = "tray", target_os = "linux")`; record
  real manual results only.
- **Ask first:** the new `zbus` direct dependency; touching
  `tray.rs` internals; CI changes; changing daemon thread topology
  beyond adding the one watchdog thread.
- **Never:** implement our own SNI item registration (stay with
  libappindicator); poll faster than 10 s; spawn per-poke threads;
  `#[allow(...)]` without a justifying comment; unsafe code.

## Success criteria

- A dead or late tray host no longer leaves Flintshot invisible:
  the daemon brings the activatable watcher back (COSMIC) or pokes
  the standard name (other DEs) within one interval.
- No behavior change on healthy sessions beyond one cheap probe per
  30 s.
- The decision logic is pinned by unit tests that fail if the
  gating, ordering, or error handling regresses.

## Decisions

- **Poke-to-activate, not NameOwnerChanged-driven resurrection** —
  the watchdog already polls `NameHasOwner` (never activates); only
  the unowned branch fires activation pokes. Event-driven
  NameLost→poke would need a match-rule task with the same zbus
  surface and more lifecycle; the 30 s poll is one cheap bus-daemon
  call.
- **Standard name first, COSMIC name second** — KDE-family sessions
  own the standard name (poke = harmless round-trip); COSMIC needs
  its vendor name for activation. Order keeps the common case
  trivially correct.
- **zbus direct dep, not gio/GLib DBus** — zbus 5 is already in the
  build via ashpd (no new crate compiled); gio would add a new
  crate and a second DBus stack. tokio backend matches the
  workspace's pinned choice.
- **Watchdog thread, not daemon-loop integration** — the main loop
  is GLib-pumped and must stay non-blocking; a self-contained
  thread with its own bounded runtime joins cleanly at shutdown
  (mirrors the `flintshot-tokio` thread precedent).

## Open questions

1. Should the startup poke block tray creation until the watcher is
   confirmed (up to the 2 s bound), or fire-and-forget before
   `tray::build`? Leaning fire-and-forget — libappindicator
   re-registers on appearance anyway; blocking only delays daemon
   startup for zero added correctness. Resolve in plan.md.
   **Resolved (plan):** fire-and-forget — the watchdog's first
   tick *is* the startup poke (immediate at thread start); no
   separate startup path exists.
   **Amended (task 006, live evidence):** fire-and-forget lost the
   startup race (poke-activated watcher appeared during
   libappindicator init; `NameOwnerChanged` missed; icon never
   registered). OQ1 now resolves to the blocking variant:
   `ensure_watcher()` runs a bounded (≤ ~2 s) probe-poke-confirm
   barrier synchronously before `tray::build`; the watchdog thread
   keeps mid-session liveness.
2. Does the COSMIC watcher exit when it has zero items (idle exit)?
   If yes, the 30 s poll creates a spawn/exit cycle on trays the
   user deliberately removed. Check during the L5 pass; if observed,
   add a "back off after N futile activations" guard in a follow-up,
   not this effort.
   **Closeout:** checked by runbook step 4 (same-PID residency
   assertion); the back-off guard stays fenced as a follow-up and
   is recorded in ADR-0032's consequences.
