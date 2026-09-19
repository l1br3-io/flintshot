---
status: active
started: 2026-09-19
---

# Spec — Text select & re-edit: select committed text, restyle it, delete it

> Mini-effort spec. For product context, see
> [`../../VISION.md`](../../VISION.md); for the editor architecture,
> see [Phase 1](../phase-1-capture-editor/spec.md); for the
> selection-chrome lineage, see
> [annotation-move](../annotation-move/spec.md) and
> [annotation-delete](../annotation-delete/spec.md).

## Objective

Today a committed text annotation is a dead end: clicking it with the
Text tool starts a *new* text on top of it (`dispatch_text_event`,
editor.rs), its style can never be changed (the panel edits session
tool state only — committed annotations are style snapshots), and
deleting requires knowing the Move-tool + `Del` dance. Users expect
text to behave like text: click it to select it, edit it again,
restyle it, remove it.

With `Tool::Text` active:

- **Single click on an existing text** selects it — dashed selection
  chrome, and the panel's Text section shows *that annotation's*
  values instead of session defaults.
- **Double-click on an existing text** re-opens content editing
  (caret at end of text) — full modal path: type, arrows, Home/End,
  Shift+Enter newlines, Backspace.
- **While selected (or editing)**, panel style changes write through
  to the annotation live — colour, size, family, weight, style,
  align, line-spacing, outline, background — *and* update the session
  defaults for the next text.
- **`Del` / `Backspace` with a selection** deletes the text (one undo
  entry). **`Esc` with a selection** deselects without closing the
  editor. **`Esc` mid-edit** reverts the whole edit session.

**Success looks like:**

1. Place "hello", press `Enter`. Text tool, single click on it →
   dashed outline; the panel shows its actual size / colour / family.
2. Double-click → caret at end; append " world"; `Enter`. Exactly
   **one** `Ctrl+Z` reverts the whole re-edit (back to "hello").
3. With it selected, pick a new colour in the panel → the text
   recolours immediately; the *next* text placed uses that colour
   too; one `Ctrl+Z` per discrete control change.
4. `Del` → text gone; `Ctrl+Z` → back.
5. Click on empty canvas still starts a fresh draft; every existing
   fresh-draft behaviour is byte-for-byte unchanged.

## Assumptions

1. "Existing text" means a **committed** `Annotation::Text` in
   `anns`. The in-flight draft already has full caret editing.
2. Selection / edit semantics live in `flintshot-core`'s dispatcher;
   both shells (editor window + fullscreen overlay) get identical
   behaviour via routing only.
3. Deleting text is technically possible today (Move + `Del`); this
   effort makes it first-class *from the Text context* — no Move
   detour.
4. Only **Text** annotations are selectable under `Tool::Text`.
   Clicking a non-text annotation with the Text tool is a miss (new
   caret starts, as today).
5. Committed-annotation restyling is **Text-only** this effort.
   Generalizing write-through to shapes / blurs / numbers is a future
   effort; the core restyle API is the hook.
6. Text hit-testing keeps the coarse bounding-box approximation
   (`Annotation::contains`) — no glyph-level hit regions.

## Scope (in / out)

**In:**

- Core: `Tool::Text` `PointerDown` hit-tests first — text hit ⇒
  select; miss ⇒ `begin_annotation` (fresh draft, unchanged).
- Core: double-click on a text enters **re-edit** via lift-out: the
  annotation moves out of `anns` into `pending` (reusing the entire
  existing modal path — cursor ops, `is_editing_text`, commit /
  cancel), with the original stored for revert + exact-once undo.
- Core: `InputEvent` gains a `DoubleClick { pos }` variant so shells
  can surface toolkit double-clicks without churning every existing
  `PointerDown` construction site (~80 test sites vs two prod
  pumps).
- Core: `set_tool` clears a Text-tool selection when leaving Text
  (mirrors the Move rule); `undo` / `redo` / `crop` already clear
  selection generically.
- Core: a gesture-bracketed restyle API (`begin_style_gesture` /
  `restyle_selected_text` / `end_style_gesture`) that writes the
  annotation **and** syncs session defaults, pushing exactly one undo
  snapshot per gesture.
- egui: selection chrome widens from Move-only to Move + Text
  (`selected_chrome`); hover affordance (dashed outline) on text
  under the pointer with the Text tool active.
- egui: panel Text section reads from the selected text when one is
  selected (else session defaults) and routes changes through the
  core restyle method.
- egui: double-click detection at both shells' event boundaries.
- Tests: core dispatcher semantics (select / re-edit lifecycle, undo
  exactness, revert, defaults sync); shell routing + chrome gating.

**Out:**

- Style write-through for non-Text annotation types.
- Caret-from-click placement on re-edit (v1 places the caret at the
  end; glyph-metric hit mapping needs shell-side shaping info).
- Drag-moving a text under the Text tool (Move owns translation).
- Marquee / multi-selection.
- Live style application to a *fresh* draft mid-typing (unchanged:
  style is snapshotted at placement; see Open Questions).
- Adding typography controls to the overlay shell — its text surface
  is the shared colour popover (`popover.rs`); the full typography
  panel is editor-window UX.
- Any change to Save / Copy / Upload composition — selection is
  chrome-only and never composed into output.

## Deliverables

1. `flintshot-core`: text-select / re-edit state machine, restyle
   method, `click_count` on `PointerDown`, selection-lifecycle
   guards, tests.
2. `flintshot-editor-egui`: chrome widening, panel read/write
   retarget, double-click plumbing in both shells, tests.
3. `CHANGELOG.md` entry + `docs/TEST_MATRIX.md` rows.
4. ADR for the committed-annotation write-through model
   (annotations stop being pure style snapshots — selection makes
   them restylable).
5. This spec + task files.

## Exit criteria

1. `cargo run -- capture` → place text → `Enter` → single click on
   it with Text tool: selection chrome appears, panel reflects its
   values; `Esc` deselects; editor stays open.
2. Double-click → caret at end → edit → `Enter`: one `Ctrl+Z`
   reverts the entire session; `Esc` mid-edit reverts without an
   undo entry.
3. Selected → change size and colour via panel: text updates live,
   next placed text inherits both, each discrete change is one undo
   step. A size-slider drag is a single step.
4. `Del` on selection deletes (one undo entry); click-miss starts a
   fresh draft exactly as before.
5. Overlay shell: same select / re-edit / delete flows; region
   handles and Move-tool flows untouched.
6. Standing gates green (`fmt --check`, `clippy --workspace
   --all-targets -- -D warnings`, `cargo test --workspace
   --all-targets`, doc tests for the two lib crates).

## Technologies

No new dependencies. Pure logic in `flintshot-core::editor` /
`input`; routing + chrome in `flintshot-editor-egui` (`chrome.rs`,
`panel.rs`, both shells' event boundaries).

## Commands

```sh
cargo test -p flintshot-core                 # dispatcher + lifecycle tests
cargo test -p flintshot-editor-egui          # chrome + routing tests
# standing gate (AGENTS.md)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

## Project structure

| Change                        | Location                                  |
| ----------------------------- | ----------------------------------------- |
| Select / re-edit / restyle    | `crates/flintshot-core/src/editor.rs`     |
| `DoubleClick` plumbing        | `crates/flintshot-core/src/input.rs` + both shells |
| Chrome widening (select/hover)| `crates/flintshot-editor-egui/src/chrome.rs` |
| Panel retarget                | `crates/flintshot-editor-egui/src/panel.rs` |
| Event boundaries (dblclick)   | `crates/flintshot-editor-egui/src/{shell,overlay}.rs` |
| Docs                          | `CHANGELOG.md`, `docs/TEST_MATRIX.md`, `docs/adrs/` |

## Code style

Per [`AGENTS.md`](../../../AGENTS.md): `#[must_use]` on pure
queries, doc comments on every new public item (`missing_docs`),
`#[non_exhaustive]` where plausible, no `unwrap()` / `expect()`
outside tests, no GUI types in core, snapshot-before-mutate
ordering.

## Testing strategy

- **Core unit tests** (alongside existing dispatcher tests):
  - select on hit / deselect on miss / `set_tool` away clears;
  - re-edit lifecycle: enter, edit, commit (one undo entry),
    revert (no entry), no-op edit (no entry), splice-back index
    stability;
  - restyle: write-through + defaults sync + one-entry-per-change;
  - `click_count` routing (single vs double on text / on empty).
- **egui unit tests**: chrome gating (`selected_chrome` under Text),
  hover affordance, panel reflection source-switch.
- **Manual**: exit criteria 1–5 in both shells.

## Boundaries

- **Always:** every mutation flows through the dispatcher or the
  documented core restyle method; undo exactness (one entry per
  contract) is asserted by tests; selection never leaks into
  composed output; standing gate before declaring done.
- **Ask first:** any change to fresh-draft (new-text) modal
  behaviour; extending selection / write-through beyond Text; new
  public API beyond `PointerDown.click_count` + the restyle method +
  the re-edit entry points; changing `Esc`-closes-editor defaults
  when *nothing* is selected / editing.
- **Never:** GUI types in `flintshot-core`; shell-side duplication
  of selection / edit / undo math; restyle bypassing the session
  defaults sync.

## Success criteria

- Committed text feels alive: selectable, re-editable, restylable,
  deletable — without leaving the Text tool.
- Zero behaviour change for fresh drafts and every non-Text tool.
- Undo history stays honest: one gesture ⇒ one entry, everywhere.

## Decisions

- **Click selects, double-click edits** — matches Move-tool
  selection precedent and Figma/Excalidraw convention; single-click
  re-edit was rejected as too easy to trigger accidentally when
  placing adjacent labels.
- **Lift-out re-edit** — the double-clicked annotation moves into
  `pending`, reusing the *entire* existing text modal path (cursor
  ops, key suppression, commit / cancel) with zero duplication. The
  original is stored in the session record; commit splices the
  edited text back at its index and pushes a snapshot containing the
  original (exactly one undo entry, constructed at commit — no lazy
  push / pop dance); `Esc` splices the original back with no history
  touch.
- **Write-through also sets session defaults** — restyling the
  selection updates the "current style" for the next text
  (Flameshot-like); avoids the two-styles mental model.
- **Undo granularity** — re-edit session = one entry (covering
  content *and* any style changes made mid-session: the text is
  lifted out of `anns` during the session, so an independent
  mid-session style snapshot would undo-delete the text — absorption
  is an invariant of the lift-out design, not a convenience);
  selected-but-not-editing = one entry per gesture (a discrete
  control click is a one-tick gesture; a continuous slider /
  DragValue drag coalesces into one entry per drag).
- **`Esc` semantics** — mid-edit: revert whole session (the re-edit
  analog of cancel-draft); selected-not-editing: deselect only;
  otherwise: unchanged (`Cancel` outcome).

## Open questions

1. Should panel style changes apply *live* to a fresh (new-text)
   draft mid-typing? Today style is snapshotted at placement;
   **default: leave unchanged** this effort (the re-edit path covers
   the "oops, wrong colour" case — type, commit, restyle).
2. Caret-from-click on re-edit (v2?): needs shell-side glyph
   metrics; revisit if end-of-caret feels wrong in practice.
3. Generalization path: once write-through proves out on Text,
   extend the same restyle method to Move-tool selections of other
   variants (stroke / fill / blur strength / number diameter)?
