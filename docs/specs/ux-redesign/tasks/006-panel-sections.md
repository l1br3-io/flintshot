# Task 006: Properties panel — binding section map, sections only

**Description:** `panel.rs` becomes the mockup's 236px properties
panel: `{Tool} · Properties` uppercase title + contextual collapsible
`widgets::section` cards — no tools card (Task 4), no export island,
no `+` popup, no collapse toggle. The spec's **binding section map**
is implemented: per-tool section visibility (Move → Selection;
Rect/Ellipse/Pencil → Stroke+Fill; Line/Arrow/Marker → Stroke; Text →
Stroke+Text; Number → Stroke+Number; Blur → Effects; Select keeps its
crop bay as a section; Selection always present). Section inventories:
Stroke (swatches + width wells), Fill (switch), Text (size wells +
weight/style toggles + family), Effects (blur kind + strength wells),
Number (next-number stepper), Selection (danger Delete).

**Acceptance criteria:**
- [ ] Section map table from the spec implemented + unit-tested per
      tool; Selection disabled-state gating on `selected_index`.
- [ ] Export island, copy hero/square, `+` chip, collapse toggle
      removed; `ToolbarAction`/`PageChromeAction` variants dissolved —
      Save/Upload/Copy fire from the topbar, Paste from overflow
      (routing stays in `app.rs`).
- [ ] Sections use `widgets::section` (spring collapse, chevron,
      persisted open/closed); wells/switches from `widgets`.
- [ ] Every former panel control has a new home (mapping table in the
      PR); bay tests adapt, none deleted.
- [ ] Panel width pinned 236px; token/widget-only styling.

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: click through all ten tools — only mapped sections show;
      collapse animates; Delete works on a selected annotation

**Dependencies:** Tasks 2, 4, 5 (widgets, rail, topbar destinations)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/panel.rs`
- `crates/flintshot-editor-egui/src/app.rs`

**Estimated scope:** Medium (2 files, large diff)
