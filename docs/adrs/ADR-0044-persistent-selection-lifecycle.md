# ADR-0044: Persistent selection survives release, dies with every mutation — and is chrome-only

**Context.** Deleting exactly one annotation (annotation-delete
spec) needs a *selection* to delete: a Move-tool click picks the
annotation, `Del` removes it. The pre-effort `EditorState` had no
such concept — `end_move` cleared `selected_index` on every pointer
release, and nothing rendered it. Three lifecycle questions had
non-obvious answers:

1. **When does a selection live?** A click-and-release (no drag) and
   a post-move-release both end the gesture; the old code treated
   "gesture over" as "selection over".
2. **Who owns deselection?** Six distinct events can invalidate a
   selection (miss press, tool switch, delete, undo, redo, crop) and
   the shells add a seventh (`Esc`, layered deselect-first-then-
   cancel). Spraying `selected_index = None` across shells would
   duplicate the invariant per shell — the exact drift the dispatcher
   exists to prevent.
3. **Is selection content or chrome?** A visible outline around the
   selected annotation could leak into Save / Copy / Pin / Upload
   output if it rode the shared `render()` path, breaking WYSIWYG
   (ADR-0015) — the saved PNG must be exactly what was annotated.

Alternatives considered:

- **Ephemeral selection** (clear on release, like hover chrome):
  rejected — `Del` after releasing the click would have nothing to
  act on; the delete affordance requires the selection to outlive
  the selecting gesture.
- **A `deselect()` method the shells call from each invalidating
  event**: rejected — every future mutation site becomes a
  forgetting hazard; core owning the invariants keeps `Del`-able
  state consistent by construction.
- **Selection as an annotation-adjacent render property** (draw the
  outline inside `render_ann`): rejected — the compose path would
  need a suppression flag, one mistake away from shipping editor
  chrome in a user's PNG.

**Decision.** The selection is core state with a
mutation-kills-it lifecycle, rendered exclusively as shell chrome:

- **Release keeps it.** `PointerUp` on a Move grab sets
  `selected_index` and it survives the gesture's end (click-without-
  drag and post-move release alike). A Move press that hits no
  annotation clears it. Re-grabbing a different annotation replaces
  the selection (last click wins — spec open question 1's default).
- **Every invalidator lives in core.** `set_tool` drops the
  selection when leaving Move (a selection is meaningless under a
  drawing tool — `set_tool` is the single tool-switch chokepoint, so
  hotkeys and chip clicks cannot bypass it); `delete_selected`,
  `undo`, `redo`, and `crop` all clear it (the first three via
  `end_move`, which the mutations already call to end in-flight
  drags — the persistent selection rides the same seam). Shells add
  only the layered `Esc`: with a selection, the first `Esc` clears
  it and is consumed; only a selection-less `Esc` forwards to the
  dispatcher's unchanged `Cancel` contract.
- **Selection is chrome-only.** The dashed ember outline (and the
  hover/grab outlines before it) is computed by
  `flintshot-editor-egui::chrome` from `selected_index` /
  `bounding_box()` and painted *after* the shared `render()` call.
  `compose_for_save` / `compose_final` never see it; a degenerate
  bbox (nothing to outline) deselects shell-side rather than
  rendering an empty affordance.

**Consequences.**

- `Del` / `Backspace` and the rail's Delete button act on exactly
  the annotation whose outline is on screen, however long ago it was
  clicked; there is no time-boxed or hover-scoped selection to
  explain.
- One invariant, one owner: adding a future mutation (restyle,
  reorder, …) that should drop the selection means touching the
  core method, not every shell. Shells that forget the Esc layer
  merely lose the deselect-first nicety — the dispatcher's Cancel
  meaning is untouched.
- Composed output is selection-free by construction, not by testing
  discipline; the WYSIWYG guarantee needs no suppression flag that
  a new output path could forget to pass.
- The selection is Move-tool-scoped: switching tools is also a
  deselect, so a selection can never dangle under a tool that cannot
  act on it (and `selected_index` is not public mutation surface —
  it moves only through dispatcher input).
- Cost: `Esc` semantics are shell-layered rather than
  dispatcher-uniform — accepted (spec decision) to keep
  `DispatchOutcome::Cancel` stable for future shells; the editor
  window and overlay implement the identical layering, unit-pinned
  per shell.
