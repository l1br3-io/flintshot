# Task 001: Rework upload trait types + add keyring to core

**Description:** Swap the Phase-0 placeholder types (`CredentialHandle`,
`ProgressSink`) for real types. `UploadContext.credentials` becomes
`Option<&'a keyring::Entry>` (direct reference, not a trait object —
Decisions §1). The `progress` field and `ProgressSink` type are removed
entirely (Decisions §2 — YAGNI). `keyring` is added as a
`flintshot-core` dep because `UploadContext` references it as a
parameter type. The `no_gui_types_in_core.rs` boundary test stays green
(`keyring` is a secrets handle, not a GUI toolkit — it's not in
`FORBIDDEN_PREFIXES`).

**Acceptance criteria:**
- [ ] `CredentialHandle` and `ProgressSink` types are removed from
      `upload.rs`
- [ ] `UploadContext` has two fields: `credentials: Option<&'a
      keyring::Entry>` and `filename: Option<&'a str>`
- [ ] `keyring` is added to `[workspace.dependencies]` (version ≥ 3) and
      to `flintshot-core`'s `[dependencies]`
- [ ] `lib.rs` re-exports updated (no re-export of removed types)
- [ ] Doc comments on `UploadContext` updated to reflect the real types
- [ ] `no_gui_types_in_core.rs` passes unchanged
- [ ] All existing tests pass

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo test --doc -p flintshot-core`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`
- [ ] Manual check: `no_gui_types_in_core.rs` still green (no new
      forbidden imports)

**Dependencies:** None

**Files likely touched:**
- `Cargo.toml` (workspace deps)
- `crates/flintshot-core/Cargo.toml`
- `crates/flintshot-core/src/upload.rs`
- `crates/flintshot-core/src/lib.rs`

**Estimated scope:** Small: 4 files
