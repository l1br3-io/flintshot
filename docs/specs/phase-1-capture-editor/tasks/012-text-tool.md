# Task 012 — Text tool with `cosmic-text`

> Phase 1.E · size: M · depends on: 008

## Description

Highest-risk task. `Tool::Text` lets the user click to place a text
cursor, type a label, and commit on `Enter`. Text shaping via
`cosmic-text` (already in the dep tree via `flintshot-cosmic`). Adds a
`flintshot-core` rendering concern: text shaping results need to flow
through `Canvas::draw_text` without leaking `cosmic_text` types into
core's API.

## Acceptance criteria

- [ ] Clicking with `Tool::Text` places a text input caret at the click
      location.
- [ ] Typing inserts characters; `Backspace` deletes; `Enter` commits
      the `Annotation::Text` to `anns`.
- [ ] `Esc` while editing cancels (no annotation added).
- [ ] Committed text renders via the shared `render()` path; the shell
      uses `cosmic-text` to shape glyphs and emits them through
      `Canvas::draw_text`.
- [ ] `cosmic_text` types do not appear in `flintshot-core`'s public
      API. (`no_gui_types_in_core` test extended to cover `cosmic_text`
      prefix.)
- [ ] Default font loaded via system fontconfig (`cosmic-text`'s
      default); missing-font error surfaces as a readable message.

## Verification

- [ ] Standing gate passes.
- [ ] Unit test: `EditorState` text-commit flow (place, type, commit,
      undo) — pure state machine, no rendering.
- [ ] Manual: place "hello world" over a screenshot, save, verify
      readable glyphs in the PNG at the correct location.
- [ ] Manual: type a multi-line label (`Shift+Enter`), verify line
      spacing.

## Dependencies

Task 8.

## Files likely touched

- `crates/flintshot-core/src/editor.rs` (Text tool state — caret,
  in-progress string)
- `crates/flintshot-core/src/render.rs` (Text dispatch; decide whether
  to extend `Canvas::draw_text` signature for shaped runs)
- `crates/flintshot-core/tests/no_gui_types_in_core.rs` (extend the
  forbidden-prefixes list)
- `crates/flintshot-editor-egui/src/canvas.rs` (cosmic-text wiring for
  `draw_text`)
- `crates/flintshot-editor-egui/src/shell.rs` (text input handling)
- `crates/flintshot-editor-egui/Cargo.toml` (dep: `cosmic-text`,
  `ab_glyph`)

## Size

M
