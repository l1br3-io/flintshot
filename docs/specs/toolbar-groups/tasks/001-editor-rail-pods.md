# Task 001: Editor rail pods — table, islands, rail rewrite

**Description:** Partition the flat `TOOLS` table into `TOOL_PODS`
(ToolPod groups), add the pod-island theme constants, introduce a
shared island-container helper, and rewrite the editor rail
(`draw_rail`) to walk pods: capture `+` · selection (Move, Select) ·
shapes (Rect, Ellipse, Line, Arrow) · freehand (Pencil, Marker) ·
labels (Text, Number) · redact (Blur) · history (Undo, Redo,
Delete) · outputs (Save, Pin, Upload, Copy). Islands + gaps replace
the `ui.separator()` hairlines. Chip ids, glyphs, tooltips,
shortcuts, and actions are frozen.

**Acceptance criteria:**
- [x] `TOOL_PODS` unit test: every `Tool` variant appears in exactly
      one pod; no empty pods; tool pods in spec order (selection,
      shapes, freehand, labels, redact); Move leads and Select
      seconds overall
- [x] Editor rail renders every pod as a rounded island (one
      graphite step off the panel) separated by gaps wider than
      intra-pod chip spacing; no hairline separators remain in the
      rail
- [x] All chip ids unchanged (`("tool", key)`, `undo`, `redo`,
      `delete`, `save`, `pin`, `upload`, `copy_action`,
      `new-capture`) — the existing synthetic-context click-through
      tests pass **without modification**
- [x] `ToolGate::SelectOnly` greys exactly the same chips as before
      (gated chip inside a pod keeps its disabled visuals)
- [x] Bay (row 2) untouched; its tests pass unchanged

**Verification:**
- [x] `cargo test -p flintshot-editor-egui` green
- [x] Full gates: `cargo fmt --all --check`;
      `cargo clippy --workspace --all-targets -- -D warnings`;
      `cargo test --workspace --all-targets`;
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual: `cargo run -- app` — pods readable at a glance; no
      clipped/overlapping chips; screenshot for the PR

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-editor-egui/src/toolbar.rs` (TOOL_PODS, island
  helper, draw_rail, tests)
- `crates/flintshot-editor-egui/src/theme.rs` (pod fill / corner /
  padding constants + a distinctness test alongside the existing
  `accent_and_fills_stay_distinct`)

**Docs:**
- If the right-aligned outputs island needs a non-obvious layout
  trick (right-to-left inside a Frame), a `/* why */` comment at
  the site; no separate doc expected.

**Estimated scope:** Medium: 2 files
