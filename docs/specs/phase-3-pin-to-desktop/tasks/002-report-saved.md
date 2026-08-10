# Task 002: `Request::ReportSaved { path }` — worker→daemon outcome reporting

**Description:** Wire the feedback loop the daemon needs to (a) emit the real `Event::Saved { path }` to IPC subscribers (currently a Phase 2 gap — the daemon emits `EditorOpened` as a process-started proxy but never learns the save path), and (b) know which path `PinLast` (Task 004) should re-pin. Add one new `#[non_exhaustive]` `Request` variant; the daemon stores the last saved path in shared state and broadcasts the event; the worker (the `flintshot region`/`fullscreen` process) sends the report fire-and-forget after `run_editor` returns `Saved`. The send is non-fatal — standalone captures (no daemon) skip it and succeed.

This task closes the Phase 2 "later events need the worker to report back" note at `worker.rs:100-103`.

**Acceptance criteria:**
- [ ] New variant `Request::ReportSaved { path: PathBuf }` in `flintshot-core/src/ipc.rs` (alongside `ReportSaved`'s siblings at `ipc.rs:34-67`). The variant carries a doc comment noting it's worker→daemon (the rest of the enum is client→daemon; the doc clarifies the direction shift is intentional and the daemon is still the server).
- [ ] `Request::ReportSaved` postcard roundtrip covered in `ipc.rs`'s `request_postcard_roundtrip` test (`:175-200`) — add the new variant to the `cases` array.
- [ ] Daemon-side shared state: `crates/flintshot-app/src/daemon.rs` (or a new small `daemon_state.rs` if `daemon.rs` is getting bulky — see [Files](#files-likely-touched)) gains `last_saved: Arc<Mutex<Option<PathBuf>>>` (shared between the IPC server and the dispatch path, same sharing pattern as `SubscriberBus` at `ipc_server.rs:108`). Constructed in `daemon::run` alongside the bus (`daemon.rs:75`); passed to `IpcServer::bind_with_bus` via an extended constructor OR threaded into `handle_connection`.
- [ ] `Request::ReportSaved` handling in `ipc_server.rs::handle_connection` (`:264`): store the path in `last_saved` (lock + replace), broadcast `Event::Saved { path }` to subscribers via the bus. No response is sent (fire-and-forget, matching `Subscribe`'s no-response shape).
- [ ] Worker send in `cli.rs::run_editor_for_capture` (`:228`): after `run_editor` returns `Ok(EditorOutcome::Saved(path))`, call `ipc_client::try_delegate(&Request::ReportSaved { path: path.clone() })` (or a thinner `send_request_blocking` wrapper). On `NoDaemon` or error, log at `warn` and continue — the editor outcome is unaffected.
- [ ] The `EditorOutcome::Pinned(path)` arm (added in Task 003) will also report via this path; for now Task 002 only wires the `Saved` arm. Structure the send so adding `Pinned` in Task 003 is a one-line extension (a helper `fn report_outcome(outcome: &EditorOutcome)` is reasonable).
- [ ] `no_gui_types_in_core.rs` boundary test stays green — `Request::ReportSaved` carries only `PathBuf` (std), no transport types. No changes to the test expected; re-run to confirm.

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets` — incl. extended `request_postcard_roundtrip`, new unit test for the daemon's `ReportSaved` handler (store + broadcast, via a `RecordingSpawner`-style seam or by exercising `handle_connection` with a mock stream), and a test that the worker send is non-fatal when no daemon is reachable.
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Integration (`#[ignore]`, needs real socket — mirror `ipc_server.rs:508`): spin a daemon on a temp socket, connect a client, send `ReportSaved { path: "/tmp/x.png" }`, assert the bus delivered `Event::Saved { path: "/tmp/x.png" }` to a subscriber.

**Dependencies:** None (independent foundation; does not depend on Task 001). Lands the mechanism Task 004 reads and Task 003 extends.

**Files likely touched:**
- `crates/flintshot-core/src/ipc.rs` — `Request::ReportSaved { path: PathBuf }` variant + doc; extend `request_postcard_roundtrip`.
- `crates/flintshot-core/src/lib.rs` — no change needed (`Request` is already re-exported as `IpcRequest` at `:40`; the new variant rides along automatically).
- `crates/flintshot-app/src/ipc_server.rs` — `handle_connection` arm for `ReportSaved` (store + broadcast); the shared `last_saved` handle threaded into `IpcServer` / `handle_connection` alongside `bus`.
- `crates/flintshot-app/src/daemon.rs` — construct `last_saved`, pass to the IPC server.
- `crates/flintshot-app/src/cli.rs` — `run_editor_for_capture` sends `ReportSaved` on `Saved` (extract a `report_outcome` helper if it keeps the `Pinned` addition in Task 003 to one line).
- `crates/flintshot-app/src/ipc_client.rs` — possibly a thin `report_saved(path)` wrapper if `try_delegate`'s signature is awkward for a worker that has no fallback path (it always wants fire-and-forget, never `Delegated`/`NoDaemon` branching).

**Docs:**
- The `Request::ReportSaved` doc comment notes the worker→daemon direction inversion and why fire-and-forget is correct (the worker's job is done; the report is a courtesy for the subscriber stream + `PinLast`).
- ADR-0024 (worker→daemon outcome reporting) — draft during this task; full text lands in Task 005.
- The stale comment at `worker.rs:100-103` ("later events need the worker to report back, which is wired in Task 006") is now live — update or remove it.

**Estimated scope:** Medium (4-5 files; the shared-state threading is the fiddly part, each individual change is small).

**Notes for the implementer:**
- `Arc<Mutex<Option<PathBuf>>>` over `Arc<RwLock<...>>` — the write is a single `replace`, reads are rare (only on `PinLast`); `Mutex` is simpler and the lock is held for microseconds.
- The `last_saved` handle must reach both (a) `handle_connection` (where `ReportSaved` arrives) and (b) `worker::dispatch` (where `PinLast` reads it, Task 004). If threading it through `dispatch`'s signature is awkward, a `DaemonState` struct holding `last_saved` + the `SubscriberBus` + the upcoming pin-id `AtomicU64` is a clean container — consider introducing it here to make Task 004 a smaller diff.
- The worker's `report_saved` send happens **after** the editor window closes (the editor blocks in `run_editor`). This is correct — we only want to report a finalised outcome, not a pending one.
- If `try_delegate`'s `DelegateOutcome::NoDaemon` path is noisy for a worker (which by definition has no daemon in standalone mode), suppress the log to `debug!` for this specific call site — the worker isn't "falling back" to anything, it's just reporting.
