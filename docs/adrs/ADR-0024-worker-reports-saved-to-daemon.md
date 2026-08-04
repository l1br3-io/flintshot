# ADR-0024: Worker reports saved-path to daemon over the existing IPC socket

**Status.** Accepted.

**Context.** Phase 2's daemon emits `Event::CaptureStarted` the moment
it spawns a worker, then `Event::EditorOpened` as a *process-started
proxy* — because the daemon never learns whether the editor actually
opened, saved, or was cancelled. The worker is a separate process
(ADR-0019); the daemon has no channel for it to report back.

Two Phase 3 needs make that gap costly:

1. **Subscribers were promised `Event::Saved { path }`.** The IPC
   schema (Phase 2) defines it; the daemon never emits it. A subscriber
   (the future COSMIC applet, a script) can observe that a capture
   *started* but not that it *produced a file*.
2. **`Request::PinLast` (Task 004) needs the saved path.** "Pin the last
   capture" has no path to pin if the daemon doesn't track it.

So the daemon needs to learn, for each worker it spawns, the path the
editor saved to (if any). The question is *how* the worker reports
back.

**Considered alternatives.**

- **Side-channel file** (worker writes a `last_capture.txt`, daemon
  reads it): racey (the daemon must poll or watch), fragile (two
  processes writing the same file), and ignores the user-chosen save
  path when the editor's file dialog is involved.
- **Daemon-side directory watch** (`notify` on the save dir): fires on
  *any* write to the dir (including non-flintshot files), can't
  distinguish a save from a temp write, and doesn't know the path the
  *user* picked in the editor's `rfd` dialog (which may be anywhere).
- **A second IPC socket for worker→daemon**: doubles the transport
  surface for one message per capture.

**Decision.** Reuse the existing Phase 2 IPC socket with one new
`#[non_exhaustive]` `Request` variant: `Request::ReportSaved { path }`.
The worker sends it fire-and-forget *after* the editor closes (we report
a finalised outcome, not a pending one). The daemon intercepts it in
`handle_connection` (before the dispatch table), stores the path in
`DaemonState::last_saved`, and broadcasts the real `Event::Saved { path }`
to subscribers.

**Direction inversion:** every other `Request` variant is client →
daemon (a client asks, the daemon acts). `ReportSaved` flows worker →
daemon — the worker *reports*, the daemon *records*. The daemon is still
the IPC server; the inversion is contained to one variant whose doc
comment calls it out. This is cleaner than a separate "worker→daemon"
enum because the transport, framing, and dispatch are already in place.

**Non-fatal by design.** A standalone capture (no daemon running) logs
`NoDaemon` at `debug` and continues; the editor outcome is unaffected.
The report is a courtesy for the subscriber stream and `PinLast`, not a
dependency of the save itself.

**Consequences.**

- The subscriber stream finally carries the real `Event::Saved { path }`
  that the Phase 2 schema promised — closes the proxy gap.
- `PinLast` (Task 004) reads `last_saved` to re-pin the most recent
  capture, whether it was a Save or a Pin (Task 003's editor Pin button
  reports via the same seam).
- The `DaemonState` struct (bus + `last_saved` + pin-id counter) is the
  shared handle between the IPC server thread and the dispatch path — a
  single container for "things both halves need."
- The report is post-hoc: if the worker crashes between saving and
  reporting, the daemon's `last_saved` is stale. Acceptable — the file
  exists on disk, and a future "open last" could fall back to the
  pin-cache dir.
- One new wire variant; `#[non_exhaustive]` makes it non-breaking for
  older clients (they see an unknown variant and ignore it).

**Cross-references.** ADR-0010 (`interprocess` transport); ADR-0011
(postcard encoding); ADR-0019 (the worker-spawn model whose feedback
loop this wires); ADR-0023 (pin-as-process, which relies on
`last_saved` for `PinLast`).
