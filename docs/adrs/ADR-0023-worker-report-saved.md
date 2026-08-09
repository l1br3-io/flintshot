# ADR-0023: Worker save reporting (`Request::ReportSaved`) + daemon last-saved state

**Status.** Accepted.

**Context.** ADR-0019 chose worker processes over in-process capture so
an editor crash can't take down the tray. The cost was asymmetry of
information: the daemon learns a capture *started* (`CaptureStarted` at
spawn) but never learns what the editor *saved* — a detached `flintshot
region` child writes its PNG and exits with no way to tell the daemon
the path. Phase 3's `PinLast` / `OpenLast` (Task 4) and Phase 4's
upload-from-last all need that path: "pin the last save" is meaningless
if the daemon doesn't know what was last saved. The Phase 2 `Event`
stream had an unused `Event::Saved { path }` variant waiting on exactly
this wiring. So we need a worker → daemon back-channel for save
outcomes.

**Decision.**

- **Add a worker-originated `Request::ReportSaved { path }` IPC
  variant** to the existing `Request` enum (not a new message type). The
  worker already connects to the daemon socket for delegation; reporting
  a save reuses the same length-prefixed-postcard framing
  (`ipc_client::delegate_to`'s connect+send path). After
  `EditorOutcome::Saved(path)`, the CLI's `run_editor_for_capture` calls
  a new `report_saved_to_daemon(&path)` helper. The report is
  **best-effort**: `NoDaemon` (absent socket / refused connect) is a
  silent no-op, because a standalone worker (`--standalone`, or no
  daemon running) has no one to report to and must not error.

- **Daemon state is an `Arc<std::sync::Mutex<Option<PathBuf>>>`**,
  wrapped in a `LastSaved` newtype in `ipc_server` (mirroring
  `SubscriberBus`'s shape). It is created in `daemon::run`, cloned into
  the tokio side thread, and shared with the IPC server via
  `IpcServer::bind_with_bus`. The critical section is a single
  clone/swap — never held across an `.await` — so a `std::sync::Mutex`
  (not a `tokio::sync::Mutex`) is correct and avoids async-mutex
  overhead.

- **The connection layer intercepts `ReportSaved`, not the worker
  dispatch table.** `dispatch` maps `Request → argv + Event`s for spawn
  actions; it has no handle to daemon state and shouldn't gain one.
  `handle_connection` updates `LastSaved` and broadcasts
  `Event::Saved { path }` to subscribers directly, then continues. A
  defensive `Request::ReportSaved { .. } => Continue` arm in `dispatch`
  ensures the variant is never misreported as "unsupported" if a future
  refactor routes it there.

- **`NoDaemon`-skip semantics**: `report_saved_to_daemon` reuses
  `delegate_to`, which treats an absent socket file or a refused/not-found
  connect as `DelegateOutcome::NoDaemon`. That same detection backs
  Phase 2's detect-and-delegate, so a standalone capture and a failed
  report share one consistent "is the daemon up?" check.

**Consequences.**

- Closes the Phase 2 "worker never reports back" gap: the daemon now
  knows the path of the most recent save, and subscribers see a real
  `Event::Saved { path }` (previously never emitted) when a worker
  saves.
- `PinLast` / `OpenLast` (Task 4) and upload-from-last (Phase 4) read
  `LastSaved::get()`; the state is the single source of truth for "last
  save."
- The back-channel is one new `Request` variant, not a new transport —
  wire format stays length-prefixed postcard, `Request` stays
  `#[non_exhaustive]`, so existing clients compile unchanged.
- `ReportSaved` is fire-and-forget (no ACK): a worker that reports then
  exits doesn't wait for the daemon to confirm receipt. A lost report
  (daemon down between save and report) means `PinLast` operates on a
  stale path — acceptable for a desktop tool; the user can re-save.
- `LastSaved` is in-process state only (no persistence); a daemon
  restart loses the last-saved path. Persistence is out of scope for
  Phase 3.
