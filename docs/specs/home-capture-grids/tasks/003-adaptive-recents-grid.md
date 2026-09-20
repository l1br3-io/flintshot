# Task 003: adaptive recents grid

**Description:** Replace the fixed 4-per-row / 112×148 recents layout
with an adaptive grid: a pure `columns_for_width(available, min_card,
gap) -> usize` (floor-fill, clamped 1–8), cards stretch to fill their
column (`card_w = (available − (cols−1)·gap) / cols`), and the card's
thumbnail box grows with the card (square-ish thumb area of
`card_w − 2·inset`, captions keep their 36 px band). Session keeps its
current fixed layout until Task 6. Grid math is pure and unit-tested;
row chunking uses the computed count.

**Acceptance criteria:**
- [ ] `columns_for_width` unit-tested at 700 / 1024 / 1600 px and at
      both clamp ends (tiny and huge widths)
- [ ] A 1600 px render lays out more columns than an 800 px render
      (`read_response` rect comparison in the existing
      `welcome_frame_sized` harness)
- [ ] Cards in one row share equal widths spanning the available width
- [ ] Hover chips stay inside the card at every width; chip geometry
      scales its row from the card rect (mirror helper updated)
- [ ] Existing overflow test reworked to the adaptive truth: section
      still scrolls, hero still pinned (updated, still passing)
- [ ] Card click, Copy/Reveal/Delete two-step confirm tests pass
      unchanged in behavior

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: `cargo run -- app`, resize ~700 → ~1800 px, confirm
      reflow without dead margins

**Dependencies:** None (parallel-safe with 001/002)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs`

**Docs:** Gotcha (chips hit-test topmost → `contains_pointer` gate
already handles it; keep the gate when resizing) → inline comment if
touched.

**Estimated scope:** Medium — 1 file, wide test churn
