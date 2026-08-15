# Task 005: All ten tools active inside the selection — annotation confinement

**Description:** The overlay's Flameshot annotation confinement
(plan, Stage 3): with a region established, every annotation tool
draws *inside* it through the unchanged dispatcher, and annotation
rendering is clipped to the region. Two halves:

1. **Input gating** — a pure helper `annotation_press_allowed(tool,
   sel, at)`: `Tool::Select` presses always flow (fresh selection
   drags + the Task 003 gestures); annotation-tool presses dispatch
   only when they land inside the usable selection. A gated press is
   swallowed whole — `drag_active` stays false, so the drags and
   release of that press never reach the dispatcher either. Drags
   that start inside and wander outside keep dispatching raw
   positions: dispatcher tool semantics stay byte-identical to the
   editor window (parity), the region edge just stops painting.

2. **Render clipping** — `EguiCanvas::set_clip(Option<Rect>)`
   (canvas-physical px): every annotation `draw_*` call routes
   through `painter.with_clip_rect` (which intersects the panel
   clip); the background blit stays unclipped so the frozen screen
   outside the region remains visible under the dim mask (the user
   needs it to aim / resize). The editor window never sets a clip,
   so editor-window rendering is byte-identical. The overlay sets
   the clip to the usable selection around its `render()` call each
   frame and clears it after.

Parity verification (the plan's "verify text/number/blur/marker
parity with editor window"): a ten-tools table test drives
`EditorState` with the exact event sequences the overlay emits
(press / drags / release inside the region; Text = press +
`TextInput` + Enter commit; Number = click-to-place) and asserts
one committed annotation of the right kind per tool — Select sets
the selection and commits nothing. The sequences are the same
mapping `EditorApp::logic` produces, pinning overlay behaviour to
editor-window behaviour.

**Acceptance criteria:**
- [x] `annotation_press_allowed`: Select always flows; annotation
      tools dispatch only inside the usable selection (inclusive
      edges); no/degenerate selection denies annotation presses.
- [x] A gated press leaves `drag_active` false — no `PointerDrag` /
      `PointerUp` from that press reaches the dispatcher.
- [x] `EguiCanvas::set_clip` constrains every annotation primitive
      (`draw_rect_*`, `draw_ellipse`, `draw_line`, `draw_polyline`,
      `draw_arrow`, `draw_text`, `draw_image_region`) to the clip;
      `draw_image` (background blit) ignores it; `set_clip(None)`
      restores panel clipping.
- [x] Overlay `ui()` clips the shared `render()` pass to the usable
      selection each frame and clears the clip after.
- [x] Ten-tools table test: Rect / Ellipse / Arrow / Line / Marker
      / Pencil / Text / Blur / Number each commit exactly one
      annotation of the right kind via the overlay's dispatch
      sequence; Select updates the selection, commits nothing.
- [x] Editor-window mode untouched (no clip ever set there).

**Verification:**
- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace --all-targets` — incl. the new
      canvas-clip, gating, and ten-tools parity tests.
- [x] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual (X11, real desktop session): drag a region, switch to
      each tool from the rail, draw inside the region; strokes
      dragged past the edge stop at the edge; presses outside the
      region with a draw tool do nothing; the dimmed area outside
      still shows the frozen screen; text drafts type / commit with
      Enter inside the region; blur and number bubbles clip at the
      edge.

**Dependencies:** Task 004 (floating toolbar — the tool rail that
switches tools).

**Files likely touched:**
- `crates/flintshot-editor-egui/src/canvas.rs` — `set_clip` + clipped
  painter routing + clip unit tests.
- `crates/flintshot-editor-egui/src/overlay.rs` —
  `annotation_press_allowed` + press-branch gating + render-clip
  wiring + gating / parity tests + module docs.
- `CHANGELOG.md` — Unreleased entry.

**Estimated scope:** Medium (~250 lines across two modules, no core
changes).
