# Task 008 — Rectangle tool end-to-end

> Phase 1.C · size: M · depends on: 005, 007

## Description

First full vertical slice of an annotation tool. `Tool::Rect` becomes
the active tool. The egui shell translates mouse events to `Point`s
(physical px), feeds them to the new state machine (Task 7), and the
shared `render()` path draws both committed and pending rects. Toolbar
gets a single button to confirm the tool switching plumbing works (full
picker UI lands in Task 15).

## Acceptance criteria

- [x] With `Tool::Rect` active, mouse-down begins a rect annotation
      with the current color/stroke; mouse-drag updates the opposite
      corner; mouse-up commits. _(Pointer handling lives in
      `EditorApp::logic`; down→`begin_annotation`, drag→
      `update_annotation`, up→`commit_annotation`. `canvas_rect`
      cached by the previous frame's `ui()` so `logic()` can translate
      screen-logical → canvas-physical. Begins only fire when the press
      lands inside the canvas rect; updates/commits proceed regardless
      so drags that leave the canvas still produce a sensible rect.)_
- [x] The in-progress rect is visible during drag (via `pending`).
      _(Already wired in Task 7's `render()` dispatch — Task 8 confirms
      it works end-to-end once `draw_rect_outline` has a real egui
      impl.)_
- [x] Committed rect persists in `anns` across frames; visible after
      next render.
- [x] `Ctrl+Z` undoes the last commit (visible immediately). _(Wired in
      `EditorApp::logic` via `ctx.input(|i| i.key_pressed(egui::Key::Z)
      && i.modifiers.ctrl)`.)_
- [x] Toolbar shows at least one button (Rect) and clicking it sets
      `state.tool`. _(Uses `egui::Panel::top("editor_toolbar").show`
      with `ui.selectable_value(&mut self.state.tool, Tool::Rect, "▭
      Rectangle")`. egui 0.35 unified `TopBottomPanel::top` into
      `Panel::top`.)_
- [x] Clicking outside the canvas area doesn't begin an annotation.
      _(Pointer hit-tested against cached `canvas_rect`; clicks on the
      toolbar fall outside the rect and don't call `begin_annotation`.)_
- [x] No `egui::Painter` calls in the egui crate for the rect — only
      `Canvas::draw_rect_*`. _(Rect drawing routes exclusively through
      `EguiCanvas::draw_rect_outline` / `draw_rect_filled`, which call
      `painter.rect_stroke` / `painter.rect_filled`. The `render()`
      free function in core emits only `Canvas` calls; egui-specific
      stroke kind, color conversion, and physical → logical scaling all
      live in the canvas impl.)_

## Verification

- [x] Standing gate passes. _(`fmt --check`, `clippy --workspace
      --all-targets -- -D warnings`, `test --workspace --all-targets`,
      doc tests — all green. Plus the Task 6 `--no-default-features`
      clippy gate still clean.) Test count: 53 pass (was 52), 1 ignored
      (capture smoke), 0 fail._
- [x] `MockCanvas` test: render an `EditorState` with two committed
      `Annotation::Rect`s; assert recorded `draw_rect_outline` count
      and arguments. _(New test
      `render_two_committed_rects_emits_two_outlines` in
      `crates/flintshot-core/src/render.rs`. Covers rect 1 outline only
      + rect 2 fill+outline, exact geometry/stroke/color assertions,
      and the fill-before-outline dispatch order.)_
- [x] Manual: `cargo run -- region`, draw a red rectangle, see it
      persist, press Ctrl+Z, see it disappear. _(Maintainer-verified on
      Linux + Wayland + XWayland, 2026-07-21. The egui 0.35 pointer
      API, the stroke conversion, and the canvas-rect caching all
      behaved correctly under interactive use.)_

## Dependencies

Task 5, Task 7.

## Files likely touched

- `crates/flintshot-core/src/render.rs` (Rect dispatch in `render()`)
- `crates/flintshot-editor-egui/src/canvas.rs` (or `lib.rs` —
  `draw_rect_outline` + `draw_rect_filled` impls)
- `crates/flintshot-editor-egui/src/shell.rs` (input handling, toolbar)

## Size

M
