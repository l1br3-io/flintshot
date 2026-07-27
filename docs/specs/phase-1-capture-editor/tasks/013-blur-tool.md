# Task 013 — Blur tool with `imageproc` (computed in core)

> Phase 1.E · size: M · depends on: 008

## Description

`Tool::Blur` crops the background image to the dragged rect, applies
either Gaussian or pixelate (config-selected), and emits the result via
`Canvas`. The blur *computation* lives in `flintshot-core::image_ops`;
the shell just blits. Two render paths in core: `Annotation::Blur`
produces a sub-image; the `Canvas` emits it via `draw_image` at the rect.

## Acceptance criteria

- [x] Dragging with `Tool::Blur` produces an `Annotation::Blur` with
      `kind = BlurKind::Gaussian` (config-switchable). _(Default is
      Gaussian; the toolbar's `BlurKind` toggle switches to Pixelate.)_
- [x] `flintshot-core::image_ops::blur(region: &RgbaImage, kind:
      BlurKind, strength: u8) -> RgbaImage` runs entirely in core.
- [x] Gaussian uses `imageproc::filter::gaussian_blur_f32`; pixelate
      uses a simple block average. Both deterministic.
- [x] `render()` for `Annotation::Blur`: extract region from
      `state.image`, blur, upload as a fresh texture (or cache by
      ann-index), blit at the rect. _(Implemented via the new
      `Canvas::draw_image_region` primitive; `render_ann` takes the
      source image so it can crop → blur → blit. No cross-frame
      caching in Phase 1 — egui re-uploads each frame, acceptable for
      typical annotation counts.)_
- [ ] No seams between blurred and non-blurred regions at the rect
      boundary (manual check). _(Interactive verification owed.)_
- [x] `BlurKind` selector in the toolbar (Gaussian / Pixelate toggle).

## Verification

- [x] Standing gate passes.
- [x] Unit test in `image_ops`: feed a known fixture image + known
      strength; assert the output matches a checked-in fixture PNG
      (snapshot test). _(Pixel-level assertions on a half-black /
      half-white fixture — not a checked-in PNG, but deterministic
      given the same `imageproc` version; see
      `gaussian_blur_smooths_sharp_boundary`,
      `pixelate_averages_blocks`.)_
- [ ] Manual: drag a blur over a screenshot region containing text;
      verify the text is unreadable after Gaussian and blocky under
      Pixelate. _(interactive verification owed — see Checkpoint E.)_

## Dependencies

Task 8.

## Files likely touched

- `crates/flintshot-core/src/image_ops.rs` (new)
- `crates/flintshot-core/src/lib.rs` (re-export)
- `crates/flintshot-core/src/render.rs` (Blur dispatch)
- `crates/flintshot-editor-egui/src/canvas.rs` (already-blurred image
  blit via existing `draw_image`)
- `crates/flintshot-editor-egui/src/shell.rs` (Blur tool input, BlurKind
  toggle)

## Size

M
