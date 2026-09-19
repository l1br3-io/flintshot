# Task 003: Shell — `PROPERTIES | HISTORY` tab header

**Description:** Add tab navigation to `panel::show` in
`crates/flintshot-editor-egui/src/panel.rs`. `PanelState` gains
`tab: PanelTab` (`Properties` default, `History`). The header is
the panel's first row — two `segmented_chip`s (the blur-kind
control's idiom: cutout at rest, ink-filled when active), ids
`("panel-tab", PanelTab)`, uppercase labels. Below the header,
the Properties tab renders today's content byte-for-byte
(contextual title + sections); the History tab renders a minimal
placeholder body (an empty-state hint) that Task 004 replaces.

**Acceptance criteria:**
- [ ] Default tab is Properties; existing panel tests pass
      **unmodified**
- [ ] Both tab chips register responses; clicking HISTORY swaps
      the body (placeholder visible, tool sections absent);
      clicking PROPERTIES restores them
- [ ] Tab choice persists across frames (same `PanelState`)
- [ ] Properties content identical: contextual title still
      renders (`N selected · Properties` / `{Tool} · Properties`)
      beneath the header on the Properties tab
- [ ] The header respects the collapse rail (unreachable while
      collapsed — the rail path is unchanged)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`
- [ ] Manual: `cargo run -- app` → editor → tab swap looks right
      against the paper/ink theme

**Dependencies:** None hard (panel-local), ordered after Task 002
to keep Phase 1 / Phase 2 checkpoints clean

**Files likely touched:**
- `crates/flintshot-editor-egui/src/panel.rs` (impl + tests using
  the existing `draw` / `click_events` / `read_response` harness)

**Docs:**
- Panel module doc gains a line on the tab header (History tab
  body detail lands with Task 004).

**Estimated scope:** Small: 1 file
