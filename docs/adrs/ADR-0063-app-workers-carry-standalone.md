# ADR-0063: App workers carry `--standalone` — the broadcast did the work

**Context.** The tray's *Settings* / *Open app window* dispatch is
broadcast-first: the daemon emits `SettingsFocus` / `AppFocus` (a
running window shows, focuses, routes), then spawns an `app` worker
as the lock-free fallback that creates a window when none runs. The
design assumed a lock-held child — a window is already running —
"exits promptly." It did not: `focus_existing` delegates
`Request::OpenApp` to the daemon, whose dispatch arm spawns the next
`app` worker, which finds the lock held, delegates again… The chain
never terminates while the window lives. One tray click forked a
continuous stream of `flintshot app` processes and re-broadcast
`AppFocus` on every turn (the window re-raising itself), and closing
the window handed the lock to the next child in the chain — a new
window popping open that the user never asked for. The
`capture` / `open` workers solved the identical shape (ADR-0019's
L5 fork loop) with a trailing `--standalone` that skips delegation;
`app_argv` deliberately carried no such flag, relying on the lock
alone — which bounds nothing here, because the delegation happens
*before* the exit.

**Decision.**

- `flintshot app` gains `--standalone` (hidden intent: daemon-spawned
  workers only). `worker::app_argv` appends it to every app argv it
  spawns. A standalone child that finds the app lock held exits
  quietly: the daemon's broadcast — always sent *before* the spawn —
  already performed the focus/route work; delegating would only
  re-arm the loop.
- A user-run `flintshot app` (no flag) keeps the focus ping: its
  single `OpenApp` delegation focuses the running window, and the
  worker that spawn creates is standalone and stops there.
- With the lock-held path silenced, a *Settings* click landing on a
  window already showing Settings has no route to swap to; the
  window raises the in-window toast ("Settings is already open") so
  the ping is answered rather than silently consumed. Other pages
  still route to Settings in place; a closed window still opens
  directly on Settings.

**Consequences.**

- One tray click = one broadcast + one short-lived lock-fallback
  process, regardless of window state. No fork storm, no focus
  flapping, no resurrected windows.
- The daemon-side dispatch arms stay unchanged (broadcast + spawn
  unconditionally): the app lock remains the single window-presence
  truth, and the fallback spawn is now harmless when it loses the
  race.
- Version skew is benign: an old binary without the flag never sees
  it (flags arrive only from a same-build `app_argv`); a new daemon
  paired with an old app-worker binary reintroduces the loop only in
  that mixed deployment — same-build upgrades are the supported
  path.
- Any future request that spawns an `app` worker must append
  `--standalone` (pinned by `app_workers_spawn_standalone_to_break_delegation_loop`)
  or re-derive its focus semantics from scratch.
