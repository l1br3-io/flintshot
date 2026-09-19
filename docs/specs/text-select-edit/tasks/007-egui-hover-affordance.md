# Task 007: egui — hover affordance under Text

**Description:** With `Tool::Text` active, hovering a committed
Text annotation shows the dashed hover outline (same visual rhythm
as the Move tool's) and a text cursor icon, announcing
selectability. Add a `text_hover_chrome`-style query to `chrome.rs`
(Text-tool gated, Text-annotation hits only — non-text hits are
misses under Text by Task 001's rule) and wire paint + cursor in
both shells, mirroring the `move_chrome` pattern.

**Acceptance criteria:**

- [x] Hovering a committed text under Text tool: dashed outline +
      text cursor icon, both shells. Hover on empty canvas /
      non-text annotation: no affordance.
- [x] Move-tool hover/grab chrome behavior unchanged.

**Verification:**

- [x] `cargo test -p flintshot-editor-egui` (chrome query tests)
- [ ] Manual: hover behaviours in both shells.

**Dependencies:** Task 002 (chrome pattern), Task 004 (feature
complete enough to feel)

**Files likely touched:**

- `crates/flintshot-editor-egui/src/chrome.rs`
- `crates/flintshot-editor-egui/src/app.rs` (cursor + paint)
- `crates/flintshot-editor-egui/src/overlay.rs` (cursor + paint)

**Docs:**

- None.

**Estimated scope:** Small (3 files, tiny diffs)
