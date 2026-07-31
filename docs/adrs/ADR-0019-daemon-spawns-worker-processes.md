# ADR-0019: Daemon spawns worker processes, resolves binary via `current_exe`

**Status.** Accepted.

**Context.** Phase 2's daemon must trigger capture+editor work on tray
clicks, global hotkeys, and IPC requests. It could capture in-process
(`flintshot-core::capture` + the egui editor in the daemon's own
threads), but that couples two things best kept apart: the daemon's
event loop (tray/hotkey/IPC, ADR-0018) and the editor's UI thread
(eframe). An editor panic would take down the tray; the editor's render
loop would contend with the daemon's async runtime; and we'd re-plumb
the Phase 1 capture+editor path into a new call site.

**Decision.** The daemon never captures in-process. Every capture action
spawns a **`flintshot` child worker** running the Phase 1
`region` / `fullscreen` CLI verbatim:

- The binary is resolved with [`std::env::current_exe`], **not `PATH`**.
  This guarantees the worker is the exact same `flintshot` binary as the
  daemon (no shadowing, no `PATH` surprises), and works whether the
  daemon was launched by absolute path or discovered on `PATH`.
- The argv is the subcommand + flags: `region [--monitor N]
  [--delay-ms M]` and `fullscreen [--monitor N]` — identical to what a
  user types, so the worker code path is reused, not forked.
- stdin/stdout are null'd (the editor reads/writes nothing there);
  stderr is inherited so the worker's tracing logs survive for
  debugging.
- Each worker gets a lightweight reaper thread that blocks on
  [`Child::wait`], so a finished worker is reaped rather than lingering
  as a zombie while the daemon runs (captures are infrequent, so a
  thread-per-worker is cheap).
- The spawn sits behind a `Spawner` trait with a `CommandSpawner` (real)
  and a `RecordingSpawner` (test), so the dispatch table mapping
  `Request → argv + Event`s is unit-testable without forking.

**Consequences.**

- Isolation: an editor crash or hang can't kill the tray/hotkey/IPC
  layer — they're separate processes.
- Reuse: the Phase 1 capture+editor CLI is the worker; no second
  capture path to maintain. Phase 1's standalone `flintshot region`
  keeps working with no daemon (Task 011's detect-and-delegate).
- The daemon learns a worker's *outcome* (`EditorOpened` / `Saved`) only
  if the worker reports back — a detached `flintshot region` child does
  not. `CaptureStarted` is emitted at spawn time; the later events need
  a worker→daemon result channel (deferred; tracked for the full
  `Event` stream).
- `current_exe` resolution means a relocated/renamed binary still
  spawns itself correctly; the one caveat is setuid/setgid scenarios
  (out of scope for a user desktop tool).
