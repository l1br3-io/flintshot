# ADR-0067: Session captures — an advisory temp-dir store behind the editor

**Context.** Every navigation away from the app window's editor
page destroyed in-session annotations: the route slot keeps only a
construction snapshot (`window.rs`), so Home / Settings swaps,
external captures superseding the editor, and `NewCapture` closes
all dropped committed annotations on the floor — documented as
"the Replace policy's loss" and pinned by a test that asserted
the loss. The unsaved middle ground (captured, annotated, poked
around Home, wants to come back) had no representation:
`recents.toml` only records saves.

**Decision.**

- **A `SessionStore` in `flintshot-core`, owned once per
  app-window process.** In-memory index (id, created-at, dims)
  mirrored to `$XDG_RUNTIME_DIR/flintshot/session/` (temp-dir
  fallback): `<id>.png` written once on arrival, off-thread,
  existence-checked under the store lock; `<id>.json`
  (created-at + committed annotations) rewritten synchronously on
  every commit. All IO is advisory — failures warn and degrade to
  memory-only, exactly like the recents index. `serde_json` was
  promoted from a core dev-dependency (already in the lockfile).
- **Insert at one choke point.** Every image enters the editor
  epoch through `Shared::arm_editor(state, selection_first,
  resume)` — `None` inserts a fresh entry, `Some(id)` resumes one
  (never re-inserts); the pending editor then carries its
  `session_id` end to end (`PendingEditor` → `RoutedEditor` →
  restore page).
- **Flush live state, never the snapshot.** `request_route` hands
  the live `EditorState` to `RouteHost::editor_route_left` before
  any swap that ends an editor (kind change or same-route image
  replacement), and `Drop for WindowApp` covers window-closing
  exits (`NewCapture`, hidden-by-event). Per-commit, the egui
  editor detects annotation changes by content equality at the
  tail of both frame passes and calls
  `EditorPageBridge::annotations_committed` — one choke point
  covering tool commits, deletes, moves, crops, history restores,
  and panel edits. `Shared.active_session` tracks the open
  editor's entry (set on route open and at cold starts).
- **Removal semantics.** `Saved` graduates (entry removed,
  recents records it; post-save edits re-file as a fresh unsaved
  capture on the next leave). `Uploaded` / `CopiedToClipboard` /
  `Cancelled` keep the entry — no disk file is guaranteed for an
  upload, and Esc-as-discard would surprise. The only user
  discard is Home's two-step Discard chip. Graceful exit runs one
  `clear()`; a crashed run's leftovers are swept at next startup —
  **no resume across restarts**.
- **Surface.** Home renders a "This session" section above the
  recents grid: 4-per-row cards in the pop-up-book language,
  on-demand thumbnails off the `<id>.png` (the recents machinery),
  an "Unsaved" caption, click-to-resume
  (`HomeBridge::open_session_capture` → `arm_editor(Some(id))`,
  undo/redo empty by design), hidden when empty. The hidden-close
  restore rebuilds from the store when the stashed entry still
  lives; a store miss keeps the bare-stash fallback.

**Consequences.** Navigating away no longer costs work; mid-edit
supersedes stash cards instead of losing edits. Annotation JSON is
crash-safe between route changes for free (synchronous tiny
rewrites). Costs: a PNG write per capture (off-thread, advisory),
a per-frame `session_captures()` Vec clone on Home (in-memory
index, a few hundred bytes), and one more `RouteHost` /
`EditorPageBridge` / `HomeBridge` seam each (all defaulted, so
doubles and worker paths stay untouched). Standalone worker
editors and the daemon remain out of scope by spec; persistence
across app restarts is a deliberate non-goal.
