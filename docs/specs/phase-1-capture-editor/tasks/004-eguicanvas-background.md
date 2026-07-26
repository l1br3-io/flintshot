# Task 004 — `EguiCanvas` background primitives + `render()` body

> Phase 1.B · size: S · depends on: 002 (real image); 007 (MockCanvas)

## Description

Implement `EguiCanvas::upload_image` and `draw_image`. Fill in
`flintshot-core::render::render()` so it uploads the background image
(caching the handle in the canvas) and blits it to the canvas's full rect
on each frame. No annotations yet. The canvas needs an `egui::Context`
(or `Painter`); re-shape the struct to receive one per frame.

## Acceptance criteria

- [x] `EguiCanvas::upload_image` loads via `ctx.load_texture` with
      `TextureFilter::Nearest` (sharp pixels matter for screenshots).
- [x] `EguiCanvas::draw_image` blits at the requested `Rect` (physical
      px → logical px conversion done in the canvas).
- [x] `flintshot-core::render::render()` uploads once (cached),
      re-blits each frame, returns `Ok(())`.
- [x] No GUI types leak: the `render()` free function is in core and
      takes `&mut impl Canvas`; it never names `egui`.

## Verification

- [x] Standing gate passes (including the `no_gui_types_in_core`
      integration test).
- [x] Unit test in `render.rs`: with an empty `EditorState`, the
      `MockCanvas` (added in Task 7) records exactly one `draw_image`
      call. *(If Task 7 hasn't landed yet, defer this assertion to Task
      7 and instead assert via a temporary inline mock.)* _(Used an
      inline `MockCanvas` covering only `draw_image` for now; Task 7
      promotes it to a public struct with a full `DrawCall` enum. Three
      render tests cover: single-blit, cache reuse across frames, and
      re-upload after invalidation.)_

## Dependencies

None strictly; benefits from Task 2 so there's a real image to upload.
Task 7's MockCanvas is needed for a meaningful unit test — co-design
with Task 7 if it lands first.

## Files likely touched

- `crates/flintshot-editor-egui/src/lib.rs` (or split into `canvas.rs`
  — decision in Task 4)
- `crates/flintshot-core/src/render.rs` (fill body)

## Size

S
