---
status: active
started: 2026-09-07
---

# Spec — Annotation delete (select an annotation, remove it)

> UX + core effort. The editor can move a committed annotation (Move
> tool, `region-selection-first` era) but cannot remove an individual
> one: the only removal paths are Ctrl+Z (LIFO, unwinds everything
> after the target) and crop (incidental). This effort adds the
> Flameshot-standard select-then-delete affordance.

## Objective

Let the user delete exactly one annotation without disturbing the
rest of the history:

1. **Select.** With the Move tool, a click on a committed annotation
   selects it and keeps it selected after pointer release (today
   `selected_index` is cleared on release — `editor.rs` `end_move` —
   and nothing renders it).
2. **See.** A selected annotation shows a dashed bounding-box outline
   in the theme's selection colour. Selection is editor chrome: never
   composed into Save / Copy / Pin / Upload output.
3. **Delete.** `Del` (and `Backspace`) or the rail's **Delete**
   button removes the selected annotation. Delete pushes an
   `anns`-only snapshot onto `undo`, clears `redo`, and clears the
   selection — fully undoable, exactly like every other mutation.
4. **Button affordance.** A Delete button sits beside Undo/Redo on
   both rails (editor toolbar, overlay rail), greyed until an
   annotation is selected — the same replay-a-keyboard-dispatch
   pattern the Undo/Redo buttons already use, and the same
   disabled-reads-as-"soon" discoverability stance as
   `select-first-tool-lock`.
5. **Deselect.** Clicking empty canvas, pressing `Esc` with a
   selection, switching away from the Move tool, deleting, undo /
   redo, or crop all drop the selection.

**User story.** I've annotated a screenshot with arrows, text, and a
blur. One arrow is wrong. Today I must Ctrl+Z four times and redo
three annotations. After this effort I click the arrow with Move, it
shows its dashed outline, I press `Del`, it's gone; Ctrl+Z brings it
back if I misclicked.

**Success looks like:**

- Click-annotation → dashed bbox appears and stays after release.
- `Del` / `Backspace` removes it; `Ctrl+Z` restores it (redo of a
  delete works symmetrically).
- The rail's Delete button is greyed with no selection, enabled with
  one, and its click dispatches identically to `Del`.
- Click empty canvas → outline disappears.
- `Esc` with a selection only deselects (a second `Esc` cancels the
  editor — behaviour otherwise unchanged).
- Deleting while a text draft is being edited is impossible:
  `Backspace` edits the draft (existing text-editing modal mode gates
  this by construction).

## Scope (in / out)

**In — `flintshot-core`:**

- `geom.rs`: new `Key::Delete` variant (enum is `#[non_exhaustive]`;
  additive).
- `editor.rs`:
  - Persistent selection: `PointerUp` on a Move grab keeps
    `selected_index` (click-without-drag *and* post-move release);
    a Move press that misses all annotations clears it.
  - `EditorState::delete_selected()` — removes
    `anns[selected_index]`, pushes `Snapshot::anns_only`
    (pre-delete), clears `redo` + selection. No-op when nothing is
    selected. If a move drag is in flight for the deleted annotation,
    the drag ends first.
  - `dispatch_key`: `Key::Delete` and `Key::Backspace` (no
    modifiers) → `delete_selected()`. Esc stays `Cancel` at the
    dispatcher level; the **shell** layers deselect-before-cancel
    (see below).
  - Selection-clearing invariants: `undo()`, `redo()`, `crop()` clear
    `selected_index` (they already end the move; extend to the
    persistent selection). Tool switch away from Move clears it.
- Re-export / doc-comment updates in `lib.rs` as needed.

**In — `flintshot-editor-egui`:**

- `app.rs` key translation: `egui::Key::Delete =>
  Some(Key::Delete)`.
- `toolbar.rs`: `ToolbarAction::Delete` variant + trash glyph
  (`g_trash`); the button renders on the editor rail beside
  Undo/Redo and on the overlay rail's undo/redo row, with an
  enabled-state input (`selected_index.is_some()`) computed by each
  shell — same shell-agnostic pattern as the
  `select-first-tool-lock` enable-set.
- Esc layering (both editor window and overlay): `Esc` when
  `selected_index.is_some()` → dispatch a deselect; otherwise
  today's Cancel. Implemented shell-side so the dispatcher's Esc
  contract is untouched.
- Selection outline rendering: dashed bbox from
  `anns[selected_index].bounding_box()` in the theme's selection
  colour, drawn with the same chrome approach as the region
  selection marching-ants. `bounding_box() == None` (degenerate
  annotation) → deselect instead of rendering.
- Move-tool hint copy gains the delete affordance (e.g.
  "Drag to move · Del remove") — subject to the hint-copy ask-first
  boundary, confirmed in review.
- Overlay (`overlay.rs`): inherits core semantics for free; verify
  the keyboard gate (`tool_for_key`-adjacent hotkey handling) passes
  `Del` through to the dispatcher rather than swallowing it.

**Out:**

- Delete via right-click context menu or Alt+click quick-delete.
- A modal Eraser *tool* (Win11 Snipping Tool / Greenshot style): an
  11th `Tool` variant that arms click-to-delete. Considered and
  rejected — it fights `flip_to_move`, duplicates Move-select, and
  the action-button pattern covers the same affordance (see
  Decisions).
- Multi-select / marquee select / delete-all.
- Deleting regions of the image itself (crop already does that).
- Any config key (`[ui]` or otherwise) — hardcoded, matching
  region-selection-first's v1 philosophy.
- Resizing or re-styling annotations (bbox outline is visual only).
- `flintshot-cosmic`, daemon / tray / IPC — untouched.

## Deliverables

1. `flintshot-core/src/geom.rs`: `Key::Delete`; exhaustive-match
   fallouts updated.
2. `flintshot-core/src/editor.rs`: persistent-selection release
   semantics, miss-deselect, `delete_selected()`, `Delete`/
   `Backspace` dispatch, selection-clear invariants; unit tests (see
   testing strategy).
3. `flintshot-editor-egui/src/toolbar.rs`: `ToolbarAction::Delete`,
   trash glyph, selection-gated enable state on both rails; unit
   tests (enabled predicate, action emission).
4. `flintshot-editor-egui/src/app.rs`: key translation, Esc
   layering, selection outline rendering, hint copy; unit tests.
5. `flintshot-editor-egui/src/overlay.rs`: `Del` passthrough
   verification + rail button + tests.
6. CHANGELOG under `[Unreleased]`; ADR only if a non-obvious choice
   emerges during planning (persistent-selection lifecycle is the
   candidate); `docs/TEST_MATRIX.md` manual rows.

## Exit criteria

1. Move-tool click on an annotation leaves it selected with a
   visible dashed bbox after release; a move-drag release behaves
   identically (still selected).
2. `Del` / `Backspace` deletes it; `Ctrl+Z` restores exactly it;
   `Ctrl+Shift+Z` re-deletes. Other annotations and `next_number`
   untouched by the delete itself.
3. Click empty canvas / switch tool / `Esc` deselects; second `Esc`
   cancels the editor. `Esc` with no selection cancels immediately
   (today's behaviour).
4. The rail Delete button: greyed + inert with no selection, enabled
   + deletes with one, on both the editor toolbar and the overlay
   rail.
5. Undo / redo / crop drop the selection (no dangling indexes).
6. Backspace inside a text draft edits text — never deletes a
   committed annotation (text modal mode gates it).
7. Selection-first phase (`select-first-tool-lock`): `Del` and the
   button are inert mid-phase (nothing deletable; no phase-behaviour
   change).
8. Editor window and X11 overlay both behave per 1–7; `fullscreen`
   and `open` sessions identical to today plus the new affordance.
9. Standing gates green: `fmt`, `clippy --workspace --all-targets --
   -D warnings`, workspace tests, doc tests,
   `no_gui_types_in_core.rs`.

## Technologies

Unchanged stack, no new dependencies. Core gets a new key variant
and a new state transition; the shell gets chrome rendering (egui
painter, no new widget machinery).

## Commands

```sh
cargo build                            # default members
cargo test --workspace --all-targets   # all gates' unit + integration
cargo test -p flintshot-core           # tight loop while editing core
cargo test -p flintshot-editor-egui    # shell loop
cargo run -- fullscreen                # fastest manual check: draw, click, Del
cargo run -- region                    # phase interaction + overlay path
```

## Project structure

| Touch                                        | What                                    |
| -------------------------------------------- | --------------------------------------- |
| `crates/flintshot-core/src/geom.rs`          | `Key::Delete`                           |
| `crates/flintshot-core/src/editor.rs`        | selection lifecycle + `delete_selected` |
| `crates/flintshot-editor-egui/src/toolbar.rs`| `ToolbarAction::Delete`, glyph, enable  |
| `crates/flintshot-editor-egui/src/app.rs`    | translation, Esc layering, outline, hint |
| `crates/flintshot-editor-egui/src/overlay.rs`| `Del` passthrough + rail button         |
| `crates/flintshot-core/src/lib.rs`           | re-export/doc fallout (if any)          |
| `CHANGELOG.md`, `docs/TEST_MATRIX.md`        | docs                                    |

No new files; no crate-level layout change.

## Code style

House rules (AGENTS.md): `#![forbid(unsafe_code)]`, pedantic clippy,
no `unwrap()` outside tests, comments only for non-obvious why,
`#[must_use]` on pure getters, `#[non_exhaustive]` n/a (existing
types). New public core surface:

```rust
/// Remove the selected annotation (`selected_index`).
///
/// Pushes an `anns`-only snapshot onto `undo` (pre-delete state),
/// clears `redo`, and drops the selection. Any in-flight move drag
/// ends first (its index dies with the removal). No-op when nothing
/// is selected.
pub fn delete_selected(&mut self) {
    let Some(index) = self.selected_index else { return };
    self.end_move();
    self.undo
        .push(Snapshot::anns_only(self.anns.clone(), self.next_number));
    self.redo.clear();
    self.anns.remove(index);
    self.selected_index = None;
}
```

## Testing strategy

- `editor.rs` unit tests (same module as the existing undo/move
  suite):
  - delete removes the right annotation, restores via undo, redo
    re-deletes; `next_number` preserved;
  - no-op with no selection / empty `anns`;
  - release keeps selection (click-without-drag and post-move);
  - Move-miss press clears selection;
  - selection cleared by undo / redo / crop / tool switch;
  - `Del` + `Backspace` dispatch (no modifiers); with Ctrl they do
    nothing (reserved);
  - text-draft mode: `Backspace` edits the draft, committed anns
    untouched;
  - delete of the annotation being dragged ends the drag cleanly.
- `toolbar.rs` unit tests: Delete button enabled iff selection
  exists; emits `ToolbarAction::Delete` only when enabled (greyed =
  hover-only, no click sense — same predicate style as the
  select-first lock chips).
- `app.rs` unit tests: egui→core key translation for `Delete`; Esc
    layering (selection → deselect, then cancel); selection-first
    phase leaves `Del` inert; `fullscreen`/`open` constructors
    unaffected.
- `overlay.rs` unit test: `Del` reaches the dispatcher (deletes)
    with a usable selection.
- `no_gui_types_in_core.rs` stays green by construction (new core
  surface is pure state).
- Manual rows in `docs/TEST_MATRIX.md` (needs display): both shells,
  plus confirm the outline never appears in saved output.

## Boundaries

- **Always:** run the standing gates before declaring done; every
  new mutation participates in undo/redo snapshot discipline; keep
  the text-editing modal mode's key monopoly intact; selection is
  chrome-only — never in composed output.
- **Ask first:** Esc-semantics changes beyond the layered
  deselect-first behavior described here; hint-line copy; any
  `flintshot-core` API beyond `Key::Delete` +
  `delete_selected()` + the selection-lifecycle transitions;
  touching daemon / tray / IPC / COSMIC.
- **Never:** delete without undo; multi-select scope creep; context
  menu UI; `#[allow]` to silence clippy without a justifying
  comment; changing `DispatchOutcome::Cancel`'s meaning.

## Success criteria

- One wrong annotation in a stack of many: click it, `Del`, done —
  zero collateral history damage, full undo symmetry.
- The affordance is discoverable (outline on select, greyed rail
  button, hint mentions `Del`) and Mac-viable (`Backspace`).
- Both shells behave identically via the shared dispatcher; no
  duplicated delete logic.

## Decisions

- **Click-select + Del key** (user-confirmed) — Flameshot behavior;
  context menu and quick-delete modifiers considered, out of scope.
- **Undoable delete** (user-confirmed) — snapshot discipline like
  every other mutation.
- **Rendered selection outline** (user-confirmed) — persistent
  selection needs a visible affordance; dashed bbox matches the
  region-selection chrome language.
- **Backspace aliases Del** outside text mode — Mac keyboards label
  Backspace "delete"; the modal text mode already owns Backspace
  during drafts, so the alias is collision-free.
- **Esc layered shell-side** — deselect-first-then-cancel keeps the
  dispatcher's Esc → `Cancel` contract stable for future shells.
- **Rail action button over modal Eraser tool** (user-confirmed) —
  Win11 Snipping Tool and Greenshot expose delete as a modal eraser
  *tool*; rejected here because it adds an 11th `Tool` variant,
  fights `flip_to_move`, and duplicates Move-select. The Delete
  *button* (beside Undo/Redo, enabled iff selected) is the same
  replay-a-keyboard-dispatch pattern those buttons already use, and
  greyed-but-visible keeps it discoverable.
- **Keyboard + button, no config** — `Del`/`Backspace` and the rail
  button share one dispatch path; context menu / quick-delete
  modifiers deferred until someone asks.

## Open questions

1. Should the selection survive a Move-tool re-grab of a *different*
   annotation (replace-on-click), or require an explicit deselect
   first? Default: replace-on-click (last click wins).
2. Number-tool bubbles: deleting a middle bubble leaves a numbering
   gap (1, 2, 4). Acceptable (renumbering is surprising and
   history-hostile)? Default: yes, leave gaps.
3. ADR needed for the persistent-selection lifecycle, or is the spec
   + code docs sufficient? Default: only if planning surfaces a
   non-obvious tradeoff.
