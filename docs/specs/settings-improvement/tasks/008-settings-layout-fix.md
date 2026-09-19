# Task 008: Settings layout fix (CentralPanel; overlap regression RED first)

**Description:** Fix the settings page's layout bug: its `ScrollArea`
is drawn directly on the window's root `ui` without a
`CentralPanel`, so scrolled content renders under the 44 px topbar.
Wrap the body in a `CentralPanel` (mirroring `home`'s paper-cream
frame), and pin the fix with a machine-checked regression test
written RED first.

**Acceptance criteria:**
- [ ] RED first: a headless test renders the topbar + settings route
  at a small screen height and asserts no painted shape *crosses*
  the topbar's bottom edge — every shape's bounds lie entirely
  above (the bar itself) or entirely at/below `TOPBAR_HEIGHT`
  (small epsilon for the bar's edge stroke). Precedents:
  `topbar_is_a_44px_top_strip` (topbar.rs) and the `shapes_of`
  scans (widgets.rs). If the exact assertion fights egui's clip
  behavior, reformulate while keeping it machine-checked — the pin
  is "content never overlaps the bar"
- [ ] The window router's Settings arm draws the page inside an
  `egui::CentralPanel` (paper-cream fill, matching home's frame),
  with `SettingsPage::show` receiving the panel's inner `ui`
- [ ] Scroll behavior intact at small heights (the ScrollArea
  owns the panel's body region)
- [ ] Existing settings tests still pass (they render through
  `SettingsPage::show` with their own context — adjust harness if
  the signature gains the panel)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui --all-targets` green
  (new regression test included; verified RED before the fix)
- [ ] `cargo build` green

**Dependencies:** None (independent of the IPC work)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/settings.rs`
- `crates/flintshot-editor-egui/src/window.rs`

**Docs:**
- A short why-comment at the CentralPanel (the bug it prevents).

**Estimated scope:** Small: 2 files
