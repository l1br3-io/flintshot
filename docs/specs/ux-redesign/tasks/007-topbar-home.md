# Task 007: Topbar on home + welcome re-skin

**Description:** The router's topbar spans Home (Capture enabled +
hero-key, Copy disabled with the mockup tooltip "No image is open —
copy is available in the editor", Open enabled, overflow = About +
note + Settings) and Loading (all three disabled). The welcome body
re-skins to the mockup: forest-deep hero card (rotated flame, headline
"Capture. Annotate. Hand it off — *in seconds*." with gold emphasis),
three hcards (New capture / Open image / Paste), recents grid with
relative-time captions (now/mins/hours/yday thresholds — pure,
unit-tested formatter over recents mtimes).

**Acceptance criteria:**
- [ ] One `topbar::show` across all routes; per-route enablement
      matrix unit-tested (Home/Editor/Settings/Loading × buttons).
- [ ] Home Capture/Open dispatch to existing `HomeApp` actions; Copy
      disabled with tooltip; no statusbar on home/settings/loading.
- [ ] Welcome body matches the mockup: hero card (paper shadow, white
      inner outline, cream text on forest), hcards with hover lift,
      uppercase "Recent captures" title, 4-col recents grid.
- [ ] Recents caption: stem + relative time (formatter unit-tested);
      clicking a recent routes through the existing open path.
- [ ] Home's existing tests updated, none deleted; welcome content
      actions unchanged (never-blank preserved).

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: topbar Capture starts a capture; Open picks + routes;
      Copy visibly disabled; recents show relative times

**Dependencies:** Task 5 (topbar component)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/window.rs`
- `crates/flintshot-editor-egui/src/home.rs`
- `crates/flintshot-editor-egui/src/topbar.rs`

**Estimated scope:** Medium (3 files)
