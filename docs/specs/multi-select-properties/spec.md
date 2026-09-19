---
status: active
started: 2026-09-19
---

# Spec — Multi-select annotations + Properties right-panel refactor

> Follow-on effort activating the seams `popup-book-ui` reserved
> (multi-select-capable selection model) and `annotation-delete`
> deferred (multi-select delete). For product context see
> [`../../VISION.md`](../../VISION.md).

## Objective

Today the editor selects exactly one annotation (`selected_index`:
`Option<usize>`), and the right panel edits **session tool defaults**
only — changing the color never affects what is already drawn. Users
annotating in bursts (highlight three regions, then recolor them all)
must re-draw or undo.

Two linked changes fix this:

1. **Multi-select** — under `Tool::Move`, Shift+click toggles
   annotations into a selection **set**; plain click replaces the set
   with the hit annotation; a miss press clears it. The whole set
   moves together (one drag, one undo entry) and deletes together
   (one `Del`, one undo entry).
2. **Properties right-panel refactor** — when a selection is live,
   the panel becomes a **selection inspector**: it shows the
   intersection of properties shared by every selected annotation and
   edits apply **live to the selected annotations**, undoably. With no
   selection, the panel stays exactly today's tool-defaults mode.

**Success looks like:** draw three rects, Shift+click all three,
recolor them in one panel gesture, drag them as a group, `Del` once,
`Ctrl+Z` once — everything returns.

**User**: the existing Linux screenshot user; no new entry surface.

## Assumptions (validated in clarification)

1. **Selection input is Shift+click only.** No marquee / rubber-band,
   no `Ctrl+A`. Smallest coherent step; the set representation leaves
   room for both later.
2. **Live property editing is the point of the panel refactor** — not
   a layout reshuffle.
3. **Group operations = move + delete.** Z-order, align/distribute,
   named groups are out.
4. **Mixed-type selections show the property intersection** — a
   section renders only when every selected annotation supports it;
   an empty intersection renders a count + delete-only state.
5. Selection remains **Move-tool chrome** (the standing rule): it
   persists after pointer release, and `set_tool` away from Move,
   `undo` / `redo` / `crop`, and delete clear it. Property editing
   happens while the tool is Move.

## Selection model (core)

- `EditorState::selected_index: Option<usize>` becomes an ordered,
  deduplicated index set (exact type named in plan; note the name
  collision hazard with `selection: Option<Rect>`, the region
  selection — a different concept that must keep its name).
- `hit_test` unchanged (topmost-first). Press routing under Move:
  - hit + Shift → toggle that index in/out of the set;
  - hit, no Shift → set = {index};
  - miss → set = {}.
- **Multi-drag**: an in-flight drag records the press position; each
  `PointerDrag` translates *all* selected annotations by the
  incremental pointer delta (single-annotation grab-offset tracking
  generalizes; degenerate-bbox members are skipped, matching
  `begin_move`'s miss rule). Lazy undo snapshot on first actual
  movement — one gesture, one entry — exactly today's contract.
- **Delete-all**: `delete_selected` removes every selected index in
  one pass, one `anns`-only snapshot. Undo restores all of them.
- Stale-index safety: crop / undo / redo keep clearing the set
  outright (indices into a swapped-out `anns` are meaningless).

## Property intersection matrix

A section appears iff **every** selected annotation supports it:

| Section        | Rect | Ell | Line | Arrow | Marker | Pencil | Text | Number | Blur |
| -------------- | ---- | --- | ---- | ----- | ------ | ------ | ---- | ------ | ---- |
| Stroke (color) | y    | y   | y    | y     | y      | y      | y¹   | y      | n    |
| Stroke width   | y    | y   | y    | y     | y      | y      | n    | n      | n    |
| Fill           | y    | y   | n    | n     | n      | n      | n²   | n      | n    |
| Text style     | n    | n   | n    | n     | n      | n      | y    | n      | n    |
| Effects (blur) | n    | n   | n    | n     | n      | n      | n    | n      | y    |
| Number size    | n    | n   | n    | n     | n      | n      | n    | y      | n    |

¹ Text color is its text color. ² Text `background` stays in the text
popover; Fill means closed-shape interior fill.

Core exposes a capabilities query (pure, `#[must_use]`) plus one
mutator per property; mutators push an `anns`-only snapshot and write
only the fields every selected annotation actually carries. Marker
members re-apply `MARKER_ALPHA` on color edits (their translucent
contract, ADR-0022). Property edits **also write the corresponding
session tool default** (`color`, `stroke_width`, …) so the next draw
matches what the user just set — the inspector teaches the defaults.

## Panel behavior (egui)

- No selection: title `{Tool} · Properties`, sections exactly
  `sections_for(tool)` — byte-for-byte today's mode.
- With selection: title `N selected · Properties`; body renders the
  intersection sections (reusing the existing widgets — swatch row,
  stroke chips, fill switch, text popover, blur bay, number row)
  bound to **selection properties**, plus the Selection section
  (count, Delete-all, enabled iff selection non-empty).
- One edit gesture = one undo entry. Discrete clicks (swatch, preset
  chip, toggle) are one entry each; continuous widgets (DragValue,
  slider, color editor) coalesce — mechanism decided in plan
  (candidate: burst coalescing on consecutive changed-frames).
- Undo never restores the *selection set* — it is chrome, not
  content; undoing a delete leaves the set empty (current behavior
  for the single case).

## Shell chrome

- `chrome.rs`: the persistent-selection outline renders **per
  selected annotation** (same dashed forest language); the grab
  outline covers every member during a live multi-drag. No union
  bbox this effort (polish option, deferred).
- Both shells (editor window `app.rs`, capture overlay `overlay.rs`)
  route Shift+click through the dispatcher and render the same
  chrome; Esc-deselect and miss-deselect behave as today.

## Scope (in / out)

**In:**

- `flintshot-core`: selection set + press routing, multi-drag,
  delete-all, capabilities query, property mutators, default-sync;
  unit tests throughout.
- `flintshot-editor-egui`: panel selection mode, chrome pluralization,
  popover/ColorTarget reuse where needed; unit tests.
- CHANGELOG + `docs/TEST_MATRIX.md` rows; ADR if the per-gesture undo
  coalescing choice is non-obvious when made.

**Out:**

- Marquee select, `Ctrl+A`, z-order / arrange, align/distribute,
  grouping + Objects panel slot, tabs / AI seams, dark theme.
- Overlay region-selection (`Tool::Select`) semantics — untouched.
- Any new config field.

## Deliverables

1. `crates/flintshot-core/src/editor.rs` — selection set, routing,
  multi-move, delete-all, capabilities + mutators (+ tests).
2. `crates/flintshot-editor-egui/src/panel.rs` — selection-mode
  sections + title (+ tests).
3. `crates/flintshot-editor-egui/src/{chrome,app,overlay}.rs` —
  plural chrome + Shift routing (+ tests).
4. `crates/flintshot-editor-egui/src/popover.rs` — color targets for
  selection edits if the existing two targets don't cover it.
5. `CHANGELOG.md`, `docs/TEST_MATRIX.md`, ADR as noted.

## Exit criteria

1. Draw 2 rects + 1 arrow; Shift+click each → three dashed outlines;
   panel title `3 selected · Properties` showing Stroke + Stroke
   width sections only.
2. Recolor via the swatch row → all three change immediately; one
   `Ctrl+Z` restores all three colors; the Stroke default swatch now
   matches the chosen color.
3. Drag any selected member → all translate together; one undo entry.
4. `Del` → all three gone; one `Ctrl+Z` → all three back, selection
   empty.
5. Mixed set (rect + text) → Fill/width hidden, Stroke (color) shown;
   set of texts only → Text style section with working popover edits.
6. No selection → panel identical to today (tool-defaults mode);
   switching tools away from Move clears the set.
7. Standing gates green:
   `cargo fmt --all --check`;
   `cargo clippy --workspace --all-targets -- -D warnings`;
   `cargo test --workspace --all-targets`;
   `cargo test --doc -p flintshot-core -p flintshot-editor-egui`.

## Technologies

No new dependencies. Policy and mutation live in `flintshot-core`
(both shells inherit); chrome is painter strokes in
`flintshot-editor-egui` reusing the dashed selection language.

## Commands

```sh
cargo test -p flintshot-core                  # selection + mutator tests
cargo test -p flintshot-editor-egui           # panel + chrome tests
# standing gate (AGENTS.md)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo run -- app                              # manual check
```

## Project structure

| Change                       | Location                                          |
| ---------------------------- | ------------------------------------------------- |
| Selection set + group ops    | `crates/flintshot-core/src/editor.rs`             |
| Capabilities + mutators      | `crates/flintshot-core/src/editor.rs`             |
| Panel selection mode         | `crates/flintshot-editor-egui/src/panel.rs`       |
| Plural selection chrome      | `crates/flintshot-editor-egui/src/chrome.rs`      |
| Shift routing + panels       | `crates/flintshot-editor-egui/src/{app,overlay}.rs` |
| Color popover targets        | `crates/flintshot-editor-egui/src/popover.rs`     |
| Docs                         | `CHANGELOG.md`, `docs/TEST_MATRIX.md`, `docs/adrs/` |

## Code style

Per [`AGENTS.md`](../../../AGENTS.md): `#[must_use]` on pure queries,
doc comments on new public items, `#[non_exhaustive]` on the
capabilities type, no `unwrap()` / `expect()` outside tests, no GUI
types in core, painter-drawn glyphs only, tokens (not literals) at
widget sites.

## Testing strategy

- **Core unit tests**: toggle/replace/clear routing; multi-drag delta
  + single lazy snapshot; delete-all + one-step undo; the full
  intersection matrix (property × annotation type); mutators skip
  non-supporting members; Marker alpha re-application; default-sync
  writes; `set_tool` / `undo` / `redo` / `crop` clear the set;
  degenerate-bbox members skipped by drag, not fatal.
- **Panel unit tests** (existing `ctx.run_ui` harness): title switches
  by mode; sections render per matrix; delete button enabled iff
  selection; a swatch click routes through the mutator (observable as
  annotation color change + one undo entry).
- **Chrome tests**: one outline rect per selected annotation; grab
  outlines during drag.
- **Regression**: `no_gui_types_in_core.rs` and the full existing
  editor/overlay suites pass unchanged.

## Boundaries

- **Always:** selection policy and mutators live in core only; the
  set is chrome — never composed into Save / Copy / Upload output,
  never restored by undo; standing gate before declaring done.
- **Ask first:** new public core API beyond the set, capabilities
  query, and mutators; new theme tokens; any config field; changes to
  region-selection (`Tool::Select`) semantics.
- **Never:** history entries for selection changes; marquee / Ctrl+A
  scope creep; GUI types in core; emoji/icon-font glyphs; scattering
  style literals.

## Success criteria

- Select → inspect → edit reads as one continuous gesture chain.
- One gesture (move, delete, property edit) is always exactly one
  undo entry, in both shells.
- The no-selection panel is indistinguishable from today's.

## Decisions

- **Shift-click only** — the smallest input that proves the set model;
  marquee/Ctrl+A build on it later without shape changes.
- **Intersection inspector** — the classic empty-state-safe choice;
  never shows a control that would silently no-op on part of the set.
- **Edits sync session defaults** — the inspector doubles as the
  tool-defaults editor once a selection exists; one mental model.
- **Per-annotation outlines** — clearer than a union bbox for
  non-contiguous sets, and free; union polish deferred.
- **Marker keeps its alpha** — color edits re-apply `MARKER_ALPHA`
  rather than opaque-izing the translucent highlight contract.

## Open questions

1. Continuous-widget undo coalescing mechanism (per-frame burst vs
   begin/end bracketing) — resolve in plan with an egui-informed
   spike.
2. Should Shift+click on the *pending* annotation under
   `flip_to_move` interact with the post-commit flip? (Leaning: no
   interaction — the flip already lands on Move with the new
   annotation selected singly.)
