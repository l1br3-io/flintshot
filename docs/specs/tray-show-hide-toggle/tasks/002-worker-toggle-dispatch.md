# Task 002: Daemon-side toggle resolution — presence forward-or-spawn

**Description:** Teach the daemon to resolve `ToggleWindow` safely:
join it to `forwards_to_app_window()` so the connection layer's
existing routing sends it through `CaptureDispatch::
dispatch_open_blocking`; there, presence armed → broadcast
`Event::AppToggle` and spawn **nothing** (the hide-then-reshow race
pin); presence absent → fall through to a new plain-`dispatch` arm
that runs the `OpenApp` choreography byte-identically (broadcast
`Event::AppFocus` + spawn `app_argv(AppRoute::Home)`). Also register
the variant name.

**Acceptance criteria:**
- [ ] `forwards_to_app_window(Request::ToggleWindow) == true`
      (extend the predicate's test)
- [ ] `dispatch_open_blocking(ToggleWindow)` with presence armed
      emits exactly `[Event::AppToggle]` and performs **zero**
      spawns (RecordingSpawner pin — the race pin)
- [ ] `dispatch_open_blocking(ToggleWindow)` with no presence runs
      the plain arm: emits `[Event::AppFocus]` and spawns one
      worker with `app_argv(AppRoute::Home)` (`["app",
      "--standalone"]`), `DispatchOutcome::Continue`
- [ ] Plain `dispatch(ToggleWindow)` (the fallback arm) matches the
      `OpenApp` arm's events + argv exactly (mirror of the
      `open_settings_dispatches` precedent tests)
- [ ] `variant_name(ToggleWindow) == "toggle-window"` (extend
      `variant_name_covers_every_current_variant`)
- [ ] `spawns_capture_worker(ToggleWindow) == false` (extend the
      request-side test — the icon must not vanish)
- [ ] No changes to `ipc_server.rs`'s routing logic (the predicate
      already routes it) and none to `drain_hotkeys` (its plain
      `dispatch` call lands on the fallback arm — the spawn
      direction — which is correct for a hotkey context)

**Verification:**
- [ ] `cargo test -p flintshot-app --all-targets` green
- [ ] `cargo build` green

**Dependencies:** Task 1 (the IPC variants exist)

**Files likely touched:**
- `crates/flintshot-app/src/worker.rs`

**Docs:**
- Why-comments on the race pin (spawn-nothing when present) and the
  fallback arm; house style.

**Estimated scope:** Small: 1 file
