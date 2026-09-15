# ADR-0053: Single-window app — routed pages, in-process grabs, forwarded triggers

**Context.** Since the app-window effort, `flintshot app` was one
window among several: captures delegated to the daemon, which spawned
worker processes whose editor windows opened alongside (or after) the
home window, and `flintshot open` spawned yet another child. Three
windows across two processes for one annotation session is exactly
the multi-window friction the industry tools (Win11 Snipping Tool,
CleanShot X) retired years ago — one window that routes. The
close-before-grab machinery (ADR-0046's in-process recapture,
ADR-0047's surface destruction over hiding) already proved that
window *close* is the portable not-in-the-shot guarantee; what was
missing was a window that comes back as the *right page* without a
spawn.

**Decision.**

1. **One window, page epochs.** The app window is a page router:
   `Page::{Home, Editor(state)}` picks which epoch runs — one
   `eframe::run_native` lifetime per page, with per-page viewport
   (geometry + title). No runtime resize; a page transition is window
   close + recreate, reusing the ADR-0047 close/reopen machinery
   as-is. Home is the empty state (Capture / Open… / Paste); `Esc`
   from the editor routes back to it; X / Alt+F4 exits the process.
2. **In-process grabs.** The app window's pump spawns a grab thread
   running the shared `capture_flow` module (extracted from the CLI
   worker path — one grab implementation, two callers). The prepare
   handshake (ADR-0048) runs inside it; the window is already
   destroyed when `PrepareCapture` leaves, so the `ReportAppHidden`
   ack is immediate. A generation-counted state machine supersedes an
   in-flight grab on re-trigger (the Replace policy, in-process).
3. **Forwarded triggers.** With a window present (presence per
   ADR-0052), the daemon's capture dispatch broadcasts
   `Event::CaptureTriggered` instead of spawning a worker — hotkeys,
   tray, and delegated `flintshot capture` all land in the running
   window. Open intents converge the same way
   (`Request::OpenImage` / `OpenLast` → `Event::OpenRequested`).
   Without a window, the worker paths run byte-identical.
4. **Stay-in-editor outcomes.** Save / Copy / Upload apply their side
   effects and the editor page stays open (the chrome exposes the
   same three actions, so new content loads without going home);
   the daemon reports ride an app-side bridge.

This supersedes the app-window spec's multi-window stance (home
window + worker editors in parallel flows) while preserving every
ADR-0046/0047 invariant: every capture entry point still closes the
window before the grab, and the worker fallback keeps the same
semantics when no window answers.

**Consequences.**

- The window is never resized at runtime: each new image is a fresh
  epoch with its own geometry (close/reopen, ~250 ms grace absorbed
  by the handshake). Flicker-per-transition is the accepted cost; it
  is indistinguishable from the recapture loop users already know.
- Unsaved annotations are lost when newer content replaces the page
  (capture, open, paste, supersede) — the same Replace semantics the
  worker editor had, now uniformly reachable from the editor chrome.
- `Event::CaptureStarted` fires for forwarded captures but no
  `SessionEnded` follows (no worker process); subscribers that count
  the pair must treat `CaptureTriggered` as the balancing event. The
  app window's own core does; the COSMIC applet only reads status
  labels.
- Three new wire variants (`CaptureTriggered`, `OpenRequested`,
  `OpenImage`), appended per the postcard variant-index rule; a
  daemon older than this ADR answers `unsupported` and the CLI falls
  back to in-process editing.
- The daemon-spawned `flintshot open` worker carries `--standalone`
  (capture argv's loop-breaker rule) now that `open` delegates.
- The editor-egui crate grew page lifecycle (`run_editor_page`,
  `EditorPageBridge`) next to the worker `run_editor` — one file
  serves both lifecycles, diverging only at outcome-publication
  points.
