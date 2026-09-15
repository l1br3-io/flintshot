# Task 004: Stable editor viewport

**Description:** The editor page stops sizing its window to the
image. `run_editor_page`'s viewport builder uses a fixed default
logical size (~1024×640, OQ-2) clamped to the monitor's work area;
the canvas area fits the image per Task 003's transform (fit-scale
computed per frame from the live canvas rect). Crop no longer
issues `ViewportCommand::InnerSize` (`handle_image_resize` refits
the scale instead — a smaller image simply re-fits inside the
unchanged window; the texture-cache invalidation stays). The worker
path (`run_editor_blocking`) keeps its image-sized window — out of
scope per the spec.

**Acceptance criteria:**
- [ ] Editor page opens at the fixed default size for every image,
      clamped to the work area (pure clamp helper + unit tests)
- [ ] Image is centered and fitted (never upscaled) inside the
      canvas panel; resizing the window refits live (fit recomputed
      per frame from the canvas rect)
- [ ] Crop keeps the window size; the image refits (no viewport
      command); bg texture + blur caches still invalidate on crop
- [ ] Selection-first landing phase works fitted (mask + hint
      track the fitted image)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` green (clamp helper
      tests; crop-refit test)
- [ ] Full gate green (see Task 001's gate commands)
- [ ] Manual: capture a large region and a tiny one — same window
      size both times; resize the window — image refits; crop via
      selection-first — window unchanged

**Dependencies:** Task 003 (the fit transform this task activates)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/shell.rs`
- `crates/flintshot-editor-egui/src/app.rs`

**Docs:**
- None expected.

**Estimated scope:** Small: 2 files
