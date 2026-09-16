# Task 001: Palette + token refresh to the reference of record

**Description:** Sweep `theme/palette.rs` + `theme/mod.rs` to the
mockup's design system: 14 refreshed/new color tokens (spec §Color
tokens), the six-step type ramp (10.5/12/13.5/16/21/30), cutout/pod
radii, and **four** shadow levels (resting/lift/pop/paper). All
existing consumers move to the new values mechanically; nothing else
changes visually yet beyond color/size drift.

**Acceptance criteria:**
- [ ] `palette.rs` holds exactly the spec's token table (old v1 gold
      `#FFB347`, `#2D7A3E` forest-mid, `#FFF9F0` cream etc. retired);
      no `Color32` literal appears outside `palette.rs`.
- [ ] Type ramp constants = the six sizes; display family weight map
      unchanged (Plus Jakarta Sans).
- [ ] `SHADOW_RESTING` / `SHADOW_LIFT` / `SHADOW_POP` / `SHADOW_PAPER`
      defined; two-level consumers updated (pop for active tool/menus,
      paper for window/hero surfaces).
- [ ] Existing palette tests updated: distinctness, ramp ordering,
      spacing scale {4,8,12,16,24} unchanged; token-scan test green.
- [ ] Full gate green — the app still builds and runs (colors visibly
      refreshed everywhere).

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: `cargo run -- app` — cream/gold/forest surfaces read as
      the mockup palette; no leftover ember/v1-gold anywhere

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-editor-egui/src/theme/palette.rs`
- `crates/flintshot-editor-egui/src/theme/mod.rs`

**Estimated scope:** Medium (2 files + mechanical consumer sweep)
