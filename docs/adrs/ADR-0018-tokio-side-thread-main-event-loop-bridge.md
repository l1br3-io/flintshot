# ADR-0018: Tokio side thread + main-thread platform loop, bridged by mpsc

**Status.** Accepted — proven by the Task 003 de-risk spike.

**Context.** Phase 2 makes `flintshot daemon` a long-running process that
must juggle three fundamentally different event sources:

1. An async tokio runtime — the future IPC server (`interprocess`,
   length-prefixed postcard) and the `notify` config watcher are
naturally `async`.
2. `tray-icon` — on several platforms its events must be pumped from the
   thread that created the tray (the main thread on macOS, where the
   NSRunLoop lives there).
3. `global-hotkey` — likewise tied to a platform event loop.

Running tokio's runtime *and* a winit-style event loop on the same
thread is the classic integration hazard: `runtime.block_on` wants to
own the thread, and so does the platform run loop. Doing both in one
place risks deadlock, dropped events, or a contorted `select`. The
hardest open question in the Phase 2 plan (#2) was specifically whether
raw channel polling would suffice or whether a `winit` event loop would
be forced on us.

**Decision.** Split the daemon across two threads, bridged by a plain
`std::sync::mpsc` channel:

- **Tokio side (background thread).** A dedicated thread builds a
  multi-thread tokio runtime (`Builder::new_multi_thread().enable_all()`)
  and owns it for the daemon's life. The IPC server and config watcher
  will be spawned there (Tasks 004/010). For the spike it runs a
  heartbeat ticker (`tokio::time::interval`) and the clean-shutdown
  sentinel `tokio::signal::ctrl_c()`. It forwards actions to the main
  thread over an `mpsc::Sender<DaemonAction>`.
- **Main thread (platform loop).** Acquires the single-instance lock,
  builds the tray (`--features tray`) and registers hotkeys, then runs a
  tight loop that drains `tray_icon::TrayIconEvent::receiver()` and
  `global_hotkey::GlobalHotKeyEvent::receiver()` (both `&'static`
  crossbeam receivers; both libraries pump the OS internally in their
  own threads on Linux) and then blocks on `mpsc::Receiver::recv_timeout`
  for at most ~100 ms before re-polling. Every event is forwarded to a
  single `ActionHandler`.
- **Shutdown.** `ctrl_c()` in the tokio side ends its task; dropping the
  owned `Sender` makes the main loop's `recv_timeout` return
  `Disconnected`, so the daemon exits cleanly with no signal-handling
  crate and no `unsafe`.

Start with **raw polling**, not a `winit` event loop. Escalate to winit
only if a platform proves it needs main-thread run-loop pumping that
polling can't satisfy.

**Decision (proven).** The Task 003 spike ran the daemon and observed:

- tokio-side heartbeats (`source="tick"`) reaching the main-thread
  handler every 500 ms — the bridge works;
- the single-instance lock blocking a second `flintshot daemon`;
- Ctrl-C producing an orderly shutdown (`Ctrl-C received` → `daemon
  channel closed` → `daemon exited cleanly`);
- `global-hotkey` registering successfully under X11/XWayland.

So raw polling **does** suffice on Linux; no winit is needed there. The
tray-click and hotkey-press paths share the identical drain-to-handler
mechanism as the proven tick path; the actual press/click is a manual
TEST_MATRIX item (needs interactive input the harness can't inject
headlessly).

**Consequences.**

- The daemon's main loop never blocks on capture or `eframe` — that
  invariant is preserved structurally, and Task 005's worker-spawn model
  reinforces it (capture runs in a child process, never the loop).
- Adding the IPC server (Task 004) and config watcher (Task 010) is
  "spawn another task on the tokio side"; the bridge is unchanged.
- Tray/hotkey latency is capped at the ~100 ms poll interval. Acceptable
  for human-paced interactions; can be tightened (crossbeam `Select` or
  an eventfd-style wake) if a future task needs sub-frame latency.
- **macOS will need revisiting.** `tray-icon`/`global-hotkey` need the
  main NSRunLoop pumped there; raw `try_recv` polling alone may not
  deliver events. Recorded as future work — do *not* pull `winit` into
  the default build until a macOS run proves it necessary (keeps the
  Linux/CI path lean).
- No new dependency: the bridge is `std::sync::mpsc` + the existing
  `tokio` runtime. The handler sits behind a trait (`ActionHandler`) so
  Task 005 swaps in the worker-spawning implementation without touching
  the loop.
