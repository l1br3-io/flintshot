# Task 001: Default config asset + template writer

**Description:** Add the documented default config as a checked-in
TOML asset in `flintshot-core`, expose it via
`default_config_toml()`, and add `ensure_user_config()` — the
template writer that materializes the user's `config.toml` (header
comment + asset body) only when the file is missing. Pure
foundation; nothing consumes these yet.

**Acceptance criteria:**
- [ ] `crates/flintshot-core/src/config.default.toml` exists, is
  fully commented (every section + field documented), and parses as
  valid TOML
- [ ] `pub fn default_config_toml() -> &'static str` returns the
  embedded asset (`include_str!`), with a doc comment
- [ ] The asset deserializes to a `Config` equal **field-by-field**
  to `Config::default()` (unit test; the types lack `PartialEq`, so
  compare each field / sub-field, or serialize-and-compare the
  resolved TOML — pick one, pin it)
- [ ] `pub fn ensure_user_config_at(path: &Path) -> Result<bool>`
  testable seam: missing file → creates it (parent dirs included)
  with a leading header comment naming it as flintshot's config,
  returns `true`; existing file → no-op returning `false` (user
  edits are never clobbered — pin by writing a sentinel file and
  asserting byte-identical after the call)
- [ ] `pub fn ensure_user_config() -> Result<Option<PathBuf>>`
  platform wrapper over [`config_path`] (`None` when no platform
  config dir; doc-commented `# Errors`)
- [ ] Core stays GUI-type-free (`tests/no_gui_types_in_core.rs`
  still passes)

**Verification:**
- [ ] `cargo test -p flintshot-core` green (new tests included)
- [ ] `cargo build` green

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/config.rs`
- `crates/flintshot-core/src/config.default.toml` (NEW)
- `crates/flintshot-core/src/lib.rs` (re-export, if the convention
  demands it — check how `config_path` is exported)

**Docs:**
- The asset *is* the documentation; keep comments user-facing
  (VSCode default-settings tone).

**Estimated scope:** Small: 2 files
