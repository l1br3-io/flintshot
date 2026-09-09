---
status: active
started: 2026-09-08
---

# Spec — App window: a Win11-Snipping-Tool-style capture home

> Mini-effort spec. Product context: [`../../VISION.md`](../../VISION.md).
> Builds on Phase 2 (daemon/tray/IPC), the launcher-desktop-entry
> effort (`.desktop` surface), and the region-selection-first landing
> experience. The launcher spec's referenced `snipping-modes` effort
> (freeform/window capture) stays future; this effort ships region +
> fullscreen only.

## Objective

Today a capture starts from the tray menu, a hotkey, or the CLI.
Windows 11's Snipping Tool shows there is a fourth, more discoverable
surface: a small **app window** the user can open from the launcher
(or keep around) with the capture controls right in it.

Add `flintshot app`: a themed window with *Capture region* and
*Capture fullscreen* controls (plus a monitor picker for fullscreen).
Clicking a control hides the window (so it never lands in the shot —
Win11 minimizes itself the same way), runs the standard delegated
capture flow, and restores the window when the capture session
finishes. With no daemon running, the app starts one.

**Target users:** anyone who wants a visible, clickable home for
flintshot instead of hunting the tray or remembering chords.

**Success looks like:**

- `flintshot app` (or the launcher entry) shows the window; the
  daemon is started if absent.
- *Capture region* / *Capture fullscreen* hide the window, freeze
  the screen without the window in frame, open the editor, and
  bring the window back when the editor closes.
- The tray menu gains *Open app*; clicking it (or a second
  `flintshot app` invocation) focuses the existing window instead of
  stacking a second one.

## Interview decisions (2026-09-08)

1. **Process model** — new `flintshot app` subcommand: a standalone
   egui window that talks IPC to the daemon, mirroring the tray
   helper (ADR-0042). The daemon stays GUI-free.
2. **No daemon** — auto-start `flintshot daemon` detached, then wait
   for the socket to answer before enabling controls.
3. **Capture UX** — hide the window before the grab, restore after
   the capture session ends (crash-safe; see plan).

## Assumptions

1. The window is always daemon-backed: it auto-starts the daemon,
   so captures always ride the standard delegated worker path
   (Event broadcasting, tray-hide handshake).
2. Region capture opens the editor selection-first (existing
   `region` landing experience); the window's *hide* window covers
   the initial freeze-frame grab, and restore waits out the whole
   editor session.
3. ~~A ~250 ms grace between hiding the window and delegating the
   capture is best-effort on Wayland (unmap is asynchronous)~~
   **Revised 2026-09-09 (ADR-0047)**: hiding never worked on
   Wayland (winit's `set_visible` is a no-op there; reported live
   on COSMIC). The window now *closes* for the capture — surface
   destruction is causal everywhere — and the process reopens it
   on `SessionEnded`. The grace now measures close-to-grab
   teardown, uniform across platforms.

## Scope (in / out)

**In:**

- `flintshot app` subcommand (egui-gated, like `region`/`fullscreen`
  without `--out`).
- Home window in `flintshot-editor-egui`: *Capture region* /
  *Capture fullscreen* buttons, monitor picker for fullscreen,
  daemon status line. Reuses the editor theme.
- `Request::OpenApp` + `Event::AppFocus` (focus the existing
  window) and `Event::SessionEnded` (a capture worker exited) —
  appended at enum ends (postcard variant-index stability).
- Daemon: dispatch `OpenApp` (spawn the app worker); the worker
  reaper broadcasts `SessionEnded` for capture workers.
- Tray menu: *Open app* item.
- Desktop entry revision: primary `Exec=flintshot app` (the window
  auto-starts the daemon, making it the better launcher citizen);
  `StartupWMClass` updated; desktop-files test stays green.
- ADR-0045, CHANGELOG, TEST_MATRIX rows, AGENTS.md run table.

**Out:**

- Capture-window / freeform / delay-picker UI (`snipping-modes` and
  later efforts).
- Settings editing, pin/upload/upload-last controls in the window.
- Daemon-side tracking of "app window alive" beyond the lock+focus
  ping.
- macOS/Windows packaging concerns.

## Exit criteria

1. With no daemon running, `flintshot app` starts one and reaches
   the connected state (bounded wait, visible status on failure).
2. Clicking *Capture region* (and *Capture fullscreen*, incl. a
   chosen monitor) hides the window, the frozen frame contains no
   app window, the editor opens, and closing the editor restores
   the window focused.
3. A capture worker killed mid-session still restores the window
   (reaper-based `SessionEnded`).
4. Daemon death while the window is hidden restores it with an
   error status (subscription disconnect).
5. Tray *Open app* and a second `flintshot app` focus the existing
   window; no second window.
6. Launcher "Flintshot" opens the window (daemon auto-started);
   desktop-file validation test passes.
7. Standing gates: `fmt`, `clippy --workspace --all-targets
   -- -D warnings`, workspace tests, doc tests.

## Technologies

No new dependencies. egui/eframe (existing `flintshot-editor-egui`),
`interprocess` sync client (existing `ipc_client` path), postcard
IPC schema (append-only variants).

## Boundaries

- **Always:** append-only IPC variants; no GUI types in
  `flintshot-core`'s API; `#![forbid(unsafe_code)]`; egui crate
  stays IPC-free (bridge trait carries actions).
- **Ask first:** new dependencies (goal: zero); CI changes;
  touching the tray gate state machine (not needed here).
- **Never:** capturing in the daemon process; secrets; vendor dirs.

## Open questions

1. Should the window offer a *delay* spinner next? Deferred — the
   CLI keeps the escape hatch (`--delay-ms` via… the window always
   delegates, so delay lands with `snipping-modes` if wanted).
2. Should `flintshot app` also surface *pin last* / *upload last*
   quick actions? Deferred; the window stays capture-focused.
