# Task 002: Shared cutout widgets module

**Description:** New `widgets.rs` — the reusable cutout controls the
mockup uses everywhere: `ghost_button` (icon+label, normal / `HeroKey`
gold-border / disabled variants), `well_row` (segmented value picks),
`switch` (pill toggle, on/off/locked), `section` (collapsible card:
chevron rotate + spring height), `kbd_chip`. All consume Task 1's
tokens; spring microinteractions go through `theme::ease` behind a
single gate function so Task 11's `ui.animations` setting can switch
them off in one place.

**Acceptance criteria:**
- [ ] Each widget renders headlessly (existing `Context` test
      pattern); enable/disable and variant states asserted.
- [ ] Section: open/closed persists across frames, chevron rotates,
      body height animates with the spring; state stored by caller id.
- [ ] Switch: on = gold + knob translated; locked = 55% opacity,
      non-interactive.
- [ ] Animation gate: a single `widgets::animate(ctx) -> bool`
      (default true) — all spring transitions no-op when false.
- [ ] No `Color32` literals; doc comments on public items.

**Verification:**
- [ ] `cargo test --workspace --all-targets` (new widget tests)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: widgets not yet wired — verified via unit tests + a
      temporary scratch render if needed

**Dependencies:** Task 1

**Files likely touched:**
- `crates/flintshot-editor-egui/src/widgets.rs` (new)
- `crates/flintshot-editor-egui/src/lib.rs`

**Estimated scope:** Medium (2 files)
