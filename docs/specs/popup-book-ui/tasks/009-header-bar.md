# Task 009: Header bar with tab-strip and AI seams

**Description:** New `header.rs` drawn as a top panel by the window
router for Home and Editor routes: Flintshot identity left; a
**tab-strip region rendering exactly one tab** (the open document /
"New capture" placeholder on Home) — inert seam for future tabs; a
**single hidden AI entry slot** (constant-gated, compiles to
nothing today); overflow menu right (About + status line + a hint
pointing at `flintshot backends` for diagnostics — no new plumbing).
Header is a cutout-style bar: cream fill, hairline bottom stroke, no
grain (keeps it quiet).

**Acceptance criteria:**
- [ ] Header renders on both routes; one tab visible; AI slot and
      Objects-slot code present but inert/hidden
- [ ] Header height fixed and modest; canvas/panel layout absorbs it
      without scrolling
- [ ] Existing window titles/WM classes unchanged (ADR-0047
      close/reopen flow unaffected)

**Verification:**
- [ ] Full suite + clippy green
- [ ] Manual: `cargo run -- app` — header on home; capture lands on
      editor page with header; close/reopen cycle still works

**Dependencies:** Task 001

**Files likely touched:**
- `crates/flintshot-editor-egui/src/header.rs` (new)
- `crates/flintshot-editor-egui/src/window.rs`
- `crates/flintshot-editor-egui/src/{home,app}.rs` (layout offset)

**Docs:**
- Comment at the tab-strip and AI-slot constants naming the seam
  owner spec sections (forward-compat 1 & 3).

**Estimated scope:** Medium (3–4 files)
