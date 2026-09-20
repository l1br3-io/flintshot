# Task 002: Core `output` save helpers + recents limit/remove plumbing

**Description:** Move the save-path machinery editor-egui owns into a new
`flintshot-core::output` module so both the editor Copy path and the
(daemon-spawned, possibly egui-less) capture path share it: the UTC
date/time renderers + `civil_from_days`, the `-2`/`-3` collision probe
(`next_free_path`), a `save_image` writer (creates the parent dir,
encodes PNG or JPEG-at-quality), and a `clipboard_copy_path` composer
(template if set, else `flintshot-{date}-{time}`, resolved save dir,
collision-suffixed). Refactor editor-egui's `save_direct` onto it
(behavior unchanged, existing tests stay green). Parametrize
`Recents::record` by `limit: usize` (replace `MAX_RECENTS` with
`DEFAULT_RECENTS_LIMIT = 50`); load never truncates. Add
`Recents::remove(path)` (+ free `remove_from`) for the card Delete.
Update the four `recents::record` call sites (app_window ×3, cli ×1) to
pass the clamped config limit.

**Acceptance criteria:**
- [ ] `output::save_image` creates a missing parent dir, writes png/jpeg per format, and collision-suffixes `-2`, `-3` (tempdir tests)
- [ ] `clipboard_copy_path` uses the configured template when set, the built-in `flintshot-{date}-{time}` stem otherwise, and `resolve_save_dir` for the directory
- [ ] `Recents::record(path, 3)` caps at 3; a legacy 20-entry index loads with all 20 and grows to 50 on the next record; an old-format TOML fixture (path + mtime only) loads with zero migration
- [ ] `Recents::remove` drops exactly the given entry, keeps order of the rest; removing an absent path is a no-op
- [ ] Editor-egui's existing direct-save tests pass unchanged on the refactored `save_direct`

**Verification:**
- [ ] `cargo test --workspace --all-targets` (RED first on new core tests)
- [ ] fmt + clippy gates

**Dependencies:** Task 001 (config fields, `resolve_save_dir`, clamp accessor)

**Files likely touched:**
- `crates/flintshot-core/src/output.rs` (new)
- `crates/flintshot-core/src/recents.rs`
- `crates/flintshot-core/src/lib.rs`
- `crates/flintshot-editor-egui/src/app.rs` (refactor `save_direct` onto core helpers)
- `crates/flintshot-app/src/app_window.rs`, `crates/flintshot-app/src/cli.rs` (call-site limit args)

**Docs:**
- None

**Estimated scope:** Medium
