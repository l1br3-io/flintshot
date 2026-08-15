# ADR-0027: Painter-drawn toolbar glyphs over icon fonts or emoji

**Context.** The Phase 1/4 egui toolbar labelled its buttons with
emoji (`🖍 Marker`, `☁ Upload`, `📋 Copy`, `📌 Pin`, `💾 Save`) and
Unicode pictographs (`⬚`, `▭`, `◯`, `░`, `✂`, `↵`). Three problems
compounded:

1. **Font roulette.** egui bundles no color-emoji font; whether 🖍
   renders as a crayon or a tofu box depends on the system font
   fallback chain (Noto Color Emoji present or not, fontconfig
   version, portal-snapped Flatpak environments). Screenshots of the
   tool from a bare container show rows of boxes.
2. **Inconsistent weight.** Where emoji *do* render, they carry each
   platform's emoji style (Apple color, Noto flat, Samsung gradients)
   — no shared stroke weight, no shared color, visually unrelated to
   the annotations the tools draw.
3. **Dependency temptation.** The standard fix is an icon font
   (e.g. `egui-phosphor`). That adds a crate + a megabyte of
   font atlas for ~15 glyphs, in a project whose vision explicitly
   budgets a <25 MB static binary and prefers few, load-bearing
   dependencies.

**Decision.** The toolbar's glyphs are drawn by `egui::Painter` calls
inside the editor crate (`toolbar.rs`): each icon is a small fn
(`fn(&Painter, Rect, Color32)`) composing 2px-stroke line segments,
circles, and (for the undo/redo arcs) short sampled polylines, laid
out in a ±7-unit glyph space centered on its chip. Two glyphs use
`Painter::text` with the bundled proportional face ("T" for Text,
"1" for Number) because they depict *characters*, which the bundled
font always has.

This ties the toolbar's visual language to the product's own
subject: the icons are drawn with the same 2px stroke grammar the
tools leave on the screenshot, they inherit the theme's chalk /
slate / ember colors at paint time, and they render identically on
every platform because no font is consulted.

**Consequences.**

- Zero new dependencies; the glyph fns are ~150 lines total and
  unit-testable in principle (they are pure Painter calls, so the
  existing headless test story is unchanged).
- Adding a chip icon means writing one small fn — the cost that an
  icon font amortizes away is accepted deliberately, in exchange for
  the render-anywhere guarantee and the stroke-language consistency.
- Glyph quality is bounded by what 2px strokes at 28 px can express.
  If a future control genuinely needs a complex pictogram (e.g. a
  layers stack), the escape hatch is a tiny embedded SVG-like path
  table in `toolbar.rs` — still painter-drawn, still font-free — not
  an icon-font dependency.
- Tooltips carry the text names ("Rectangle · R"), so glyph
  legibility is a recognition aid, not the only channel.
