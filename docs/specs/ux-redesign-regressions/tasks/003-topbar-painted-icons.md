# Task 003: Topbar consumes painted glyphs + tofu-ban scan

**Description:** Wire the Task-2 glyph kit into every topbar text-icon
site: the three hero buttons (`ghost_button` signature changes from
`icon: &str` to `glyph: Glyph`), the `⋯` overflow chip, the overflow
rows (`om_row` icon param), the about row, and the breadcrumb's `▸`
separator (painted chevron). Add the tofu-ban scan test (mirrors the
`Color32` token scan in `theme/mod.rs`): the known-tofu characters
must not appear anywhere in `crates/flintshot-editor-egui/src` — icons
are painted glyphs, full stop. The scan's needle list is assembled at
runtime so the test's own source doesn't trip it; the list derives
from the cmap evidence documented in the spec (U+22EF `⋯`, U+29C9 `⧉`,
U+25B8 `▸`, U+2913 `⤓`, U+21E7 `⇧`, U+2713 `✓`). Consumers today:
`topbar.rs` (heroes, overflow, breadcrumb) and `widgets.rs` tests
(update them to glyph fixtures). The flame badge 🔥 and home hcard
icons (✚ 🗀 📋) render via the emoji fallbacks and stay out of scope.

**Acceptance criteria:**
- [ ] `ghost_button` takes a `Glyph` fn; the only call sites (topbar
      heroes, widgets tests) pass glyphs; all existing ghost_button
      behavior tests still pass
- [ ] The overflow chip paints three dots (no `⋯` text); overflow rows
      (Save/Upload/Paste/About/Settings) and the breadcrumb separator
      paint glyphs
- [ ] Tofu-ban scan test green: none of the banned codepoints appear
      in any `.rs` under `crates/flintshot-editor-egui/src`
- [ ] Topbar layout tests (`topbar_is_a_44px_top_strip`,
      `heroes_and_overflow_register_ai_stays_gone`, etc.) unchanged
      and green

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` (quiet)
- [ ] `cargo fmt --all` && `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: `cargo run -- app` — every topbar icon renders on Home
      and Editor routes; side-by-side with
      `docs/specs/ux-redesign/reference/zone-map.html`

**Dependencies:** Task 2

**Files likely touched:**
- `crates/flintshot-editor-egui/src/topbar.rs` (hero/overflow/breadcrumb consumption + scan test)
- `crates/flintshot-editor-egui/src/widgets.rs` (ghost_button signature + tests)

**Docs:**
- The tofu-ban test comment documents the cmap method so the char list
  can be re-derived if fonts change.

**Estimated scope:** Medium: 2 files

**Git Commit:** `git commit -m "fix(topbar): painted icons everywhere + tofu-ban scan — no more font-chain glyph roulette"`
