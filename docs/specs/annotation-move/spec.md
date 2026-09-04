---
status: active
started: 2026-09-03
---

# Spec — Annotation move: reposition committed annotations

> Mini-effort spec. For product context, see
> [`../../VISION.md`](../../VISION.md); for the editor architecture,
> see [Phase 1](../phase-1-capture-editor/spec.md) and the overlay
> chrome in [region-selection-first](../region-selection-first/spec.md).

## Objective

Today an annotation, once placed, is fixed: the only remedies are
undo (`Ctrl+Z`) or starting over. Users expect to grab a drawn
rectangle / arrow / text / blur and drag it to a better spot —
Flameshot-class Editors all offer this.

With `Tool::Select` active, pressing on a committed annotation grabs
it and dragging repositions it. Works in both shells (editor window
and fullscreen overlay) because the state machine lives in
`flintshot-core`'s dispatcher.

**Success looks like:**

- Draw a red rect, press `S`, drag the rect — it moves, size and
  style untouched, grab point tracking the cursor.
- Same for every annotation variant (Rect, Ellipse, Line, Arrow,
  Marker, Pencil, Text, Blur, Number).
- One `Ctrl+Z` restores the pre-move position (one undo step per
  move gesture, not per pointer event).
- Click-without-drag on an annotation is a no-op (no history entry).
- Region selection, region move/resize, and drawing new annotations
  behave exactly as before.

## Assumptions

1. Move is `Tool::Select`-only. Annotation tools keep their
   "press starts drawing" contract (overlay comment at
   `gesture_for_press` is explicit about this).
2. Annotation hit wins over region-interior drag in the overlay
   (grab-the-topmost-thing); region handles still win over
   annotations (the handle is the region's affordance).
3. No z-order change while moving — the annotation keeps its index
   (raising on grab is a separate concern if ever wanted).
4. No per-annotation chrome (bounding box, resize handles) this
   effort — move only. `selected_index` gets wired as the hook for
   that future work.
5. Movement is unclamped: paint-time clip (overlay) and
   `crop` (output) own the overflow semantics, exactly like
   mid-drag stroke overflow today. Pointer bounds keep moves
   in-image in practice.

## Scope (in / out)

**In:**

- `Annotation::translate(dx, dy)` — verbatim per-variant geometry
  shift, mirroring `crop_and_translate`'s field mapping.
- Dispatcher state machine: Select-tool `PointerDown` hit-tests
  first (start move), `PointerDrag` translates, `PointerUp` ends.
- Lazy undo snapshot on first actual movement; `redo` cleared then.
  Click-without-move leaves history untouched.
- Wholesale `anns` replacement paths (`undo`, `redo`, `crop`) cancel
  an in-flight move.
- `EditorState::selected_index` wired: set on grab, cleared on
  release (was an unused placeholder).
- Overlay: `gesture_for_press` gains an annotation-hit branch that
  falls through to the dispatcher (before the region-interior
  `Move` branch, after handles); hover cursor shows `Move` over an
  annotation under Select.
- Tests: core dispatcher semantics + per-variant translate; overlay
  gesture fall-through + cursor.

**Out:**

- Per-annotation resize handles / bounding-box chrome.
- Nudge (arrow-key) movement, snapping, alignment guides.
- Move for the in-flight `pending` annotation (only committed
  `anns` are grabbable).
- Z-order raise/lower.
- Editor-window hover cursor (it has no hover-cursor logic at all
  today; adding it is polish, not this effort).

## Deliverables

1. `flintshot-core`: `Annotation::translate`,
   `EditorState::moving` + `AnnotationDrag`, dispatcher wiring,
   undo/redo/crop guards, `selected_index` lifecycle.
2. `flintshot-editor-egui`: overlay `gesture_for_press` annotation
   fall-through + `hover_cursor` branch.
3. CHANGELOG entry + TEST_MATRIX row.
4. This spec + task files.

## Exit criteria

1. `cargo run -- region` → draw rect → press `S` → drag the rect →
   it moves; `Ctrl+Z` puts it back; `Ctrl+Y` re-applies.
2. Same interaction over a text label and a blur region (spot-check
   two non-rect variants).
3. In the overlay: region move/resize still work (press on empty
   region interior moves the region; a handle grab resizes it);
   drawing with an annotation tool still starts on press.
4. Press on empty canvas with Select still starts a fresh region
   selection drag (both shells).
5. One move gesture = exactly one undo entry, regardless of how
   many pointer events it took.
6. Standing gates green (`fmt --check`,
   `clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace --all-targets`, doc tests for the two
   lib crates).

## Technologies

No new dependencies. Pure logic in `flintshot-core::editor`; two
small touch-points in `flintshot-editor-egui::overlay`. Blur
recompute during a blur move is per-frame (parameter-keyed
`BlurCache` sees a new rect each frame) — identical cost profile to
drawing a fresh blur, accepted.

## Commands

```sh
cargo test -p flintshot-core                 # dispatcher + translate tests
cargo test -p flintshot-editor-egui          # overlay gesture tests
# standing gate (AGENTS.md)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

## Project structure

| Change                    | Location                             |
| ------------------------- | ------------------------------------ |
| Translate + move state    | `crates/flintshot-core/src/editor.rs` |
| Gesture fall-through      | `crates/flintshot-editor-egui/src/overlay.rs` |
| Docs                      | `CHANGELOG.md`, `docs/TEST_MATRIX.md` |

## Code style

Per [`AGENTS.md`](../../../AGENTS.md): `#[must_use]` on pure
queries, doc comments on every new public item (`missing_docs`),
no `unwrap()`/`expect()` outside tests, no GUI types in core.

## Testing strategy

- **Core unit tests** (alongside the existing dispatcher tests in
  `editor.rs`): move start / translate / end lifecycle; single
  lazy undo snapshot; click-without-move; undo-mid-drag cancel;
  fresh-selection regression; per-variant `translate` geometry.
- **Overlay unit tests** (`overlay.rs` test mod): annotation press
  inside the region falls through (`gesture_for_press` → `None`),
  handle precedence preserved.
- **Manual**: exit criteria 1–4 via `cargo run -- region`.

## Boundaries

- **Always:** every mutation flows through the dispatcher or a
  documented pure helper; snapshot-before-mutate ordering; run the
  standing gate before declaring done.
- **Ask first:** new public API beyond `AnnotationDrag` /
  `Annotation::translate` / the `moving` field; any change to the
  text-editing modal path.
- **Never:** shell-side duplication of move math (both shells get
  it via core only); clamping moves in core (paint/crop own it).

## Success criteria

- Annotations feel repositionable, not frozen.
- Zero behavior change for every non-Select press.
- Core owns the semantics; the overlay change is routing only.

## Decisions

- **Core owns the move** — the dispatcher handles Select-tool
  pointer events for both shells already; the overlay only needs to
  *stop intercepting* annotation presses. No new `Gesture` variant.
- **Lazy snapshot** — pushing undo state at grab time would pollute
  history with click-without-move events; first actual movement is
  the honest commit point.
- **Topmost-hit grab** — `hit_test` already returns the highest
  index (drawn last); that is the visual topmost and the intuitive
  grab target.
- **`selected_index` set on grab** — the field existed as a
  documented placeholder for exactly this; wiring it now gives the
  future per-annotation chrome (bbox, handles, delete) its hook
  without rendering anything yet.

## Open questions

1. Should pressing on an annotation while it sits under a region
   handle grab the annotation instead of the handle? **Resolved:
   handles win** — the handle is the region's affordance and
   annotation-press fallback still reaches the annotation from any
   non-handle pixel of it.
