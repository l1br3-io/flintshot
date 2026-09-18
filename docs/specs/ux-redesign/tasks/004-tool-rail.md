# Task 004: Left tool rail (tools leave the right panel)

**Description:** New `rail.rs` — the 48px forest-mid strip from the
mockup: ten real tools as 36×36 cutout chips grouped by the existing
five semantic `TOOL_PODS` pods with hairline separators. Active tool:
gold fill + white border + pop shadow + spring bounce (`tool-pop`).
Locked tools: lock-grey, hover-only, stable layout. The tool
vocabulary (`TOOL_PODS`, glyphs, `ToolGate`) moves from `panel.rs`;
the right panel drops `tools_card` and the collapse-to-rail mode;
tests travel with the code.

**Acceptance criteria:**
- [ ] `egui::Panel::left("tool_rail")` after top/bottom; 48px pinned;
      forest-mid fill + grain.
- [ ] All ten tools; click → `state.set_tool`; `ToolGate` greys
      locked chips without hiding (selection-first phase verified).
- [ ] Active styling + spring pop on switch (through the animation
      gate); tooltips `name · key` (+ lock clause when gated).
- [ ] Right panel: `tools_card`, `rail_body`, `PanelState.collapsed`
      gone; `PanelState::width()` contract updated with its consumer.
- [ ] Vocabulary tests (order, glyphs, gate) pass from `rail.rs`; no
      coverage deleted; fresh panel id.

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: switch through all ten tools; selection-first capture
      gates the rail

**Dependencies:** Tasks 1-3 (tokens, widgets, statusbar draw-order
pattern)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/rail.rs` (new)
- `crates/flintshot-editor-egui/src/panel.rs`
- `crates/flintshot-editor-egui/src/app.rs`

**Estimated scope:** Medium (3 files)
