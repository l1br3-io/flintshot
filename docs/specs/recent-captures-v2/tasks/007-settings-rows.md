# Task 007: Settings — save-location picker, clipboard toggle, limit row

**Description:** The Settings Output & saving section grows up. The
readonly "Save location" row becomes a live row: the effective path
display (resolved dir when `save-dir` is empty — the UI never shows a
path the save path won't honor), a **Browse** button
(`rfd::FileDialog::pick_folder()`, the `pick_image_path` blocking-modal
precedent; the picked folder lands in the draft through a pure
`apply_picked_folder` seam so tests skip the dialog), and a **Use
default** reset (clears to empty). New rows: "Save clipboard copies"
switch (`output.save-clipboard-copies`) and "Recent captures" limit
control (`ui.recent-captures-limit`, clamped 1–200). All edits flow the
existing mark-dirty → debounce → save path.

**Acceptance criteria:**
- [ ] The row renders the resolved default dir when `save-dir` is empty, the configured path otherwise (frame test with injected config)
- [ ] `apply_picked_folder(Some(path))` writes the draft + config field and arms the debounce; `None` (cancel) changes nothing; "Use default" empties the field
- [ ] The clipboard-copies switch flips the config field; the limit control writes clamped values (0→1, 500→200 via the pure seam)
- [ ] A save round-trip through an existing config.toml persists all three (Task 001's merge)
- [ ] Existing settings tests stay green

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` (RED first)
- [ ] fmt + clippy gates; workspace tests; manual `cargo run -- app` pass deferred to Task 008's checklist

**Dependencies:** Task 001 (fields, merge, resolution)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/settings.rs`

**Docs:**
- None

**Estimated scope:** Medium
