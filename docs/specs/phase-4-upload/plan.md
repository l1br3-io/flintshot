# Plan: Phase 4 — Upload

## Overview

Make `flintshot` able to share a capture without leaving the tool. An
Imgur backend (anonymous + Client-ID authenticated), reachable from
three entry points (editor toolbar, `flintshot upload` CLI, daemon
`UploadLast`), with URL → clipboard + notification + IPC event broadcast
on success. Credential storage via OS keychain; zero-config anonymous
mode.

## Architecture Decisions

1. **`ImgurBackend` lives in `flintshot-app::upload::imgur`; `keyring`
   becomes a `flintshot-core` dep.** The COSMIC applet is a pure IPC
   client and must not link `reqwest`. `UploadContext.credentials` is
   `Option<&'a keyring::Entry>` (direct reference, not a trait object).
   See Task 001, ADR-0024.

2. **`UploadContext.progress` is dropped; `ProgressSink` removed.**
   YAGNI — no consumer needs progress. See Task 001, ADR-0024.

3. **Worker→daemon upload-result reporting uses `Request::ReportUploaded`.**
   Matches the Phase 3 `ReportSaved` precedent. See Task 003, Task 008,
   ADR-0025.

4. **Image-format handling: sniff via `image::guess_format`, send as-is.**
   No `--mime` flag, no re-encode. See Task 005.

5. **`reqwest` for HTTP** — tokio-native, multipart support. See ADR-0026.

## Task Index

### Phase 1: Core foundation (flintshot-core only)
- [ ] Task 1: Rework upload trait types + add keyring to core → `tasks/001-rework-upload-trait-types.md`
- [ ] Task 2: Add `[upload]` config section → `tasks/002-upload-config-section.md`
- [ ] Task 3: Extend IPC schema → `tasks/003-extend-ipc-schema.md`

### Checkpoint: Core foundation
- [ ] Core types compile; all existing tests pass; `no_gui_types_in_core.rs` green

### Phase 2: Backend + CLI (first end-to-end path)
- [ ] Task 4: ImgurBackend impl + workspace deps → `tasks/004-imgur-backend-impl.md`
- [ ] Task 5: `flintshot upload` CLI body → `tasks/005-upload-cli-body.md`
- [ ] Task 6: `flintshot auth imgur` subcommand → `tasks/006-auth-imgur-subcommand.md`

### Checkpoint: Upload works from CLI
- [ ] `flintshot upload /tmp/shot.png` prints URL; clipboard + notification fire

### Phase 3: Daemon integration
- [ ] Task 7: Daemon `UploadLast` dispatch → `tasks/007-daemon-uploadlast-dispatch.md`
- [ ] Task 8: Worker→daemon upload result reporting → `tasks/008-worker-reports-upload-result.md`

### Checkpoint: Upload works from tray/IPC
- [ ] `Event::UploadStarted` + `Event::Uploaded` reach subscribers

### Phase 4: Editor + docs
- [ ] Task 9: Editor Upload button → `tasks/009-editor-upload-button.md`
- [ ] Task 10: Docs (ADRs, CHANGELOG, TEST_MATRIX) → `tasks/010-docs-adrs-changelog.md`

### Checkpoint: Complete
- [ ] All acceptance criteria met; four-command gate green
- [ ] Ready for review / PR

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| `keyring` needs D-Bus / Secret Service on Linux CI | Med | `keyring` is a compile-time dep; runtime keychain access is only exercised in `#[ignore]`'d tests. CI compiles but doesn't run keychain tests. |
| `reqwest` adds significant build time | Low | Already a common dep; tokio-native. Accepted. |
| Imgur API response shape changes | Low | Response parsing is isolated in `imgur.rs`; mockito tests pin the expected shape. |
| Editor closes before upload URL is known | Med | `EditorOutcome::Uploaded` signals "upload initiated"; real URL arrives via `Event::Uploaded`. Documented in Task 009 gotcha. |

## Open Questions

None — all six open questions from the draft spec were resolved during
spec review (see `spec.md` §Decisions).
