# Task 002: Entry-state wiring — config → state, phase-end → Move

**Description:** Make the production entry points honor the new
knobs: `state_from_config` (cli.rs) copies `ui.flip_to_move` into
`EditorState` — one site covering both the editor window and the
overlay — and the editor window's selection-first phase flips the
tool from `Select` to `Move` when the phase ends on crop, so the
edit phase opens on Move (the capture landing still opens on
Select; that contract is untouched).

**Acceptance criteria:**

- [ ] `state_from_config` sets `state.flip_to_move =
      cfg.ui.flip_to_move` alongside the existing color / stroke /
      font mappings
- [ ] `end_phase_on_crop` (app.rs): when the phase latches to
      `Ended`, a still-`Select` tool becomes `Move`; a tool the
      user picked mid-phase (e.g. `Rect`) is left alone
- [ ] Overlay entry untouched: `OverlayApp::new` still forces
      `Tool::Select` (region-first); the overlay reaches Move via
      the commit flip / chip / `V`
- [ ] No new config loading in the shells for this flag (the
      magnifier-style `Config::load` in `EditorApp::new` stays
      magnifier-only)

**Verification:**

- [ ] `cargo test -p flintshot-app` — `state_from_config` maps
      both `true` and `false` configs (follow the existing cli.rs
      test patterns); app tests: phase-end tool transition, both
      arms
- [ ] Manual: `cargo run -- region` → drag region → Enter → edit
      phase opens on Move; edit config to `flip-to-move = false`,
      take a new capture → commits keep the drawing tool active

**Dependencies:** Task 001 (the field and enum default it wires)

**Files likely touched:**

- `crates/flintshot-app/src/cli.rs` (`state_from_config` + test)
- `crates/flintshot-editor-egui/src/app.rs`
  (`end_phase_on_crop` + tests)

**Docs:**

- None beyond code comments (per-session read rationale already
  lives in the spec).

**Estimated scope:** Small: 2 files
