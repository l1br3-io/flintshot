# Task 002: egui — selection chrome under Text

**Description:** Make the persistent-selection outline visible for
Text-tool selections. `chrome::selected_chrome` currently gates on
`Tool::Move` only; widen to `Tool::Move | Tool::Text`. Both shells
paint through this shared query, and both delete affordances (panel
`Section::Selection` — already in Text's section map — and the
overlay's delete chip, gated tool-agnostically on
`selected_index.is_some()`) light up for free once Task 001 lands.
Check the shells' paint call sites for any duplicated
`tool == Move` gate that would suppress the outline.

**Acceptance criteria:**

- [ ] `selected_chrome` returns the selected annotation's bbox when
      `tool == Tool::Text` and a selection exists; `None` for every
      other tool; Move behavior unchanged.
- [ ] Dashed selection outline renders in both shells (editor
      window + overlay) for a Text-tool selection; never composed
      into Save / Copy / Upload output.

**Verification:**

- [ ] `cargo test -p flintshot-editor-egui` (chrome gating tests)
- [ ] Manual: `cargo run -- capture` → place text → Enter → click
      it with Text tool → dashed outline; Del deletes; Esc
      deselects, editor stays open — both editor window and overlay
      flows.

**Dependencies:** Task 001

**Files likely touched:**

- `crates/flintshot-editor-egui/src/chrome.rs` (gate + tests)
- `crates/flintshot-editor-egui/src/{app,overlay}.rs` (only if a
  paint call site duplicates the Move gate)

**Docs:**

- None.

**Estimated scope:** Small (1–2 files)
