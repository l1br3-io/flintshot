# ADR-0022: Shared Marker/Pencil freehand path — Catmull-Rom smoothing in core + per-pixel-once polyline compositing

**Context.** Phase 2b deferred the two freehand tools — `Marker` and
`Pencil`. Both were already scaffolded end-to-end: `Annotation::Marker`
/ `Annotation::Pencil` carry a `Vec<Point>` polyline, `render_ann`
emitted consecutive `draw_line` calls over `points.windows(2)`, and
`Annotation::contains` hit-tested by walking segments. Three gaps
remained for the exit criterion (*"a translucent Marker highlight, text
visible through it, and a smoothed Pencil sketch"*).

1. **No smoothing.** `update_annotation` pushed raw pointer samples,
   so fast strokes rendered as jagged polylines.
2. **Marker was opaque.** `begin_annotation` stored full-opacity
   `self.color` for Marker, making it indistinguishable from Pencil —
   no highlight semantics.
3. **Stamp compounding.** `ImageCanvas::stroke_segment` (the save/copy
   rasterizer) stamps a filled square per pixel-of-length. Adjacent
   stamps overlap by ~`width`, so each pixel along a thick stroke is
   blended ~`width` times. For an opaque stroke this is invisible
   (replace is idempotent). For a translucent marker it compounds:
   alpha `0x66` at width 8 reaches ≈ 98% opacity in the saved PNG.
   After Catmull-Rom subdivision (which multiplies the point count, so
   consecutive segments overlap almost entirely) it is worse. The
   "text visible through it" exit criterion is unmet, and WYSIWYG with
   egui breaks (egui's per-segment `line_segment` also compounds at
   joints, but far less than the save path's full-square overlap).

**Decision.**

- **Share, don't split.** Marker and Pencil already share the polyline
  render path and the hit-test path; they diverge on exactly one thing
  — stroke alpha. New pure module `flintshot-core::freehand` owns the
  shared concern (smoothing + the marker alpha constant) so neither
  `editor.rs` nor `render.rs` grows a marker-specific branch.
- **Catmull-Rom, pure fn, applied at render time.**
  `smooth_polyline(points, tension) -> Vec<Point>` is a pure function
  in core. `tension` scales the cardinal tangents (`0.5` = classic
  Catmull-Rom, the default). Phantom endpoints (mirrored about the
  first/last real point) let 2- and 3-point strokes smooth the same
  way as longer ones. It is called inside `render_ann` — the single
  chokepoint both the pending preview and committed annotations pass
  through — so WYSIWYG between the live editor and saved/copied output
  holds by construction, not by parity testing. (The per-frame cost on
  the pending preview is O(n) in the raw sample count with a bounded
  per-span subdivision; negligible for screenshot-scale strokes.)
- **Marker alpha at construction, not at render.**
  `begin_annotation` stores `self.color.with_alpha(MARKER_ALPHA)` on
  the `Annotation::Marker.color` field. The render path receives a
  color whose alpha is already reduced — **no render-path
  special-casing** — so every backend (egui, `ImageCanvas`,
  `MockCanvas`) renders the marker translucently through the unchanged
  polyline path. Pencil keeps the user's full-opacity color; this one
  line is the entire Marker/Pencil divergence.
- **New `Canvas::draw_polyline` method, per-pixel-once compositing.**
  Because the per-segment `draw_line` loop fundamentally cannot avoid
  stamp compounding for translucent strokes (no segment knows the
  whole-stroke boundary), the trait gains
  `Canvas::draw_polyline(points, stroke)`. It carries a default impl
  that loops `draw_line` (so a future minimal backend still works), but
  the two real backends override:
  - `ImageCanvas` unions the per-segment stamp squares into a `HashSet`
    and source-over blends each covered pixel **exactly once**
    (`stamp_polyline`). Opaque strokes are byte-identical to the old
    per-segment stamping (source-over at α=1 is a hard replace); only
    the translucent case changes, and only toward correctness.
  - `EguiCanvas` issues one `PathShape::line` — egui tessellates the
    whole stroke into a single strip, so each pixel is covered once
    (modulo the AA fringe) and the marker stays uniformly translucent
    on screen too.
  `render_ann`'s Marker/Pencil arm now smooths then calls
  `canvas.draw_polyline`. Line / Arrow still use `draw_line` (single
  segments, no compounding concern). `MockCanvas` records a
  `DrawPolyline { points, stroke }` so render tests assert one polyline
  call, not N line calls.

**Consequences.**

- The `Canvas` trait surface grows by one method. This is the first
  addition since ADR-0012/ADR-0021; it is justified by a real
  correctness gap (translucent-stroke compounding) the per-segment API
  cannot express. Future freehand-related behaviour (variable-width
  pressure, eraser) has a natural home in `draw_polyline` overrides
  rather than widening `draw_line`.
- WYSIWYG is exact for Pencil (opaque, identical coverage to the old
  path) and **near-exact for Marker**: egui's `PathShape::line` and
  `ImageCanvas`'s square-stamp union use different cap/join geometry,
  so a translucent marker's silhouette differs by ±1 px between the
  live view and the saved PNG at stroke ends/corners. The interior
  translucency (the property the exit criterion names) matches.
- `smooth_polyline` is deterministic and tension-parameterized. The
  default tension is a `const` in `render.rs` (`FREEHAND_TENSION`); a
  future config knob can expose it without touching the data model.
- If Catmull-Rom shows overshoot artifacts on sharp corners during
  manual testing, the fallback is a tension clamp (already supported —
  lower tension flattens the curve) or a switch to centripetal
  Catmull-Rom in `freehand.rs` alone; the render path and trait are
  unaffected.
