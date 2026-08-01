# ADR-0021: Text-tool customization data model + Phase 2 multiline pulled forward

**Context.** ADR-0017 shipped the Phase 1 text tool as single-line only
(`Shift+Enter` deferred to Phase 2) with just `font_size` and a shared
`color`. The two render backends (egui live view + `ImageCanvas` save
path) used different font stacks and a `Canvas::draw_text(pos, text,
size, color)` signature with no headroom for richer styling.

A round of text-tool work asked for: a box indicator, `Shift+Enter`
multi-line, decoupled text colour, font family/weight/style, alignment,
background highlight, and outline. Each new property has to flow through
**both** render paths (ADR-0015 WYSIWYG) and through `flintshot-core`'s
public data model without leaking GUI types (`no_gui_types_in_core`
guard).

**Decision.** Expand the data model once and plumb a style bundle
through the render trait, rather than widening `Canvas::draw_text`'s
positional argument list per property:

- New core module `flintshot-core::text` owns toolkit-agnostic enums
  (`FontFamily`, `FontWeight`, `FontStyle`, `TextAlign`), a
  `TextOutline` struct, a `TextStyle` bundle (the full paint spec), and
  `TextMetrics` (width / height / last-line width). None of these
  reference egui or `ab_glyph`; the `no_gui_types_in_core` guard stays
  green.
- `Annotation::Text` gained `family`, `weight`, `style`, `align`,
  `line_spacing`, `outline`, `background` — every new field carries
  `#[serde(default)]` so existing undo snapshots / saves still decode.
  `Annotation::text_style()` / `Annotation::text()` are inverses (a
  round-trip test pins this).
- `Canvas::draw_text` now takes `&TextStyle`; `Canvas::measure_text`
  returns `TextMetrics`. One trait change covers every later property —
  backends gain behaviour without the signature moving again.
- Multi-line (`Shift+Enter`) is pulled forward from Phase 2: the
  dispatcher inserts `\n` on `Enter` + `modifiers.shift`; both backends
  split on `\n` and advance the baseline by `size * line_spacing`.
  `bounding_box` sums line heights.
- Bold is **real** on the save path (`fontdb` weight query via a
  `(family, weight, style)`-keyed font cache) but **faux** on the egui
  view, because egui 0.35's `RichText::strong()` only changes colour and
  `FontId` carries no weight. Faux = overdrawing the galley at a
  sub-pixel x offset. Outline is faux on both (offset redraws in the
  outline colour, then the fill) so no stroke rasteriser is needed.
- Serif `FontFamily` exists in the model but maps to egui's
  `Proportional` and fontdb's `SansSerif` until a serif face is bundled —
  offering it in the UI before it renders would mislead. Sans + Mono are
  the two wired families for now.
- A solid block caret + hollow bounding box replace the thin caret bar
  (live-view-only; never saved). No blink timer — the indicator is solid.

**Consequences.**

- One `Canvas` signature change (Phase 0) amortises across all text
  properties; adding a future property is a `TextStyle` field + backend
  reads, not a trait break.
- WYSIWYG is exact for family / italic / alignment / background /
  outline, and **approximate for bold** (real face vs faux). The
  difference is a slightly thinner egui glyph edge; documented here and
  in the CHANGELOG. A future egui upgrade that exposes weight (or
  bundling the bold face into `FontDefinitions`) removes the gap without
  touching the data model.
- `flintshot-core` gains no new direct dep (`ab_glyph` / `fontdb` were
  already present); the font cache just keys on a triple instead of one
  default.
- Serif remains a follow-up: bundle a serif font (or register a system
  one) and wire `FontFamily::Serif` → the chosen face in both backends.
- This supersedes ADR-0017's "multi-line deferred to Phase 2" note;
  ADR-0017 is left untouched per the never-edit-an-ADR rule.
