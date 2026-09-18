# ADR-0059: Painted vector glyphs, never font-fallback icons

**Context.** The five-zone redesign rendered topbar icons as
Unicode symbol characters (`⋯` U+22EF, `⧉` U+29C9, `▸` U+25B8,
`⤓` U+2913, `⇧` U+21E7) through the embedded font chain. A cmap
scan of every embedded face (Plus Jakarta Sans, Ubuntu-Light,
`NotoEmoji`, emoji-icon-font) proved those codepoints are in
**none of them** — the icons rendered as tofu boxes. The reference
mockups lied: browsers fall back across every font on the host;
an embedded-chain app has no such net. Meanwhile the tool rail
had already solved this with painted vector `Glyph` fns
(`fn(&Painter, Rect, Color32)` over a ±7 glyph space).

**Decision.**

- Every icon the topbar class of UI renders is a painted vector
  glyph following the rail pattern — capture, copy, open, more,
  save, upload, paste, about, settings, check, chevron. Shared
  glyphs live once (`panel::g_copy` was promoted rather than
  duplicated).
- A **tofu-ban scan test** (mirroring the `Color32` token scan)
  fails the build if any cmap-verified-absent codepoint re-enters
  `flintshot-editor-egui`'s source — including comments. The
  needle list is assembled from codepoints at runtime so the
  test's own source cannot trip it; the list is re-derivable from
  a cmap check of the actual font files if the chain changes.
- Emoji-fallback codepoints are tolerated only where they are
  demonstrably rendered by the emoji faces (the flame badge, the
  home hcards) — they are not icon infrastructure.

**Consequences.**

- Icons render identically on every host, zoom, and ppp — no
  dependence on font fallback, no tofu roulette.
- New icon needs start with a glyph fn (and a paint test pinning
  its bounds), not a character pick; adding a symbol char instead
  fails the scan only if the codepoint is already banned, so code
  review remains the first line for novel codepoints.
- The ban list is evidence-based, frozen to the current chain; a
  font-chain change requires re-running the cmap scan and
  revising the list (the test's doc records the method).
