# Task 005: Docked side panel replaces the toolbar rail

**Description:** The structural pivot. New `panel.rs`: a right
`SidePanel` hosting (top→bottom) the five tool pods (Select/Move,
Shapes, Freehand, Labels, Redact — all ten tools) as cutout cards
with white borders, hover-lift (shadow grows + translate via
`ease::spring`), forest fill on the active tool; below them, the
existing bay controls render temporarily as today (contextual
re-arrangement is Task 007); at the bottom, the existing rail
actions (new capture, open, paste, undo/redo, save/copy/upload)
relocate as restyled chips. Panel background gets the grain tile.
Re-wire `app.rs` `ui_impl` off the top-rail: canvas becomes the
central panel, **input drag-gating re-derived for the right-edge
panel** (today's code assumes a top toolbar via negative-canvas-y
and `canvas_rect.min.y`). `toolbar.rs` sheds `draw_rail`/rail
actions but keeps pods/bay/glyphs for now.

**Acceptance criteria:**
- [ ] All ten tools selectable from the panel; single-key shortcuts
      and the selection-first `ToolGate` phase behavior unchanged
      (existing `app.rs` phase tests pass unmodified)
- [ ] A press that begins on the panel never starts an annotation
      drag or canvas gesture (manual matrix: press on panel edge,
      drag onto canvas, release)
- [ ] Editor page fits 1024×640 default (canvas ≥ ~700px wide) — or
      `EDITOR_PAGE_DEFAULT_SIZE` and its pinned test are updated in
      this task
- [ ] Hover-lift microinteraction active on pods; grain on panel
      background; canvas itself never grained/shadowed

**Verification:**
- [ ] Full test suite + clippy green
- [ ] Manual: `cargo run -- app` → capture → every tool draws;
      undo/redo/copy reachable from panel bottom; window resize
      keeps canvas usable

**Dependencies:** Tasks 001, 003

**Files likely touched:**
- `crates/flintshot-editor-egui/src/panel.rs` (new)
- `crates/flintshot-editor-egui/src/app.rs` (layout + gating)
- `crates/flintshot-editor-egui/src/toolbar.rs` (rail removal)
- `crates/flintshot-editor-egui/src/lib.rs` (module list)

**Docs:**
- GOTCHA comment at the new drag-gate documenting the old top-rail
  assumption it replaces (negative-y origin trap).

**Estimated scope:** Large (4 files, high-risk gating logic)
