# ADR-0048: Capture sessions — Replace policy + a generalized pre-grab handshake

**Context.** Three gaps shared one root cause: the daemon had no
notion of an *active capture session* and no orchestration for
surfaces it did not spawn for the grab.

1. **Self-capture of non-initiating surfaces.** A grab from the
   tray / hotkey / CLI / applet while any other flintshot window was
   open (editor, overlay, app window, pin) froze that window into
   the shot. Only the *initiating* surface hid (ADR-0036 tray,
   ADR-0045/0047 app window self-trigger, ADR-0046 editor
   recapture); `capture.rs` does raw xcap grabs with zero window
   filtering — and none is possible: Wayland portal captures hand
   the client composited pixels, so client-side exclusion lists
   cannot work there, and true hide/unmap is X11-only
   (winit's Wayland `set_visible` is a no-op — the ADR-0047 lesson).
2. **Duplicate editors.** Every capture dispatch spawned a fresh
   worker unconditionally: an in-progress capture from the app
   window plus a tray capture stacked two editor windows.
3. **`Event::EditorOpened` was a spawn-time proxy**, broadcast the
   moment `Command::spawn` returned — not a signal about any window.

**Decision.** Three coordinated changes, all daemon-orchestrated
(workers stay dumb spawn targets; the daemon owns cross-window
truth):

1. **Session registry + Replace** (`flintshot-app::sessions`): the
   `CommandSpawner` registers capture, pin, and app workers (pid →
   kill handle `Arc<Mutex<Option<Child>>>`; reapers poll
   `try_wait` under short locks so handles stay killable; process
   exit stays the crash-safe truth, ADR-0046). A capture trigger
   **supersedes** before it spawns: broadcast
   `Event::CaptureSuperseded` → subscribed workers close their
   windows gracefully (thread-safe `CloseHandle` →
   `ViewportCommand::Close` → normal exit, outcome `Cancelled`) →
   bounded wait (~1.5 s) → hard kill → bounded wait (~0.5 s) →
   spawn. The supersede-then-spawn pair runs under one capture
   mutex at every entry point (IPC, X11 hotkeys, portal hotkeys),
   so simultaneous triggers serialize; latest wins, no queue.
2. **Generalized pre-grab handshake** (`PrepareCapture` →
   `CaptureReady`): the worker's `CaptureGuard` (RAII, generalizing
   the tray guard) asks the daemon to clear **every** flintshot
   surface. The daemon broadcasts `HideForCapture` (the subscribed
   app window closes and acks `ReportAppHidden` once its eframe
   call returns — the pump acks only after the window is actually
   destroyed), closes registered pins (kill; respawn argvs parked),
   drives the existing tray gate (ADR-0036), and replies once
   clear — every wait bounded, reply-anyway on timeout. An
   ack-then-grace unmap hold (250 ms, shared with ADR-0047's
   `HIDE_GRACE_MS`) covers close ≠ pixels-gone. `ReportCaptured`
   releases the hold and respawns the parked pins at their exact
   argv (position/size/opacity preserved). An old daemon answering
   `PrepareCapture` with silence or "unsupported" gets one
   `HideTrayForCapture` retry — rolling upgrades keep working.
3. **Worker-reported `EditorOpened`**: the worker sends
   `ReportEditorWindowOpened` just before each editor/overlay
   window runs (fresh capture, recapture reopen, `open`); the
   daemon broadcasts one `EditorOpened` per report. The spawn-time
   proxy is gone; `--out` headless captures emit nothing.

**Consequences.**

- One annotation surface at a time; newest intent wins. Unsaved
  annotations in a superseded session are **lost by design**
  (Win11-parity with the recapture loop, ADR-0046) — the graceful
  close is exactly what avoids partial PNG writes before the kill.
- Editor-**hosted** pins die with their superseded host and are not
  restored: durable pins (daemon-registered image + geometry) are a
  follow-up effort. Standalone pins (`PinLast`) respawn after the
  grab.
- The pin registry is daemon-lifetime: pins that outlive a daemon
  restart are unknown to the new daemon and can appear in grabs
  (pre-existing behavior, now explicit). Persisting the registry is
  a follow-up candidate.
- A worker hard-killed between its tray handshake and its capture
  report leaks a gate depth until the 30 s safety restore — rare
  (kill paths usually go through the graceful close), bounded, and
  the pre-existing safety net covers it.
- Every wait is bounded (~1.5 s + ~0.5 s supersede; 2 s client /
  3 s server prepare): no daemon or worker path can block forever,
  and a capture never fails because a window was slow
  (ADR-0036 philosophy, extended).
- Wire variants are append-only (postcard variant-index rule,
  ADR-0011); `HideTrayForCapture` keeps working for old workers,
  and new workers degrade to it against old daemons.
- A user-spawned app window (no daemon-spawned registry entry) is
  not ack-waited on: the daemon broadcasts the hide anyway and its
  grace covers the close, but the precise wait only covers
  daemon-spawned windows. Registering app windows by identity
  (via the real `EditorOpened`) is the future gate's business.
- The app window's `HomeCore` treats `CaptureSuperseded` as a
  hold: the superseded worker's `SessionEnded` must not reopen the
  window straight into the replacement capture's grab (the flicker
  Replace must never cause).
