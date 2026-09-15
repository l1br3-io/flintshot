# Task 009: Remove the editor chrome strip

**Description:** Contingent on OQ-1 (lean: remove). With the `+`
popup mirroring home (Task 001), the chrome strip under the toolbar
(`show_page_actions`) duplicates it. Remove the strip UI while
keeping the C / O page accelerators (they live in
`page_action_shortcut`, not the strip) — the popup is the visible
surface, the accelerators stay. Delete the strip's tests; keep /
port the accelerator tests.

**Acceptance criteria:**
- [ ] `show_page_actions` and its panel are gone; the editor rail
      sits directly above the canvas
- [ ] C / O accelerators still fire the capture/open intents; the
      `+` popup remains the sole visible home-parity surface
- [ ] Strip tests removed; accelerator tests retained and green

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` green
- [ ] Full gate green (see Task 001's gate commands)
- [ ] Manual: editor shows no strip; C triggers a capture, O opens
      the file dialog

**Dependencies:** Task 001; **blocked on the OQ-1 decision —
confirm with the maintainer before executing; skip (mark
cancelled) if the strip stays**

**Files likely touched:**
- `crates/flintshot-editor-egui/src/toolbar.rs`
- `crates/flintshot-editor-egui/src/app.rs`

**Docs:**
- None expected.

**Estimated scope:** Extra-small: 2 files
