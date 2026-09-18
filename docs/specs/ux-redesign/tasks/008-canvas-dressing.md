# Task 008: Canvas paper dressing + floating-chip restyle

**Description:** Canvas presentation per the mockup: the capture as a
paper cutout — white ring + forest hairline + paper shadow painted
around the image rect — plus four white corner handles (11px, chrome
only). `floating.rs` chips restyle to cream cutouts (white inner
outline, pop hover) and rearrange: undo/redo top-left, magnifier
bottom-right; the loupe-zoom chips keep their behavior. No Fit chip
(no canvas zoom exists — dropped, not stubbed). All chrome-only:
never in `render()`/composed output.

**Acceptance criteria:**
- [ ] Paper dressing + handles painted in the chrome pass after
      annotation chrome, under the magnifier; track fit transforms;
      hidden during the selection-first phase.
- [ ] Composed output unchanged (chrome-only seam test).
- [ ] Chips: cutout styling via tokens; positions per mockup;
      FloatingAction set unchanged in behavior (loupe zoom intact).
- [ ] Handle-rect + chip-anchor math unit-tested from a canvas rect.

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: paper reads as a cutout; handles show; save/copy output
      has no baked chrome

**Dependencies:** Task 4 (canvas rect composition settled)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs`
- `crates/flintshot-editor-egui/src/floating.rs`

**Estimated scope:** Small (2 files)
