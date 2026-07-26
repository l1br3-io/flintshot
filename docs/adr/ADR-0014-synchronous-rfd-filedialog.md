# ADR-0014: Synchronous `rfd::FileDialog`, not `AsyncFileDialog`

**Context.** The Phase 1 spec called for `rfd::AsyncFileDialog` for the
Save dialog. But the eframe native event loop is strictly
single-threaded and already blocking — `eframe::run_native` owns the
calling thread until the window closes (see ADR-0012 for the shell
model). There is no event loop to yield back to. Using the async API
would require either spawning a background thread (then marshalling the
result back to the UI thread to publish the outcome) or a
`set_repaint_callback` poll loop. Both add machinery for no UX gain: the
native modal is what the user interacts with regardless, so the editor
window pausing during the dialog is expected behaviour, not a bug.

**Decision.** Use `rfd::FileDialog::save_file() -> Option<PathBuf>`
directly in the editor shell. Phase 1's Save and Copy paths are
synchronous.

**Consequences.**

- Simpler code; no thread-marshalling or repaint-callback plumbing.
- The editor window does not repaint while the native modal is open —
  acceptable, and matches every other native application.
- The same sync path serves the `Ctrl+S` keyboard shortcut with no
  scheduler change.
- Phase 2's daemon may need to revisit if a long-running editor blocks
  daemon IPC; that is a Phase 2 concern, not a Phase 1 one.
