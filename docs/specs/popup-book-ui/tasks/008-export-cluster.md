# Task 008: Export cluster with Copy hero

**Description:** Final export arrangement at the panel bottom:
**Copy** as the forest-filled primary cutout card (the hero —
instant annotate-and-copy), with Save, Pin, Upload as grouped
secondary chips beside it. Existing handlers (`handle_copy`,
`handle_save`, `handle_upload`, pin flow) are reused untouched —
this is arrangement + emphasis, not behavior. Copy keeps its
copied-state feedback.

**Acceptance criteria:**
- [ ] Copy renders visually primary (forest fill, largest control);
      Save/Pin/Upload grouped and secondary
- [ ] Copy → `CopiedToClipboard` flow (page-stays-open semantics,
      close-after-copy window) behaves exactly as before
- [ ] Save dialog, pin-to-screen, and upload all launch from the
      cluster

**Verification:**
- [ ] Full suite + clippy green
- [ ] Manual: annotate → Copy → paste elsewhere works; Save writes;
      Pin appears; Upload initiates

**Dependencies:** Tasks 005, 007

**Files likely touched:**
- `crates/flintshot-editor-egui/src/panel.rs`
- `crates/flintshot-editor-egui/src/app.rs` (chip wiring only)

**Docs:**
- None expected.

**Estimated scope:** Small-Medium (2 files)
