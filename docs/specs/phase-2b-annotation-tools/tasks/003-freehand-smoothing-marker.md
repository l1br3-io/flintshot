# Task 003: Freehand smoother (Catmull-Rom) + Marker translucency

**Description:** Pencil and Marker share a polyline render path
(`render_ann` emits consecutive `draw_line` segments over
`points.windows(2)`, `render.rs:261-275`) and a hit-test path
(`contains` walks segments with stroke-width + tolerance,
`editor.rs:1190-1195`). Two gaps remain for both:

1. **No smoothing.** `update_annotation` pushes raw pointer samples
   (`editor.rs:411`), so fast strokes render as jagged polylines.
2. **Marker is not translucent.** `begin_annotation` stores
   full-opacity `self.color` for Marker (`editor.rs:355`), making it
   indistinguishable from Pencil — the "highlight" semantics from
   VISION.md / the spec are missing.

Add a pure-Rust Catmull-Rom freehand smoother in core (applied to both
the pending preview and the committed polyline so WYSIWYG holds), make
Marker translucent by reducing alpha at `begin_annotation` time, and
add toolbar entries for both tools.

This is the spine task of the freehand tools — smoothing is the real
work; the Marker alpha change and toolbar entries ride along.

**Acceptance criteria:**
- [ ] New pure fn `smooth_polyline(points: &[Point], tension: f32) -> Vec<Point>`
      in `flintshot-core` (suggested home: a new `freehand.rs` module,
      or fold into `editor.rs` / `render.rs` — pick the cleaner one and
      re-export from `lib.rs`). Catmull-Rom; handles `< 4` points
      gracefully (degenerate to the input or a 2-3-point fallback).
- [ ] Smoother is deterministic: a fixed noisy input yields a fixed
      output (unit test with a hand-checked or snapshot-style
      assertion on point count / curvature reduction).
- [ ] `render_ann`'s Marker/Pencil arm smooths the polyline before
      emitting `draw_line` segments (or smoothing is applied at
      `update_annotation` + commit — pick the option that preserves
      WYSIWYG between pending preview and committed form; document the
      choice in ADR-0022).
- [ ] Marker `begin_annotation` stores a translucent color (alpha
      reduced from 255 to ~`0x66` or a configurable
      `MARKER_ALPHA` const); the existing `Annotation::Marker.color`
      field carries it — no render-path special-casing.
- [ ] Marker renders semi-transparently in egui AND `ImageCanvas`:
      `compose_final` pixel test asserts that a committed marker over a
      known background produces blended pixels (resulting alpha / color
      between background and marker color).
- [ ] Toolbar exposes "Marker" and "Pencil" entries; both drag-commit;
      `Ctrl+Z` restores.
- [ ] Existing `render_pending_marker_emits_segment_lines` test at
      `render.rs:1851` updated: smoothing may change the segment count
      (assert the new contract, e.g. `>= 2` segments for a 3-point
      input, or the smoothed count for the fixed input).

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets` — incl. smoother
      determinism test, marker-alpha `compose_final` pixel test, and
      the updated marker render test.
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual: `cargo run -- region`, draw a fast squiggle with Pencil
      (should be smooth, not jagged); draw a Marker stroke over text
      (should be a translucent highlight, text visible through it);
      undo both.

**Dependencies:** None (independent of Tasks 001/002; the smoother is
pure core logic, marker alpha is one line, toolbar entries are local).
Ordered in Phase 2b.B for the checkpoint grouping.

**Files likely touched:**
- `crates/flintshot-core/src/freehand.rs` (NEW) **or**
  `crates/flintshot-core/src/editor.rs` — `smooth_polyline` impl +
  `MARKER_ALPHA` const.
- `crates/flintshot-core/src/lib.rs` — re-export `smooth_polyline` if
  it lives in a new module.
- `crates/flintshot-core/src/editor.rs` — Marker alpha at
  `begin_annotation` (`:353-357`).
- `crates/flintshot-core/src/render.rs` — smoothing call in the
  Marker/Pencil arm of `render_ann` (`:261-275`); updated marker test
  (`:1851`); marker-alpha `compose_final` pixel test.
- `crates/flintshot-editor-egui/src/app.rs` — Marker + Pencil toolbar
  entries.

**Docs:**
- **ADR-0022** (NEW, `docs/adrs/`): shared Marker/Pencil freehand state
  machine + Catmull-Rom smoothing in core. Records: why share (they
  already share render + hit-test), why Catmull-Rom (smooth, tunable,
  pure-Rust, no new deps), where smoothing is applied (pending +
  committed for WYSIWYG), and the marker-alpha-at-construction choice.
  Context · Decision · Consequences format.
- If Catmull-Rom overshoots on sharp corners during implementation,
  record the fallback (monotone interpolation or tension clamp) in the
  same ADR rather than a separate one.
- `// GOTCHA:` comment if the smoothing-apply-site choice has a subtle
  implication (e.g. pending smoothing re-runs every frame — note perf
  if relevant).

**Estimated scope:** Medium (4-5 files; the smoother + its tests are
the substantive work; marker alpha and toolbar entries are small).
