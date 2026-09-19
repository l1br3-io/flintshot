# ADR-0061: Property-edit undo coalescing by edit kind

**Context.** The selection inspector (multi-select-properties spec)
edits committed annotations live: every property mutator call
(`set_selected_color`, `set_selected_stroke_width`, …) pushes an
`anns`-only undo snapshot so edits are undoable. But several
inspector widgets are *continuous* — an egui `DragValue` dragged
across forty frames fires forty value changes, a colour editor or
popover rewrites its value every frame it is open. Snapshot-per-call
would bury one logical gesture under dozens of undo entries,
breaking the project-wide "one gesture = one undo entry" contract
the move tool already upholds.

The spec left the mechanism open (its open question 1 named two
candidates: per-frame burst coalescing vs begin/end bracketing).

**Decision.**

- Coalescing lives in **core**, keyed on an **edit kind**
  (`PropertyEditKind`): a mutator pushes a snapshot only when its
  kind differs from the previous mutator call's kind
  (`EditorState::last_edit_kind`, crate-private). Consecutive
  same-kind calls share the first call's pre-edit snapshot.
- Shells close a burst explicitly with `EditorState::
  end_property_edit()`:
  - discrete widgets (swatch, stroke preset, fill switch, blur
    segment, `+`/`-` steppers) call it immediately after the
    mutator — each click is its own undo entry;
  - `DragValue`s call it on `drag_stopped()` / `lost_focus()`;
  - popovers (custom colour, text style) are bracketed by their
    open/close transitions, tracked in `PanelState` — one popover
    session is one undo entry.
- Every non-property mutation (commit, move gesture, delete, crop,
  undo, redo, number placement) clears `last_edit_kind`, so an edit
  after any other gesture always starts a fresh entry.

**Consequences.**

- A continuous drag, however long, is exactly one `Ctrl+Z`; a
  sequence of distinct clicks is one entry each; an edit following
  a move never folds into the move's snapshot.
- The policy is testable in core without egui (sequence tests over
  mutator calls); the shell only owns interaction-end detection,
  which is where egui's API lives.
- Kind granularity is per *section*, not per field: a text-style
  popover session that changes family then weight is one entry.
  That matches how the popover presents (one editor surface), and a
  finer granularity can be added later by splitting kinds — the
  enum is `#[non_exhaustive]`.
- Risk accepted: a shell that forgets to bracket a future
  continuous widget still behaves safely (coalescing continues
  until the next different-kind edit or mutation) — the failure
  mode is a longer-lived undo entry, never a lost or duplicated
  one.
