# Task 003: Extend IPC schema (`Event::UploadStarted`, `Request::ReportUploaded`)

**Description:** Add two new variants to the IPC schema in
`flintshot-core::ipc`:
- `Event::UploadStarted { backend: String }` — emitted optimistically
  on `UploadLast` dispatch (mirrors `CaptureStarted`).
- `Request::ReportUploaded { url: String, delete_handle:
  Option<String> }` — worker→daemon upload result report (parallels
  Phase 3's `Request::ReportSaved`).

Both are `#[non_exhaustive]` forward-compat additions. The postcard
round-trip test in `ipc.rs` must be extended with these variants.

**Acceptance criteria:**
- [ ] `Event::UploadStarted { backend: String }` variant added
- [ ] `Request::ReportUploaded { url: String, delete_handle:
      Option<String> }` variant added
- [ ] Both variants included in the `request_postcard_roundtrip` and
      `event_postcard_roundtrip` tests
- [ ] `worker::variant_name` updated to name `report-uploaded` (for the
      defensive unsupported-error message)
- [ ] `worker::dispatch` has a defensive `Request::ReportUploaded { .. }
      => Continue` arm (mirrors `ReportSaved`)

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/ipc.rs`
- `crates/flintshot-app/src/worker.rs` (variant_name + defensive arm)

**Estimated scope:** Small: 2 files
