# ADR-0017: `ab_glyph` over `cosmic-text` for Phase 1 text tool

**Context.** Phase 1 Task 012's original spec named `cosmic-text` as the
text-shaping pipeline (it is already in the dep tree transitively via
`libcosmic`). The plan flagged this as the highest-risk task:
`cosmic-text` brings a non-trivial font-system + shaping pipeline, and
the Phase 1 architecture rule ("no GUI-toolkit types in
`flintshot-core`'s public API") requires that `cosmic_text` types not
leak into core either. The plan also pre-authorised a fallback: "if it
slips, ship Phase 1 with `ab_glyph` fallback and ADR the swap."

Two render paths must agree (ADR-0015 WYSIWYG): the live egui view and
the headless `ImageCanvas` used by `compose_final` (Save / Copy). Each
needs to rasterise glyphs:

- `EguiCanvas` — egui already ships `ab_glyph` internally and exposes a
  native `Painter::text` API; no new dep needed there.
- `ImageCanvas` (in `flintshot-core`) — needs a direct rasterisation
  path. `cosmic-text` would have added a font-system, shaping buffer,
  and swash rasteriser to core, plus a wrapper layer to keep its types
  out of the public API.

**Decision.** Phase 1 ships `ab_glyph` + `fontdb` as the text-rendering
stack, not `cosmic-text`:

- `flintshot-core` depends on `ab_glyph` (glyph outline + coverage
  rasterisation) and `fontdb` (system fontconfig discovery, queried once
  and cached in a `OnceLock`).
- `EguiCanvas::draw_text` uses egui's native `Painter::text` (which is
  itself `ab_glyph`-backed) — no new egui-side dep.
- `ImageCanvas::draw_text` rasterises via `ab_glyph` directly, with
  source-over alpha compositing over the existing pixels.
- Multi-line text (`Shift+Enter`) is deferred to Phase 2; Phase 1 ships
  single-line labels only (sufficient for the exit criteria — "add a
  text label").
- Font source is system fontconfig (`fontdb::Database::load_system_fonts`
  with a sans-serif query). Missing fonts warn + skip the annotation
  rather than panicking. A bundled-font feature flag stays deferred
  until CI demonstrates a need (plan Q2 recommendation).

`cosmic-text` remains the planned Phase 2 upgrade for richer shaping
(bidi, complex scripts, wrapping) once the editor's text UX demands it.

**Consequences.**

- `flintshot-core` gains two new direct deps (`ab_glyph`, `fontdb`).
  Both were already in `Cargo.lock` transitively, so build time and
  binary size impact is negligible.
- WYSIWYG is *approximate*, not pixel-exact: egui's font selection
  (its bundled default + system fonts via `FontDefinitions`) may differ
  from `fontdb`'s sans-serif query result. Both produce readable glyphs
  at the correct anchor; fine-grained metric differences (±1px on glyph
  edges, slightly different fallback faces) are accepted for Phase 1.
- The `no_gui_types_in_core` test does **not** need extending to
  `cosmic_text`, because `cosmic_text` is no longer in the picture.
  If Phase 2 adopts `cosmic-text`, the guard must be extended then.
- Multi-line annotations are not supported in Phase 1; the
  `Annotation::Text { text: String }` field can hold `\n` but the
  Phase 1 shell never inserts one and `ImageCanvas::draw_text`
  rasterises on a single baseline.
- Documented in `CHANGELOG.md` under Task 012 and in the task's
  acceptance-criteria notes.
