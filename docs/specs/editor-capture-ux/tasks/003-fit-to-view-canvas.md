# Task 003: Fit-to-view canvas scale

**Description:** Introduce the fit-to-view scale into the editor's
physical↔logical transform. `EguiCanvas` gains a view transform —
image-physical px → screen-logical px becomes
`x_logical = x_img * scale / ppp + origin` (today `scale` is always
1.0) — and every site that maps between the spaces goes through one
shared helper: the canvas `to_egui_*` functions, the pointer
translation in `EditorApp::logic_impl`, and the chrome paint sites
(magnifier, move chrome, selected chrome, selection-phase mask +
hint). The image is letterboxed and centered inside the canvas
panel; `fit_scale(img, viewport)` is a pure function capped at 1.0
(never upscales). This task lands byte-identical rendering: the
window is still image-sized, so fit == 1.0 everywhere; the pure
functions and the shared helper arrive tested ahead of Task 004
flipping the viewport.

**Acceptance criteria:**
- [ ] `fit_scale` pure fn + unit tests: large image scales down,
      tiny image caps at 1.0, degenerate inputs (zero dims / zero
      viewport) yield no scale, aspect-extreme images fit the
      binding axis
- [ ] Round-trip unit tests: logical → image px → logical under an
      arbitrary scale (pointer mapping) and image px → logical →
      image px (chrome placement)
- [ ] All pointer/chrome/paint sites route through the shared view
      helper (no hand-rolled `ppp` division left in `app.rs` chrome
      sites)
- [ ] Behavior unchanged at scale 1.0 (existing editor tests green;
      manual smoke identical to before)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` green (incl. new
      transform tests)
- [ ] Full gate green (see Task 001's gate commands)
- [ ] Manual smoke: annotate, move-chrome hover/grab, magnifier,
      selection-first mask + hint, Enter-crop — all visually
      unchanged

**Dependencies:** None (Phase 2 start)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/canvas.rs`
- `crates/flintshot-editor-egui/src/app.rs`
- `crates/flintshot-editor-egui/src/chrome.rs` (if paint helpers
  take the view)

**Docs:**
- GOTCHA (transform sites) → inline comment at the shared helper
  listing every mapping site that must use it.

**Estimated scope:** Medium: 2–3 files
