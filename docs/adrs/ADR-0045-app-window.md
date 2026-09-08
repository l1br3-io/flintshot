# ADR-0045: The app window as a subscription client — reaper-sourced `SessionEnded`, lock+focus-ping single instance

**Context.** Users asked for the Win11 Snipping Tool's fourth
capture surface: a visible app window with the capture controls in
it, next to the existing tray menu / hotkeys / CLI. Three design
questions shaped everything (interview 2026-09-08):

1. *Where does the window live?* eframe owns its thread; the
   daemon's main loop is a platform-event loop that must stay
   GUI-free. The tray already solved this shape by being a separate
   process (ADR-0042).
2. *How does the window come back after hiding for a capture?* It
   must know when the capture **session** ends — including the user
   cancelling the editor, and including a worker crashing mid-edit.
3. *What if the user opens it twice?* Stacked windows all
   restoring/focusing on the same events is confusing; the second
   open should focus the first.

**Decision.**

- **`flintshot app` is an IPC client process**, exactly the tray
  helper's stance: it subscribes to the daemon's `Event` stream,
  delegates `CaptureRegion` / `CaptureFullscreen` requests after a
  250 ms hide grace (the icon-grace bound's rationale; best-effort
  on Wayland), and auto-starts `flintshot daemon` when no socket
  answers. No capture ever runs in it.
- **`Event::SessionEnded` comes from the daemon's worker reaper**,
  not a worker self-report: the reaper already blocks on every
  spawned child's exit, so broadcasting there is crash-safe (a
  killed worker still "reports"; a self-report would be swallowed)
  and needs no `cli.rs` changes. Only `region` / `fullscreen` argv
  classify as capture sessions (`is_capture_argv`, the argv-side
  twin of `spawns_capture_worker`).
- **The window restores when in-flight returns to zero**, counted
  from the subscription (`CaptureStarted` +, `SessionEnded` −,
  saturating) rather than tracking its own requests — a racing
  hotkey capture can't cause an early restore, and the window never
  pops up mid-capture. A subscription disconnect force-shows with
  an error status so a daemon death can never strand it hidden;
  the pump then stays in a degraded status-only mode (no
  auto-reconnect in this effort).
- **Single window via app lock + focus ping**: `flintshot app`
  holds a named instance lock (`flintshot-app`). A second
  invocation delegates `Request::OpenApp` and exits; the daemon
  broadcasts `Event::AppFocus` (subscribed windows show + focus,
  suppressed while a capture is in flight) and spawns an `app`
  worker that exits promptly on the held lock. The tray's *Open
  app window* item rides the same request.
- **The `.desktop` primary `Exec` becomes `flintshot app`**,
  superseding the launcher effort's "daemon is the app" primary
  entry (that decision predates any window existing): the window
  auto-starts the daemon, so the launcher promise upgrades from
  "start the daemon, see the tray" to "open the app".
  `StartupWMClass=flintshot` follows the home window's title
  (eframe/winit derive WM_CLASS from the title). The autostart
  entry stays `flintshot daemon` (login → tray, no window).

**Consequences.**

- The daemon gains no GUI dependency and no per-client state: the
  window is "just another subscriber + requester", so every
  existing guarantee (worker isolation, tray-hide handshake, Event
  fan-out) applies to window-initiated captures unchanged.
- `SessionEnded` is process-exit truth, not editor-outcome truth:
  it fires for saved, copied, cancelled, and crashed sessions
  alike, and for captures started from *any* trigger. Subscribers
  wanting finer detail still pair it with `Saved` / `Uploaded`.
- The hide grace is unobservable best-effort: on slow compositors
  the window could theoretically still be mid-unmap at the grab.
  Same acceptance bar as ADR-0036's icon grace; TEST_MATRIX covers
  X11 + Wayland sessions.
- Two IPC enum variants (`Request::OpenApp`, `Event::SessionEnded`,
  `Event::AppFocus`) are appended at the enum ends, keeping
  postcard wire bytes stable across version skew (the established
  Phase 6 rule).
- A daemon death permanently degrades an open window (status-only
  mode) until the app is restarted — accepted for this effort;
  auto-reconnect is a follow-up if it bites.
