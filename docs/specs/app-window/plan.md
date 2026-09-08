# Plan: app-window — Win11-Snipping-Tool-style capture home

## Overview

`flintshot app` opens a small egui window ("home") with *Capture
region* / *Capture fullscreen* controls and a monitor picker. The
window is an IPC **client process** (same model as the tray helper,
ADR-0042): it subscribes to the daemon's `Event` stream and sends
capture requests; the daemon keeps spawning worker subprocesses
exactly as it does for tray/hotkey/CLI triggers. With no daemon it
starts one, waits for the socket, then proceeds.

Clicking a control hides the window (ViewportCommand), waits a
grace, then delegates. The window restores when captures-in-flight
return to zero, learned purely from the subscription stream, or on
subscription disconnect (daemon death).

## Architecture decisions

- **Client process, not daemon GUI** — eframe owns its thread; the
  daemon loop must stay free of GUI. The window is "just another
  subscriber + requester", so it inherits every existing daemon
  behavior (worker isolation, tray-hide handshake, Event fan-out)
  for free.
- **`Event::SessionEnded` from the worker reaper, not worker
  self-report** — the daemon already reaps every spawned worker
  (`CommandSpawner`'s reaper thread). Broadcasting `SessionEnded`
  there is crash-safe (a killed worker still "reports") and needs
  zero `cli.rs` changes. Only capture argv (`region` /
  `fullscreen`) broadcasts it — the check lives next to
  `spawns_capture_worker`, with a comment tying them together.
- **Hide-grace 250 ms** — matches the tray icon-grace bound's
  rationale: enough for the compositor to unmap, cheap enough not
  to feel laggy. Best-effort on Wayland (documented; TEST_MATRIX
  rows for X11 + Wayland).
- **In-flight counter over the subscription, not own-request
  tracking** — the window counts `CaptureStarted` (+) and
  `SessionEnded` (−, saturating at 0) and restores at 0. A hotkey
  capture racing the window's own can't cause an early restore,
  and the window never pops up mid-capture. The counter starts at
  subscription time; a capture begun before that is invisible
  (clamped) — acceptable, documented.
- **Single window via lock + focus ping** — `flintshot app` takes
  an app-purpose instance lock (`single.rs`'s named-lock seam). A
  second invocation delegates `Request::OpenApp` and exits; the
  daemon broadcasts `Event::AppFocus` to subscribers *and* spawns
  an `app` worker (which exits on the held lock). Net effect: no
  window → one opens; window exists → it focuses. No daemon-side
  liveness tracking needed.
- **egui crate stays IPC-free** — `flintshot-editor-egui` gains a
  `home` module with a `HomeBridge` trait (outgoing capture
  intents + polled directives: show/focus/status). The app crate
  implements the bridge with channels + the background IPC pump.
  Mirrors `run_overlay`'s concrete-shell-method precedent.
- **Desktop entry Exec revision** — the launcher effort decided
  "daemon is the app" when no window existed. The window
  auto-starts the daemon, so `Exec=flintshot app` is the better
  primary entry (Win11 parity: launcher shows the window).
  `StartupWMClass` updated to the home window's class;
  desktop-action quick captures unchanged.

## Task index

1. `tasks/001-core-protocol.md` — IPC schema: `Request::OpenApp`,
   `Event::SessionEnded`, `Event::AppFocus` (append-at-end), docs.
2. `tasks/002-home-window.md` — editor-egui `home` module:
   `HomeApp`, `HomeBridge`, `EguiShell::run_home`; theme reuse;
   directive tests.
3. `tasks/003-app-wiring.md` — `Command::App`; `app_window.rs`
   (daemon ensure, subscription pump, in-flight state machine,
   bridge impl, hide grace); `acquire_app_lock`; headless
   fallback; state-machine unit tests.
4. `tasks/004-daemon-tray.md` — `OpenApp` dispatch + `app` argv;
   `CommandSpawner::with_bus` reaper broadcasting `SessionEnded`
   for capture argv; tray *Open app* item + action mapping.
5. `tasks/005-docs-packaging.md` — desktop entry Exec +
   StartupWMClass revision, desktop-files test, ADR-0045,
   CHANGELOG, TEST_MATRIX, AGENTS.md run table.

## Verification

Unit: bridge directives, in-flight state machine (Started/Ended/
disconnect → show), dispatch table `OpenApp`, capture-argv
classification, tray action mapping, desktop-file Exec sync.
Manual: TEST_MATRIX rows for both session types (auto-start,
hide/restore round-trip, crash restore, focus ping, launcher
entry). Standing gates at closeout.
