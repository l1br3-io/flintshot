---
status: active
started: 2026-09-07
---

# Spec — Selection-phase tool lock (annotate only after the region is committed)

> UX effort. Revises one consequence of
> [`region-selection-first`](../region-selection-first/spec.md)
> (ADR-0035): the phase force-arms Select but leaves the whole tool
> rail live, so a user can switch to Pen mid-phase and draw on the
> uncropped shot before any region exists. For product context see
> [`../../VISION.md`](../../VISION.md).

## Objective

Make "select the region first, edit second" an enforced property of
both shells, not a suggestion:

1. **Editor window** (`flintshot region`, selection-first phase):
   while the phase is live, every rail chip except **Select** is
   visibly disabled — greyed, unclickable, hotkey inert — and
   annotation pointer input is denied anywhere on the canvas. The
   tools unlock when the phase ends on first crop (Enter / toolbar
   *Crop*).
2. **True overlay** (X11, `interaction-mode = "overlay"`): the
   floating rail never renders without a usable selection
   (`show_toolbar`'s visibility guard), and annotation presses are
   already denied (`annotation_press_allowed`) — so visually and
   behaviorally it is selection-first already. The residual gap is
   the **keyboard**: the single-key tool switch (`overlay.rs`,
   the `tool_for_key` block) is ungated, so pressing `P` with no
   region invisibly arms Pencil. The gate now keeps only `Select`
   armable until a usable selection exists.

> Planning correction (2026-09-07): the original draft claimed the
> overlay renders its chips "fully live" pre-selection — wrong. The
> floating toolbar is absent until a selection exists; only the
> keyboard path needed gating.

**User story.** I invoke *Capture region*. The screen dims, the hint
says "drag to select", and the toolbar tells me — visually — that
drawing tools are not part of this moment. My only job is the region.
Once I commit it (crop in the editor, a drawn region in the overlay),
the rail wakes up and the editing session I expect begins.

**Why disable, not hide** (user-confirmed): the rail stays
structurally stable — chips keep their positions, so the tool set is
discoverable and the layout never reflows. Locked-but-visible reads
as "soon", hidden reads as "absent". This also mirrors what the
overlay's behavior already implies, adding only the missing visual
affordance.

**Success looks like:**

- During the editor phase, clicking or hotkeying any non-Select tool
  does nothing; chips render greyed with a tooltip saying when they
  unlock. After crop, the full rail is live and stays live.
- In the overlay, before the first region drag, pressing any
  non-Select tool key does nothing; Select stays armed. Once a
  usable selection exists, tool keys and the (now-visible) rail
  work exactly as today.
- `fullscreen`, `open`, `pin`, and post-crop editor sessions are
  byte-identical to today.

## Scope (in / out)

**In:**

- `toolbar.rs`: tool chips gain an enabled/disabled rendering state —
  greyed glyph, no click sense, lock tooltip ("locked — select a
  region first" / overlay variant). The enable-set is computed by
  each shell and passed into the rail draw, so the chip primitives
  stay shell-agnostic.
- `app.rs` (editor): while `phase_active()`, the rail enable-set is
  `{Select}` only; tool-switch hotkeys (the TOOLS `key` column) are
  inert for non-Select tools; annotation pointer input is denied
  for the whole phase (simpler and stricter than the overlay's
  geometry gate — the phase has no committed region to confine to).
  On phase end (crop), the full rail + hotkeys + input restore. The
  existing flip-Select→Move-on-latch stays.
- `overlay.rs`: gate the single-key tool switch on
  `usable_selection` — only `Select` arms pre-selection. No visual
  change: the floating rail already hides without a selection
  (planning correction, above); `annotation_press_allowed` and its
  tests are unchanged.
- Doc-comment revision in `end_phase_on_crop` (the "tool the user
  picked mid-phase is their explicit choice" clause becomes dead).
- Hint-line copy review: the phase hint stays truthful (it never
  mentioned tools); no change expected, confirm during /planning.
- ADR-0043 (revises ADR-0035's consequence table: the phase is
  tool-locked; per-shell unlock semantics — editor: on crop, overlay:
  on usable selection), CHANGELOG under `[Unreleased]`, TEST_MATRIX
  rows for both shells.

**Out:**

- Any config key / escape hatch — hardcoded, matching
  region-selection-first's v1 "no new config" decision.
- Hiding chips or Flameshot-style reveal-after-selection — considered
  and rejected (rail stability, discoverability).
- Any `flintshot-core` change — gating is shell-owned state; `Tool`,
  `SelectionPhase`, and `annotation_press_allowed` are reused as-is.
- Changing the overlay's behavioral gate semantics or its keyboard
  handling beyond making tool-switch keys respect the same enable-set.
- Annotation-object visibility during region re-drags (the other
  reading of "hide annotation objects") — separate concern, not this
  effort.
- `fullscreen` / `open` / `pin` paths, daemon / tray / IPC / COSMIC
  code — untouched.

## Deliverables

1. `flintshot-editor-egui/src/toolbar.rs`: disabled-chip rendering
   (greyed glyph, hover-only sense, lock tooltip) + an enable-set
   input on the rail draw; unit tests for the rendering predicate.
2. `flintshot-editor-egui/src/app.rs`: phase-scoped enable-set,
   hotkey gating, whole-phase annotation-input denial; unit tests
   (phase-active lock, crop restores, fullscreen/open unaffected).
3. `flintshot-editor-egui/src/overlay.rs`: keyboard tool-switch
   gated on `usable_selection`; unit tests (no-selection → only
   Select armable, usable selection → all keys live, degenerate
   selection counts as absent).
4. `docs/adrs/ADR-0043-selection-phase-tool-lock.md`, CHANGELOG
   entry, TEST_MATRIX rows (editor phase lock/unlock, overlay
   pre/post-selection rail state, fullscreen unregressed).

## Exit criteria

1. Editor, phase live: every non-Select chip renders disabled;
   clicking it and pressing its hotkey both do nothing; a direct
   annotation pointer press (synthetic) commits nothing. Select,
   Esc-cancel, Enter-crop, outputs (Copy/Save/Pin/Upload), undo/redo
   keep today's phase behaviour.
2. Editor, after crop: all chips live, hotkeys live, annotation
   input flows — identical to a `fullscreen` session.
3. Overlay, no usable selection: pressing a non-Select tool key
   leaves `state.tool` unchanged; with a usable selection: all tool
   keys live. Floating-rail visibility and
   `annotation_press_allowed` tests unchanged and green.
4. `flintshot fullscreen` / `open`: no disabled chips, no gating —
   pinned by tests.
5. All standing gates green: `fmt`, `clippy --workspace
   --all-targets -- -D warnings`, workspace tests, doc tests,
   `no_gui_types_in_core.rs`.

## Technologies

Unchanged workspace stack; no new dependencies. The disabled state
is painter-level (dimmed stroke color from the existing theme) plus
a sense downgrade on the chip's `interact` call — no new widget
machinery.

## Commands

Standing gate (AGENTS.md). Effort-specific entry points:

```sh
cargo run -- region                # editor mode: phase lock visible pre-crop
cargo run -- region --overlay      # X11: overlay rail locked until first drag
cargo run -- fullscreen            # plain editor — unregressed
cargo test -p flintshot-editor-egui
```

## Project structure

| Touch                                   | What                                  |
| --------------------------------------- | ------------------------------------- |
| `crates/flintshot-editor-egui/src/toolbar.rs` | disabled-chip rendering + enable-set |
| `crates/flintshot-editor-egui/src/app.rs`    | phase gating: rail, hotkeys, input   |
| `crates/flintshot-editor-egui/src/overlay.rs`| floating-rail pre-selection gating   |
| `docs/adrs/ADR-0043-*.md`, `CHANGELOG.md`, `docs/TEST_MATRIX.md` | docs |

No new files elsewhere; no crate-level layout change.

## Code style

House rules (AGENTS.md): `#![forbid(unsafe_code)]`, pedantic clippy,
no comments restating code, `#[non_exhaustive]` not needed (private
items). New surface is `pub(crate)` — the enable-set shape:

```rust
// Shell computes the set; the rail only renders it.
fn rail_enable_set(phase_locked: bool) -> &'static [Tool] {
    if phase_locked { &[Tool::Select] } else { ALL_TOOLS }
}
```

## Testing strategy

- `toolbar.rs` unit tests: disabled chip is hover-only (no
  `ToolbarAction`/tool-switch emission), tooltip copy.
- `app.rs` unit tests (existing phase-test module): hotkey inert
  mid-phase, annotation press denied mid-phase, post-crop restore,
  fullscreen/open constructors never gate.
- `overlay.rs` unit tests: enable-set by selection state
  (none / degenerate / usable).
- Manual rows in `docs/TEST_MATRIX.md` for both shells (needs
  display).
- No core tests — core is untouched (`no_gui_types_in_core.rs`
  stays green by construction).

## Boundaries

- **Always:** run the standing gates before declaring done; keep the
  overlay's behavioral gate and its tests untouched; keep outputs
  (Copy/Save/Pin/Upload) live during the editor phase per the
  shipped spec.
- **Ask first:** any change to `flintshot-core`; any new config key;
  touching the daemon/tray/IPC path; changing hint-line copy.
- **Never:** hide/remove chips instead of disabling (decided);
  `#[allow]` to silence clippy without a justifying comment;
  annotation gating that leaks past the phase (post-crop sessions
  must be untouched).

## Success criteria

- A user in the selection phase cannot annotate by any means
  (click, hotkey, drag) on either shell — and the UI says why.
- After the region is committed, neither shell remembers the lock.
- One rendering implementation in the editor rail; the overlay adds
  only a keyboard gate — no second chip primitive, no overlay
  layout change.

## Decisions

- **Disable, not hide** → rail stability + discoverability; locked
  reads as "soon". (User-confirmed.)
- **Unlock on crop (editor), on usable selection (overlay)** — each
  shell's "region is committed" equivalent; the overlay's semantics
  are already selection-based, the editor's phase is crop-based.
  (User-confirmed for the editor; overlay follows its existing
  gate.)
- **Both shells** — the asymmetry (ungated editor rail; ungated
  overlay keyboard) was the actual bug-shaped smell. Planning
  narrowed the overlay half to its keyboard path after confirming
  the floating rail already hides pre-selection. (User-confirmed;
  scope corrected 2026-09-07.)
- **Hardcoded, no config** — matches ADR-0035's v1 philosophy;
  escape hatch deferred until someone asks. (User-confirmed.)

## Open questions

_Resolved during planning (2026-09-07):_

1. Hint-line "tools unlock after crop" clause — **no change.** The
   hint never mentioned tools; the lock tooltips answer the question
   where it arises (on the chip).
2. Overlay Move chip pre-selection — **moot.** The floating rail
   doesn't render without a usable selection; there is no chip to
   treat. Canvas Move presses keep flowing (pinned by
   `select_and_move_presses_always_flow`).
