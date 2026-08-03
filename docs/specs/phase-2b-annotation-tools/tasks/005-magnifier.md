# Task 005: Magnifier widget (egui-only, reads `EditorState::image`)

**Description:** Add the magnifier — a toolkit-owned zoomed inset
showing the pixel region under the cursor, sampled from the source
screenshot in `EditorState::image`. Per ARCHITECTURE.md §What stays
toolkit-specific, the magnifier gets **no core abstraction**; it lives
entirely in `flintshot-editor-egui` (new `magnifier.rs` module).
Triggered by holding a modifier key (default `Space`, configurable),
with a config toggle `[ui] magnifier_always_on` (default false) for
always-on-while-drawing. The pixel-sampling math is extracted into a
pure helper so it is testable without a live egui window.

**Acceptance criteria:**
- [ ] New module `crates/flintshot-editor-egui/src/magnifier.rs` with:
      - a pure helper `sample_zoomed_region(image, cursor_phys, zoom, inset_px) -> ...`
        (returns the source rect to sample / the pixels) — no egui
        types in its signature so it is unit-testable headless;
      - an egui widget that paints the zoomed inset (e.g. at a screen
        corner or offset from the cursor) using the sampled pixels,
        uploaded as a one-frame texture (mirroring how `draw_image_region`
        handles transient blur textures at `canvas.rs:162-178`).
- [ ] The editor app (`app.rs`) tracks the canvas-physical cursor
      position (already available via egui interaction) and passes it
      + `state.image` to the magnifier when the modifier is held (or
      `always_on` is set).
- [ ] Modifier-hold shows the inset; release hides it. Default
      modifier: `Space`. Configurable via `[ui] magnifier_modifier`
      (string) and `[ui] magnifier_always_on` (bool, default false).
- [ ] Config schema: add `magnifier_modifier` and `magnifier_always_on`
      to the `[ui]` section of `flintshot-core::config` (`#[serde(default)]`,
      `#[serde(rename_all = "kebab-case")]` per convention).
- [ ] **No new types in `flintshot-core`** beyond the two config
      fields. The magnifier reads `EditorState::image` directly;
      `no_gui_types_in_core.rs` stays green.
- [ ] Headless unit test for the pure `sample_zoomed_region` helper:
      given a known image + cursor + zoom, the returned region is the
      expected sub-rect (and clamps at image edges — no out-of-bounds).
- [ ] The inset is bounded: `zoom * inset_size` sampling work per
      frame is capped (e.g. 4× zoom over a 96×96 inset = 24×24 source
      region); cache the inset texture across frames while held.

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets` — incl. the pure
      sampling-helper unit test (clamp + bounds).
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual: `cargo run -- region`, activate a drawing tool, hold
      `Space` — zoomed inset appears under/following the cursor and
      shows source pixels at the configured zoom; release dismisses.
      Set `magnifier_always_on = true` in config, restart, confirm it
      stays on without the modifier.

**Dependencies:** None (independent of Tasks 001-004; pure egui widget
reading an existing `EditorState` field). Ordered in Phase 2b.D for the
checkpoint — could run in parallel with Phase 2b.B/C if multiple agents
are available.

**Files likely touched:**
- `crates/flintshot-editor-egui/src/magnifier.rs` (NEW) — sampling
  helper + egui widget.
- `crates/flintshot-editor-egui/src/lib.rs` — `mod magnifier;` +
  re-export.
- `crates/flintshot-editor-egui/src/app.rs` — wire cursor tracking +
  modifier detection + render the magnifier each frame.
- `crates/flintshot-core/src/config.rs` — `magnifier_modifier` +
  `magnifier_always_on` fields in `[ui]` (with `#[serde(default)]`).

**Docs:**
- Module doc on `magnifier.rs` explaining it is toolkit-owned per
  ARCHITECTURE.md §What stays toolkit-specific (the "why" — no core
  abstraction by design; an iced/slint shell implements its own).
- `// GOTCHA:` comment if the per-frame texture upload has a perf
  cliff (and the mitigation — caching while held).
- No new ADR — the toolkit-ownership is already documented in
  ARCHITECTURE.md; record the trigger-UX default (hold `Space`) in
  `docs/TEST_MATRIX.md` during Task 006.

**Estimated scope:** Medium (4 files; the widget + sampling helper
are the substantive work, config fields are small).
