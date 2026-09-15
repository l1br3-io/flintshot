# Plan: Single-window app (routed home → editor)

## Overview

One window, one process: the app window becomes a page router (three-button
Home empty state ↔ image-sized Editor page), captures run in-process on the
app's pump side, and external triggers (hotkey/tray/CLI) forward into the
running window via the daemon. Workers with their own editor windows remain
only as the no-window fallback. Spec: `docs/specs/single-window/spec.md`.

## Architecture decisions

1. **Page-epoch router** (spec OQ format): `app_window::run`'s outer loop —
   one `eframe::run_native` per page lifetime, `Page::{Home,
   Editor(EditorState)}` — is the router. Page transitions are window close +
   recreate with per-page viewport (geometry + title). No runtime resize; the
   ADR-0047 close/reopen machinery is reused as-is.
2. **In-process grab**: the pump thread coordinates; the grab itself
   (portal interactive / xcap fallback) runs on a dedicated thread through a
   shared `capture_flow` module extracted from `cli.rs` (the worker path
   calls the same functions). The prepare handshake runs inside the pump's
   select loop — the window is already closed when it starts, so the
   `ReportAppHidden` ack is sent immediately.
3. **Forwarding seam** (spec OQ1): daemon-side, inside the shared capture
   dispatch context (`CaptureDispatch`): if an app window is present
   (presence connection OR registry), run the existing
   supersede-then-**forward** — broadcast `Event::CaptureTriggered
   { delay_ms }` instead of spawning a worker. Rides the app window's
   existing subscription; no new connection. All `Request::Capture`s
   reaching the daemon are editor-destined (delegation already filters
   `--out`/`--standalone`/`--after`), so forwarding is always safe.
4. **Open routing** (spec OQ2 → yes): new `Request::OpenImage { path }`;
   CLI `flintshot open` delegates it, daemon broadcasts
   `Event::OpenRequested { path }` when a window is present (else spawns
   today's open worker). Tray `OpenLast` forwards the same event with the
   last-saved path.
5. **Outcome semantics** (spec OQ3): Save/Copy/Upload apply side effects via
   an app-side bridge report and the page **stays**; `Esc` exits to Home;
   `EditorOutcome::NewCapture` (existing) becomes the capture action.
6. **Per-page titles** (spec OQ4): keep `flintshot` (home) and
   `flintshot — annotate` (editor) per epoch — `StartupWMClass` routing
   unaffected.
7. **Self-supersede**: `CaptureTriggered` arriving mid-capture/mid-editor
   restarts the flow (window closes for the new grab; unsaved annotations
   lost — same Replace semantics as today's worker supersede).
8. **HomeCore shrink**: hide-on-`HideForCapture`/`CaptureStarted` stays
   (standalone `--standalone` worker grabs still broadcast it); restore
   target becomes "the page that was open" (not always Home), and
   `SessionEnded` restore only applies to worker captures.

## Task index

### Phase 1: Foundation (contracts + extraction)
- [ ] Task 1: IPC additions — `CaptureTriggered`, `OpenRequested`, `OpenImage` → `tasks/001-ipc-additions.md`
- [ ] Task 2: Extract the grab flow from `cli.rs` into `capture_flow.rs` → `tasks/002-extract-capture-flow.md`

### Checkpoint 1: Foundation
- [ ] Full gate green (fmt, clippy `-D warnings`, workspace tests, doc tests)
- [ ] Zero behavior change (`cargo run -- capture` byte-identical flows)

### Phase 2: Single-window core
- [ ] Task 3: Editor page mode in `flintshot-editor-egui` → `tasks/003-editor-page-mode.md`
- [ ] Task 4: Outer-loop page router + in-process Open → `tasks/004-router-open-in-process.md`
- [ ] Task 5: Paste from home → `tasks/005-paste-from-home.md`

### Checkpoint 2: First end-to-end paths (human review)
- [ ] `cargo run -- app`: Open and Paste route Home → Editor in one window; Esc → Home
- [ ] Review with human before the capture paths

### Phase 3: Capture paths
- [ ] Task 6: In-process capture (button → grab → editor page) → `tasks/006-in-process-capture.md`
- [ ] Task 7: Daemon forwarding for external captures + restore rework → `tasks/007-daemon-forwarding.md`

### Checkpoint 3: Full routing live
- [ ] Hotkey/tray capture with the window up routes through it
- [ ] `pkill` the window → hotkey capture still opens the worker editor

### Phase 4: Completeness
- [ ] Task 8: `flintshot open` / `OpenLast` routing → `tasks/008-open-routing.md`
- [ ] Task 9: Editor chrome actions (capture/open/paste) → `tasks/009-editor-chrome-actions.md`
- [ ] Task 10: ADR, CHANGELOG, docs → `tasks/010-adr-changelog.md`

### Checkpoint 4: Complete
- [ ] All ten spec acceptance criteria pass by hand on a Wayland session
- [ ] Full gate green; spec frontmatter ready to flip `active` on `/build` start

## Risks and mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| `EditorApp` page-mode refactor (2.5k-line file) | Med | Narrow seam: a page-mode flag only at outcome-publication points (`publish_outcome`, Save/Upload close-skip); headless tests pin each outcome's behavior |
| Prepare-handshake multiplexing in the pump (await `CaptureReady` while owing the ack) | High | Window is closed before the capture request leaves, so the ack is immediate; bounded waits everywhere (`HIDE_ACK_WINDOW` pattern); in-memory gate unit test |
| Portal grab on a background thread with a dead eframe loop | Med | Proven pattern — the worker does exactly this; pump never blocks on the grab thread |
| External-trigger races (trigger while closing/grabbing/editing) | Med | Explicit pump capture state machine (Idle/Closing/Grabbing); `CaptureTriggered` = restart; daemon's existing dispatch mutex serializes triggers |
| `SessionEnded` no longer means "editor closed" | Med | Task 7 reworks `HomeCore` with the same unit-test rigor; standalone-worker hide/restore path kept covered |

## Open questions

- None blocking. Spec OQ1–OQ4 resolved by decisions 3, 4, 5, 6.
  Implementation-level choices (bridge method shapes, toolbar placement of
  the chrome actions) live in the task files.
