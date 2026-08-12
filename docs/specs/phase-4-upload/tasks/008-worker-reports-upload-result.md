# Task 008: Worker→daemon upload result reporting

**Description:** Wire the upload result back-channel: after `flintshot
upload` completes successfully, the CLI sends
`Request::ReportUploaded { url, delete_handle }` to the daemon (mirrors
the Phase 3 `report_saved_to_daemon` pattern). The IPC server's
`handle_connection` intercepts `ReportUploaded`, broadcasts
`Event::Uploaded { url, delete_handle }` to subscribers, then continues.

The `report_uploaded_to_daemon` helper in `cli.rs` is best-effort
(`NoDaemon` → silent no-op for standalone uploads). The IPC server arm
is a mirror of the `ReportSaved` handling.

**Acceptance criteria:**
- [ ] `run_upload` calls `report_uploaded_to_daemon(&url,
      &delete_handle)` on success (best-effort, silent when no daemon)
- [ ] `report_uploaded_to_daemon_at(override_path, url, delete_handle)`
      testable seam added (mirrors `report_saved_to_daemon_at`)
- [ ] IPC server `handle_connection` intercepts `ReportUploaded`,
      broadcasts `Event::Uploaded { url, delete_handle }`
- [ ] `Event::Uploaded` reaches subscribers (not just the connection
      that sent the report)
- [ ] `#[ignore]`'d integration test: worker sends `ReportUploaded`,
      subscriber receives `Event::Uploaded` (mirror the
      `ReportSaved` integration test)
- [ ] Report failure does not break a successful upload (best-effort)

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`
- [ ] Manual check: `cargo test -p flintshot-app -- --ignored
      report_uploaded` passes (needs a real local socket)

**Dependencies:** Task 003 (Request::ReportUploaded variant), Task 005
(CLI body)

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs`
- `crates/flintshot-app/src/ipc_server.rs`

**Estimated scope:** Medium: 2 files
