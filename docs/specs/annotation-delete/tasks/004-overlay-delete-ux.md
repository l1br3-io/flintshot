# Task 004: Overlay — Del passthrough, Esc layering, outline, `set_tool` routing

**Description:** The X11 overlay's delete UX — core semantics are
inherited (Move presses already flow to the dispatcher;
`annotation_press_allowed` admits Move unconditionally), so this task
is routing + chrome: forward `egui::Key::Delete` / `Backspace` to the
dispatcher in `logic_impl`'s key block, layer Esc at the
`close_from_input` call site (selection → deselect instead of close),
paint the selection outline in `ui_impl` (the shared `chrome.rs`
query + theme stroke, with the degenerate-bbox deselect), and route
the overlay's tool switches (`overlay_rail_row` chip clicks, the
hotkey block) through core `set_tool`.

**Acceptance criteria:**
- [ ] `Del` / `Backspace` (no modifiers) reach the dispatcher: a
      synthetic-context test with a selected annotation deletes it
      through the overlay's key translation; modifiers spoil it.
- [ ] Esc layering: with a selection, Esc clears it and the overlay
      stays open (outcome slot untouched, no close); without,
      today's Cancel. `close_from_input` itself is unchanged (its
      contract stays pure; the layering lives at the call site).
- [ ] Selection outline paints over the shared render output in the
      overlay's chrome order (with `move_chrome`); degenerate bbox
      deselects instead of rendering.
- [ ] Overlay tool switches route through `set_tool` — switching
      away from Move drops the selection (pin with a test).
- [ ] Text drafting in the overlay: Backspace edits the draft (the
      modal branch runs before the new key block — pin with a test
      if not already covered by the editor-side pin).

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui --quiet`
- [ ] `cargo clippy -p flintshot-editor-egui --all-targets -- -D warnings`
- [ ] Manual (needs display): `cargo run -- region` — post-crop,
      click with Move (outline), Del removes, Ctrl+Z restores, rail
      Delete chip works, Esc/Esc cancels.

**Dependencies:** Tasks 001, 002, 003 (chrome query + theme stroke)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/overlay.rs`

**Docs:**
- None — CHANGELOG at task 005.

**Estimated scope:** Medium (1 file, routing + chrome + tests)
