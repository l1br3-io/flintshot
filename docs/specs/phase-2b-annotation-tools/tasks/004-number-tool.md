# Task 004: Number tool — click-to-place + circular bubble + contrast text + undo counter

**Description:** Wire the Number tool end-to-end. Three gaps: (a)
`begin_annotation` returns early for `Tool::Number` (`editor.rs:384`)
— the tool has no input path; (b) the `render_ann` Number arm is a
placeholder — a square bubble via `draw_rect_filled` + fixed white
text (`render.rs:306-327`, comment says "refined when Task 11/12 add
real primitives"); (c) `Snapshot` captures only `anns` (+ optional
`image`), not `next_number`, so undo wouldn't decrement the counter.
Add a click-to-place dispatch path, render a true circular filled
bubble by reusing Task 001's `draw_ellipse` (equal axes + `Some(fill)`,
matching the spec's Objective over its placeholder mention), pick
contrast-aware text color via luminance, and extend `Snapshot` to
capture `next_number`.

**Acceptance criteria:**
- [x] `dispatch_input` (or a dedicated Number entry point) handles
      `PointerDown` with `Tool::Number`: commits
      `Annotation::Number { pos, n: state.next_number, color, diameter }`
      immediately (no drag — Number is click-to-place), increments
      `next_number`, and pushes a `Snapshot`.
- [x] `Snapshot` extended with `next_number: u32` so undo after
      placing `3` restores `next_number` to `2`. All existing
      `Snapshot::anns_only` / `Snapshot::full` call sites updated;
      existing undo/redo tests still pass (they assert on `anns`).
- [x] `render_ann` Number arm renders a filled circle via
      `canvas.draw_ellipse(bbox, stroke, Some(*color))` with equal
      width/height (a circle is an inscribed ellipse) — replaces the
      current `draw_rect_filled` square placeholder.
- [x] Digit text color picked by luminance of the bubble fill: white
      (`Color::rgb(255,255,255)`) on dark fills, black on light
      (luminance threshold; pure helper, unit-tested at the boundary).
- [x] Toolbar exposes a "Number" entry; three successive clicks place
      bubbles labelled `1`, `2`, `3`; a fourth places `4`.
- [x] MockCanvas test: a committed `Annotation::Number` emits exactly
      one `DrawEllipse` (filled) + one `DrawText` (the digit string).
- [x] Hit-test: existing circle-radius `contains` at `editor.rs:1199`
      selects a placed bubble (verify, no change expected).
- [x] `next_number` default stays `1` (`editor.rs:303`); a fresh
      `EditorState` starts the count at 1.

**Verification:**
- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace --all-targets` — incl. Number
      click-to-place state-machine test, `next_number` increment +
      undo-decrement test, luminance-contrast unit test, MockCanvas
      `DrawEllipse`+`DrawText` assertion.
- [x] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual: `cargo run -- region`, switch to Number, click three
      times over sequential UI elements — see `1`, `2`, `3` filled
      circles; `Ctrl+Z` removes `3` and a fourth click places `3` again
      (counter decremented); confirm text is readable on both a dark
      and a light bubble fill.

**Dependencies:** Task 001 (reuses the real `draw_ellipse` impl —
egui + `ImageCanvas` — for the circular bubble; without 001 the bubble
would render in neither). The `Snapshot` extension is independent but
lands here.

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs` — Number dispatch in
  `dispatch_input` (`:605`) or `begin_annotation` (`:384`); `Snapshot`
  struct extension (`:176`) + constructors (`anns_only`/`full` at
  `:188`/`:195`); undo/redo restore of `next_number`.
- `crates/flintshot-core/src/render.rs` — Number arm of `render_ann`
  (`:306-327`): swap `draw_rect_filled` → `draw_ellipse(filled)`; add
  luminance helper + contrast text selection; MockCanvas test.
- `crates/flintshot-editor-egui/src/app.rs` — "Number" toolbar entry.

**Docs:**
- Inline comment at the `Snapshot` extension noting `next_number` is
  captured so undo decrements (the "why" — otherwise the counter is
  monotonic across undos, which is unintuitive).
- Inline comment at the luminance helper giving the threshold and
  referencing WCAG perception if applicable.
- No new ADR (Number's design is a plan-level refinement recorded in
  `plan.md` §Open questions Q2/Q7; the bubble-shape choice is captured
  there).

**Estimated scope:** Medium (3 files; the dispatch + Snapshot
extension + contrast logic are the substantive work).
