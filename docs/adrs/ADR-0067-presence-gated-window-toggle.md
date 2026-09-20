# ADR-0067: Presence-gated forward-or-spawn for the tray window toggle

**Context.** The tray menu had no way to dismiss the app window:
*Open app window* (`Request::OpenApp`) only ever opens or focuses.
The Telegram/Discord tray convention is one item that does both —
*Show/Hide flintshot*. Two designs were on the table for the hide
direction. **Broadcast-always** (the `OpenApp` pattern): broadcast a
toggle event and unconditionally spawn the app worker — the spawn
exits promptly when the app instance lock is held. **Presence-gated
forward-or-spawn**: ask the existing presence oracle
(`CaptureDispatch::app_window_present` — `AnnounceAppWindow`
connections plus the daemon-spawned process registry, ADR-0052)
whether a window is live, and only then broadcast; otherwise run the
plain open choreography.

The problem with broadcast-always here is a race the open path never
faces: the toggle's *hide* direction asks a live window to exit, and
its process releases the app lock some milliseconds later. An
unconditional spawn landing inside that window finds the lock free
and becomes a fresh window — resurrecting the window the user just
asked to hide, and (worse) racing the next toggle click's presence
reading.

**Decision.**

- `Request::ToggleWindow` joins `forwards_to_app_window()` and rides
  the same connection-layer routing (`ipc_server.rs`) and
  `CaptureDispatch::dispatch_open_blocking` seam that
  `OpenImage`/`OpenLast` already use: presence armed → broadcast
  `Event::AppToggle` and **spawn nothing** (machine-checked by
  `toggle_window_forwards_when_a_window_is_present_and_spawns_nothing`);
  presence absent → fall through to the plain `dispatch` arm, which
  shares `OpenApp`'s body (`Event::AppFocus` + `app --standalone`).
  No new probe, no connection-layer changes, no `single.rs` changes.
- **The window owns visibility; the daemon never tracks it.** A
  forwarded `AppToggle` is resolved by `HomeCore::on_event` from
  state it already keeps: `hidden` (capture-hidden, surface closed,
  pump waiting on a `Show`) → the existing `app_focus()` show path
  (the existing process reopens); visible → a new
  `HomeDirective::Quit` with X-close semantics — the process exits,
  the daemon is unaffected, a mid-editor session discards exactly as
  an X / Alt+F4 does (ADR-0047: no true Wayland surface-hide exists,
  so toggle = close/respawn and the spawn cost on show is accepted).
- `Quit` mirrors `Hide`'s plumbing: `push_directive(Quit)` arms
  `SharedInner::quit_requested`; both directive-consuming hosts
  (the window router on Loading/Settings, the editor-page host)
  translate it to `ViewportCommand::Close`; `close_reason` orders it
  **after** pending-capture and `hide_requested` (a capture in
  progress owns the screen and the close), and `Show` drains a
  stale `Quit` exactly as it drains a stale `Hide`.
- The tray item is one static label. A live "Show"/"Hide" flipping
  label would need a daemon→helper reverse feed the helper does not
  have; the retired `flintshot.app` id is pinned dead by test and
  never reused.

**Consequences.**

- No hide-then-reshow resurrection: dispatch never spawns an app
  worker while the presence oracle reports a window live.
- Known asymmetry: a future hotkey binding for `ToggleWindow`
  routed through plain `dispatch` (`drain_hotkeys`) would be
  spawn-only — no presence gate, so it could not hide a window. No
  such binding exists; revisit if hotkeys grow open intents.
- Benign lost-click race: a window exiting on its own just as the
  toggle arrives means the forward lands on nobody and the click is
  lost (the user re-clicks). The mirror race (spawn while a window
  is *starting*) cannot duplicate windows — the spawn side's
  `--standalone` child exits quietly on a held lock.
- A toggle during a capture is a deliberate dead click: the
  `app_focus` gate suppresses the show direction while a capture
  flies (the window reopens on `SessionEnded` anyway); a visible
  window's quit direction still closes. Documented at the
  `on_event` arm.
