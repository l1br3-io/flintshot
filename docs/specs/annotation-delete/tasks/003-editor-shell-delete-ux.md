# Task 003: Editor window — key translation, Esc layering, outline, hint

**Description:** The editor window's delete UX. `app.rs` translates
`egui::Key::Delete` / `egui::Key::Backspace` to core keys in
`dispatch_shortcut_key`; layers Esc (selection → deselect + consume,
no selection → today's Cancel); paints the selection outline; routes
its tool switches (rail chip click in `toolbar.rs` `draw_rail`, the
hotkey block, `end_phase_on_crop`'s flip) through core `set_tool`.
`chrome.rs` gains a pure `selected_chrome(state) -> Option<Rect>`
query (selected annotation's `bounding_box()`; `None` when not Move /
nothing selected / degenerate bbox) and `theme.rs` the dashed ember
`SELECTED_STROKE` (reusing `MOVE_DASH` / `MOVE_GAP` / pad rhythm).
Degenerate bbox at paint time deselects instead of rendering. The
Move-tool hint gains the Del affordance — exact copy ask-first (see
Docs).

**Acceptance criteria:**
- [ ] `egui::Key::Delete` → `Key::Delete` and `egui::Key::Backspace`
      → `Key::Backspace` forwarded in `dispatch_shortcut_key`; a
      synthetic-context test drives Del through the shell
      translation and deletes the selected annotation
      (`enter_key_crops_through_shell` pattern).
- [ ] Esc layering: with a selection, Esc clears it and the window
      stays open (outcome slot untouched); without, today's
      Cancel / publish-if-unset / close. Second Esc after a
      deselect cancels. Selection-first phase: Esc still cancels
      (no selection can exist mid-phase — pin with a test).
- [ ] `selected_chrome`: `Some(bbox)` for a selected annotation
      under Move; `None` for other tools / no selection / degenerate
      bbox. Painted as a dashed ember outline over the shared render
      output (with the other chrome), never in composed output
      (chrome-only by construction — it lives in the shell's paint
      pass, not `render()`).
- [ ] Degenerate selected bbox → `selected_index` cleared, nothing
      painted.
- [ ] Editor tool switches route through `set_tool` (rail click,
      hotkey, phase-end flip) — switching away from Move deselects
      (pin with a hotkey test).
- [ ] Move hint mentions Del (exact string approved by the user);
      `every_tool_has_a_nonempty_hint` stays green.
- [ ] Del during the selection-first phase is inert (nothing
      selectable; no phase-behaviour change) — pin with a test.

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui --quiet`
- [ ] `cargo clippy -p flintshot-editor-egui --all-targets -- -D warnings`
- [ ] Manual (needs display): `cargo run -- fullscreen` — draw,
      click with Move (outline appears + stays), Del removes, Ctrl+Z
      restores, Esc/Esc cancels; Save/Copy output has no outline.

**Dependencies:** Tasks 001, 002

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs`
- `crates/flintshot-editor-egui/src/chrome.rs`
- `crates/flintshot-editor-egui/src/theme.rs`
- `crates/flintshot-editor-egui/src/toolbar.rs` (hint string +
  `set_tool` at the chip-click site only)

**Docs:**
- **Ask first:** the Move-hint copy (spec boundary). Proposed
  default: "press an annotation and drag to reposition · Del removes".

**Estimated scope:** Medium (4 files, cohesive editor-window slice)
