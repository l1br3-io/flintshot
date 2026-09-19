# Task 005: Pan + wheel zoom

**Description:** Give the editor view user-controlled panning and cursor-
anchored wheel zoom. A `pan` offset (screen-logical px) lives on `EditorApp`
and composes with the computed letterbox offset into
`canvas.set_content_offset`. Wheel steps the existing `view_zoom` by the
same 0.25 quantization anchored at the cursor; middle-drag pans from any
tool; primary drag under Hand pans — the shell never dispatches Hand
presses to core (pan-only contract; core no-ops them defensively).

**Acceptance criteria:**
- [ ] `update_fit` keeps computing the centered letterbox separately; total
      `content_offset = letterbox + pan`; pan clamps so the image rect always
      intersects the viewport, and clamps to zero while the image is fully
      visible (no pointless letterbox sliding)
- [ ] Wheel (any button delta) zooms one 0.25 step toward/away, keeping the
      image point under the cursor fixed (pan adjusted accordingly); zoom
      clamps to `VIEW_ZOOM_MIN`/`MAX`; returning to 1.0 resets pan to zero
      (`at_fit_zoom` stays exact)
- [ ] Middle-button drag pans from any tool; primary drag under Hand pans
      (Hand cursor icon `Grab`/`Grabbing`); Hand never dispatches pointer
      events to core — annotations are untouchable under Hand (mainstream
      hand-tool contract)
- [ ] Pan composes correctly through every mapping site (pointer dispatch,
      chrome, magnifier, mask) because they all read `view_over` — verified
      by drawing after panning and checking the annotation lands where
      clicked
- [ ] Pure helper fns for clamp and zoom-anchor math (in `canvas.rs` next to
      `fit_scale`) with unit tests; overlay unaffected (its app never grows
      pan state)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui --all-targets` green
- [ ] Manual: `cargo run -- app` → zoom in with wheel (cursor-anchored),
      pan with middle-drag and Hand drag, draw an arrow while panned
      (lands under cursor), switch to Hand and try to drag an annotation
      (only pans), zoom back to 1.0 (pan resets, resize policy intact)

**Dependencies:** Task 001 (Hand exists); shares `app.rs` with Tasks 002-004
— land sequentially

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs` (pan state, wheel/middle input,
      Hand drag routing, cursor)
- `crates/flintshot-editor-egui/src/canvas.rs` (pure clamp/anchor helpers +
      tests)

**Docs:** GOTCHA at the pan field: never set `content_offset` directly
outside `update_fit` composition — pointer mapping and the magnifier read it.

**Estimated scope:** Medium-Large (2 files, one subsystem)
