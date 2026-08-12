# Task 002: Add `[upload]` config section

**Description:** Add `UploadConfig` to `flintshot-core::config` with a
single field `default_backend: String` (default `"imgur"`). Wire it into
`Config` as `pub upload: UploadConfig`, `merge_into_existing`
(comment-preserving write for `[upload]`), and the re-exports. Mirror the
Phase 3 `PinConfig` pattern exactly — `#[non_exhaustive]`, `#[serde
(default)]`, kebab-case fields.

**Acceptance criteria:**
- [ ] `UploadConfig` struct exists with `default_backend` field
      (kebab-case in TOML)
- [ ] `Config` has an `upload: UploadConfig` field with `#[serde(default)]`
- [ ] `Config::load_from` parses a `[upload]` section; missing section
      yields `default_backend = "imgur"`
- [ ] `Config::save_to` persists `[upload]` preserving existing comments
      (mirrors PinConfig round-trip tests)
- [ ] `UploadConfig` re-exported from `lib.rs`
- [ ] All existing config tests pass

**Verification:**
- [ ] `cargo test -p flintshot-core`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/config.rs`
- `crates/flintshot-core/src/lib.rs`

**Estimated scope:** Small: 2 files
