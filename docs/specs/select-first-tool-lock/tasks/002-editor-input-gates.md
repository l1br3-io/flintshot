# Task 002: Editor input gates — phase-scoped hotkey + pointer

**Description:** Close the behavioral holes the visible lock
(task 001) doesn't cover: while `phase_active()`, the single-key
tool switch (the `tool_for_key` block in `EditorApp::logic`)
ignores every mapping except `Select`, and pointer dispatch skips
annotation input entirely (`phase_active() && state.tool !=
Tool::Select → no dispatch`). This makes the phase
annotation-proof by click, hotkey, and drag. Also revise the now-
dead clause in `end_phase_on_crop`'s doc comment ("the tool the
user picked mid-phase is their explicit choice" — mid-phase
picking is no longer possible).

**Acceptance criteria:**
- [ ] Phase live, pressing a non-Select tool key (e.g. `P`) leaves
      `state.tool == Select`; post-crop the same key arms Pencil.
- [ ] `S` keeps arming Select during the phase (harmless — already
      armed).
- [ ] Phase live, a synthetic annotation-tool pointer press on the
      canvas commits nothing (no annotation in state, no undo
      snapshot); Select drags still draw/update the selection; Esc
      / Enter / outputs keep today's phase behaviour (existing
      phase tests green, untouched).
- [ ] After crop, pointer input flows identically to a `fullscreen`
      session.
- [ ] `end_phase_on_crop` doc comment no longer describes mid-phase
      tool switching.

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui --quiet` (new tests fail
      without the gates — mirror the synthetic-`egui::Context`
      pattern of `enter_key_crops_through_shell`)
- [ ] `cargo clippy -p flintshot-editor-egui --all-targets -- -D warnings`
- [ ] Manual (needs display): `cargo run -- region` — pre-crop,
      press `P` / click Pen chip / drag on canvas: no drawing, no
      tool change. Enter → everything live.

**Dependencies:** Task 001 (the rail lock and this task together
deliver exit criterion 1; the hotkey gate can be tested without
task 001 but the phase contract is only whole with both).

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs`

**Docs:**
- Gotcha if any egui input-ordering trap surfaces (e.g. the
  toolbar-press guard at `app.rs` ~561) → `// GOTCHA` comment at
  the site; ADR deferred to task 004.

**Estimated scope:** Small (1 file)
