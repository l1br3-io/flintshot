---
status: active
started: 2026-09-19
---

# Spec — Number tool: centered digits + a panel size control

> Mini-effort spec. The Number tool shipped in Phase 2b
> ([`../phase-2b-annotation-tools/`](../phase-2b-annotation-tools/spec.md));
> this effort fixes its typography and makes the bubble size adjustable.
> For product context, see [`../../VISION.md`](../../VISION.md).

## Objective

Two problems with `Tool::Number` as shipped:

1. **The digit is mis-aligned inside the bubble.** The shared render
   path draws the label with `draw_text(pos, …)` where `pos` is the
   bubble's *center* (`render.rs`, `Annotation::Number` arm) — but both
   `Canvas` implementations anchor a text run's **top-left** at `pos`
   (`EguiCanvas::draw_text` paints the galley at the anchor;
   `ImageCanvas::draw_text` rasterizes at the anchor). The digit
   therefore sits offset toward the bubble's bottom-right, worst on
   single digits. Because both paths share `render_ann`, the editor
   view and the saved/copied PNG are consistently, WYSIWYG-ly wrong.
2. **The diameter is hard-coded.** Every bubble is
   `NUMBER_DEFAULT_DIAMETER` (28 physical px). On high-resolution
   captures 28 px is too small to read, and the properties panel's
   Number section offers only a "Start at" stepper — no size control.
   `place_number`'s doc comment already anticipates this ("a
   per-state size control is currently out of scope"); this effort is
   that scope.

**Success looks like:** place a bubble — the digit is dead-center, in
the editor and in the saved file. Want bigger labels for a 4K
screenshot? Drag the Size stepper in the panel and the next bubble
lands at 60 px.

## Measures

1. **Center the digit in the shared render path.** In `render_ann`'s
   `Annotation::Number` arm, measure the label with
   `canvas.measure_text(&label, &style)` (already on the `Canvas`
   trait, used by `draw_text_indicators` for exactly this kind of
   shared-path layout), then draw at
   `pos − ½·(width, height)`. One fix; `EguiCanvas` and `ImageCanvas`
   inherit it because both render committed annotations through
   `render_ann` (WYSIWYG contract).
2. **Diameter becomes tool state.** New field
   `EditorState::number_diameter: f32`, default
   `NUMBER_DEFAULT_DIAMETER` (28.0, unchanged).
   `place_number` commits `diameter: self.number_diameter`. The
   annotation already stores its own `diameter`, so committed bubbles
   are unaffected by later panel changes — per-annotation immutability
   holds without extra work.
3. **Size stepper in the panel.** The Number section gains a "Size"
   row beside "Start at": an `egui::DragValue` bound to
   `state.number_diameter`, `range(16.0..=96.0)`, `speed(1.0)`, styled
   identically to the existing stepper row. Digit font size stays
   proportional: `diameter * 0.6`, so the label scales with the
   bubble at every size.
4. **Range constants live in core.** `NUMBER_MIN_DIAMETER` (16.0) and
   `NUMBER_MAX_DIAMETER` (96.0) next to `NUMBER_DEFAULT_DIAMETER`,
   doc-commented. The panel imports them rather than duplicating
   literals (the "Start at" range 1..=999 can stay local — it is a
   label-count bound, not a geometry constant).

## Assumptions

1. **New placements only.** The stepper affects future clicks;
   already-placed bubbles keep their diameter even when selected.
   Retroactive resize is explicitly out (see Open questions).
2. **Not history.** Tweaking the stepper pushes no undo snapshot —
   identical to the color swatches and stroke chips, which also
   mutate tool state without history entries. Undo still restores
   `next_number` via the existing snapshot in `place_number`.
3. **Not config.** `number_diameter` is session tool state like
   `color`, not a `[ui]` config key; no persistence this effort.
4. **Geometric centering first.** Center by measured bounds; if
   dogfooding shows the digit sitting optically low (galley height
   includes line leading), a small constant optical offset may be
   added in review — a tuned constant, not a new mechanism.
5. **16 px floor is legible enough** for two digits (9.6 px font);
   96 px ceiling covers high-DPI labeling without absurdity.

## Scope (in / out)

**In:**

- `flintshot-core/src/editor.rs` — `number_diameter` field (+ doc),
  `NUMBER_MIN/MAX_DIAMETER` constants, `place_number` reads state,
  doc-comment updates, unit tests.
- `flintshot-core/src/render.rs` — centered digit anchor in the
  `Annotation::Number` arm, MockCanvas render-path tests.
- `flintshot-editor-egui/src/panel.rs` — Size stepper row in the
  Number section.
- `CHANGELOG.md` + `docs/TEST_MATRIX.md` rows.

**Out:**

- Resizing selected/committed bubbles (panel or drag handles).
- Changing the `0.6` digit ratio; bubble outline strokes; color
  rings.
- Config persistence of the chosen diameter.
- Any other tool's render path.

## Commands

Standing gates (full set in [`AGENTS.md`](../../../AGENTS.md)):

```sh
cargo build                                 # default-members
cargo test --workspace --all-targets        # all unit + integration
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

Manual exercise:

```sh
cargo run -- editor          # or: capture → region → edit phase
```

## Project structure

```
crates/flintshot-core/src/editor.rs          # tool state + place_number (this effort)
crates/flintshot-core/src/render.rs          # shared render_ann + ImageCanvas + MockCanvas tests
crates/flintshot-editor-egui/src/panel.rs    # properties panel sections (Number section)
docs/specs/number-tool/                      # this effort
```

## Code style

House rules (rustfmt.toml, no GUI types in core's public API, doc
comments carry rationale). Representative shape of the changes:

```rust
/// Minimum Number-tool bubble diameter in physical px. Below this a
/// two-digit label (font `diameter * 0.6`) drops under 10 px and
/// stops being legible on the annotation's own bubble.
const NUMBER_MIN_DIAMETER: f32 = 16.0;

// render_ann, Annotation::Number arm:
let text = TextStyle::simple(*diameter * 0.6, contrast_text_color(*color));
let m = canvas.measure_text(&label, &text);
let anchor = Point::new(pos.x - m.width / 2.0, pos.y - m.height / 2.0);
canvas.draw_text(anchor, &label, &text);
```

## Testing strategy

- **Core state tests** (`editor.rs` test mod, Task-004 pattern):
  default `number_diameter` is 28 and `place_number` commits it;
  setting 64.0 commits 64.0; counter/undo semantics untouched.
- **Render-path tests** (`render.rs` MockCanvas, alongside the
  existing Number-tool render tests): the `DrawText` call's `pos`
  equals bubble center minus half the mock-measured label size, for
  one- and two-digit labels; the ellipse call (bubble disc) is
  unchanged.
- **Panel**: manual — the Number section shows Start at + Size; the
  stepper clamps at 16/96; house widgets make unit-testing the row
  low-value (matches how the Start at stepper is covered).
- **Manual**: exit criteria 1–4 below.

## Exit criteria

1. Place bubbles with 1- and 2-digit labels — digits sit centered in
   the disc in the editor window; Save/Copy output matches.
2. Panel → Number → Size stepper changes the *next* bubble's
   diameter; committed bubbles keep theirs.
3. Stepper refuses values outside 16–96; default on fresh editor is
   28.
4. Undo after a placement still decrements the counter; stepper
   tweaks never create undo entries.
5. Standing gates green (`fmt --check`,
   `clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace --all-targets`, doc tests for the two lib
   crates).

## Boundaries

- **Always:** centering lives in the shared `render_ann` path only —
  no per-canvas centering logic in `EguiCanvas`/`ImageCanvas`;
  `place_number` stays the single commit site; standing gate before
  declaring done.
- **Ask first:** new `Canvas` trait methods (none anticipated —
  `measure_text` suffices); changing the digit ratio; new theme
  constants; touching any other tool's render arm.
- **Never:** GUI types in `flintshot-core`'s public API (guarded by
  `no_gui_types_in_core.rs`); history entries for tool-state tweaks;
  mutating a committed annotation's diameter.

## Success criteria

- The digit reads as *in* the bubble, not *on* it — centered at every
  diameter, WYSIWYG between editor and saved output.
- Bubble size is a two-second adjustment, discoverable in the same
  panel section as the counter it labels.
- Zero behavior change for any tool that is not Number.

## Decisions

- **Fix at the shared call site, not in the canvases.** The offset is
  a mismatch between the call site's assumption (pos = center) and
  the trait's contract (pos = top-left). `render_ann` is the single
  shared path both canvases already route through; changing the
  canvases' anchor semantics would ripple into the Text tool, whose
  `pos` genuinely is a top-left origin.
- **Stepper over chips/slider.** Continuous px value matches "Start
  at" beside it and the DragValue precedent; chips can't express
  4K-scale needs, a slider is wide in a narrow panel.
- **Constants in core, panel imports them.** Geometry bounds are
  semantic (legibility floor, practical ceiling), not panel layout;
  core keeps them next to the default they constrain.

## Open questions

1. Retroactive resize of a *selected* bubble — deferred, not
   rejected; if Move + a future handles effort lands, diameter could
   ride the same selection-edit surface. Revisit then.
2. Should the digit ratio widen for 3+ digits (100+) at small
   diameters? The counter already allows up to 999 via "Start at".
   **Leaning:** shrink-to-fit later if it bites in practice.
