# Task 003: Select becomes the selection tool

**Description:** Give `Tool::Select` its new object-selection semantics in
core: click selects, Shift/Ctrl+click toggles, drag on empty space draws a
marquee that bbox-intersection-selects, drag on a selected annotation moves
the set. Default tool becomes Select. Reuses the existing
`route_move_press` / `update_move` machinery. Also flips the
selection-first phase's post-crop latch to Select.

**Acceptance criteria:**
- [ ] New `marquee: Option<Rect>` state (in-flight only); PointerDown miss
      starts a marquee (plain miss clears the set first; Shift/Ctrl miss
      keeps it); degenerate marquee on release (< 1 px) selects nothing
- [ ] Marquee release replaces the set (plain) or merges into it
      (Shift/Ctrl) with every annotation whose `bounding_box()` intersects
      the normalized marquee; set stays sorted+deduped
- [ ] Press on an annotation routes exactly like the old Hand tool:
      plain hit on non-member → select-only + grab; plain hit on member →
      grab (group drag); Shift **or Ctrl** hit → toggle membership (toggle-in
      grabs, toggle-out doesn't)
- [ ] PointerDrag: live `moving` drag translates the set, else the marquee
      extends; PointerUp ends the drag keeping the selection (persistent)
- [ ] `set_tool` selection clearing treats `Select | Text` as the owning
      pair (Hand is not an owner — pan-only tool, switching to it from
      Select clears, matching today's Move behavior); `crop`/`undo`/`redo`/
      `delete_selected` clear as before
- [ ] `Tool::default()` is `Select`; `EditorState::new` opens on Select
- [ ] Live marquee renders like the old region stroke (dashed/`SELECTION_STROKE`
      family) in `render()`; `selected_chrome` + `move_chrome` hover
      affordance cover Select (the `Move | Text` match becomes
      `Select | Text`)
- [ ] `end_phase_on_crop` flips a still-Crop tool to `Tool::Select`
      (Task 002 leaves it flipping to Hand; this lands once Select has
      semantics)
- [ ] Del deletes the set under Select; the inspector reflects it (existing
      tool-agnostic paths)

**Verification:**
- [ ] `cargo test -p flintshot-core -p flintshot-editor-egui --all-targets` green
- [ ] New core tests: marquee replace vs merge, degenerate marquee,
      toggle-in/toggle-out grab, group drag under Select, default tool,
      `set_tool` clearing across the owning pair
- [ ] Manual: `cargo run -- app` — draw 2 rects, marquee both, drag one,
      both move; Del removes both; Ctrl+click toggles; selection-first
      phase ends on crop with Select armed

**Dependencies:** Task 002

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs` (marquee state, routing, default, tests)
- `crates/flintshot-core/src/render.rs` (marquee stroke)
- `crates/flintshot-editor-egui/src/chrome.rs` (owning-tools match + tests)
- `crates/flintshot-editor-egui/src/app.rs` (latch flip target + tests)

**Docs:** GOTCHA at `marquee`: transient by design — release resolves it into
`selected` and clears it; never part of saves/history.

**Estimated scope:** Medium (4 files, the semantic heart of the effort)
