# Task 003: Rail emphasis — Move first + labeled chip, both rails

**Description:** Reorder the shared `TOOLS` table so Move is the
first chip (Select second), and render Move as a labeled chip
(glyph + "Move" text) following the Copy-primary's `copy_chip`
glyph+label pattern — in the editor window rail (`draw_rail`) and
the overlay's floating rail (`overlay_rail_row`). Selected-state
visuals unchanged (ember); the label is the emphasis, not new
color.

**Acceptance criteria:**

- [ ] `TOOLS[0]` is `Tool::Move`; `TOOLS[1]` is `Tool::Select`;
      chip count stays 11 with unique keys (existing test stays
      green, plus an order assertion)
- [ ] A `pub(crate)` labeled-chip helper (glyph + text, 28 px
      tall, galley-measured width) renders the Move chip in both
      rails; ids stay key-derived (`("tool", "V")` /
      `("overlay_tool", "V")`) so tooltips and click behavior are
      unchanged
- [ ] The label reads "Move" — no emoji, painter glyphs only
      (ADR-0027 language)
- [ ] Overlay rail: labeled chip participates in the same
      measured-size flow (`toolbar_size` drives
      `toolbar_anchor`), no layout regression when the selection
      is narrow (clamping still applies)
- [ ] Narrow-capture fallback (horizontal scroll on the editor
      rail) unaffected

**Verification:**

- [ ] `cargo test -p flintshot-editor-egui` — order + coverage
      tests
- [ ] Manual: `cargo run -- region` → both rails (editor window
      and overlay) show "Move" labeled first; hover tooltip still
      `Move · V`; selection ember reads correctly; narrow region
      capture keeps all controls reachable

**Dependencies:** Task 001 (rail ordering makes sense once Move is
the default/landing tool; no code dependency beyond the enum)

**Files likely touched:**

- `crates/flintshot-editor-egui/src/toolbar.rs` (TOOLS order,
  labeled-chip helper, rail usage, tests)
- `crates/flintshot-editor-egui/src/overlay.rs`
  (`overlay_rail_row` uses the labeled chip for Move)

**Docs:**

- None (visual language follows ADR-0027 and the Copy-primary
  precedent).

**Estimated scope:** Small: 2 files
