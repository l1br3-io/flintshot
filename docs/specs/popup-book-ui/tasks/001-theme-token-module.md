# Task 001: Design-token module with light visuals

**Description:** Replace `theme.rs` with a `theme/` module that is
the single style authority: `palette.rs` (every color token from the
spec's table, light-only), `mod.rs` (spacing scale 4/8/12/16/24,
pod/chip rounding, 8px cutout stroke, two-level shadow tokens, type
ramp constants, `apply(ctx)` pinning `ThemePreference::Light` and
installing cream/forest `Visuals` + `Style` with ~30–50% larger
padding than today). Migrate every existing token consumer
(`toolbar.rs`, `chrome.rs`, `app.rs`, `home.rs`, `overlay.rs`) off
graphite/ember onto the new tokens. App-wide re-skin in one move;
layout structure unchanged in this task.

**Acceptance criteria:**
- [ ] `theme::apply` pins light theme; every window (home, editor,
      overlay) renders on cream paper with forest accents
- [ ] Spacing/rounding/stroke/shadow/type-ramp tokens defined once in
      `theme/` and consumed; padding measurably larger than the
      current `7×3` button padding / `3` pod pad
- [ ] New token unit tests (palette distinctness incl. `FOREST_DEEP`
      vs neutrals, spacing scale exactly {4,8,12,16,24}, shadow
      levels) replace the old theme.rs suite; old graphite/ember
      tokens and their tests are gone
- [ ] Annotation swatch set re-anchored on the spec palette
      (berry/fox/gold/forest/sky/white/ink), same seven-slot shape

**Verification:**
- [ ] `cargo test --workspace --all-targets` and doc tests green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] Manual: `cargo run -- app` — home + editor pages re-skinned,
      no graphite remnants; overlay (`capture` flow) consistent

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-editor-egui/src/theme.rs` (deleted)
- `crates/flintshot-editor-egui/src/theme/mod.rs` (new)
- `crates/flintshot-editor-egui/src/theme/palette.rs` (new)
- `crates/flintshot-editor-egui/src/{toolbar,chrome,app,home,overlay}.rs`
  (token reference migration)
- `crates/flintshot-editor-egui/src/lib.rs` (module docs)

**Docs:**
- The "EMBER appears in exactly four places" contract in the old
  theme header dies here; note the replacement roles (forest=active,
  gold=hover-lift) in `palette.rs` docs.

**Estimated scope:** Large (5–7 files, mostly mechanical migration)
