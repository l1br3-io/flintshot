# Plan: clipboard post-grab opens the editor

## Overview

Make `after_grab = clipboard` on the native (portal/Wayland) capture
path copy the grab **and** open the editor, closing the per-backend
inconsistency with the fallback path (which always lands in the
editor). The copy + notification are unconditional after a successful
grab; the editor-open follows the existing `editor` arm's shell path
and supersede check. One behavior change in
`native_capture_and_dispatch` (`crates/flintshot-app/src/cli.rs`) plus
the doc/CHANGELOG sweep it makes stale. Spec:
`docs/specs/clipboard-opens-editor/spec.md`.

## Architecture Decisions

- **Copy before editor-open, unconditionally.** The copy is the
  requested action — a supersede between grab and shell must not
  swallow it. Only the editor-open is supersede-gated (mirroring the
  `editor` arm). Copy failure stays a hard error: no editor on a
  failed copy.
- **Reuse `run_capture_shell`, no new plumbing.** The Clipboard arm
  calls the same entry as the `editor` arm (`run_capture_shell(img,
  "capture", interaction, watch)`), so interaction-mode resolution
  (editor window vs Phase 5 overlay), config fallback, and outcome
  reporting stay identical across arms. Consequence: on Wayland with
  `[ui] interaction_mode` resolving to overlay, `after=clipboard`
  opens the overlay — Enter there re-copies the (possibly annotated)
  region, same as `after=editor` today. Consistent, not a bug.
- **CHANGELOG edited in place, not appended.** The `after_grab`
  **Added** entry describing the window-less clipboard flow is still
  under `[Unreleased]` — it never shipped. This effort amends that
  entry to describe the final behavior instead of adding a
  contradictory **Changed** bullet within the same release block.
- **ADR-0051 gets a dated addendum, not a new ADR.** The window-less
  note is a consequence line inside an existing ADR, and this change
  is a UX-consistency adjustment, not a new architectural decision. A
  short addendum pointing at this effort's spec keeps the record
  honest without ADR inflation.

## Task Index

### Phase 1 — Behavior
- [x] Task 1: Clipboard arm copies, notifies, then routes through `run_capture_shell` → `tasks/001-clipboard-routes-to-editor.md`

### Checkpoint: behavior live (HUMAN GATE)
- [x] Full gate green (`fmt`, `clippy -D warnings`, `test --workspace --all-targets`, doc-tests)
- [ ] Human runs `flintshot capture --after clipboard` on the Wayland session: clipboard holds the grab, notification fires, editor/overlay opens pre-loaded; `--after editor` / `--after upload` / `--out` / `--backend xcap` unchanged

### Phase 2 — Record
- [ ] Task 2: Doc + CHANGELOG sweep (TEST_MATRIX, CHANGELOG in-place, ADR-0051 addendum, spec reconcile) → `tasks/002-docs-changelog.md`

### Checkpoint: complete
- [ ] Spec success criteria met; harness linters green; ready for PR

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Up-front copy's arboard handle drops before the shell opens; on X11 (owner-based clipboard) the selection could die with it | Med — but only reachable via `--backend portal` pinned on an X11 session (auto never picks portal there); today's direct-copy path dies *sooner* (process exits right after) | Accept for now: strictly improves the pinned case; the editor session keeps the process alive as long as the window is open; note in task file |
| `run_capture_shell` opens the Phase 5 overlay (Enter = re-copy) rather than the classic editor window when interaction mode resolves overlay | Low | Intended consequence of arm symmetry (see Architecture Decisions); manual checkpoint verifies it feels right |
| Editing the unreleased CHANGELOG entry in place rewrites history reviewers already read | Low | Flag in the PR description; entry is under `[Unreleased]` so no user-facing history changes |

## Open Questions

- None blocking. `/build` starts from a fresh branch:
  `git checkout -b clipboard-opens-editor` (effort slug = branch name
  = spec directory).
