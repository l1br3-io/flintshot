# Task 005: egui: plural selection chrome in both shells

**Description:** The persistent-selection outline renders per
selected annotation (same dashed forest language), the grab outline
covers every member during a live multi-drag, and both shells'
deselect paths (Esc, miss, tool switch) operate on the set. Hint
copy teaches Shift+click. Depends only on Tasks 1–2 (parallelizable
with 3–4).

**Acceptance criteria:**
- [ ] `chrome.rs` selection query returns one rect per selected
      annotation (stale-index-safe); both shells paint them all
- [ ] During a multi-drag, every dragged member renders the solid
      grab outline
- [ ] Esc / miss / tool-switch deselect clears the whole set in
      both editor window and overlay
- [ ] Move-tool hint copy mentions Shift+click; no overlay
      region-selection behavior changes (full overlay suite green)
- [ ] Chrome unit tests pluralized; existing hover/grab tests pass

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui`
- [ ] `cargo test --workspace --all-targets` + clippy + fmt gates
- [ ] Manual: outlines on each member in both shells; drag group

**Dependencies:** 001, 002

**Files likely touched:**
- `crates/flintshot-editor-egui/src/chrome.rs`
- `crates/flintshot-editor-egui/src/app.rs`
- `crates/flintshot-editor-egui/src/overlay.rs`
- `crates/flintshot-editor-egui/src/panel.rs` (hint copy only)

**Docs:**
- None beyond code comments.

**Estimated scope:** Medium
