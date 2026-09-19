---
status: active       # draft → active → shipped (flipped by /build and on merge)
started: 2026-09-20
---

# Spec — Text selection & highlight: select ranges inside the text being edited

> Mini-effort spec. Direct successor to
> [text-select-edit](../text-select-edit/spec.md) (select committed
> text, re-edit it). For editor architecture see
> [Phase 1](../phase-1-capture-editor/spec.md); for product context see
> [`../../VISION.md`](../../VISION.md).

## Objective

Today the text modal path is caret-only: you can type, Backspace,
arrow/Home/End, Enter-commit, Esc-revert — but you cannot *select a
range* of the text you are editing. There is no Shift+arrow, no
Ctrl+A, no drag-select, no double-click-word, and no highlight behind
selected characters. Every other image editor (Photoshop, Snagit,
GIMP) treats in-edit text like real text: select a range, see it
highlighted, replace or delete it in one stroke.

This effort brings that industry behavior to **both** text-editing
contexts — a fresh draft *and* a re-edit session (they share the
`pending` modal path, so they get it via one implementation). The
existing commit/revert/caret semantics are fine and stay
byte-for-byte: Enter still commits, Esc still reverts a re-edit,
clicking away still commits-and-moves-on.

**User stories:**

1. While editing, **drag across characters** → the swept range
   highlights; release keeps it selected.
2. **Double-click a word** while editing → that word selects.
   **Triple-click** → the clicked line selects (which *is* the whole
   text for the common single-line annotation).
3. **Shift+Left/Right/Up/Down** extends by char / line;
   **Shift+Home/End** extends to line edges; **Ctrl+Shift+Left/Right**
   extends word-wise; **Ctrl+A** selects all.
4. **Type over a selection** → it is replaced in one stroke.
   **Backspace / Delete** with a non-empty selection deletes the range
   (not the char before the caret).
5. Any plain caret move (arrow, Home/End, click) **collapses** the
   selection.
6. The block caret is retired: editing always shows an **industry
   thin-bar caret** plus, when a non-empty selection exists,
   **translucent highlight rects** under the selected glyphs.

**Out of scope (explicit):** clipboard ops on the selection
(Ctrl+C/X/V — follow-up effort), caret-from-click positioning on
re-edit open (still caret-at-end, as in text-select-edit v1),
committed-annotation behaviors (already done), and any change to
Enter/Esc/tool-switch commit semantics.

## Assumptions

1. "Current behavior is fine" (user, 2026-09-20) — the existing
   caret/commit flow is the baseline; this effort only *adds*
   selection + highlight on top.
2. Selection semantics live in `flintshot-core`'s dispatcher
   (`editor.rs`); both shells get identical behavior via routing, per
   the established boundary.
3. Rendering needs **no new `Canvas` trait methods**: the bar caret
   and per-line highlight rects are `draw_rect_filled` calls the
   shared render path composes (thin rect for the bar, translucent
   rects for highlight). This keeps the core public API surface
   unchanged.
4. Text hit-testing keeps the coarse model: line from y ÷ line-height,
   char boundary from prefix measurement via `Canvas::measure_text`
   (same approach `caret_coords` already uses). No glyph-level
   shaping.
5. `DoubleClick` during editing means select-word, not re-edit:
   re-edit only fires on *committed* text, and during an edit session
   the annotation is lifted out of `anns` — the two contexts cannot
   collide.
6. Multi-byte safety: `text_cursor` is a char index today; the anchor
   is too. All range math goes through char-boundary helpers
   (`char_to_byte` et al.), never raw byte slicing.

## Technologies

No new dependencies. Rust workspace as-is: `flintshot-core` (state
machine + render), `flintshot-editor-egui` (event pumps, tracker,
theme color). egui 0.35 (existing).

## Commands

From repo root (full detail in [`AGENTS.md`](../../../AGENTS.md)):

```sh
cargo build                              # default-members
cargo fmt --all --check                  # formatting gate
cargo clippy --workspace --all-targets -- -D warnings   # lint gate
cargo test --workspace --all-targets     # unit + integration
cargo test --doc -p flintshot-core -p flintshot-editor-egui  # doc tests
cargo run -- app                         # manual verification window
```

## Project Structure

| Touch                                    | Why                                                            |
| ---------------------------------------- | -------------------------------------------------------------- |
| `crates/flintshot-core/src/editor.rs`    | `text_anchor` state; Shift/Ctrl+A key branches; press/drag/word/line selection APIs (char-index based); replace/delete-over-range edit ops; collapse rules; commit clears anchor |
| `crates/flintshot-core/src/text.rs`      | unchanged this effort (word-boundary helpers land in `editor.rs` beside the cursor helpers) |
| `crates/flintshot-core/src/render.rs`    | Bar caret replaces block caret in the pending-text path; per-line highlight rects between anchor and cursor; `TEXT_HIGHLIGHT` const colour (same pattern as `TEXT_BBOX_STROKE`) |
| `crates/flintshot-core/src/input.rs`     | `InputEvent::TripleClick` variant (shell-synthesized, same in-addition-to-press ordering as `DoubleClick`) |
| `crates/flintshot-editor-egui/src/dblclick.rs` | Tracker returns a consecutive-press count (1/2/3), not a bool |
| `crates/flintshot-editor-egui/src/texthit.rs` (new) | Shared pos→char-index resolver (view-aware, closure-based measurement — pure enough to unit-test); the plan-time correction to this spec's original `text.rs`/`char_at_hit` row: glyph metrics exist only behind `Canvas`, so hit resolution is shell-side, matching the shell-owns-glyph-metrics precedent |
| `crates/flintshot-editor-egui/src/canvas.rs`, `overlay.rs` | Pump wiring: press-inside-pending interception, drag-select, double/triple-click routing while editing |
| `docs/specs/text-selection-highlight/`   | This spec, plan, tasks                                         |
| `CHANGELOG.md`, `docs/TEST_MATRIX.md`    | Closeout entries; ADR only if a non-obvious choice emerges     |

## Code Style

Per [`AGENTS.md`](../../../AGENTS.md) and
`.opencode/tech/rust/coding-style.md`. Shape of the core addition:

```rust
/// Char index of the selection anchor; `None` = collapsed (no
/// selection). The selection is `min(anchor, cursor)..max(anchor,
/// cursor)`; setting the cursor without Shift clears the anchor.
text_anchor: Option<usize>,

fn select_word(&mut self) {
    if let Some(Annotation::Text { text, .. }) = &self.pending {
        let (a, b) = word_bounds_around(text, self.text_cursor);
        self.text_anchor = Some(a);
        self.text_cursor = b;
    }
}
```

- `#![forbid(unsafe_code)]`, no `unwrap()`/`expect()` outside tests,
  `thiserror` errors, doc comments on public items, kebab-case serde
  (n/a here — no new serde types).

## Testing Strategy

Same levels the predecessor efforts used (`cargo test`, per-crate
unit + doc tests; manual `cargo run -- app` for feel):

- **editor.rs dispatcher tests** (bulk): synthetic `InputEvent`
  sequences assert anchor/cursor state after each gesture — Shift+
  each arrow, Ctrl+Shift word-wise, Shift+Home/End, Ctrl+A, drag
  press/move/release, double-click word (incl. punctuation-adjacent
  words), triple-click line, collapse on plain move, replace-on-type,
  delete-range on Backspace/Delete, commit clears the anchor,
  multi-line selections spanning lines, multi-byte chars (emoji)
  never panic or split.
- **text.rs tests**: `word_left`/`word_right` and `char_at_hit`
  boundary tables (spaces, punctuation, line ends, empty text).
- **render.rs tests**: the three existing block-caret contract tests
  (`render_pending_text_emits_caret_at_end_of_run`,
  `render_pending_empty_text_emits_caret_at_pos`,
  `render_committed_text_emits_no_caret`) are **revised, not
  deleted** — they lock the *new* bar-caret geometry; a new test
  asserts one translucent `DrawRectFilled` per selected line
  segment, drawn before the glyph run; committed text still emits no
  caret and no highlight.
- **dblclick.rs tests**: count 1→2→3→1, timeout/distance reset to 1.
- **Regression**: every text-select-edit acceptance (single-click
  select, double-click re-edit, Del deletes, panel write-through)
  still passes unchanged.

## Boundaries

- **Always:** run all four gates before declaring done; keep core's
  public API GUI-type-free (`no_gui_types_in_core.rs` must stay
  green); all range math via char-boundary helpers; every public item
  documented.
- **Ask first:** any new `Canvas` trait method (assumed avoidable —
  if hit-testing or highlight truly needs one, raise it first);
  changing the pending-text render-call signature in a way that
  ripples beyond the two egui shells.
- **Never:** change Enter-commit / Esc-revert / tool-switch commit
  semantics; add clipboard ops (deferred by decision); delete or
  `#[ignore]` existing tests to make new ones pass — revise their
  contracts instead; `unsafe`, `unwrap()` outside tests.

## Success Criteria

1. Type "hello world", Shift+Left ×3 → "rld" highlighted (bar caret
   at range edge); type "X" → text is "hello woXd", one stroke.
2. Ctrl+A → whole text highlighted; Backspace → empty draft; Esc /
   click-away never commits an empty draft (unchanged rule).
3. Drag from before "hello" to after "world" → both words
   highlighted; Delete → both gone.
4. Double-click "world" → only "world" highlighted. Triple-click →
   the clicked line highlighted (all of it for single-line text).
5. Multi-byte: paste-free emoji typing, Shift+Left over "🙂" moves one
   char; selection edges never split it.
6. Bar caret visible whenever editing (empty draft included); no
   block caret anywhere; committed text shows neither caret nor
   highlight.
7. One `Ctrl+Z` after a re-edit commit that used selection edits
   still reverts the whole session (undo bracketing unchanged).
8. All four gates green; text-select-edit acceptances re-verified.

## Open Questions

- **Triple-click = line vs all:** spec'd as *clicked line* (desktop
  convention; degenerates to whole-text for the common single-line
  annotation, and Ctrl+A covers whole-text). Veto now if you want
  triple = all.
- Highlight tint: resolved at planning — a `TEXT_HIGHLIGHT` const in
  core's `render.rs` (same pattern as `TEXT_BBOX_STROKE`), so both
  renderers agree with zero API growth; exact value tuned during
  implementation against light and dark captures. Highlight draws
  after the glyph run (translucent, editor-style over-glyph tint).
