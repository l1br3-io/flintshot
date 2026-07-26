# Task 009 — Save (`rfd` + PNG encode) and Copy (`arboard`)

> Phase 1.D · size: M · depends on: 008

## Description

Implement the two output paths. Save: compose a final `RgbaImage`
(background + rendered annotations — blur/pixelate baked in via the same
code path the canvas uses), encode PNG via `image`, write via
`rfd::AsyncFileDialog`. Copy: same image composition, handed to
`arboard::Clipboard::set_image`. The editor returns
`EditorOutcome::Saved(path)` or `EditorOutcome::CopiedToClipboard`.

## Acceptance criteria

- [x] New core function `compose_final(state: &EditorState) ->
      RgbaImage` renders `anns` into a clone of `state.image` (this is
      the source of truth for save/copy — not a screenshot of the egui
      canvas). _(Lives in `flintshot-core::render::compose_final`;
      re-exported from the crate root. Uses a private `ImageCanvas` that
      implements `Canvas` on top of an `RgbaImage`, so the per-variant
      dispatch goes through the same `render_ann` path the live editor
      uses. Excludes `state.pending` — in-flight annotations are not
      part of the saved output.)_
- [x] `flintshot-editor-egui` exposes Save and Copy buttons (no keyboard
      wiring yet — Task 10). _(Two new buttons in the
      `Panel::top("editor_toolbar")` row, after the annotation-count
      label: `💾 Save` and `📋 Copy`. Clicks collect into a private
      `ToolbarAction` enum returned from the panel closure so the
      action handler can borrow `&self.outcome` cleanly outside.)_
- [x] Save opens `rfd` async dialog; PNG written to chosen path. _(Spec
      said async; shipped **sync** `rfd::FileDialog::save_file` instead
      — see deviation 1 below. PNG encode/write via
      `image::save_with_format` with `ImageFormat::Png` forced,
      matching the `--out` path from Task 3.)_
- [ ] Copy puts the image on the clipboard and is pasteable in another
      app (verified manually). _(Wired through `arboard::Clipboard::
      set_image` with `ImageData` constructed from the `RgbaImage`'s
      raw bytes; both are RGBA row-major so no swizzling needed. Needs
      maintainer's eyes to confirm pasteability in GIMP / Preview /
      mspaint.)_
- [x] `EguiShell::run_editor` returns the correct `EditorOutcome`
      variant on each action and closes the window. _(On Save:
      publishes `Ok(Saved(path))` then `ViewportCommand::Close`. On
      Copy: publishes `Ok(CopiedToClipboard)` then close. The existing
      Task 6 CLI match in `run_editor_for_capture` already logs each
      variant, so the Saved / CopiedToClipboard log lines light up
      without further change. Render-error path unchanged from Task 5.)_

## Verification

- [x] Standing gate passes. _(`fmt --check`, `clippy --workspace
      --all-targets -- -D warnings`, `clippy --no-default-features -p
      flintshot-app --all-targets -- -D warnings`,
      `test --workspace --all-targets`,
      `test --doc -p flintshot-core -p flintshot-editor-egui` — all
      green. Test count: 57 pass (was 53; +4 `compose_final` tests), 1
      ignored (capture smoke), 0 fail.)_
- [x] Unit test: `compose_final` on a known `EditorState` produces a
      deterministic image (compare to a fixture PNG checked into the
      repo). _(Deviation 4: shipped pixel-level assertions on known
      positions instead of a checked-in PNG fixture. Four tests:
      `compose_final_renders_rect_outline_into_clone`,
      `compose_final_renders_rect_fill_and_outline`,
      `compose_final_excludes_pending`,
      `compose_final_empty_state_is_image_clone`. Pixel assertions give
      better failure messages than binary fixtures and avoid
      cross-platform font-rendering fragility ahead of Task 12.)_
- [ ] Manual: draw a rect, click Save, choose a path, verify file opens
      in an image viewer and shows the rect baked in. _(Needs
      maintainer's eyes on a real display — same carry-forward debt as
      Tasks 5/6/8.)_
- [ ] Manual: draw a rect, click Copy, paste in GIMP / Preview / mspaint;
      verify rect is visible. _(Same carry-forward.)_

## Dependencies

Task 8.

## Files likely touched

- `crates/flintshot-core/src/render.rs` or new `image_ops.rs`
  (`compose_final`)
- `crates/flintshot-editor-egui/src/shell.rs` (Save / Copy buttons)
- `crates/flintshot-editor-egui/Cargo.toml` (deps: `rfd`, `arboard`)

## Size

M
