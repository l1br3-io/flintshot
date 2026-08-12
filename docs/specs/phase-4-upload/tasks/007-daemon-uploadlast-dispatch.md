# Task 007: Daemon `UploadLast` dispatch + `Event::UploadStarted`

**Description:** Wire `Request::UploadLast { backend }` into the worker
dispatch table (`worker.rs`). Replaces the current
`Event::Error { code: "unsupported" }` arm. Resolves the last-saved path
from `DispatchState` (already tracked since Phase 3 via
`ReportSaved`), spawns `flintshot upload <last-path> --backend <backend>`
as a worker process, emits `Event::UploadStarted { backend }`
optimistically (mirrors Phase 2's `CaptureStarted` pattern). With no
prior save, emits `Event::Error { code: "no-last-capture" }` (mirrors
`PinLast` / `OpenLast`).

Remove `UploadLast` from the "unsupported requests" test case (it's now
supported).

**Acceptance criteria:**
- [ ] `dispatch` handles `Request::UploadLast { backend }`: resolves
      last-saved path, spawns `upload <path> --backend <backend>`,
      emits `Event::UploadStarted { backend }`
- [ ] `UploadLast` with no last-saved path emits
      `Event::Error { code: "no-last-capture" }` (no spawn)
- [ ] `upload_argv(path, backend)` helper added (mirrors `pin_argv`)
- [ ] `RecordingSpawner` tests cover: spawn with path + backend flag,
      spawn at default (no `--backend` when backend is the config
      default), no-path error case, spawn-failure propagation
- [ ] `UploadLast` removed from the `unsupported_requests_emit_error`
      test case

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`

**Dependencies:** Task 003 (Event::UploadStarted variant)

**Files likely touched:**
- `crates/flintshot-app/src/worker.rs`

**Estimated scope:** Small: 1 file
