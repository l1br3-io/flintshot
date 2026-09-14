# ADR-0052: App-window presence by connection liveness

**Context.** The pre-grab handshake (ADR-0048) clears every
flintshot surface before a worker's grab. For the app window it
waits for a `ReportAppHidden` ack — but only when its `app_alive`
input is true, and that input came exclusively from the session
registry, which only tracks workers the **daemon spawned**
(`register_app` lives in the spawner). The normal app-window flow is
the reverse of that: the user runs `flintshot app`, which
auto-starts the daemon. Every directly-launched app window was
therefore invisible to the handshake: `app_alive()` read false, the
machine treated the ack wait as vacuous, and the only protection for
an in-flight window close was the 250 ms unmap grace — which races
the window's own close latency (≤ 250 ms directive repoll + a frame
+ compositor teardown). Live on COSMIC (2026-09-14) this surfaced as
"the app window is in the shot"; a second, independent race (a
one-shot "window destroyed" mark that a later handshake could
consume while a fresh window was open — fixed separately in
`app_window.rs` by replacing the mark with `window_open` state)
compounded it.

Tracking the app by pid was considered and rejected: the daemon is
not the parent of a directly-launched app window, so it holds no
`Child` to poll, and a heartbeat request adds a protocol the
connection already implies.

**Decision.** The app window's pump opens a dedicated **presence
connection** after subscribing: one `Request::AnnounceAppWindow`
frame, then the connection parks. The daemon registers presence on
receipt (an RAII guard around the parked read loop) and withdraws it
when the connection ends — EOF, error, or task drop. Presence is
thus *connection liveness*: process death closes the socket, which
withdraws presence with no pid, no dead-man flag, and no periodic
traffic. The daemon's main loop feeds the prepare gate
`registry-app-alive OR presence`, so the handshake waits for the
hide-ack from every app window it can see on the bus, spawned or
not. Version skew: a daemon without the interception answers the
dispatch tail's "unsupported" and parks the connection in its read
loop all the same — the client cannot tell and does not need to;
behavior degrades to today's grace-only handshake.

**Consequences.**

- The vacuous-handshake race closes for directly-launched app
  windows: an external capture (hotkey, tray, applet) with the app
  window open now waits — bounded by the prepare gate's existing
  3 s — for the window's truthful ack, which the app window sends
  only while no window exists (the `window_open` state).
- One extra parked local-socket connection per app window for the
  daemon's lifetime — the same order of cost as the subscription
  connection beside it.
- Presence is per-connection, so a future multi-window app (or a
  second app process) needs no protocol change: each window's
  presence is independent, and the count reaching zero is what
  matters.
- Rolling-skew safe by construction: old daemon + new app degrades
  to grace-only; new daemon + old app simply sees no presence (the
  old app never announces), which is exactly its registry-blind
  behavior today.
