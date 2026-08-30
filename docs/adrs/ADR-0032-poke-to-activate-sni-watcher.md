# ADR-0032: Poke-to-activate the SNI watcher

**Context.** On L5 (COSMIC, Wayland, 2026-08-30) the session's
StatusNotifierWatcher service
(`cosmic-applet-status-area --status-notifier-watcher`) had exited
while Flintshot's daemon kept running. The tray item registers
correctly the moment a watcher exists — the GLib pump from
ADR-0031 works; the item appeared within a second of the watcher
being D-Bus-activated manually (`busctl --user call
com.system76.CosmicStatusNotifierWatcher /StatusNotifierWatcher
org.freedesktop.DBus.Peer Ping`). The gap was on our side: when no
watcher exists, Flintshot waits passively forever. Watching
`NameOwnerChanged` — what libappindicator does — never *activates*
anything; the only client-side trigger for D-Bus activation is a
method call against an activatable well-known name. And the
standard name `org.kde.StatusNotifierWatcher` is not activatable
on COSMIC (no owner, no service file — observed); COSMIC's
activatable name is the vendor name
`com.system76.CosmicStatusNotifierWatcher`. On KDE / GNOME-with-
extension the watcher is owned by the shell, so any poke there is
a harmless round-trip.

Alternatives considered:

- **Event-driven resurrection (NameLost → poke)** — needs a
  match-rule task with the same zbus surface plus its own
  lifecycle, to save one cheap bus-daemon call per interval.
  Rejected.
- **gio/GLib D-Bus instead of zbus** — would add a new crate and a
  second D-Bus stack to the build. Rejected.
- **Managing SNI item registration ourselves** — libappindicator
  already re-registers on watcher appearance (observed); taking
  that over doubles the surface for zero gain. Rejected.
- **Config knob for the interval** — no user can currently want a
  different cadence; surface area now, tuning later if ever.
  Rejected (spec fences it out).

**Decision.** The daemon (Linux, `tray` feature) runs a dedicated
SNI watchdog thread that probes
`NameHasOwner(org.kde.StatusNotifierWatcher)` **immediately at
startup — that first probe is the startup poke — and then every
30 s**. When the watcher is unowned it pokes the standard name
first, then `com.system76.CosmicStatusNotifierWatcher`, stopping
at the first success. `NameHasOwner` targets `org.freedesktop.DBus`
(the bus daemon) and therefore never triggers activation — safe as
the poll gate; only the unowned branch fires activation pokes
(`Peer.Ping` at `/StatusNotifierWatcher`, matching the invocation
verified live).

*Amended after ship-review on live evidence (task 006):* the
fire-and-forget startup lost a race — the poke-activated watcher
appeared *during* libappindicator's init, its `NameOwnerChanged`
event was missed, and the icon never registered until a daemon
restart. Startup therefore flipped to the blocking variant of the
spec's open question 1: `ensure_watcher()` runs synchronously
before `tray::build` (probe → poke if unowned → poll until owned,
bounded ≤ 2 s; returns immediately when no activatable service
answers, so tray-less desktops pay nothing beyond two failed
pokes). The watchdog thread is unchanged and owns **mid-session**
liveness only.

- `NameHasOwner` is answered by any live host under the standard
  rendezvous name — COSMIC's watcher included; it's how
  libappindicator finds the watcher too.
- Shape: a self-contained `std::thread` (the GLib-pumped main loop
  from ADR-0031 must never block on a bus call) whose blocking
  poke runs on its own current-thread runtime — the
  `send_request_blocking` precedent, `block_on`-shaped because the
  poke is fire-and-collect, not ambient-handle-dependent
  (ADR-0030 owns the ambient case for *window* workers).
- Pokes are best-effort, bounded by a 2 s `tokio::time::timeout`
  wrapper (a poke that outlives the bound is not wasted — the bus
  keeps activating; the next probe reaps it); every failure logs at
  `debug`, because a tray-less desktop is a normal steady state.
- Shutdown mirrors the daemon's channel pattern: dropping the
  watchdog's mpsc sender reads as `Disconnected` in
  `recv_timeout(30 s)` instantly, so the join is bounded by at most
  one in-flight poke.
- **`zbus` becomes a direct dependency of `flintshot-app`**
  (workspace-pinned `5`, `default-features = false`,
  `features = ["tokio"]`, target-scoped + optional behind the
  `tray` feature — the same shape as `dep:gtk`). zbus 5.18 is
  already compiled into the tree via ashpd (ADR-0029), so this
  adds a dep *edge*, not a new crate build; the tokio backend
  matches the workspace's pinned choice. The zbus 4.4 under
  keyring's secret-service path already coexists in the lock and
  is unrelated. Decision logic sits behind a sync `SniBus` trait
  seam, unit-tested with fakes; real activation is bus-daemon
  behavior CI cannot simulate, so the live path is proven by the
  manual test matrix — stated honestly, not pretended.

**Consequences.**

- A dead or late tray host no longer leaves Flintshot invisible:
  on COSMIC the activatable watcher is brought back within one
  interval; elsewhere the standard-name poke is a no-op
  round-trip. Healthy sessions see one `NameHasOwner` probe per
  30 s and nothing else — no extra processes, no log output above
  `debug`.
- A desktop whose tray was deliberately removed but whose watcher
  remains activatable keeps one headless watcher process alive
  (items register, no icon renders). If the COSMIC watcher also
  idles out with zero items, the poll could create a spawn/exit
  cycle; the L5 runbook checks for it and a back-off-after-N-
  futile-activations guard is fenced off as a follow-up, not this
  effort.
- The watchdog thread adds one more shutdown ordering the daemon
  must respect (join after the event loop); it is bounded and
  already mirrored by the tokio-side join.
- If COSMIC ever ships an activatable standard-name service file,
  the vendor-name branch becomes dead weight but stays harmless;
  removal is a one-line deletion plus a test.
