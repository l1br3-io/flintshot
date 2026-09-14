# Plan: portal `clipboard:///` grab is read back, not rejected

## Overview

COSMIC's screenshot portal answers Copy-confirmed captures with
`clipboard:///` — the grab is on the system clipboard, not in a
file. flintshot currently hard-errors at `parse_portal_uri`,
killing every post-grab action invisibly (error only in
daemon.log). Fix at the grab seam: the portal backend marks the
outcome through the existing error channel (exported canonical
constant), and the app layer detects the marker, reads the pixels
back via arboard, and continues normal dispatch. Spec:
`docs/specs/portal-clipboard-uri/spec.md`.

## Architecture Decisions

- **Marker through the existing error channel, not a trait
  change.** `FlintshotError::Capture { detail }` already carries
  classified outcomes across the core→app boundary
  (`quiet_capture_exit` matches "cancelled"/"timed out" the same
  way). A `pub const` in `capture::portal` keeps the string
  single-sourced; both producer and consumer reference it. A
  `GrabOutcome` enum would be more explicit but costs a `Capturer`
  trait break across two backends for a one-consumer seam.
- **Read-back in the app layer, arboard stays out of core.**
  `flintshot-core` is types/traits/pure logic; a clipboard client
  is I/O. `flintshot-app` already owns arboard and the clipboard
  failure semantics (`copy_image_to_clipboard`).
- **Marker resolves before dispatch, arms stay symmetric.** The
  read-back happens in `native_grab_prepared`, so every post-grab
  arm (`--out`, editor, clipboard, upload) gets pixels without
  knowing how the grab arrived. No per-arm special cases.

## Task Index

### Phase 1 — Behavior
- [ ] Task 1: Portal marks `clipboard:///`; app reads the grab back → `tasks/001-marker-and-read-back.md`

### Checkpoint: behavior live (HUMAN GATE)
- [ ] Full gate green (`fmt`, `clippy -D warnings`, `test --workspace --all-targets`, doc-tests)
- [ ] Human confirms on the COSMIC session: OS screenshot UI **Copy** → capture completes (post-grab action runs, no `non-file uri` error in daemon.log); **Save/Enter** path unchanged

### Phase 2 — Record
- [ ] Task 2: CHANGELOG entry + spec reconcile (skip if Task 1's entry suffices) → `tasks/002-changelog.md`

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Selection read race: another app takes the clipboard between portal copy and read-back | Low — milliseconds, portal serves from a daemon process | Read-back failure is a hard, contextual error (never a wrong image); user retries |
| arboard `get_image` returns non-RGBA bytes or mismatched dimensions on odd portals | Low | `RgbaImage::from_raw` `None` → explicit capture error naming the mismatch |
| Marker string drift between producer and consumer | None | Single `pub const`, both sites compile against it; unit tests on both sides |

## Open Questions

- None blocking.
