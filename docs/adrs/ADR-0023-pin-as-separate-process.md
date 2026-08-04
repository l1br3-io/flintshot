# ADR-0023: A pin is a separate process, spawned via `worker::CommandSpawner`

**Status.** Accepted.

**Context.** Phase 3 adds pin-to-desktop: a borderless always-on-top
floating window showing a captured image, which the user can drag around
the desktop after the editor closes. Two design pressures conflicted:

1. **The pin must outlive its origin.** A pin launched from the editor
   must survive the editor closing (the user pins, then closes the
   editor to reclaim screen space). A pin launched from the daemon must
   survive a daemon restart (`--replace`, crash). If the pin lived in
   the editor's or daemon's process, exiting that process would kill the
   pin.

2. **egui viewports die with their root window.** The obvious
   in-process approach — a deferred `eframe::Viewport` inside the
   editor's eframe instance — fails because egui (0.35) tears down
   viewports when their parent window closes. The editor closing would
   close the pin. There is no "detach viewport" path in eframe.

So the pin cannot be an in-process viewport and cannot be a thread in
the editor/daemon process. It has to be its own OS process.

**Decision.** A pin is a `flintshot pin <path>` child process, spawned
detached by whoever creates it:

- **From the editor** (Task 003): the editor's *Pin* button composes
  the final image, writes it to a pin-cache file
  (`pin_cache_dir()/pin-<timestamp>.png`), publishes
  `EditorOutcome::Pinned(path)`, and closes. The CLI's
  `run_editor_for_capture` then spawns `flintshot pin <path>` via
  [`worker::CommandSpawner`] (Phase 2's spawn + reaper pattern).
- **From the daemon** (Task 004): `Request::PinLast` reads the daemon's
  `last_saved` path and spawns `flintshot pin <path>` through the same
  `CommandSpawner` + `pin_argv` helper used by capture requests.
- **From the CLI** (Task 001): `flintshot pin <path>` loads any
  decodable image and opens the borderless window directly — this is
  the canonical pin primitive the other two routes converge on.

The binary is resolved via [`std::env::current_exe`] (not `PATH`),
matching ADR-0019 — the pin binary is the same `flintshot` as its
spawner. stdin/stdout are null'd; stderr is inherited so tracing logs
survive; a reaper thread waits on the child so it doesn't linger as a
zombie.

`EditorShell::run_pin` is the single primitive in `flintshot-editor-egui`
(where eframe lives); the spawn decision lives in `flintshot-app` (where
process orchestration lives). The crate boundary holds — no eframe types
cross into core.

**Consequences.**

- A pin survives an editor crash, a daemon restart, or the spawning
  process exiting for any reason — the OS re-parents the child to init.
- The pin's egui event loop never contends with the editor's; a
  rendering hitch in one can't stall the other.
- Reuses ADR-0019's worker-spawn model verbatim (`CommandSpawner`,
  `pin_argv`, reaper thread). A third caller (the daemon's `PinLast`)
  vindicates extracting `pin_argv` as a shared helper.
- Multiple pins coexist as independent processes — no shared state, no
  coordination needed. A future "list pins" / "close pin N" feature
  will need an IPC channel back (the per-daemon `Event::Pinned { id }`
  is the handle; closing is not yet wired).
- The pin-cache file is the handoff artifact: the editor writes it, the
  pin process reads it (once, into memory) and never holds a file
  handle. Stale files are safe to reap (the daemon may clean on start;
  the OS cache dir is independently reaped).
- Cost: one process per pin (a few MB RSS each). Acceptable for a
  handful of pins; a "pin wall" of dozens would want a single-process
  multi-window mode, which egui viewports would enable if they ever
  support detaching — tracked as future work.

**Cross-references.** ADR-0019 (daemon spawns worker processes — the
spawn pattern this reuses); ADR-0024 (the worker→daemon feedback loop
that gives `PinLast` a path to pin).
