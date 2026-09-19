# ADR-0065: The worker editor's Home link delegates to the app window

**Context.** Tray Capture with no app window running follows the
worker fallback (ADR-0053's "worker-editor stays the fallback when
no window answers"): the daemon spawns a capture worker, the worker
grabs and opens its own editor window. That window shares the
unified topbar, whose breadcrumb renders `Home › Editor` with Home
as a live link on the Editor surface. In the app window the router
intercepts `TopbarAction::NavigateHome` and swaps to the Home route
in place (ADR-0054). In the worker window the action reached
`EditorApp::handle_topbar_action`, which — having no home route
behind it — published `Cancelled` and closed the window: the link
was a close in navigation clothing, a leftover from when the
Esc-contract stance predates the topbar giving every editor a Home
affordance.

The Settings entry had already solved the same shape
(`open_settings_from_worker`): worker windows delegate
window-needing navigation to the daemon and keep no route of their
own (spec §Settings route).

**Decision.**

- **The worker Home link delegates `Request::OpenApp` and then
  exits.** The `NavigateHome` arm fires the shell's `open_home`
  hook (a shape-mate of `with_open_settings`) *before* the Esc
  contract — the focus/spawn must be requested while the worker is
  still alive to ask — then publishes `Cancelled` and closes as
  ever. The app crate's delegate runs the daemon's existing
  broadcast-first `OpenApp` choreography: a running app window
  focuses; otherwise the daemon spawns `flintshot app` on Home.
  No daemon at all → the worker performs the spawn itself
  (detached `flintshot app`, mirroring `spawn_settings_app`).
- **No new IPC.** `Request::OpenApp` already implements
  focus-or-spawn; the Home link adds no request variant and no
  daemon-side changes.
- **A running window focuses on its *current* route** — the
  broadcast carries no destination (that is `OpenSettings`'s job),
  so the user's in-progress page wins over a force-route to Home.
- **Hook-absent keeps the Esc contract.** Headless tests (and any
  future non-worker caller that never installs the hook) get
  today's publish + close: never a panic, never a dead click.

**Consequences.**

- After a tray capture, clicking Home lands the user on
  flintshot's home page in an app window instead of a vanishing
  window; the worker editor always closes behind the handoff (a
  brief two-window overlap is the by-design transient).
- The delegate runs the IPC round-trip on the UI thread inside the
  click handler — the accepted `open_settings_from_worker`
  precedent; revisit only if latency shows.
- The app window's routing path is untouched
  (`navigate_home_from_the_editor_routes_home` and the no-hook
  worker test stay as regression pins); Esc / X / capture-intent
  close semantics are unchanged.
