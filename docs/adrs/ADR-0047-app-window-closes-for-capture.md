# ADR-0047: The app window closes for a capture — hiding is not a portable guarantee

**Context.** Reported live on COSMIC (2026-09-09): captures
started from the app window still contained the window — "it does
not change anything at all." Source reading of winit 0.30.13
confirms why: the Wayland backend implements `set_visible` as a
**hard no-op** (`// Not possible on Wayland.` in
`platform_impl/linux/wayland/window/mod.rs`). The ADR-0045 design
hid the window with `ViewportCommand::Visible(false)` before the
grab — a strategy that only ever worked on X11, and the scripted
XV1 pass (X11-only) could not catch it. `set_minimized` is no
better: xdg-shell has no minimize request for clients to send.

**Decision.** The home window **closes** for a capture, on every
platform — surface destruction is causal on every compositor,
which is exactly the guarantee the editor's recapture loop already
relies on (ADR-0046). The flow:

1. A control click registers the intent
   (`Shared::set_pending_capture`) and sends
   `ViewportCommand::Close`; the eframe loop exits, destroying the
   window.
2. The process persists (app lock + subscription pump held). An
   outer loop in `app_window::run` — one `run_home` iteration per
   window lifetime — waits out the 250 ms unmap grace, delegates
   via the pump, then blocks on a condvar until a `Show` directive
   (or the pump's death) reopens a fresh window.
3. `Show` still means "bring the window forward": in an open window
   it is the focus ping (`Visible(true)` + `Focus`); with no
   window it is the reopen signal. One directive, two consumers,
   rendezvoused through a mutex + condvar `Shared` state (no
   channels — a reopened window's bridge picks up where the last
   left off, including the status line).

**Consequences.**

- The not-in-the-shot guarantee is now uniform across X11, Wayland,
  macOS, and Windows — no platform branches to keep honest.
- The window is recreated per capture: it re-centers (it was
  `centered` anyway) and costs one eframe startup (~50 ms),
  both acceptable for a capture launcher.
- The ADR-0045 "hidden window polls directives" machinery is gone:
  with no window there is no event loop to keep alive — the outer
  loop's condvar wait replaces the repaint heartbeat, and the
  dead-loop risk disappears.
- `HIDE_GRACE_MS` keeps its name and value; it now measures
  close-to-grab teardown instead of hide-to-grab unmap, shared
  with the editor's recapture loop (ADR-0046).
- Session accounting is unchanged: the worker process persists
  across the window's close/reopen, so `SessionEnded` (and the
  app window's own restore) still fires once, at chain end.
