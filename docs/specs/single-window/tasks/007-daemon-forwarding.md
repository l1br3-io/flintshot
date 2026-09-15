# Task 007: Daemon forwarding for external captures + restore rework

**Description:** Fork 2 lands. In the daemon's shared capture-dispatch seam
(`CaptureDispatch`, used by IPC, X11 hotkeys, and portal hotkeys): when an
app window is present (`prepare.app_window_present()` OR
`sessions().app_alive()`), run the existing supersede cycle against live
workers, then broadcast `CaptureStarted` + `Event::CaptureTriggered {
delay_ms }` instead of spawning — no worker. Otherwise today's
supersede-then-spawn, untouched. On the app side: the pump handles
`CaptureTriggered` — idle: close window (grace) + start the Task-006 grab;
mid-grab: restart; editor page open: same (page replaced, unsaved
annotations lost — Replace policy). `HomeCore` shrinks per plan decision 8:
hide-on-`HideForCapture`/`CaptureStarted` stays (standalone `--standalone`
grabs still broadcast), but restore targets **the page that was open**
(stash it when hiding), and only worker `SessionEnded`s restore anything.
Note: the app window's own delegated `Request::Capture` now round-trips as
a `CaptureTriggered` — the pump must treat its own echo as the trigger
(exactly one grab per click; the local state machine from Task 006 makes
this idempotent).

**Acceptance criteria:**
- [ ] Hotkey capture with the window open (idle or editing) routes the
      window through the close → grab → editor-page dance
- [ ] No window running: hotkey capture spawns the worker editor exactly as
      today (argv tests unchanged)
- [ ] `flintshot capture` CLI (delegated) routes into the window;
      `--out`/`--standalone`/`--after`/interaction flags never do (existing
      delegation filters — tests pin it)
- [ ] Two rapid hotkey presses: one grab wins (restart semantics), no
      zombie workers, no double editor
- [ ] `HomeCore` unit tests rewritten for same-page restore;
      standalone-worker hide/restore case kept covered

**Verification:**
- [ ] Full gate green
- [ ] Manual matrix: hotkey idle / hotkey mid-edit / no-window hotkey /
      rapid double-trigger / standalone CLI grab with window open
      (window closes and its page restores after `SessionEnded`)

**Dependencies:** Tasks 001, 004, 006

**Files likely touched:**
- `crates/flintshot-app/src/worker.rs` (forward-or-spawn in
  `CaptureDispatch`)
- `crates/flintshot-app/src/ipc_server.rs` (presence wiring into dispatch)
- `crates/flintshot-app/src/app_window.rs` (`CaptureTriggered` handling,
  `HomeCore` rework)
- `crates/flintshot-app/src/daemon.rs` (hotkey path passes the gate/prepare
  refs if the signature moves)

**Docs:** GOTCHA on the echo path (self-delegated capture returning as
`CaptureTriggered`); comment on why forwarding emits `CaptureStarted`
(keep subscriber semantics stable).

**Estimated scope:** Large: 4 files — the daemon seam + app handling are
one atomic behavior change; tests carry the weight.
