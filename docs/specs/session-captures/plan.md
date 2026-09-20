# Plan: Session Captures

## Overview

Unsaved captures survive editor navigation until app exit. A
`SessionStore` (core, temp-dir-backed: `$XDG_RUNTIME_DIR/flintshot/session/`
with `<id>.png` + `<id>.json` per capture) is owned by the app-window
process via `Shared`. Every image landing in the editor gets an entry;
committed annotations flush per-commit; Home gains a resumable "This
session" section above recents; save graduates an entry into recents,
and only the Home chip's two-step discard (or app exit) destroys one.
See `spec.md` for full decisions.

## Architecture Decisions

- **Store in core, owned by the app window** (`app_window.rs` `Shared`) —
  the daemon and standalone worker editors never touch it (spec
  out-of-scope). Mirrors the `recents.rs` core-persistence pattern.
- **Insert-on-arrival via one `arm` choke point** — `PendingEditor` is
  armed at 6 sites (pump grab, forwarded open, Home Open/Paste ×2,
  editor-chrome Open/Paste ×2); a single helper inserts into the store
  and arms, with `PendingEditor.session_id: Option<SessionId>`
  distinguishing new arrivals (insert) from resume (reuse).
- **Flush at the leave point, not from route snapshots** —
  `window.rs`'s route slot holds a *construction snapshot* (~line 260),
  which is exactly why annotations are lost today (`app_window.rs:2062`).
  The live `EditorState` is only reliably in hand inside
  `request_route` when leaving `Route::Editor` → new
  `RouteHost::editor_route_left(&EditorState)` seam, plus the outer
  loop's close path (`exit.route` is Editor) for window-closing exits
  (NewCapture, hidden-by-event).
- **Per-commit annotation flush** (`EditorPageBridge` new default-no-op
  method) — tiny synchronous JSON rewrite, crash-safe annotations for
  free; PNG written once on arrival off-thread.
- **Removal semantics**: `Saved` removes (graduates to recents);
  `Uploaded`/`Cancelled`/Esc **keep** the card (spec amended — upload
  guarantees no disk file, Esc-as-discard would surprise); the Home
  chip's two-step confirm is the only user discard. `NewCapture`
  stashes instead of discarding — the documented "Win11-style loss"
  (`shell.rs:29`) is deliberately superseded by this effort.
- **Graceful exit = one `clear()`** at `run_app_window`'s tail
  (`app_window.rs:893`) — every `break` path falls through it; crashes
  skip it and the next startup's `sweep_stale()` cleans up. Never
  resume across restarts.

## Task Index

### Phase 1: Foundation
- [ ] Task 1: Core `SessionStore` + temp-dir persistence → `tasks/001-core-session-store.md`
- [ ] Task 2: App ownership + lifecycle + insert-on-arrival → `tasks/002-app-store-lifecycle.md`

### Checkpoint: Foundation
- [ ] All tests + fmt + clippy pass; captures land in the store (no UI yet)

### Phase 2: Core Flows
- [ ] Task 3: Flush-on-leave + active-session plumbing → `tasks/003-flush-on-leave.md`
- [ ] Task 4: Save graduation + per-commit flush → `tasks/004-save-and-commit-flush.md`
- [ ] Task 5: Resume path + reopen-restore rebuild → `tasks/005-resume-and-restore.md`

### Checkpoint: Core Flows
- [ ] Capture → annotate → Home → resume round-trip preserves annotations (test-verified)
- [ ] Save removes the entry; NewCapture stashes instead of losing

### Phase 3: Surface + Polish
- [ ] Task 6: Home "This session" section → `tasks/006-home-session-section.md`
- [ ] Task 7: Docs — CHANGELOG + ADR + AGENTS row → `tasks/007-docs.md`

### Checkpoint: Complete
- [ ] All spec success criteria met; full gate commands green; human review

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Route-slot snapshots mask live annotations (window.rs:260) | High — flush writes stale state | Flush via `editor_route_left` at the leave point where live state is in hand; unit-test the round-trip, not the snapshot |
| PNG encode thread races `remove` (card discard while writing) | Med — orphan file | Writes check the entry still exists under the store's inner mutex; orphan files are harmless (swept next start) |
| `XDG_RUNTIME_DIR` unset (non-systemd) | Med — store unusable | Fallback `std::env::temp_dir()/flintshot/session/`; both paths unit-tested |
| tmpfs pressure from many large captures | Low | Advisory degradation: failed PNG write → memory-only entry + warn log |
| Resume rebuild resets editor invariants (selection, pending) | Med | Rebuild via `EditorState::new` + `anns` assignment only; pin with egui test |

## Open Questions

- None blocking; spec open questions (scroll behavior, caption wording)
  are settled inline by Task 6 using recent-captures-v2's grid pattern.
