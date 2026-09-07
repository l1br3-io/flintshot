# ADR-0043: Selection-phase tool lock — annotate only after the region is committed

**Context.** ADR-0035's selection-first phase force-armed
`Tool::Select` but left the rest of the tool rail live: a user
could click Pencil mid-phase — or press `P` — and draw on the
uncropped shot before any region existed. The phase's own
semantics ("the region is the only job") were a suggestion, not a
property. The overlay had the mirror-image gap: its floating rail
already refuses to render without a usable selection and
`annotation_press_allowed` already denies early presses, but the
single-key tool switch (`tool_for_key` block) was ungated — `P`
pre-region invisibly armed Pencil.

Planning correction (2026-09-07): the original draft assumed the
overlay rendered its chips "fully live" pre-selection. Wrong —
`show_toolbar`'s visibility guard hides the whole rail until a
usable selection exists. Only the overlay's *keyboard* path needed
gating; its pointer and visual paths were already selection-first.

Alternatives considered:

- **Hide the chips until unlock (Flameshot-style reveal).**
  Rejected: the rail would reflow at the unlock moment and the
  tool set would be undiscoverable exactly when a new user is
  deciding what this app can do. Locked-but-visible reads as
  "soon"; absent reads as "absent". (User-confirmed.)
- **A config escape hatch** (`[ui] phase-tool-lock = false`).
  Rejected for v1: matches ADR-0035's no-new-config philosophy;
  defer until someone asks.
- **Core-level gating** (a lock flag on `EditorState`). Rejected:
  gating is shell-owned state — the two shells unlock on
  different events, and `flintshot-core` stays GUI-free and
  unaware of phases beyond what ADR-0035 already seeded.

**Decision.** "Select first, edit second" is enforced per shell,
each at its own notion of "the region is committed":

- **Editor window** — while the selection-first phase is live:
  - The rail draws every non-Select chip **disabled** — greyed
    glyph, no click sense, lock tooltip ("locked — Enter crops to
    unlock"). A `ToolGate` enum (`All` / `SelectOnly`) computed by
    `rail_gate(phase_active)` is passed into the rail draw, so the
    chip primitives stay shell-agnostic and there is exactly one
    disabled-chip rendering.
  - Tool-switch hotkeys are inert for non-Select tools (the same
    gate as the chips), and annotation pointer input is denied
    for the whole phase — simpler and stricter than the overlay's
    geometry gate, because the phase has no committed region to
    confine strokes to.
  - Unlock is **the first crop** (Enter / toolbar *Crop*), which
    already latches the phase off permanently; the full rail,
    hotkeys, and input restore with it and never re-lock.
- **Overlay** — the single-key tool switch arms only when the
  pressed key maps to `Select` or `usable_selection(state.selection)`
  is `Some` (degenerate sub-pixel selections count as absent).
  No visual change: the rail already hides pre-selection, and
  `annotation_press_allowed` / its tests are untouched.
- **Hardcoded**, no config key.

**Consequences.**

- Mid-phase tool switching is impossible by design on both
  shells — there is no input path (click, hotkey, drag) that
  annotates before the region exists. Select, Esc-cancel,
  Enter-crop, outputs (Copy / Save / Pin / Upload), and undo/redo
  keep their ADR-0035 phase behaviour; `fullscreen`, `open`, and
  post-crop sessions are untouched and pinned as such by tests.
- The rail is structurally stable across the unlock: chips keep
  their positions, so the layout never reflows and the future
  tool set stays discoverable during the phase.
- The editor's whole-phase pointer denial means a stray
  annotation press during the phase is a no-op rather than a
  queued stroke — acceptable because the phase's pointer story is
  "drag the region", and Select presses keep flowing.
- No escape hatch until asked for; if one is requested, it
  follows ADR-0035's open-question shape (opt-out config key)
  rather than reopening the gate design.
- Revises ADR-0035's consequence table: the phase is now
  tool-locked, with per-shell unlock semantics (editor: on crop;
  overlay: on usable selection).
