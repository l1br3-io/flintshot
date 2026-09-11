---
status: active
started: 2026-09-11
---

# Spec — Toolbar grouped pods: organize the rail the way industry editors do

> Mini-effort spec. Layout-only reorganization of the toolbars shared
> by the editor window and the Phase 5 overlay. No new tools, no
> shortcut changes, no core-crate changes. For product context, see
> [`../../VISION.md`](../../VISION.md).

## Objective

Both toolbars today are flat, semantically unstructured chip rows:

- **Editor rail** (`toolbar.rs`): `+` New, then all 11 tool chips in
  legacy order, then Undo/Redo/Delete, then right-aligned
  Save/Pin/Upload/Copy — 19 chips separated by exactly three
  hairlines.
- **Overlay floating toolbar** (`overlay.rs`): tool row (11 tools +
  history) and accept row (✓/✕ + outputs), same flat structure.

Every industry reference groups these controls semantically. Windows
Snip & Sketch separates draw / shapes / text clusters with hairlines;
Shottr and CleanShot X chunk their horizontal toolbars into
draw / annotate / output groups; Excalidraw renders gap-separated
island pods; Snagit keeps a shape family and a callout family
conceptually coherent. A first-time user coming from any of these
apps cannot map flintshot's rail onto those mental models because it
has none.

**The change:** reorganize both surfaces into **grouped pods** —
visually distinct semantic clusters. Every tool stays exactly one
click away (no flyouts, no dropdowns, no collapsing).

### Pod structure (order fixed, left → right)

| Pod          | Chips                                    | Why / industry echo                                    |
| ------------ | ---------------------------------------- | ------------------------------------------------------ |
| Capture      | `+` New capture                          | standalone leading affordance (Win11 snip pattern)     |
| Selection    | **Move** (labeled), Select               | object/region navigation pair                          |
| Shapes       | Rect, Ellipse, Line, Arrow               | Snagit shape family; Snip & Sketch shapes cluster      |
| Freehand     | Pencil, Marker                           | Snip & Sketch draw family (pen + highlight together)   |
| Labels       | Text, Number                             | callout family (Snagit Callout, Shottr numbering)      |
| Redact       | Blur                                     | effects/redaction stands alone                         |
| History      | Undo, Redo, Delete                       | edit cluster, universally grouped                      |
| Outputs      | Save, Pin, Upload, Copy (ember primary)  | terminal actions, right-aligned in the editor rail     |

The overlay's accept row keeps its current order (✓ Accept, ✕
Cancel, then outputs) — it already reads as two pods and gains only
the shared pod visuals.

### Visual mechanism

Pods are **islands**: each group sits in its own subtle rounded
container (theme graphite, one step off the rail background, small
inner padding), separated by gaps wider than the intra-pod chip
spacing. Island containers replace the current `ui.separator()`
hairlines as the grouping signal. Exact metrics land in the plan;
the spec fixes the intent: **group membership and boundaries must be
perceivable at a glance, without tooltips.**

**Success looks like:** a user who has never seen flintshot can
point at the rail and say "these are the shapes, these are the
labels, these finish the job" — because the pods say so visually.

## Assumptions

1. **Bay (row 2) is untouched.** Per-tool controls, the contextual
   hint, and their tests stay as they are.
2. **Move stays the labeled leading chip**, Select second
   (prominent-move-tool contract; its tests adapt, not relax).
3. **Shortcuts, tooltip copy, glyph set, and actions are frozen.**
   Single-key hints, `ToolbarAction`, and `ToolGate` semantics
   (SelectOnly greys out everything but Select) are unchanged.
4. **Grouping lives entirely in `flintshot-editor-egui`.** The flat
   `Tool` enum, hotkey dispatch, and core state machine do not move.
5. **No config surface.** Pod grouping is not user-configurable;
   adding a toggle would be scope creep for zero demonstrated need.
6. **The overlay's measured-size / anchor logic adapts**, not
   relaxes: the floating toolbar gets wider (island padding +
   gaps), and `toolbar_anchor` clamping must still keep every chip
   on-screen.

## Scope

**In:**

- `toolbar.rs`: `TOOLS` becomes a pod-partitioned table
  (`TOOL_PODS`); rail rendering iterates pods; island container
  helper shared with the overlay.
- `overlay.rs`: `overlay_rail_row` and the accept row render
  through the same pod structure.
- `theme.rs`: pod container color / padding constants.
- Tests: pod-table coverage + order, adapted click-through and
  anchor tests.

**Out:** `flintshot-core`, new tools or actions, hotkey changes,
bay controls, the home window, tray, any flyout/dropdown grouping,
vertical-rail redesign (considered; rejected as a bigger departure).

## References (what the industry does)

| App                    | Pattern                                                | Taken                     |
| ---------------------- | ------------------------------------------------------ | ------------------------- |
| Windows Snip & Sketch  | flat rail, hairline-separated semantic clusters        | cluster membership        |
| Shottr / CleanShot X   | horizontal toolbar: draw · annotate · outputs, grouped | pod order, outputs right  |
| Excalidraw             | gap-separated island pods, per-tool style island       | island visual mechanism   |
| Snagit                 | shape family + callout family + properties bar         | shapes/labels pod content |
| Flameshot              | vertical left panel, per-tool expansion                | rejected: bigger redesign |

## Commands

From the repo root (see `AGENTS.md`):

```sh
cargo build                              # default-members
cargo fmt --all --check                  # formatting gate
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo run -- app                         # editor window, manual look
cargo run -- region                      # overlay path, manual look
```

## Project structure

| Path                                       | Role                                    |
| ------------------------------------------ | --------------------------------------- |
| `crates/flintshot-editor-egui/src/toolbar.rs` | `TOOL_PODS` table, island helper, rail |
| `crates/flintshot-editor-egui/src/overlay.rs` | floating toolbar pod adoption           |
| `crates/flintshot-editor-egui/src/theme.rs`   | pod container constants                 |
| `docs/specs/toolbar-groups/`               | this effort                             |

## Code style

Matches repo conventions (`#![forbid(unsafe_code)]`, pedantic
clippy, no comments restating code, doc comments on `pub(crate)`
primitives). The new table:

```rust
/// One rail pod: a semantic group of tool chips rendered as a
/// single island. Shared by the editor rail and the overlay's
/// floating tool row.
pub(crate) struct ToolPod {
    /// Group name (debugging + test messages only — pods carry no
    /// label in the UI).
    pub(crate) name: &'static str,
    /// Chips in pod order.
    pub(crate) tools: &'static [ToolSpec],
}

/// The tool chips partitioned into pods, rail order.
pub(crate) const TOOL_PODS: &[ToolPod] = &[
    ToolPod { name: "selection", tools: &[/* Move, Select */] },
    ToolPod { name: "shapes", tools: &[/* Rect, Ellipse, Line, Arrow */] },
    // …freehand, labels, redact
];
```

`ToolSpec`, `chip`, `labeled_chip`, `copy_chip`, and all glyphs are
reused verbatim — the pods are a partition of the existing table,
not a redesign of its primitives.

## Testing strategy

- **Table tests (pure):** every `Tool` variant appears in exactly
  one pod; keys unique; pod order is the spec's order; Move leads /
  Select seconds overall; no empty pods.
- **Behavioral tests (existing pattern, synthetic egui context):**
  Pencil-chip click under `ToolGate::All` still switches tool;
  under `SelectOnly` it stays inert. Delete chip liveness with /
  without selection. These adapt their chip lookup to the pod walk.
- **Overlay tests:** `toolbar_anchor` clamping with the wider
  pod-padded toolbar size keeps the toolbar on-panel.
- **Gates:** `cargo fmt --all --check`, clippy `-D warnings`,
  `cargo test --workspace --all-targets`, doc tests for the two lib
  crates — all green.

## Boundaries

- **Always:** preserve every existing shortcut, tooltip, glyph, and
  action; keep every tool one click away; run the full gate before
  declaring done; adapt (never delete) failing tests.
- **Ask first:** touching `flintshot-core`, changing any hotkey or
  `ToolbarAction` variant, introducing a config key, altering the
  ember Copy primary's visual weight, adding a dependency.
- **Never:** flyout/dropdown grouping in this effort; GUI-toolkit
  types in core; `unsafe`; `#[allow(clippy)]` without a justifying
  comment; comments restating code.

## Success criteria

1. Both surfaces render the eight pods (overlay: seven — no capture
   pod; its `+` lives in the editor rail) in spec order with
   island-and-gap grouping perceivable without tooltips.
2. All 11 tools + every action remain single-click; shortcuts
   unchanged; `ToolGate::SelectOnly` behaves identically.
3. Pod-table unit tests and the adapted click-through / anchor
   tests pass; all four gate commands green.
4. Manual check on `cargo run -- app` and `cargo run -- region`:
   no clipped or overlapping chips at a 500 px-wide region capture
   (the rail's scroll fallback still holds).

## Open questions

1. **Islands vs. hairlines:** spec commits to island containers; if
   plan-review finds them visually heavy against the graphite theme,
   the fallback is wider gaps + the existing hairlines. Decide at
   plan review, before implementation.
2. **Overlay width:** pod padding widens the floating toolbar on
   small selections; if anchor clamping proves cramped, may the
   overlay's tool row wrap to two rows? (Default: no — shrink gaps
   first; revisit only with evidence.)
