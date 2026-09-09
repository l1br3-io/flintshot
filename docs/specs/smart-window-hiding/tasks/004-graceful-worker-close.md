# Task 004: Worker subscription + graceful editor/overlay close

**Description:** Make superseded capture workers close their
windows gracefully instead of eating the kill. Two halves:

1. **editor-egui `CloseHandle`** (new, mirrors the `HomeBridge`
   IPC-free pattern — no flintshot-app types in editor-egui):
   - `pub struct CloseHandle { ctx: Arc<Mutex<Option<egui::Context>>> }`
     with `install(&Context)` (called from `app_creator`, where the
     real context exists) and `close()` — thread-safe: sends
     `ViewportCommand::Close` + `request_repaint()` through the
     cloned context; no-op when not installed.
   - `EguiShell::with_close_handle(handle)` stores it; editor and
     overlay `app_creator`s install it. The window closes; the
     outcome slot stays `None` → resolves to `Cancelled` (existing
     default) — no save, clean exit.
2. **app-crate session subscription** (`cli.rs`): `capture_and_dispatch`
   opens a `Subscribe` connection for the worker's lifetime:
   - listener thread: on `Event::CaptureSuperseded` → `handle.close()`
   - pre-shell check: after the grab, before `run_capture_shell`,
     if superseded was seen → exit `Ok(())` (Cancelled semantics)
     without opening a window
   - subscription dropped when the worker exits (guard)

Standalone/no-daemon: `Subscribe` fails with `NoDaemon` → no
listener, no flag — behavior unchanged (one socket probe).

**Acceptance criteria:**
- [ ] `CloseHandle`: install-then-close sends a close (does not
      panic on a `Context::default()`); close-before-install is a
      no-op; handle is `Send`/`Sync` (compile assertion in test)
- [ ] Superseded-before-grab: fake subscription test — flag set
      after "grab" → `run_capture_shell` skipped, worker exits
      cancelled, no editor spawn (assert via injected shell stub or
      log)
- [ ] Recapture loop: a supersede arriving mid-editor-session (flag
      set between sessions in the loop) stops the chain instead of
      recapturing
- [ ] Existing cli.rs tests updated; no editor-egui dependency on
      IPC types (boundary test still green)

**Verification:**
- [ ] `cargo test -p flintshot-app -p flintshot-editor-egui`
- [ ] Standing gates incl. `cargo test --doc -p flintshot-editor-egui`
- [ ] Manual: editor open → new capture from tray → old editor
      closes **gracefully** (window vanishes, process exits 0, no
      kill in logs), new editor opens

**Dependencies:** 001 (variant), 003 (supersede broadcast exists;
graceful path now beats the kill deadline in practice)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/shell.rs` (CloseHandle + builder)
- `crates/flintshot-editor-egui/src/app.rs` / `overlay.rs` (install)
- `crates/flintshot-app/src/cli.rs` (subscription + pre-shell check)
- `crates/flintshot-app/src/ipc_client.rs` (subscribe-stream helper,
  if not reusable from existing code)

**Docs:** CloseHandle doc comment explains the thread-safety
rationale (egui Context is the documented cross-thread channel).

**Estimated scope:** Medium: 4 files
