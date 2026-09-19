# Task 006: Panel / statusline polish

**Description:** Final pass over the right panel and statusline copy now that
the tool semantics are final. Sections map, per-tool hints, panel title, and
rail glyph polish land here — pure presentational, no behavior.

**Acceptance criteria:**
- [ ] `sections_for`: `Tool::Select` → `[Selection]` (inspector when a set is
      live), `Tool::Hand` → `[Selection]` may be omitted (Hand never
      selects; if kept, it renders the empty state — prefer omitting),
      `Tool::Crop` → `[Crop]`; other tools unchanged
- [ ] `tool_hint` copy updated: Select = "click selects · drag marquees ·
      drag on selection moves · Del removes" (or better), Hand = "drag pans
      · wheel zooms", Crop keeps today's region hint
- [ ] Panel title shows the inspector ("N selected · Properties") under
      Select when a set is live
- [ ] Rail glyphs read distinctly at 28 px: Select (dashed marquee), Hand
      (hand idiom), Crop (corner brackets); unique-fn test still passes
- [ ] Statusbar / floating-cluster tooltips mention pan+zoom affordances if
      they reference tools

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui --all-targets` green (sections
      map + hint tests updated)
- [ ] Manual: switch through every tool; panel sections, hints, and glyphs
      all match the tool

**Dependencies:** Tasks 002, 003

**Files likely touched:**
- `crates/flintshot-editor-egui/src/panel.rs` (sections, hints, title, tests)
- `crates/flintshot-editor-egui/src/rail.rs` (glyph polish if needed)
- `crates/flintshot-editor-egui/src/statusbar.rs` (copy only, if anything)

**Docs:** None

**Estimated scope:** Small (2-3 files)
