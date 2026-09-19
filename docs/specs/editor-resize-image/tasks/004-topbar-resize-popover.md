# Task 004: Topbar overflow Resize entry + popover

**Description:** Surface the feature: a **Resize** item in the
editor topbar's overflow menu (the `⋯` menu holding Save / Upload /
Paste — `crates/flintshot-editor-egui/src/topbar.rs`), opening a
popover with preset chips (25% / 50% / 75%) and a custom width field.
Presets apply immediately and close; custom applies on Enter.
Aspect ratio locked — the derived `× H` is a label, not an input.

**Implementation notes:**

- `TopbarAction` grows a `Resize` variant; the item renders under
  `Enable::EDITOR_ITEMS` (editor surface only — Home / Settings /
  Loading get nothing; the overlay is untouched)
- egui menu + popover interplay: either a submenu-style container or
  the menu item setting a flag that `app.rs` answers with an
  `egui::Popup` next frame — whichever the toolkit makes natural;
  acceptance is the interaction, not the mechanism
- Popover state (open flag, width field contents) lives in the app
  (mirrors existing patterns like `copied_visible`); applying calls
  `state.resize(w)` directly
- Clamp at the UI too: width field max = current image width; apply
  disabled at ≥ current width; entry disabled when image width ≤ 1
- Percent semantics: `new_w = (w * pct).round().max(1)` clamped to
  `w`; presets hover-show resulting `WxH`
- View re-fit needs no work — fit is recomputed from image dims
  each frame (same path crop exercises)

**Acceptance criteria:**

- [ ] Editor surface: overflow menu shows Resize; clicking opens the
      popover with 25/50/75% chips + custom width row
- [ ] Preset click calls `EditorState::resize` with the computed
      width and closes the popover
- [ ] Custom width clamps to `[1, image_width]`; Enter applies;
      apply disabled at ≥ current width
- [ ] Home / Settings / Loading surfaces and the overlay render no
      Resize entry
- [ ] Unit tests: action routing (menu → `state.resize`), preset
      width computation (incl. odd dims + min-1 guard), clamp +
      disabled states, no- upscale path

**Verification:**

- [ ] `cargo test -p flintshot-editor-egui` (topbar/app tests)
- [ ] All four gate commands pass
- [ ] Manual: `cargo run -- app` → open image → 50% → view re-fits,
      magnifier tracks, Save produces correct dims, Ctrl+Z restores

**Dependencies:** Task 003

**Files likely touched:**

- `crates/flintshot-editor-egui/src/topbar.rs`
- `crates/flintshot-editor-egui/src/app.rs` (popover state + routing)
- possibly `crates/flintshot-editor-egui/src/theme` (no new tokens
  expected — reuse)

**Docs:**

- Gotchas about the menu/popover mechanism → inline comment at the
  chosen mechanism.

**Estimated scope:** M — 2-3 files
