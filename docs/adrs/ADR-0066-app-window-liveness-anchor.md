# ADR-0066: The app window is the liveness anchor

**Context.** Daemon death was unrecoverable by design: the tray
helper's one-shot daemon watch followed the daemon within ~100 ms
(unregistering the SNI item — the tray "crashes"), and the app
window's pump never reconnected, permanently showing *"daemon
connection lost — restart `flintshot app`"*. Nobody respawned the
daemon, so hotkeys, IPC and the tray all stayed gone until the user
intervened. A degraded window also poisoned the fresh daemon's
presence oracle after a manual restart: the next capture spawned a
worker editor *beside* the still-visible window — which then landed
in the shot. Companion faults fed the same symptom family: a
slow-prepare fallback double-began the tray gate (a 30 s ghost tray),
helper spawn failures were silently dropped until the next capture,
and an IPC-less daemon produced a ~0.5 s helper respawn churn loop.

**Decision.**

- **The app window owns daemon recovery.** On subscription loss (or
  initial connect failure) the pump keeps servicing commands and
  in-process grabs while probing for the daemon on a capped
  exponential backoff (250 ms base, 5 s cap), respawning it through a
  rate limiter (one spawn per 30 s — a crash-looping daemon cannot
  storm process spawns), and on reconnect re-announces presence,
  flips the status to "connected", and re-enters the connected loop.
  There is no terminal degraded state anymore; the old
  "restart `flintshot app`" banner is replaced by a *reconnecting*
  status. The re-announced presence is what keeps capture triggers
  routing in-window after a daemon death (no worker-editor
  duplicate).
- **A reply timeout on `PrepareCapture` is not skew.** The daemon
  arms the tray gate *before* awaiting its own handshakes, so a
  timed-out prepare means the capture is already registered: the
  client owes exactly one `ReportCaptured` and must not run the
  tray-only fallback (the double-begin was the 30 s ghost). Only the
  explicit `unsupported` reply — a known-old daemon that never began
  — keeps the legacy fallback.
- **Helper failures back off; presence is truthful.** Failed spawns
  and consecutive fast self-exits retry on a capped exponential curve
  (first exit free — it may be a real capture/quit click; stable
  uptime resets the count), and the supervisor's `is_present` reports
  the real child state so the gate treats a missing tray as a
  vacuous hide instead of waiting on absent pixels. The helper's
  daemon watch retries its connect inside a bounded ~2 s window (the
  daemon-startup socket race) before declaring the daemon gone;
  a connected stream's end still means gone immediately.
- **Worker handoffs close behind themselves.** The worker editor's
  Settings entry mirrors the ADR-0065 `NavigateHome` choreography
  (delegate, publish `Cancelled`, close), and the no-daemon fallback
  spawns route through `worker::app_argv` so they carry
  `--standalone` like every daemon spawn.

**Consequences.**

- A daemon death self-heals wherever an app window is running: the
  window shows "reconnecting…", the daemon is respawned (at most once
  per 30 s), and the tray, hotkeys and IPC return with it — no user
  action. With *no* window running, daemon death behaves as before
  (nothing anchors it; relaunch `flintshot app` or the daemon).
- Every new wait is bounded and every retry capped; backoff curves
  and the spawn limiter are pure functions pinned by unit tests
  (`reconnect_delay_grows_from_the_base_and_caps`,
  `daemon_spawn_limiter_rate_limits_respawns`,
  `helper_backoff_delay_curve`), with the anchor cycle itself pinned
  by local-socket tests (`reconnecting_loop_*`).
- Runtime proof of the full matrix (kill the daemon, watch the tray
  return, capture in-window) remains a manual-matrix item — the unit
  seams cannot spawn real daemons/trays headlessly.
- The pump's structure changed shape (`degraded_loop` is gone); any
  future "offline mode" work builds on the anchor loop rather than
  reviving a terminal state.
