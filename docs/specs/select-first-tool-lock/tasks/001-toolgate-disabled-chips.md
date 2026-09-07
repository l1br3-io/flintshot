# Task 001: `ToolGate` + disabled-chip rendering + editor rail wiring

**Description:** Introduce the enable-set the shells compute and
the rail renders. `toolbar.rs` gains a `pub(crate) ToolGate`
(`All` / `SelectOnly`) with a pure `contains(tool)`; the shared
chip primitives (`chip`, `labeled_chip`) gain an `enabled: bool`
parameter — disabled chips render greyed (glyph at reduced alpha,
no hover-lighting, no border accent), downgrade to hover-only
sense, and carry a lock tooltip; `show()` takes the gate and
`draw_rail` disables each spec's chip accordingly, skipping the
click→`state.tool` assignment. `app.rs`'s `ui()` passes
`SelectOnly` while `phase_active()`, `All` otherwise (via a small
pure `rail_gate(phase_active)` helper so the choice is
unit-testable without a live `Ui`). Non-tool chip call sites
(undo/redo, outputs, Copy, the overlay's tool/undo chips) pass
`true` — behaviour unchanged there this task.

**Acceptance criteria:**
- [ ] `ToolGate { All, SelectOnly }` (`Debug, Clone, Copy,
      PartialEq, Eq`) with documented variants; `contains` returns
      true for `Select` under both variants, false for every other
      `Tool` under `SelectOnly`, true for all under `All`.
- [ ] Disabled chip: no hover fill/border change, glyph dimmed,
      `Sense::hover()` (no click), tooltip ends with a lock clause
      (editor copy: "locked — Enter crops to unlock").
- [ ] `toolbar::show(ui, state, copied, gate)`: with
      `SelectOnly`, clicking a non-Select chip (synthetic context)
      leaves `state.tool` unchanged; clicking Select still selects
      it; undo/redo/output chips unchanged.
- [ ] `rail_gate(true) == SelectOnly`, `rail_gate(false) == All`;
      editor `ui()` passes `rail_gate(self.phase_active())`.
- [ ] Post-phase (`SelectionPhase::Ended` / non-selection
      constructors) the rail is fully live — existing tests green.

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui --quiet`
- [ ] `cargo clippy -p flintshot-editor-egui --all-targets -- -D warnings`
- [ ] Manual (needs display): `cargo run -- region` — pre-crop the
      non-Select chips are greyed with lock tooltips and don't
      respond to clicks; Enter crops; the rail is fully live.

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-editor-egui/src/toolbar.rs`
- `crates/flintshot-editor-egui/src/app.rs`
- `crates/flintshot-editor-egui/src/overlay.rs` (call sites pass `true` only)

**Docs:**
- None — the ADR lands in task 004 with the full rationale.

**Estimated scope:** Medium (2–3 files; the chip-primitive param
touches every call site mechanically)
