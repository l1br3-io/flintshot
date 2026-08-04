# Task 004: `Request::PinLast` over IPC

**Description:** The capstone. Wire the daemon's `Request::PinLast` handler (currently returning `Error { code: "unsupported" }` via the `worker::dispatch` wildcard at `worker.rs:117-127`): read the `last_saved` path (populated by Task 002's `ReportSaved`), spawn `flintshot pin <path>` as a worker, and broadcast `Event::Pinned { id }` with a per-daemon monotonic id. Empty `last_saved` (no capture saved yet) → clear `Error { code: "no-last-capture" }`. This is the entry point the future COSMIC applet and a tray *Pin last* item will call.

**Acceptance criteria:**
- [ ] `crates/flintshot-app/src/worker.rs` gains a `pin_argv(path: &Path) -> Vec<String>` builder (alongside `region_argv`/`fullscreen_argv` at `:131-152`) returning `["pin", path.to_string_lossy().as_ref()]`. Unit-tested.
- [ ] `worker::dispatch` (`:89-128`) handles `Request::PinLast` explicitly (no longer the wildcard's `unsupported`):
  - If `last_saved` (read from the shared `Arc<Mutex<Option<PathBuf>>>` wired in Task 002) is `Some(path)`: spawn via `spawn_argv(spawner, &pin_argv(&path))`, broadcast `Event::Pinned { id }` where `id` comes from a per-daemon `AtomicU64` (fetch_add(1)), return `DispatchOutcome::Continue`.
  - If `last_saved` is `None`: broadcast `Event::Error { code: "no-last-capture", message: "..." }`, return `DispatchOutcome::Continue` (no spawn).
- [ ] `dispatch`'s signature gains the `last_saved` handle and the pin-id counter. Preferred shape: a `DaemonState` struct (holding `last_saved: Arc<Mutex<Option<PathBuf>>>`, `next_pin_id: AtomicU64`, and the `SubscriberBus`) passed by reference — this collapses the bus handle `dispatch` already takes and gives Task 002's `ReportSaved` handler a single home. If Task 002 already introduced `DaemonState`, this task extends it with `next_pin_id`; if not, introduce it here and refactor the existing `&bus` parameter into `&DaemonState`.
- [ ] The `Request::ReportSaved` handler (Task 002) is updated to store into `DaemonState::last_saved` (if not already) and the `PinLast` handler reads from the same struct — both via the shared `Arc`, so the IPC server thread and the dispatch path agree.
- [ ] The `Request::PinLast` postcard roundtrip is already covered (Phase 2 added it at `ipc.rs:189`); no new test there.
- [ ] `Event::Pinned { id }` postcard roundtrip is already covered (Phase 2, `ipc.rs:214`); confirm still green.
- [ ] `Request::OpenLast` and `Request::UploadLast` still return `unsupported` (they're their phases; only `PinLast` is promoted out of the wildcard). The wildcard arm shrinks by one case.
- [ ] `daemon.rs` / `ipc_server.rs`: the `DaemonState` (or the individual handles) is constructed alongside the bus (`daemon.rs:75`) and threaded into both the IPC server (for `ReportSaved` reception) and the dispatch callsites (tray `drain_tray_menu` at `daemon.rs:208`, hotkey `drain_hotkeys` at `daemon.rs:259`, IPC `handle_connection` at `ipc_server.rs:264`).

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets` — incl.:
  - `pin_argv("/tmp/x.png")` → `["pin", "/tmp/x.png"]`.
  - `dispatch(&Request::PinLast, RecordingSpawner, ...)` with `last_saved = Some("/tmp/x.png")` → spawns `pin /tmp/x.png`, emits exactly one `Event::Pinned { id }`, returns `Continue`.
  - Same with `last_saved = None` → emits `Event::Error { code: "no-last-capture" }`, spawns nothing, returns `Continue`.
  - Consecutive `PinLast` calls produce monotonic ids (`id_2 > id_1`).
  - Existing `unsupported_requests_emit_error_and_continue` test (`worker.rs:321`) is updated to remove `PinLast` from the `cases` array (it's no longer unsupported) and still passes for `OpenLast`/`UploadLast`/`Subscribe`.
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Integration (`#[ignore]`, needs real socket + display): spin a daemon on a temp socket, send `CaptureFullscreen`, observe `Event::Saved { path }` (from Task 002's worker reporting), send `PinLast`, observe `Event::Pinned { id }`, assert a `flintshot pin` worker process is alive (or that the `RecordingSpawner`'s real equivalent fired). The full end-to-end may need `xvfb`; the pure-dispatch test above covers the no-display case.

**Dependencies:**
- Task 001 (`flintshot pin <path>` must exist for the spawned worker to do anything).
- Task 002 (`last_saved` must be populated by `ReportSaved` for `PinLast` to have a path to read).

**Files likely touched:**
- `crates/flintshot-app/src/worker.rs` — `pin_argv` builder; `PinLast` arm in `dispatch`; signature change to accept `DaemonState` (or `last_saved` + `next_pin_id`); update `unsupported_requests_emit_error_and_continue` test.
- `crates/flintshot-app/src/daemon.rs` — construct `DaemonState` (or the new handles); thread into dispatch callsites.
- `crates/flintshot-app/src/ipc_server.rs` — thread `DaemonState` into `handle_connection`; the `PinLast` arm routes through `worker::dispatch` like the capture requests do (`:310-313`).
- `crates/flintshot-app/src/tray.rs` — only if a tray *Pin last* item is added (out of scope per spec; leave for a follow-up). No change required for the IPC path.

**Docs:**
- `Event::Pinned { id }` doc at `ipc.rs:99-104` currently says "Phase 3 variant — defined now, emitted later." → update to "emitted by `PinLast`" and note the id is per-daemon monotonic (resets on daemon restart; not globally unique).
- `Request::PinLast` doc at `ipc.rs:58-59` currently says "Open a pin … (Phase 3)." → update to describe the `no-last-capture` error case.
- A `// RATIONALE:` comment at the `PinLast` dispatch arm noting it reuses the worker-spawn model (ADR-0019) and the pin-as-process model (ADR-0023).

**Estimated scope:** Medium (3-4 files; the `DaemonState` threading is the bulk of the diff, the dispatch arm itself is small).

**Notes for the implementer:**
- The `DaemonState` refactor is optional but recommended — if threading three new parameters (`&bus`, `&last_saved`, `&next_pin_id`) through `dispatch`, `drain_tray_menu`, `drain_hotkeys`, and `handle_connection` produces an ugly signature, the struct pays for itself immediately. If Task 002 already introduced it for `last_saved`, this task just adds `next_pin_id: AtomicU64`.
- The `PinLast` dispatch must not hold the `last_saved` lock across the spawn — read the path into a local `Option<PathBuf>` clone, drop the lock, then spawn. The lock is a `Mutex`, holding it across a spawn (which may block on `fork`/`exec`) is a contention risk.
- `Event::Pinned { id }` is broadcast **after** the spawn succeeds (mirrors `CaptureRegion`'s order at `worker.rs:97-104`: emit `CaptureStarted`, spawn, emit `EditorOpened`). For `PinLast`: spawn, then emit `Pinned { id }`. On spawn failure, propagate the error (no event emitted) — same as `CaptureRegion`'s failure path.
- The pin id doesn't need to be globally unique or persisted — it's a per-daemon handle for a future "list pins" / "close pin N" feature. `AtomicU64::fetch_add(1, Relaxed)` is sufficient.
