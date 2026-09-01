# ADR-0036: Worker-driven tray hiding for the capture grab window

**Context.** Live finding (L5, 2026-08-31, surfaced by the
selection-first effort): every daemon-dispatched capture froze the
panel **with flintshot's own tray icon in the shot** — tray clicks,
hotkeys, CLI delegation, COSMIC-applet actions, desktop actions. The
icon must not be in the frozen frame, but it must come back: the tray
is how you quit the daemon or fire the next capture while an editor
is open.

Constraints that shape the fix:

- The capture runs in a **worker process** (ADR-0019); the tray icon
  lives in the daemon. Only the worker knows when the grab happens.
- The daemon's IPC server runs on the tokio side thread; GTK /
  libappindicator calls are main-thread-only (ADR-0018, ADR-0031).
- The panel removes an SNI item **asynchronously** — a DBus property
  flip, a watcher relay, a repaint. Hiding and grabbing in the same
  instant races the repaint and captures the icon's ghost.
- Wayland's first capture per session may sit on a portal permission
  dialog for an unbounded time.

Alternatives considered:

- **Hide at the dispatch site only.** Cannot serve IPC-triggered
  captures (wrong thread) and no dispatch site knows when its worker
  grabs; restore timing would be a guess.
- **Hide + fixed-delay restore.** Restores at an arbitrary time: too
  early re-ices the icon into a slow (portal-dialog) capture; too
  late leaves the tray gone long after the pixels stopped mattering.
- **Unregister/re-register the SNI item.** Re-enters the
  registration races ADR-0032/fix-tray-sni fought; a status flip is
  enough.
- **Hide for the whole editor session.** Over-broad: the tray is
  useful while annotating.

**Decision.** A worker→daemon handshake spans exactly the grab:

- Pre-grab, the worker sends `Request::HideTrayForCapture`
  (request/reply like `BindShortcuts`). The daemon's main loop flips
  the icon to SNI `Passive` (`TrayIcon::set_visible(false)` — the
  same mechanism its `Drop` uses), pumps `GLib` on Linux, and acks
  `Event::TrayHidden` **only after a propagation grace** (250 ms) —
  an ack that races the panel's repaint is worthless.
- Post-grab, the worker sends `Request::ReportCaptured`
  (fire-and-forget like `ReportSaved`) from a RAII guard that fires
  on success *and* failure, so the icon returns the instant live
  pixels stop mattering — never held for the editor session.
- A **depth counter** spans concurrent captures; the last report
  restores. Main-thread dispatch sites (tray click, hotkey)
  **pre-hide** at the interaction itself — side effect only, no
  depth, held through a 500 ms handshake window — for instant visual
  feedback and a grace that counts from the click.
- Every bound degrades to the pre-fix behaviour, never to a hang:
  the daemon's confirm wait (3 s) acks anyway; the worker's reply
  wait (2 s, deliberately shorter) proceeds anyway; a 30 s **safety
  bound** force-restores if a worker dies between hide and report.
- The machinery is a pure `TrayVisibility` state machine (injected
  clock, unit-tested) inside a thread-safe `TrayGate` shared with
  the IPC side; GTK side effects are applied by the main loop from
  `TrayAction`s. Zero core behaviour beyond two appended `Request`
  variants and a reply-only `Event` (postcard wire stability).
- One choke point in `capture_and_dispatch` covers every trigger
  path; a standalone capture (no daemon) costs a single socket
  probe.

**Consequences.**

- The icon never lands in a frozen frame on any daemon path, and is
  back within ~a poll interval (100 ms) of the grab.
- Daemon-triggered captures gain the propagation grace (~250–400 ms
  click-to-grab): the price of the panel actually re-rendering
  before the pixels are taken.
- A worker killed mid-grab leaves the icon hidden up to 30 s
  (logged, self-healing). The bound deliberately encloses a dawdled
  first-capture portal dialog rather than re-icing the icon into
  that capture.
- macOS / Windows ride the same handshake through native
  `set_visible`; the shared grace is untested there until those
  platforms get a TEST_MATRIX pass.
- The handshake is IPC surface: any future trigger source (or
  third-party client) that grabs pixels should send the same pair,
  or its captures will include the icon.

**Open questions.**

1. Per-platform grace tuning (native tray repaint beats the SNI
   round trip) — one constant until a platform actually ghosts.
2. Whether `--delay-ms` should hide for the whole delay — deferred;
   the delay is the user-controlled "clear the screen" window, and
   changing its semantics is a product call, not a fix.
