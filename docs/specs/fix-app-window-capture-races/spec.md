---
status: active
started: 2026-09-14
---

# Spec — Fix the app-window capture races

> Fix effort. Stands on the app window (ADR-0045, ADR-0047), the
> prepare handshake (ADR-0048), and the native capture path
> (ADR-0050/0051). It closes two live-reported defects (COSMIC,
> 2026-09-14): *the app window ends up in the shot after pressing
> Capture*, and *the transition between capture phases is a mess —
> sometimes the app window has to be closed by hand for anything to
> happen*.

## Objective

The ADR-0047/0048 not-in-the-shot guarantee must hold for **every**
capture trigger while an app window exists, and the capture-phase
transitions (app window → OS selection UI → editor → app window)
must never strand the user with a stuck window.

**Success looks like:** pressing Capture in the app window (or a
hotkey/tray capture while it is open) never leaves the window — or
its ghost — in the frozen frame; the tray icon returns the moment
pixels are taken (never held hostage through the editor session); a
dropped close command self-heals instead of wedging the armed
capture intent.

## Root causes (the diagnosis)

1. **Vacuous handshake for directly-launched app windows.** The
   prepare gate waits for the app's hide-ack only when its
   `app_alive` input is true, but the session registry only tracks
   app workers the **daemon** spawned (`register_app` lives in the
   spawner). The normal flow — user runs `flintshot app`, which
   auto-starts the daemon — is invisible: `app_alive()` is false
   forever, the handshake blesses the grab after the 250 ms unmap
   grace alone, and that grace races the window's own close latency
   (directive repoll ≤ 250 ms + a frame + compositor teardown).
   *Sometimes the window loses the race.*
2. **One-shot ack mark.** `Shared::window_destroyed` was a `bool`
   set on every close and consumed on `HideForCapture`. A close with
   no follow-up handshake (delegate hiccup, worker death pre-grab,
   recapture chains) leaves a stale mark; a later handshake consumes
   it and acks instantly **while a fresh window is open**.
3. **No stuck-close recovery.** A compositor dropping the home
   window's `ViewportCommand::Close` strands the armed capture
   intent — the outer loop delegates only once the window dies, so
   nothing happens until the user closes the window by hand (the
   reported "press the close button for it to be closed").
4. **Tray held through the editor session (native path).**
   `native_capture_and_dispatch` kept its `CaptureGuard` for the
   whole post-grab dispatch **including the editor shell**, unlike
   the fallback path's `capture_prepared` (grab-only scope) and the
   guard's own doc ("the tray returns the instant the pixels are
   taken"). Annotating for minutes with no tray removes the user's
   only cancel affordance while no grab is in flight.

## Mechanism (the decision)

1. **App-window presence by connection liveness** (ADR-0052). The
   app window's pump opens a dedicated **presence connection** after
   subscribing: one `Request::AnnounceAppWindow` frame, then the
   connection parks. The daemon tracks presence as "the announcing
   connection is still open" — process death closes the socket and
   withdraws presence (no pid, no dead-man flag, no heartbeat). The
   main loop feeds the prepare gate `registry-app-alive OR
   presence`, so the handshake waits for `ReportAppHidden` even from
   an app window it never spawned. Old daemons answer "unsupported"
   and park us in their read loop — identical outcome, so the skew
   degrades to today's grace-only behavior.
2. **Close state, not a mark.** `Shared` now tracks
   `window_open` (outer loop sets open before `run_home`, closed
   after it returns). The pump's ack waits for *currently closed* —
   a truthful reading that cannot be consumed stale.
3. **Stuck-close watchdog.** `HomeApp` latches the Capture click; if
   the window is still alive 500 ms later (the repaint heartbeat
   guarantees the check runs), it re-issues
   `ViewportCommand::Close` every frame (idempotent) and warns once.
   The window's death is the only exit — as it always was.
4. **Grab-only guard scope on the native path.**
   `native_grab_prepared` (the twin of the fallback's
   `capture_prepared`) owns the guard: handshake → grab → release.
   The editor session never holds the tray.
5. **Copy closes the editor on Wayland.** The stay-open-after-Copy
   rule exists solely for X11's owner-based clipboard; under
   Wayland's compositor-owned `zwlr_data_control` the window closes
   immediately after a copy (the OS-owned-clipboard platforms'
   semantics).

## What is explicitly not fixed here

- The COSMIC portal's own interactive-selection flakiness (Spike C:
  unreliable Escape, wedge after sustained hammering). The 5-minute
  interactive budget stays (user think-time); recovery from a wedge
  remains the supersede path.
- The pump's degraded mode (no auto-reconnect after a daemon death).

## Testing

Unit: presence counter + machine composition (`prepare_gate.rs`),
announce interception liveness (`ipc_server.rs`, duplex-stream
integration), close-state semantics + cross-thread wake
(`app_window.rs`), watchdog bound/arming (`home.rs`), copy-close
platform policy (`app.rs`). Live: the new TEST_MATRIX section
(self-trigger ×5, external-trigger-with-window-open ×5, supersede
chains, stuck-close healing, tray-restore-after-grab, copy-close
paste round-trip).
