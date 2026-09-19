# Task 001: Config fields, `resolve_save_dir`, and the `[output]` re-save merge

**Description:** The config foundation. Add `output.save-clipboard-copies`
(bool, default `true`) and `ui.recent-captures-limit` (u32, default `50`)
with a clamped accessor (1–200). Add `resolve_save_dir` (explicit path
passthrough; empty → `pictures_base/flintshot`, base injected for tests)
implementing the fallback documented three times but never coded. Fix
`merge_into_existing` to merge the `[output]` table (all fields) and the
`[ui]` keys it currently drops on re-save (`show-statusbar`,
`statusbar-size-mode`, `animations`) — without this, a Settings-driven
`save-dir` change would not survive the next save. Update
`config.default.toml` (asset-parity test forces this in the same commit).

**Acceptance criteria:**
- [ ] `resolve_save_dir_with("/tmp/shots", base)` → `/tmp/shots`; `("", Some("/home/u/Pictures"))` → `/home/u/Pictures/Flintshot`; whitespace-only string resolves like empty
- [ ] New fields default correctly from an old (field-less) TOML; round-trip through save/load
- [ ] Saving a config **into an existing file** now writes every `[output]` value and the three missing `[ui]` values (pre-existing values and their comments survive, per `set_value` semantics)
- [ ] `ui.recent_captures_limit_clamped()` clamps 0→1, 500→200, 50→50
- [ ] `config.default.toml` gains `save-clipboard-copies = true` and `recent-captures-limit = 50`; the asset-parity test passes

**Verification:**
- [ ] `cargo test -p flintshot-core` (new unit tests, RED first)
- [ ] `cargo fmt --all --check` + `cargo clippy --workspace --all-targets -- -D warnings`

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/config.rs`
- `crates/flintshot-core/src/config.default.toml`
- `crates/flintshot-core/src/lib.rs` (re-export `resolve_save_dir` pieces if public free fns)

**Docs:**
- None (schema docs land in Task 008)

**Estimated scope:** Medium
