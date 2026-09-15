# Task 001: IPC additions — `CaptureTriggered`, `OpenRequested`, `OpenImage`

**Description:** Append the three wire types the single-window model needs to
`flintshot-core/src/ipc.rs`: `Event::CaptureTriggered { delay_ms: u32 }` (the
daemon forwarding a capture intent to a subscribed app window instead of
spawning a worker), `Event::OpenRequested { path: PathBuf }` (same, for
image-open intents), and `Request::OpenImage { path: PathBuf }` (CLI `open`
→ daemon). Appended at the end of each enum — postcard encodes variant
indices, so appending keeps every existing wire byte stable (the rule every
prior effort followed). Schema only; no daemon or client behavior consumes
them yet.

**Acceptance criteria:**
- [ ] All three variants exist, doc-commented per the module's house style,
      `#[non_exhaustive]` enums untouched otherwise
- [ ] Postcard round-trip tests cover each new variant (field-carrying cases
      included, e.g. non-zero `delay_ms`, a path with spaces)

**Verification:**
- [ ] `cargo test -p flintshot-core` (round-trip tests)
- [ ] Full gate: fmt, clippy `--workspace --all-targets -D warnings`,
      workspace tests, core doc tests

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/ipc.rs`
- `docs/ARCHITECTURE.md` (§IPC protocol variant table, one line each)

**Docs:** Variant doc comments carry the why (forwarding rule); no gotchas
expected.

**Estimated scope:** Small: 1 file + doc touch
