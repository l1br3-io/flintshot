# Task 004: Floating toolbar — tool rail + output actions + accept row, anchored to the selection

**Description:** The Flameshot-style floating toolbar (plan decision 4):
a compact rail + accept row in a foreground `egui::Area`, anchored to
the selection — below its bottom-left, flipped above when the space
below is short, clamped into the viewport. Reuses `toolbar`'s
`ToolSpec`/`TOOLS`, `chip`/`copy_chip`, glyphs, and action semantics
(`ToolbarAction`); the layout is new and overlay-owned. Row 1 is the
tool rail: the ten tool chips + undo/redo. Row 2 is the accept row:
accept (✓, Enter-equivalent), cancel (✕, Esc-equivalent), separator,
Save / Pin / Upload, and the ember Copy primary (Enter-equivalent —
Enter = Copy per the spec). The toolbar shows only while a usable
selection exists and no fresh region drag is in progress (same guard
as the handles) and follows the live region during resize/move
gestures. Pointer presses on the toolbar are swallowed before chrome
interception / dispatcher dispatch (the toolbar floats inside the
overlay panel, so the existing `panel.contains` guard is not enough);
the hover cursor and the magnifier stand down over it. Tool-chip
clicks while a text draft is active commit the draft first (same
guard `EditorApp::logic` has). Anchoring is a pure, unit-tested
helper. The Save / Pin / Upload output pipelines land in Task 006 —
their buttons are present and click through to a `tracing::info!`
notice (the same placeholder pattern as Task 002's accept).

**Acceptance criteria:**
- [x] `toolbar_anchor(sel, panel, size)` (pure): below-left placement
      with room; flips above the selection when below doesn't fit;
      clamps x into the panel at the right edge; pins to the panel
      top when neither side fits; tolerates a zero/stale size seed.
- [x] Toolbar visible iff a usable selection exists and no fresh
      dispatcher drag is in progress; hidden otherwise (rect cache
      cleared so stale areas don't swallow presses).
- [x] Tool rail: all ten tool chips (reused `ToolSpec` + glyphs),
      click switches `state.tool`; undo/redo chips replay the
      Ctrl+Z / Ctrl+Y dispatches.
- [x] Accept row: ✓ = accept close (Enter semantics), ✕ = cancel
      close (Esc semantics), Copy = accept close (Enter = Copy),
      Save / Pin / Upload placeholders log the Task 006 notice.
- [x] Toolbar area rect cached per frame; primary presses inside it
      never start a drag, gesture, or dispatcher `PointerDown`
      (in-progress drags/gestures that wander over it continue).
- [x] Hover cursor + magnifier suppressed over the toolbar.
- [x] Tool-chip click mid-text-draft commits the draft (mirrors
      `EditorApp::logic`).
- [x] Idle repaint behaviour untouched (Task 008).

**Verification:**
- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace --all-targets` — incl. new
      `toolbar_anchor` unit tests.
- [x] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual (X11, real desktop session): drag a region → toolbar
      appears below it; tools switch from the rail; ✓/Enter accept,
      ✕/Esc cancel; toolbar flips above a bottom-edge selection and
      clamps at the right edge; clicks on the toolbar never disturb
      the selection; magnifier/cursor stand down over it.

**Dependencies:** Task 003 (selection chrome — anchor target).

**Files likely touched:**
- `crates/flintshot-editor-egui/src/overlay.rs` — anchor helper +
  tests, toolbar draw + action handling, press/cursor/magnifier
  guards, draft-commit guard.
- `crates/flintshot-editor-egui/src/toolbar.rs` — `pub(crate)` on
  the shared chips/spec/glyphs + new ✕ glyph.
- `CHANGELOG.md` — Unreleased entry.

**Estimated scope:** Medium (~300 lines across two modules, no core
changes).
