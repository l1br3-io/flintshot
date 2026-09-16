# Task 007: Contextual tool options + picker popover

**Description:** Gradia patterns #3/#4 land. The panel's options
area shows **only the selected tool's options** (Rect/Ellipse:
stroke+fill; Line/Arrow/Pencil/Marker: stroke; Text: style + size;
Number: start value; Blur: strength; Move: hint line). Color and
stroke-width pickers collapse behind a single popover pod button
(`popover.rs` — egui popup with cutout card styling containing the
swatch row, custom color chip, stroke presets, fill toggle). The
existing bay code (`draw_bay`, `text_style_popup`, swatch/stroke
chips) migrates into these two modules; `toolbar.rs` is deleted at
the end — glyphs/helpers move to `panel.rs`/`floating.rs`.

**Acceptance criteria:**
- [ ] Selecting a tool renders exactly that tool's options; nothing
      else (manual check across all ten tools)
- [ ] Color/stroke/fill open in one popover from one pod button;
      popover commits apply live to the draft annotation
- [ ] `toolbar.rs` no longer exists; no dead code, lib.rs docs
      updated
- [ ] Text-style controls (size, font-size spinner) reachable in
      Text's contextual section

**Verification:**
- [ ] Full suite + clippy green (incl. any migrated bay tests)
- [ ] Manual: cycle all ten tools; each shows only its own options;
      popover pick applies on next annotation

**Dependencies:** Task 005

**Files likely touched:**
- `crates/flintshot-editor-egui/src/popover.rs` (new)
- `crates/flintshot-editor-egui/src/panel.rs`
- `crates/flintshot-editor-egui/src/toolbar.rs` (deleted)
- `crates/flintshot-editor-egui/src/{app,lib}.rs`

**Docs:**
- GOTCHA if `text_style_popup`'s open-state management needs
  rehoming (egui `Id` stability across popup parents).

**Estimated scope:** Medium-Large (4–5 files, mostly migration)
