# Task 007: Delete `PinLast`/`Pinned`/`PinConfig` from `flintshot-core`

**Description:** Remove the pin types from core now that every
reference is gone: `Request::PinLast` + `Event::Pinned` from
`ipc.rs` (with roundtrip/label test entries), `PinConfig` + the
`Config.pin` field from `config.rs` (with defaults/merge/serialization
tests), and the `PinConfig` re-export from `lib.rs`. This re-indexes
the postcard wire format — accepted per spec.

**Acceptance criteria:**
- [ ] `Request::PinLast` and `Event::Pinned { id }` variants deleted;
      remaining-variant roundtrip coverage kept (edit the existing
      roundtrip test, don't delete it — spec § Testing strategy)
- [ ] `PinConfig` struct + `Default` impl + `Config.pin` field
      deleted; config merge/serialization tests updated
- [ ] `PinConfig` re-export gone from `lib.rs`
- [ ] A user config containing `[pin]` still loads (serde ignores
      unknown sections — verify with a test if one exists for unknown
      sections, otherwise leave as documented behavior)
- [ ] `flintshot-core` doc comments (e.g. `ipc.rs` "last saved"
      wording) no longer reference pin

**Verification:**
- [ ] `cargo test --workspace --all-targets` green
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
      (cosmic included — proves no stale references)
- [ ] `rg -n "PinLast|Event::Pinned|PinConfig|pin_ids" crates/` empty

**Dependencies:** Tasks 002, 003, 004, 005, 006

**Files likely touched:**
- `crates/flintshot-core/src/ipc.rs`
- `crates/flintshot-core/src/config.rs`
- `crates/flintshot-core/src/lib.rs`

**Docs:**
- None expected.

**Estimated scope:** Medium (3 files, test-heavy)
