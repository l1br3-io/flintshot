# Task 004: Core — selection highlight rendering

**Description:** Make the selection visible: `draw_text_indicators` gains an `anchor: Option<usize>` parameter (threaded from `render()`, which reads the new `state.text_anchor`); when a non-empty selection exists it paints one translucent `DrawRectFilled` per selected line segment — from the selection's start column to its end column on each spanned line, x from prefix measurement (generalize `caret_coords`'s prefix/align logic into a reusable per-line segment resolver), y from the line index × line height, height = line height. Colour: new `TEXT_HIGHLIGHT` const in render.rs (alpha-tinted, `TEXT_BBOX_STROKE` pattern), drawn after the glyph run (translucent over-glyph tint, editor-style). Draw order within `draw_text_indicators`: bbox outline, highlight rects, then the Task-003 bar caret.

**Acceptance criteria:**
- [ ] anchor=None (or anchor==cursor) → zero highlight rects; identical output to Task 003 state
- [ ] Single-line "hello world", anchor=0/cursor=5 → exactly one highlight rect covering chars 0–5's advance width at y=0, full line height, colour `TEXT_HIGHLIGHT`, emitted before the bar caret
- [ ] Multi-line selection spanning 3 lines → 3 rects: full first line (from sel start), full middle line, partial last line (to sel end); per-line x accounts for `TextAlign` (centre/right offsets match caret math)
- [ ] Selection touching a trailing `\n` extends to the line's end, not into the next line's start
- [ ] Highlight value readable over both a white and a black test image (pick/verify the const alpha in tests or a quick manual look); committed text emits no highlight (no regressions to `render_committed_text_emits_no_caret`)

**Verification:**
- [ ] `cargo test -p flintshot-core` (new render tests with a synthetic anchor set directly on the state field)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: `cargo run -- app` + temporarily set an anchor via a debug path is NOT required — visual confirmation happens at Checkpoint B; here tests carry the contract

**Dependencies:** Task 003 (bar caret ordering inside the same function); Task 001 (field exists)

**Files likely touched:**
- `crates/flintshot-core/src/render.rs`

**Docs:**
- `TEXT_HIGHLIGHT` const doc (why a core const, why over-glyph); segment-resolver doc comment.

**Estimated scope:** Small: 1 file
