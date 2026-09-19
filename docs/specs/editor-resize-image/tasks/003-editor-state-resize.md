# Task 003: `EditorState::resize` — state transition + undo/redo

**Description:** Add `pub fn resize(&mut self, target_w: u32)` to
`EditorState` in `crates/flintshot-core/src/editor.rs`, mirroring
`crop`'s exact sequence (editor.rs:976-998): guard → snapshot →
mutate → clear transient state. The image resizes via
`image_ops::resize` (Task 001) with a height derived from the aspect
ratio; every annotation scales via `Annotation::scale` (Task 002) by
the exact applied factor.

**Behaviour contract:**

- `target_w` clamps to `[1, width]`; at current width it's a no-op
  (no snapshot pushed)
- Derived height: `(h * new_w / w).round().max(1)`
- Applied factor: computed from the *actual* clamped target
  (`new_w / w`), not the requested one
- Sequence (crop's): `restore_active_reedit` → `Snapshot::full(anns,
  image, next_number)` → swap image → `anns.iter_mut().scale(f)` →
  `pending = None` → `text_anchor = None` → `end_move()` → push
  undo → `redo.clear()` → `last_edit_kind = None`

**Acceptance criteria:**

- [ ] 100×80 state with a Rect (stroke 4) + Text (font 16): resize
      to 50 ⇒ image 50×40; stroke 2; font 8; positions halved
- [ ] Undo restores the 100×80 image and annotation values
      bit-for-bit; redo replays the resize
- [ ] `target_w ≥ width` is a no-op — no snapshot, no mutation
- [ ] `target_w = 0` clamps to 1 (1-px-wide image, anns scaled by
      `1/w`)
- [ ] In-flight pending annotation / move / text session cleared
      (same guarantees as crop)
- [ ] Selection survives geometrically (indices unchanged — resize
      drops nothing) but any move drag ends

**Verification:**

- [ ] `cargo test -p flintshot-core editor` (unit tests for resize +
      undo/redo roundtrip)
- [ ] All four gate commands pass (this closes the Phase-1
      checkpoint)
- [ ] Manual sanity via a test: two successive resizes (50% then
      50% again) + undo ×2 returns the original

**Dependencies:** Tasks 001, 002

**Files likely touched:**

- `crates/flintshot-core/src/editor.rs`

**Docs:**

- Note in the method doc: interaction with app.rs's selection-phase
  latch (one-way, keyed on image-carrying snapshots) is safe —
  resize mid-phase behaves like crop mid-phase.

**Estimated scope:** S — 1 file
