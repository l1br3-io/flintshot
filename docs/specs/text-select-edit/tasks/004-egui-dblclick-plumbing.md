# Task 004: egui — double-click plumbing in both shells

**Description:** Give both event pumps (`app.rs` editor window,
`overlay.rs`) double-click detection. A small shared tracker module
holds the last press time + position and fires when a new press
lands within egui's double-click thresholds (read them from
`ctx.style().interaction`; verify the exact egui API
source-driven during implementation). The pump emits
`InputEvent::DoubleClick { pos }` *in addition to* the second
press's `PointerDown`, same frame, after it — the dispatcher relies
on that ordering (select, then upgrade to session). The overlay
emits it only where a `PointerDown` would dispatch (inside the
`annotation_press_allowed` fall-through arm), matching its existing
gating.

**Acceptance criteria:**

- [ ] Two rapid presses at the same canvas spot synthesize
      `PointerDown` + `DoubleClick` in both shells; slow or distant
      presses do not fire the tracker.
- [ ] Overlay gating preserved: no `DoubleClick` where presses are
      swallowed (toolbar, outside-region under a live selection).

**Verification:**

- [ ] `cargo test -p flintshot-editor-egui` (tracker unit tests +
      pump tests via the existing egui-ctx frame harness)
- [ ] Manual: `cargo run -- capture` → place text → Enter →
      double-click → caret at end → append text → Enter → text
      updated; `Ctrl+Z` once reverts the whole edit; Esc mid-edit
      reverts. Both shells.

**Dependencies:** Task 003

**Files likely touched:**

- `crates/flintshot-editor-egui/src/dblclick.rs` (new; tracker)
- `crates/flintshot-editor-egui/src/lib.rs` (module line)
- `crates/flintshot-editor-egui/src/app.rs` (pump)
- `crates/flintshot-editor-egui/src/overlay.rs` (pump)

**Docs:**

- Module doc on the tracker states the ordering contract from
  Task 003.

**Estimated scope:** Medium (3–4 files)
