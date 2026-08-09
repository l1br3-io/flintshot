# Task 3: Worker result reporting (`Request::ReportSaved`) + daemon last-saved state

**Description:** Close the Phase 2 "worker never reports back" gap so
the daemon can learn what the editor saved — the prerequisite for
`PinLast`/`OpenLast` (Task 4) and Phase 4 upload-from-last. Add a
worker-originated IPC variant `Request::ReportSaved { path }`: after
the editor closes with `EditorOutcome::Saved(path)`, the worker
connects to the daemon socket and sends it (silently skipped when no
daemon answers, reusing `ipc_client`'s `NoDaemon` detection). The
daemon keeps an `Arc<Mutex<Option<PathBuf>>>` as the last-saved state,
updated on receipt. This task lands the schema variant, the worker
send, and the daemon-side state + update; Task 4 reads the state for
`PinLast`/`OpenLast`. Records ADR-0023.

**Acceptance criteria:**
- [ ] `Request::ReportSaved { path: PathBuf }` added to
      `crates/flintshot-core/src/ipc.rs` (`Request` enum, `:34`),
      `#[non_exhaustive]`-safe, postcard round-trip test added to the
      existing `request_postcard_roundtrip` (`ipc.rs:175`).
- [ ] Worker reports on save: in
      `crates/flintshot-app/src/cli.rs::run_editor_for_capture`
      (`cli.rs:228`), after matching `EditorOutcome::Saved(path)`, call
      a new `report_saved_to_daemon(&path)` helper that connects to the
      daemon socket and sends `Request::ReportSaved { path }`. Reuses
      `ipc_client::delegate_to`'s framing/connect (or a sibling helper);
      `NoDaemon` / connect-refused is a silent no-op (the worker may be
      standalone).
- [ ] Daemon last-saved state: `Arc<std::sync::Mutex<Option<PathBuf>>>`
      created in `daemon::run` (`daemon.rs:47`), shared with the IPC
      server. `handle_connection` (`ipc_server.rs:264`) updates it on
      `Request::ReportSaved`.
- [ ] `variant_name` (`worker.rs:166`) handles the new variant
      (`"report-saved"`).
- [ ] `worker::dispatch` (`worker.rs:89`) treats `ReportSaved` as a
      no-op state update (it's handled in the connection layer, not the
      spawn table) — OR the connection layer handles it before reaching
      dispatch. Pick the cleaner seam; document in ADR-0023.
- [ ] A delegated capture (daemon spawns worker) → save in editor →
      daemon's last-saved state holds the path. A standalone capture
      (`--standalone` or no daemon) → save succeeds, no report sent,
      no error.
- [ ] ADR-0023 drafted at `docs/adrs/ADR-0023-worker-report-saved.md`
      (Context · Decision · Consequences): why workers now report back,
      the `Request` variant choice, the `Arc<Mutex<Option<PathBuf>>>`
      state, and the NoDaemon-skip semantics.

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets` — incl. new postcard
      round-trip for `ReportSaved`; a daemon-state unit test
      (`ReportSaved` received → state updated; read-back returns the
      path); `variant_name` covers the new variant.
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual: `flintshot daemon` + `flintshot region` (delegates) → save
      in editor → daemon log shows the reported path. `flintshot region
      --standalone` → save → no report attempt (or a debug-level
      "no daemon" skip).

**Dependencies:** None from this effort (extends Phase 2 IPC + worker +
daemon). Independent of Tasks 1–2.

**Files likely touched:**
- `crates/flintshot-core/src/ipc.rs` — `Request::ReportSaved` variant + round-trip test.
- `crates/flintshot-app/src/cli.rs` — `report_saved_to_daemon` helper + call in `run_editor_for_capture`.
- `crates/flintshot-app/src/ipc_client.rs` — reuse/extend the connect+frame helpers for the report send.
- `crates/flintshot-app/src/daemon.rs` — last-saved `Arc<Mutex<Option<PathBuf>>>`; share with IPC server.
- `crates/flintshot-app/src/ipc_server.rs` — handle `ReportSaved` in `handle_connection`; update state.
- `crates/flintshot-app/src/worker.rs` — `variant_name` arm; dispatch treatment.
- `docs/adrs/ADR-0023-worker-report-saved.md` — NEW.

**Docs:**
- ADR-0023 (above).
- `CHANGELOG.md` `[Unreleased]` → Added: `Request::ReportSaved`,
  worker save reporting, daemon last-saved tracking.

**Estimated scope:** Medium-Large (6–7 files; the IPC + daemon threading is the substantive part). If the `Arc<Mutex>` sharing across the tokio/main bridge is fiddly, split the daemon-state wiring into a Task 3b.
