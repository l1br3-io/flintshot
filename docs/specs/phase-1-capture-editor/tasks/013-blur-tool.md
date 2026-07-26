# Task 013 — Blur tool with `imageproc` (computed in core)

> Phase 1.E · size: M · depends on: 008

## Description

`Tool::Blur` crops the background image to the dragged rect, applies
either Gaussian or pixelate (config-selected), and emits the result via
`Canvas`. The blur *computation* lives in `flintshot-core::image_ops`;
the shell just blits. Two render paths in core: `Annotation::Blur`
produces a sub-image; the `Canvas` emits it via `draw_image` at the rect.

## Acceptance criteria

- [ ] Dragging with `Tool::Blur` produces an `Annotation::Blur` with
      `kind = BlurKind::Gaussian` (config-switchable).
- [ ] `flintshot-core::image_ops::blur(region: &RgbaImage, kind:
      BlurKind, strength: u8) -> RgbaImage` runs entirely in core.
- [ ] Gaussian uses `imageproc::filter::gaussian_blur_f32`; pixelate
      uses a simple block average. Both deterministic.
- [ ] `render()` for `Annotation::Blur`: extract region from
      `state.image`, blur, upload as a fresh texture (or cache by
      ann-index), blit at the rect.
- [ ] No seams between blurred and non-blurred regions at the rect
      boundary (manual check).
- [ ] `BlurKind` selector in the toolbar (Gaussian / Pixelate toggle).

## Verification

- [ ] Standing gate passes.
- [ ] Unit test in `image_ops`: feed a known fixture image + known
      strength; assert the output matches a checked-in fixture PNG
      (snapshot test).
- [ ] Manual: drag a blur over a screenshot region containing text;
      verify the text is unreadable after Gaussian and blocky under
      Pixelate.

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
