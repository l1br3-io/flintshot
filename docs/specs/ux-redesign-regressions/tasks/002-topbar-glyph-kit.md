# Task 002: Topbar glyph kit — painted vector icons

**Description:** The topbar renders icons as Unicode text through the
Jakarta-first font chain; `⋯ ⧉ ▸ ⤓ ⇧` are tofu in every embedded font
(cmap-verified — see spec §3). Build the painted-glyph replacements
following rail/panel precedent (`Glyph = fn(&Painter, Rect, Color32)`
with rail's `pt`/`seg`/`arrowhead` helpers, all already `pub(crate)`).
This task creates the glyph fns and their paint tests; Task 3 wires
them into the topbar. Needed glyphs: capture (plus in a ring), copy
(two overlapping rounded rects — promote `panel::g_copy` to
`pub(crate)` rather than duplicating), open (folder), more (three
centered dots), save (down arrow into a tray), upload (up arrow),
paste (clipboard), about (i), settings (gear or cog-dot), check
(reuse `panel::g_check`, already `pub(crate)`), breadcrumb chevron
(small right-pointing triangle).

**Acceptance criteria:**
- [ ] Every listed glyph exists as a `Glyph` fn usable by the topbar
      (local to `topbar.rs` or promoted from `panel.rs`; no
      duplication of `g_copy`/`g_check` logic)
- [ ] Glyphs paint only with palette tokens (no `Color32::from_*` —
      the existing token-scan test enforces this for free)
- [ ] Headless paint test: each glyph fn, given a 16-28 px rect on a
      test painter, produces at least one clipped shape whose bounds
      stay inside (or stroke-adjacent to) the rect — no zero-size or
      runaway geometry

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` (quiet) — new glyph tests green, token scan still green
- [ ] `cargo fmt --all` && `cargo clippy --workspace --all-targets -- -D warnings`

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-editor-egui/src/topbar.rs` (glyph fns + tests)
- `crates/flintshot-editor-egui/src/panel.rs` (promote `g_copy` to `pub(crate)`)

**Docs:**
- A short comment at the glyph block citing the cmap finding (why
  painted, never text): the font chain lacks U+22EF U+29C9 U+25B8
  U+2913 U+21E7.

**Estimated scope:** Small: 1-2 files

**Git Commit:** `git commit -m "feat(topbar): painted glyph kit — vector icons replace font-chain text glyphs"`
