# Task 001: Core: selection set + Shift routing + delete-all

**Description:** Replace the single-selection model with an ordered
set, teach the dispatcher Shift-aware press routing, and make
`delete_selected` remove every selected annotation. The input model
change (`PointerDown` carries `Modifiers`) lands here because
routing cannot exist without it. All `selected_index` consumers in
both shells are adapted mechanically in the same task so the
workspace stays green.

**Acceptance criteria:**
- [ ] `InputEvent::PointerDown` carries `modifiers: Modifiers`; every
      construction site (app.rs, overlay.rs, and all tests) updated
- [ ] `EditorState::selected: Vec<usize>` (sorted, deduped —
      documented invariant) replaces `selected_index`; no
      `selected_index` remains
- [ ] Move-tool press routing: hit + Shift toggles membership; hit
      without Shift replaces the set; miss clears; Shift+click on a
      selected member deselects it and starts no drag
- [ ] Pointer release keeps the set; `set_tool` away from Move,
      `undo`, `redo`, `crop` clear it (unchanged semantics)
- [ ] `delete_selected` removes all selected annotations in one
      pass with one `anns`-only snapshot; one `undo` restores all
- [ ] Core unit tests cover every routing arm and the delete/undo
      round-trip; existing editor tests adapted and passing

**Verification:**
- [ ] `cargo test -p flintshot-core`
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo fmt --all --check` && `cargo clippy --workspace --all-targets -- -D warnings`

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/input.rs`
- `crates/flintshot-core/src/editor.rs`
- `crates/flintshot-editor-egui/src/{app,overlay,chrome,panel}.rs` (mechanical)

**Docs:**
- Invariant comment on `selected` in editor.rs (sorted/dedup contract).

**Estimated scope:** Medium
