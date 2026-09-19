# Task 002: Crop tool cutover

**Description:** Introduce `Tool::Crop` and move the entire region-selection
machinery from Select to it in one working slice: the `EditorState.selection`
field renames to `crop_region`, `confirm_selection` → `confirm_crop`, the
dispatcher's region branch / Enter path key on Crop, Copy-with-region
follows, the rail gains the Crop chip with final hotkeys (V=Select, H=Hand,
C=Crop), `ToolGate::SelectOnly` → `CropOnly`, and the overlay +
selection-first phase pre-arm Crop instead of Select. After this task Select
is dispatcher-dead (no branch) — Task 003 gives it semantics.

**Acceptance criteria:**
- [ ] `Tool::Crop` variant exists; dispatcher: PointerDown/Drag/Up under Crop
      drive `crop_region` exactly as Select did before; Enter under Crop with
      an active region crops; `begin_annotation` no-ops for Crop
- [ ] `EditorState.selection` field renamed `crop_region`; no public API
      references the old name (`grep -rn "\.selection\b" crates/flintshot-core crates/flintshot-editor-egui` clean of EditorState uses)
- [ ] Copy with an active Crop region copies just the region
      (`compose_final_region` path), full canvas otherwise
- [ ] Rail: selection pod = Select · Hand · Crop with keys V/H/C; `S` no
      longer switches tools anywhere (editor + overlay key maps); Crop gets a
      corner-bracket glyph, Hand a hand glyph (may be simple strokes)
- [ ] `ToolGate::CropOnly` gates the selection-first phase; lock clause
      unchanged; overlay opens with Crop pre-armed and its gesture system
      (`gesture_for_press`, `annotation_press_allowed`, hotkey map) keys on
      Crop/Hand
- [ ] The old Select region test set (editor.rs, overlay.rs, app.rs) is
      renamed to Crop and passes unchanged in behavior

**Verification:**
- [ ] `cargo test --workspace --all-targets` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] Manual: `cargo run -- capture` overlay → drag region → Enter crops;
      `cargo run -- app` → editor → Crop tool drag + Enter crops; Copy uses
      the region

**Dependencies:** Task 001

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs` (variant, field rename, dispatcher, tests)
- `crates/flintshot-core/src/render.rs` (region stroke doc + `compose_final_region` doc refs)
- `crates/flintshot-editor-egui/src/rail.rs` (pods, keys, glyphs, gate, tests)
- `crates/flintshot-editor-egui/src/app.rs` (pre-arm, gates, copy path, key map, tests)
- `crates/flintshot-editor-egui/src/overlay.rs` (pre-arm, gestures, allowlist, keys, tests)
- `crates/flintshot-editor-egui/src/panel.rs` (`sections_for`: Crop gets the Crop bay)

**Docs:** GOTCHA comment at `crop_region`: "formerly `selection`; not the
annotation selection set (`EditorState::selected`)".

**Estimated scope:** Large (6 files, mostly mechanical renames + branch flips)
