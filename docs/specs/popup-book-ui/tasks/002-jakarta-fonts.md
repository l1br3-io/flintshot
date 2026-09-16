# Task 002: Embed Plus Jakarta Sans + type ramp

**Description:** Download Plus Jakarta Sans static TTFs (Regular,
Medium, ExtraBold; OFL) into
`crates/flintshot-editor-egui/assets/fonts/`, commit the OFL license
text beside them, and wire them into `FontDefinitions` from
`theme/mod.rs` (replacing egui's default Ubuntu for `Proportional`;
keep emoji fallback). Define the type-ramp helper (display 22–28
ExtraBold, title 16, body 13, caption 11) and switch existing
`FontId` uses to ramp constants.

**Acceptance criteria:**
- [ ] Three Jakarta weights + OFL license committed under
      `assets/fonts/`; embedded via `include_bytes!`
- [ ] `FontDefinitions` installed by `theme::apply`; all UI text
      renders Jakarta at ramp sizes
- [ ] Editor text-annotation rendering still uses its own text path
      unchanged (annotation font is not the UI font)

**Verification:**
- [ ] Build + tests + clippy green
- [ ] Manual: welcome + editor text visibly Jakarta; text tool
      annotation output unchanged

**Dependencies:** Task 001

**Files likely touched:**
- `crates/flintshot-editor-egui/assets/fonts/*` (new, 4 files)
- `crates/flintshot-editor-egui/src/theme/mod.rs`
- `crates/flintshot-editor-egui/src/{home,toolbar}.rs` (FontId
  call sites)

**Docs:**
- GOTCHA comment in `theme/mod.rs` if any egui default-font slot
  must be kept (e.g. monospace fallback for dims readouts).

**Estimated scope:** Small (2–3 code files + assets)
