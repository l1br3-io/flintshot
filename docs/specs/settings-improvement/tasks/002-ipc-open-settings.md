# Task 002: IPC schema append (`OpenSettings` / `SettingsFocus`)

**Description:** Append `Request::OpenSettings` and
`Event::SettingsFocus` to `flintshot-core::ipc`, following the
postcard variant-index rule (append-only, never insert). Schema
only — no consumer wiring in this task.

**Acceptance criteria:**
- [ ] `Request::OpenSettings` appended as the **last** variant,
  doc-commented as mirroring [`Request::OpenApp`] (broadcast +
  app-worker spawn) but carrying the destination route
- [ ] `Event::SettingsFocus` appended as the **last** variant,
  doc-commented as the Settings analogue of [`Event::AppFocus`]
  (broadcast on `OpenSettings`; a subscribed app window shows,
  focuses, and routes itself to its settings page)
- [ ] Both added to the postcard roundtrip tests
  (`request_postcard_roundtrip`, `event_postcard_roundtrip`) with a
  settings-improvement comment, matching every prior effort's style
- [ ] Wire bytes of existing variants untouched (append-only; the
  roundtrip tests + variant order are the pin)

**Verification:**
- [ ] `cargo test -p flintshot-core` green
- [ ] `cargo build` green

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/ipc.rs`

**Docs:**
- None beyond the variant doc comments (spec's representative
  snippet is the model).

**Estimated scope:** Small: 1 file
