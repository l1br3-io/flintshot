# Task 002: Overlay floating toolbar adopts the pods

**Description:** Rework `overlay_rail_row` to walk the shared
`TOOL_PODS` table with the same island visuals (tool pods + history
pod; the overlay has no capture `+`), and render the accept row as
two islands: accept/cancel (✓, ✕) and outputs (Save, Pin, Upload,
Copy ember). The floating toolbar's measured-size repaint and
`toolbar_anchor` clamping adapt to the wider pod layout without
relaxing their tests.

**Acceptance criteria:**
- [ ] Overlay tool row renders the same pod membership and order as
      the editor rail (selection → shapes → freehand → labels →
      redact → history) with island visuals; no hairline separator
      remains
- [ ] Accept row reads as two islands: ✓/✕ pair, then outputs
      (Copy stays the ember primary)
- [ ] All overlay chip ids unchanged (`("overlay_tool", key)`,
      `overlay_undo`, `overlay_redo`, `overlay_delete`,
      `overlay_accept`, `overlay_cancel`, …) — existing overlay
      click-through tests pass unmodified
- [ ] `toolbar_anchor` tests pass unchanged; the measured-size
      repaint path (`measured != toolbar_size`) still tracks the
      pod-padded size, keeping every chip on-panel

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` green
- [ ] Full gates (fmt / clippy `-D warnings` / workspace tests /
      doc tests)
- [ ] Manual: `cargo run -- region` — toolbar anchored beside a
      small selection stays fully on-screen; pods readable;
      screenshot for the PR

**Dependencies:** Task 001 (TOOL_PODS table + island helper)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/overlay.rs` (rail + accept row
  rendering, tests only if pod walk shifts any rect expectations)

**Docs:**
- None expected; anchor/measured-size behavior is already
  documented in `overlay.rs` module docs.

**Estimated scope:** Small-Medium: 1-2 files
