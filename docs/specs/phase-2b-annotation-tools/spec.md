---
status: draft
started: 2026-08-03
---

# Spec — Phase 2b: Annotation Tools + Magnifier

> Draft effort spec. For product context, see
> [`../../VISION.md`](../../VISION.md); for sequencing, see
> [`../../ROADMAP.md`](../../ROADMAP.md); for the technical substrate
> this builds on, see [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md)
> (especially §Data model, §UI abstraction layer, §Canvas trait). The
> shipped Phase 1 spec lives at
> [`../phase-1-capture-editor/spec.md`](../phase-1-capture-editor/spec.md);
> the in-progress Phase 2 spec (which deferred this effort) lives at
> [`../phase-2-daemon-tray-hotkeys/spec.md`](../phase-2-daemon-tray-hotkeys/spec.md).

## Objective

Close the annotation-tool gap deferred by Phase 1 and Phase 2. After
Phase 1, the editor ships five working tools — **Select, Rect, Arrow,
Text, Blur** — plus undo/redo, color/stroke pickers, and Save/Copy
outputs. The `Tool` and `Annotation` enums already enumerate the
remaining variants (Ellipse, Line, Marker, Pencil, Number) and the
`Canvas` trait already declares `draw_ellipse` / `draw_line`, but the
egui `Canvas` impls are incomplete (e.g. `draw_ellipse` is a no-op at
`crates/flintshot-editor-egui/src/canvas.rs:208`), the editor state
machines for pending/commit are absent, the toolbar exposes only the
five shipped tools, hit-testing doesn't cover the new shapes, and the
Magnifier does not exist at all.

This effort completes each deferred tool as a full vertical slice —
editor input → pending preview → commit → render → selection
hit-test → toolbar entry — and adds the magnifier (toolkit-owned,
reads from `EditorState::image`).

**Target users:** the same power users from
[`VISION.md`](../../VISION.md) §Target users — Linux/Wayland, COSMIC,
cross-platform developers. The Shape/Freehand tools and auto-numbering
are the difference between "I can highlight one thing" and "I can
annotate a complex screenshot the way Flameshot lets me."

**Success looks like:** a user captures a region, switches to Ellipse
via the toolbar, drags an outlined oval, then switches to Marker and
highlights a region with a translucent stroke, clicks the Number tool
three times to place `1`, `2`, `3` bubbles over sequential UI steps,
and switches to Pencil to freehand-sketch — every tool behaves with
snappy pending preview, commits cleanly into the undo history, and
renders identically to the eventual iced/slint shell because all
drawing is routed through `flintshot-core::render::render()`. While
any drawing tool is active, holding the magnifier modifier shows a
zoomed inset of the pixel region under the cursor.

### Why this scope, and what's explicitly out

Phase 1 deferred these five tools and the magnifier to "Phase 2"; the
Phase 2 effort then scoped them out to a dedicated `phase-2b-annotation-
tools` so system integration (daemon, tray, hotkeys, IPC) could ship as
a coherent vertical slice (`docs/specs/phase-2-daemon-tray-hotkeys/
spec.md:80-82`). This is that dedicated effort.

**Out of scope** — deliberately kept tight so the effort stays a single
reviewable vertical slice:

- "UX polish" sweep (cursor crosshair feedback, move/resize of
  *existing* annotations, stroke/color quick-swap palette, tool
  keyboard shortcuts) → its own effort, per Phase 2 spec line 83.
- `cosmic-text` migration (ADR-0017's deferral) → its own effort; this
  effort stays on `ab_glyph`, which renders the Number tool's digits
  fine.
- Live overlay (Phase 5), Pin-to-desktop (Phase 3), Upload (Phase 4).
- Wayland global hotkeys (Phase 6) — unrelated; that's system layer.

## Scope (in / out)

**In:**

- **Ellipse tool** — outlined/filled ellipse inscribed in a drag rect;
  real egui `Canvas::draw_ellipse` impl (replaces current no-op);
  pending preview while dragging; commit on release; toolbar entry;
  hit-test for selection.
- **Line tool** — straight line via two-point drag (rubber-band
  preview); commit on release; toolbar entry; hit-test with sane
  pick tolerance for thin strokes.
- **Marker tool** — semi-transparent thick polyline (alpha-blended);
  point-collection state machine; commit on pointer release; toolbar
  entry; hit-test.
- **Pencil tool** — opaque freehand polyline with point smoothing to
  reduce jaggedness on fast strokes; commit on pointer release;
  toolbar entry; hit-test.
- **Number tool** — click-to-place filled circular bubble containing
  an auto-incrementing integer; uses `EditorState::next_number`;
  toolbar entry; render via existing `draw_rect_filled` (bubble) +
  `draw_text` (digit); hit-test.
- **Magnifier** — egui widget (toolkit-owned, per ARCHITECTURE.md
  §What stays toolkit-specific) showing a zoomed inset of the pixel
  region under the cursor; reads source pixels from
  `EditorState::image`; active while a drawing tool is held and a
  modifier is held (or toggled via config).
- Audit + complete the shared `render()` dispatch and `render_ann()`
  per-variant path so every `Annotation` variant draws correctly
  through the toolkit-agnostic path (some scaffolding exists — e.g.
  the marker segment-line test at `render.rs:1851`; `/planning` pins
  the precise gap per tool).
- Extend selection hit-testing (`EditorState`) to cover the new shape
  variants.
- Toolbar reorganization to fit nine tools cleanly (grouping: Select,
  shapes, freehand, Text, Number, Blur).
- Snapshot/coverage of every new tool in the `MockCanvas` render
  tests and the editor state-machine tests.

**Out (deferred):**

- `cosmic-text` text-rendering migration → own effort (stays
  `ab_glyph` per ADR-0017).
- Move/resize of committed annotations → "polish" effort.
- Tool keyboard shortcuts (R/E/P/…), last-used color palette,
  stroke-width quick-swap → "polish" effort.
- Any new `Backend`/IPC/daemon/tray surface (this effort is pure
  editor: core logic + egui shell).
- New crates or workspace-shape changes — fits in existing
  `flintshot-core` + `flintshot-editor-egui`.

## Deliverables

- [ ] **Ellipse** end-to-end: real egui `Canvas::draw_ellipse` (no
      longer a no-op); editor pending-preview + commit; toolbar
      entry; hit-test; `MockCanvas` test asserts the dispatch +
      egui test asserts a non-empty primitive sequence.
- [ ] **Line** end-to-end: editor rubber-band pending + commit;
      toolbar entry; hit-test with tolerance; `MockCanvas` test.
- [ ] **Marker** end-to-end: point-collection state machine with
      alpha-blended stroke; toolbar entry; hit-test; `MockCanvas`
      test asserts semi-transparent stroke segments.
- [ ] **Pencil** end-to-end: point-collection + smoothing; toolbar
      entry; hit-test; `MockCanvas` test asserts smoothed polyline.
- [ ] **Number** end-to-end: click-to-place bubble, draws
      `next_number` then increments; toolbar entry; hit-test;
      `MockCanvas` test asserts filled circle + digit text.
- [ ] **Magnifier** in `flintshot-editor-egui`: zoomed inset reads
      pixels from `EditorState::image`; triggered by modifier-hold
      (default) with a config toggle; documented as
      toolkit-specific (no core abstraction).
- [ ] Selection hit-testing in `EditorState` covers Ellipse, Line,
      Marker, Pencil, Number.
- [ ] Toolbar in `flintshot-editor-egui/src/app.rs` exposes all nine
      tools with sensible grouping.
- [ ] `no_gui_types_in_core.rs` boundary test stays green; no new
      egui types cross into core (magnifier lives in the egui
      crate, not core).
- [ ] `CHANGELOG.md` `[Unreleased]` entries; an ADR for the
      Marker/Pencil shared freehand state-machine decision if the
      plan converges on sharing (drafted during `/planning`).

## Exit criteria

A user can, in a single editor session over one capture:

1. Switch to **Ellipse**, drag an outlined oval (with optional fill),
   release, and see it committed; select it again by clicking inside
   its bounds; undo with `Ctrl+Z`.
2. Switch to **Line**, drag a straight stroke, release; the line is
   selectable by clicking near it (thin-stroke tolerance).
3. Switch to **Marker**, drag across an area, and see a translucent
   highlight stroke; release commits it.
4. Switch to **Pencil**, freehand-sketch, and get a smooth opaque
   stroke — not a jagged polyline of raw samples.
5. Switch to **Number**, click three times over sequential UI
   elements, and see `1`, `2`, `3` filled bubbles; the count auto-
   increments and is correct in the undo history.
6. While any drawing tool is active, hold the magnifier modifier and
   see a zoomed inset of the pixels under the cursor; release to
   dismiss.
7. Save the result; re-open the saved PNG and every annotation is
   rasterized identically to what the editor showed (because all
   drawing routed through `flintshot-core::render::render()`).

The abstraction holds: the `MockCanvas` (no GPU, no egui) renders
every new variant correctly, and `no_gui_types_in_core.rs` stays
green.

## Technologies

No new crates. All work fits the existing dependency set established
in [`ARCHITECTURE.md`](../../ARCHITECTURE.md) §Tech stack:

| Concern                       | Crate       | Status                                          |
| ----------------------------- | ----------- | ----------------------------------------------- |
| Geometry / drawing primitives | `flintshot-core::render::Canvas` | Trait already declares `draw_ellipse`/`draw_line`; complete the egui impls |
| Freehand smoothing            | `flintshot-core` (pure)          | Smoothing routine (e.g. Catmull-Rom or moving-average) lives in core so output is toolkit-independent; pick during `/planning` |
| Alpha-blended marker stroke   | existing `Color::a`              | Marker uses semi-transparent color; computed in core, emitted via `Stroke` |
| Text shaping for Number digit | `ab_glyph`   | Stays on ab_glyph per ADR-0017; cosmic-text migration is out of scope |
| Magnifier pixel sampling      | `image::RgbaImage` | Egui widget samples `EditorState::image` directly; no new image crate |
| Errors                        | `thiserror` (core) / `color_eyre` (app) | Per existing convention |
| Tests                         | existing `MockCanvas` + `insta`-style assertions | No new test infra |

If `/planning` finds that freehand smoothing would duplicate a mature
crate already in the workspace dep tree, surface it then (ask-first
boundary). Otherwise, a ~30-LOC pure-Rust smoother in core is the
default.

## Commands

All run from the repo root. Full reference in
[`AGENTS.md`](../../../AGENTS.md) §Commands; the standing gate is the
source of truth.

```sh
# Build (default-members: flintshot-app + deps)
cargo build

# The Phase 1 entry point — this effort exercises it heavily
cargo run -- region                    # capture + open editor (test bed for new tools)
cargo run -- fullscreen

# Gate — run in order before declaring done
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

The editor needs a display server on Linux. Headless CI exercises the
new tools through `MockCanvas` unit tests and editor state-machine
tests (no GPU required); `#[ignore]`'d integration tests cover the
visual path under `xvfb`, mirroring Phase 1's capture smoke-test
handling.

## Project structure

No new crates, no workspace-shape changes. Touched files (crate
boundaries from [`ARCHITECTURE.md`](../../ARCHITECTURE.md) hold):

```
crates/flintshot-core/src/
├── editor.rs        # EXTEND — pending/commit state machines for
│                    #          Ellipse, Line, Marker, Pencil, Number;
│                    #          hit-test coverage for the new variants;
│                    #          freehand smoothing helper (or new mod)
├── render.rs        # AUDIT/COMPLETE — render_ann() dispatches every
│                    #          variant; some scaffolding exists (marker
│                    #          segment test at :1851); fill the gaps
└── lib.rs           # re-export any new public helper (smoother?)

crates/flintshot-editor-egui/src/
├── canvas.rs        # EXTEND — real draw_ellipse impl (replaces no-op
│                    #          at :208); verify draw_line; any new
│                    #          primitive impls the trait gains
├── app.rs           # EXTEND — toolbar entries for the five new tools
│                    #          (currently only Select/Rect/Arrow/Text/
│                    #          Blur at :557-561); tool grouping
└── magnifier.rs     # NEW — zoomed-inset widget; reads EditorState::image;
                     #          modifier-hold trigger + config toggle

crates/flintshot-core/tests/
└── no_gui_types_in_core.rs   # UNCHANGED but must stay green
```

Rationale: the new tools are *content*, not new surface area. Every
deferred `Annotation` variant is already declared; this effort wires
behavior into the existing three-file editor substrate (core editor +
core render + egui canvas/app). The magnifier is the only genuinely
new module, and it lives in the egui crate because ARCHITECTURE.md
§What stays toolkit-specific explicitly assigns the magnifier to the
toolkit layer.

## Code style

Unchanged from [`AGENTS.md`](../../../AGENTS.md) §Conventions and the
`.opencode/tech/rust/` docs (loaded at `/build` time). Two shapes
worth previewing — the only surfaces this effort touches:

**Marker vs Pencil** — both are polylines; the spec recommends a shared
freehand state machine in `EditorState` that diverges only on stroke
style (marker emits a semi-transparent `Color`, pencil an opaque one).
`/planning` confirms. Sketch:

```rust
// crates/flintshot-core/src/editor.rs (illustrative — not prescriptive)
// Marker and Pencil share a point-collection pending state; commit
// produces Annotation::Marker { .., color: color.with_alpha(MARKER_ALPHA) }
// or Annotation::Pencil { .., color } depending on Tool.
```

**Number commit** — single click places the bubble at the cursor and
increments `next_number`; no drag state. Undo decrements back.

```rust
// crates/flintshot-core/src/editor.rs
// On Number-tool pointer down: commit Annotation::Number { pos, n:
// state.next_number, color, diameter }; state.next_number += 1; push
// Snapshot so Ctrl+Z restores the prior count.
```

Conventions this effort locks in (all already standing): every
`Annotation` variant is `#[non_exhaustive]`-safe, doc comments on all
public items, no `unwrap()`/`expect()` outside tests, comments only
when the "why" is non-obvious.

## Testing strategy

Mirrors [`ARCHITECTURE.md`](../../ARCHITECTURE.md) §Testing strategy
and the Phase 1 split: heavy headless unit coverage in core (the
`MockCanvas` records drawing commands, asserts the sequence);
toolkit-specific concerns in `flintshot-editor-egui`.

### Unit — `flintshot-core` (headless, no display)

- **`MockCanvas` per tool** — for each new variant, construct an
  `EditorState` with a committed annotation of that variant, run
  `render()`, and assert the recorded primitive sequence (e.g.
  Ellipse emits one `draw_ellipse`; Marker emits N `draw_line`
  segments with alpha < 1.0; Number emits `draw_rect_filled` +
  `draw_text`). The marker test at `render.rs:1851` is the template.
- **Pending-preview parity** — pending Ellipse/Line emits the same
  primitive kinds as the committed form (so WYSIWYG holds).
- **Editor state machine** — per tool: pointer-down → pointer-move →
  pointer-up produces the expected `Annotation` in `state.anns` and
  pushes a `Snapshot`. Freehand tools accumulate points; Number
  commits on down.
- **Hit-testing** — for each new variant, a click inside / on / near
  the shape selects it; a click elsewhere does not. Thin-line
  tolerance is exercised at the boundary.
- **Freehand smoothing** — given a noisy point stream, the smoother
  output is deterministic and reduces point count (or curvature
  variance) — pure-function test, no canvas.
- **Number auto-increment** — three commits produce `n = 1, 2, 3`;
  one undo leaves `next_number == 2` (or whatever the snapshot policy
  decides — see Open Questions).

### Unit — `flintshot-editor-egui` (headless where possible)

- **egui `Canvas` impls non-empty** — `draw_ellipse` (and any other
  completed impl) emits at least one `egui::Painter` primitive;
  guards against the current no-op regression.
- **Magnifier widget** — given an `EditorState` with a known image
  and a cursor position, the widget samples the expected source
  pixels and draws a zoomed inset of the configured factor. Testable
  without a live window via egui's headless `Context`.

### Boundary

- `no_gui_types_in_core.rs` unchanged and green. The magnifier is in
  `flintshot-editor-egui`; nothing new in core references egui.

### Manual test matrix

Tracked in [`docs/TEST_MATRIX.md`](../../TEST_MATRIX.md); Phase 2b
adds rows for each new tool (pending preview, commit, undo, select,
hit-test) and the magnifier (modifier-hold show/hide, zoom factor).

## Boundaries

- **Always:**
  - Run the four-command gate (`fmt`, `clippy`, `test`, doc tests)
    before declaring done.
  - Route every annotation's drawing through
    `flintshot-core::render::render()` — **no direct
    `egui::Painter` calls for annotations** (locked by ADR-0012;
    the `MockCanvas` tests enforce toolkit-independence).
  - Keep the magnifier in `flintshot-editor-egui` — it is
    toolkit-owned per ARCHITECTURE.md §What stays toolkit-specific.
  - Preserve Phase 1/Phase 2 behavior bit-for-bit (the five shipped
    tools, the daemon/tray/hotkeys if landed by then).
  - Push a `Snapshot` for every annotation commit so undo/redo
    covers the new tools uniformly.

- **Ask first:**
  - Adding any workspace dependency not already in the
    ARCHITECTURE.md table (e.g. a freehand-smoothing crate).
  - Changing the `Canvas` trait shape (adding methods beyond
    completing existing ones) — surface during `/planning`.
  - Changing CI config (`.github/workflows/ci.yml`).
  - New ADRs — draft during `/planning`, flag for review.

- **Never:**
  - Introduce `egui` / `libcosmic` / `iced` / `slint` types into
    `flintshot-core`'s public API (boundary test enforces).
  - `unwrap()` / `expect()` outside tests and `const` contexts.
  - Silence a clippy warning with `#[allow]` without a justifying
    comment.
  - Add a "polish" feature (move/resize, shortcuts, palette) under
    this effort's banner — keep the slice honest.

## Success criteria

- All five deferred tools (Ellipse, Line, Marker, Pencil, Number)
  work end-to-end: toolbar → drag/click → pending preview → commit →
  undo → select → hit-test.
- Magnifier shows a correct zoomed inset under the cursor while a
  drawing tool is active and the modifier is held.
- `MockCanvas` render tests cover every `Annotation` variant; output
  is toolkit-independent (an iced/slint shell would rasterize
  identically).
- `no_gui_types_in_core.rs` stays green; no new egui types in core.
- The four-command gate is green on Linux; `cargo build` green on
  macOS and Windows (cross-compile smoke where a native run isn't
  possible in CI).
- No new workspace crates; the effort fits in `flintshot-core` +
  `flintshot-editor-egui`.

## Open questions

1. **Marker vs Pencil: share or split?** Both are polylines. Sharing
   a freehand state machine (diverge only on stroke alpha) is less
   code; splitting keeps each tool's future divergence cheap.
   Recommend share, with the divergence point isolated in one
   function. Confirm during `/planning`.
2. **Number `next_number` undo policy.** Does undo after placing `3`
   restore `next_number` to `2` (snapshot the counter) or leave it at
   `3` (monotonic)? Flameshot resets per-capture. The `Snapshot`
   struct currently captures `anns` (+ optional `image`); extending
   it to capture `next_number` is one field. Recommend snapshot it so
   undo is intuitive. Decide during `/planning`.
3. **Freehand smoothing algorithm.** Catmull-Rom spline, moving
   average, or simple point decimation? Affects perceived quality and
   test determinism. Recommend Catmull-Rom with a configurable tension
   — pure function in core, easy to snapshot-test. Pick during
   `/planning`.
4. **Magnifier trigger UX.** Hold-modifier ( Flameshot-style) vs
   always-on-while-drawing vs config toggle? Recommend hold-modifier
   (default `Space` or a config key) with a config flag to make it
   always-on — matches Flameshot without forcing it. Confirm during
   `/planning`.
5. **Line/Arrow hit-test tolerance.** Thin strokes are hard to click;
   what pixel radius? Recommend `max(stroke_width / 2, 6.0)` physical
   px. Decide during `/planning`.
6. **Ellipse fill UI.** Rect has an optional fill exposed in the
   toolbar; should Ellipse mirror it? Recommend yes, identical UI.
