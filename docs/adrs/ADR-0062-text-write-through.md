# ADR-0062: Committed-annotation write-through via lift-out sessions

**Context.** Until the text-select-edit effort, a committed
`Annotation` was a pure style snapshot: the properties panel edited
*session tool state* only, so committed annotations could never be
restyled, and the only path to changing a text's content was delete +
retype. Users expect text to behave like text — click to select,
re-open to edit, restyle in place, delete from the same context. Two
design problems follow: how a selected annotation accepts style writes
without abandoning "annotations are snapshots" everywhere else, and
how a *content re-edit* reuses the entire fresh-draft modal path
(cursor ops, key suppression, commit/cancel) without duplicating it.

Toolkit double-click detection was a third, smaller problem: egui
0.35 classifies clicks on *release*, later than the press-frame
contract the dispatcher wants, and the pump's own press gating
(toolbar swallows, region holdout) must gate the gesture too.

**Decision.**

- **Selection makes an annotation restylable; nothing else does.**
  The Text tool gains a persistent selection (mirroring Move's):
  a press that hit-tests a committed Text selects it; the panel's
  Text section and the shared colour pod read from it (source order:
  re-edit draft > selection > session defaults — byte-identical to
  the old behaviour when nothing is selected). Writes flow through a
  gesture-bracketed core API — `begin_style_gesture` /
  `restyle_selected_text` / `end_style_gesture` — that writes the
  annotation **and** syncs the session defaults (one mental model:
  restyling the selection also sets the style for the next text),
  pushing exactly one undo entry per gesture. Fresh drafts still
  snapshot style at placement; live-restyle of a fresh draft stays
  out of scope.

- **Re-edit is a lift-out session, not a parallel editor.** A
  double-clicked text moves *out of* `anns` into `pending`, so the
  existing modal path applies verbatim. A `TextReeditSession
  { index, original }` record closes the loop: commit splices the
  edited text back at `index` and pushes one snapshot containing the
  original (one entry covers content *and* mid-session style changes
  — an independent mid-session snapshot would undo-delete the text,
  so absorption is a lift-out invariant, not a convenience); Esc
  splices the original back with no history touch; edited-to-empty
  removes the annotation with the same single entry. Shell-direct
  `crop` / `undo` / `redo` mid-session restore the original first
  (defensive guards; their indices refer to the pre-session list).

- **`InputEvent::DoubleClick { pos }` is an additive event, not a
  `click_count` on `PointerDown`.** Two prod pumps construct
  `PointerDown` but ~80 test sites would churn on a new field; a
  variant no-ops cleanly. Emission contract: shells emit it *in
  addition to* the second press's `PointerDown*, same frame, after
  it — press selects, event upgrades to a session. Detection is a
  shared press-time tracker using egui's own thresholds
  (`Options::input_options.max_double_click_delay` /
  `max_click_dist`), fed exactly when a `PointerDown` dispatches so
  swallowed presses never count toward a pair.

**Consequences.**

- Committed text is selectable, re-editable, restylable, and
  deletable without leaving the Text tool; every non-Text tool and
  every fresh-draft behaviour is unchanged (pinned by tests).
- Selection chrome (`selected_chrome`, the delete affordances, Esc
  layering) widens from Move-only to Move | Text; both shells get
  identical semantics via the shared queries, per the
  routing-not-duplication boundary.
- Undo history stays honest: one re-edit session = one entry, one
  style gesture = one entry (a DragValue drag coalesces via a
  frame-level primary-button-up close signal — the section body
  renders twice per frame, so per-render drag state lies).
- Write-through is Text-only for now; the restyle API is the hook
  for extending to Move-tool selections of other variants (stroke /
  fill / blur strength / number diameter).
- The lift-out means mid-session the annotation is briefly absent
  from `anns`; anything iterating `anns` shell-side (Save / Copy /
  Upload composition) sees the pre-edit state if it races the
  session — acceptable because the modal path suppresses those
  shortcuts anyway, and the defensive guards close the session
  before any shell-direct mutation.
