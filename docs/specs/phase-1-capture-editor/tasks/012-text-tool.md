# Task 012 — Text tool with `cosmic-text`

> Phase 1.E · size: M · depends on: 008

> **Deviation (shipped).** The originally-specced `cosmic-text` pipeline
> was swapped for `ab_glyph` + `fontdb` to keep this highest-risk Phase
> 1 task achievable — see [`docs/adrs/ADR-0017`](../../../adrs/ADR-0017-ab-glyph-over-cosmic-text-phase-1.md).
> The plan pre-authorised this fallback ("if it slips, ship Phase 1
> with `ab_glyph` fallback and ADR the swap"). Multi-line text
> (`Shift+Enter`) is deferred to Phase 2.

## Description

Highest-risk task. `Tool::Text` lets the user click to place a text
cursor, type a label, and commit on `Enter`. Text shaping via
`cosmic-text` (already in the dep tree via `flintshot-cosmic`). Adds a
`flintshot-core` rendering concern: text shaping results need to flow
through `Canvas::draw_text` without leaking `cosmic_text` types into
core's API.

## Acceptance criteria

- [x] Clicking with `Tool::Text` places a text input caret at the click
      location.
- [x] Typing inserts characters; `Backspace` deletes; `Enter` commits
      the `Annotation::Text` to `anns`.
- [x] `Esc` while editing cancels (no annotation added).
- [x] Committed text renders via the shared `render()` path; the shell
      uses `cosmic-text` to shape glyphs and emits them through
      `Canvas::draw_text`. _(Shipped via `ab_glyph` instead — ADR-0017.)_
- [x] `cosmic_text` types do not appear in `flintshot-core`'s public
      API. (`no_gui_types_in_core` test extended to cover `cosmic_text`
      prefix.) _(Moot — `cosmic-text` is not a dependency; the guard
      extension is deferred to whenever Phase 2 adopts it.)_
- [x] Default font loaded via system fontconfig (`cosmic-text`'s
      default); missing-font error surfaces as a readable message.
      _(Via `fontdb`; missing font warns + skips the annotation.)_

## Verification

- [x] Standing gate passes.
- [x] Unit test: `EditorState` text-commit flow (place, type, commit,
      undo) — pure state machine, no rendering.
- [ ] Manual: place "hello world" over a screenshot, save, verify
      readable glyphs in the PNG at the correct location. _(interactive
      verification owed — see Checkpoint E close-out)_
- [~] Manual: type a multi-line label (`Shift+Enter`), verify line
      spacing. _(Deferred to Phase 2 — single-line only in Phase 1.)_

## Dependencies

Task 8.

## Files likely touched

- `crates/flintshot-core/src/editor.rs` (Text tool state — caret,
  in-progress string)
- `crates/flintshot-core/src/render.rs` (Text dispatch; decide whether
  to extend `Canvas::draw_text` signature for shaped runs)
- `crates/flintshot-core/tests/no_gui_types_in_core.rs` (extend the
  forbidden-prefixes list) _(skipped — see deviation note)_
- `crates/flintshot-editor-egui/src/canvas.rs` (cosmic-text wiring for
  `draw_text`) _(uses egui's native `Painter::text` instead)_
- `crates/flintshot-editor-egui/src/shell.rs` (text input handling)
  _(handled in `app.rs::dispatch_text_input`)_
- `crates/flintshot-editor-egui/Cargo.toml` (dep: `cosmic-text`,
  `ab_glyph`) _(no egui-side dep change; `ab_glyph`/`fontdb` added to
  `flintshot-core` instead)_
- `crates/flintshot-core/Cargo.toml` (dep: `ab_glyph`, `fontdb`)
- `crates/flintshot-core/src/input.rs` (`InputEvent::TextInput(char)`)
- `crates/flintshot-editor-egui/src/app.rs` (text input translation,
  toolbar button)

## Size

M
