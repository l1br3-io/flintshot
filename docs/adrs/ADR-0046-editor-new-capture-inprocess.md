# ADR-0046: New-capture from the editor re-captures in-process (the worker persists)

**Context.** The app-window effort (ADR-0045) gave the editor a
"start over" ask: a Win11-style `+` chip (and `Ctrl+N`) to begin a
fresh region/fullscreen capture from inside an editing session.
Two mechanisms could service it:

1. **Delegate** — the editor sends `Request::CaptureRegion` to the
   daemon, the daemon spawns a *new* worker, the old editor exits.
2. **Re-capture in-process** — the editor window closes, the same
   worker process grabs a fresh monitor and opens a new session.

**Decision.** In-process. The editor publishes
`EditorOutcome::NewCapture { mode }` and closes its window; the
app crate waits the 250 ms unmap grace (ADR-0045's bound), grabs
with the tray hidden (the standing worker path), and re-runs the
session — selection-first for region, plain for fullscreen. The
overlay shell's `+` twin does the same through its own loop.

Why not delegate:

- **ADR-0045 counts sessions by worker process.** Delegation
  overlaps two workers, and the exit/spawn ordering races the app
  window's in-flight counter: an old-editor exit can hit zero
  before the new `CaptureStarted` lands, flashing the app window
  visible mid-chain. The in-process loop keeps the counter at one
  throughout — the app window restores exactly when the user
  finally leaves the editor.
- **No editor stacking** — delegation briefly shows two editor
  windows; the loop shows exactly one at a time.
- **It is the standing worker model** — a `region --standalone`
  worker already owns capture+edit in one process; the loop is
  that path repeated.

The closing window is the not-in-the-shot guarantee (a destroyed
window cannot be captured); the grace covers the compositor
finishing the unmap, mirroring the app window's hide.

**Consequences.**

- Unsaved annotations are discarded on a new capture, Win11-style
  (no confirmation in v1; revisit if it bites).
- `EditorOutcome::NewCapture` is terminal for the *window*, not
  the process: `SessionEnded` still fires once, at chain end.
- `report_editor_outcome` never sees the variant (the loop
  intercepts it); its wildcard arm remains the safety net.
- The recapture monitor is the default (active) — a per-monitor
  picker would ride the `snipping-modes` effort if wanted.
- A capture failure mid-chain exits the worker with an error (the
  editor already closed); acceptable for v1.
