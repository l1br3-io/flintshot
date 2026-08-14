# Task 010: Docs — ADRs, CHANGELOG, TEST_MATRIX

**Description:** Write the documentation artifacts the spec's
Deliverables section requires:
- Three ADRs formalizing the key decisions
- CHANGELOG `[Unreleased]` entry for all upload features
- `docs/TEST_MATRIX.md` rows for the manual test matrix

**ADRs to create:**
- **ADR-0024: Upload trait rework** — why `Option<&keyring::Entry>` over
  a `CredentialSource` trait; why `progress` was dropped (Decisions §1,
  §2).
- **ADR-0025: Worker reports upload result** — extends ADR-0023's
  pattern with `Request::ReportUploaded` (Decisions §3).
- **ADR-0026: `reqwest` for HTTP** — extends the tech-stack ADRs; why
  reqwest over ureq/attohttpc (tokio-native, multipart support).

**CHANGELOG:** Add entries under `## [Unreleased] → Added` for:
ImgurBackend, `flintshot upload` CLI, `flintshot auth imgur`, daemon
`UploadLast` dispatch, `Event::UploadStarted`, editor Upload button,
`[upload]` config section, URL→clipboard + notification on upload.

**TEST_MATRIX:** Add rows for: editor Upload button, `flintshot upload`
(success / no-such-file / network-error / anonymous / authenticated),
daemon `UploadLast` dispatch, `flintshot auth imgur` (set / clear).

**Acceptance criteria:**
- [x] ADR-0024, ADR-0025, ADR-0026 created in `docs/adrs/` (Context ·
      Decision · Consequences format)
- [x] CHANGELOG `[Unreleased]` section updated with all upload
      features
- [x] `docs/TEST_MATRIX.md` rows added for all upload entry points
- [x] All doc comments on public types are present and accurate

**Verification:**
- [x] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
      (doc examples still build)
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo fmt --all --check`

**Dependencies:** All prior tasks (documents the completed work)

**Files likely touched:**
- `docs/adrs/ADR-0024-upload-trait-rework.md` (NEW)
- `docs/adrs/ADR-0025-worker-reports-upload-result.md` (NEW)
- `docs/adrs/ADR-0026-reqwest-for-http.md` (NEW)
- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`

**Estimated scope:** Small: 5 files (documentation only)
