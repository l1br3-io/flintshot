# app-window — task checklist

Single status source; mirrors `plan.md`'s task index.

- [x] Task 1: core protocol — `Request::OpenApp`, `Event::SessionEnded`, `Event::AppFocus`, append-at-end (`tasks/001-core-protocol.md`)
- [x] Task 2: home window — editor-egui `home` module, `HomeBridge`, `run_home` (`tasks/002-home-window.md`)
- [x] Task 3: app wiring — `Command::App`, daemon ensure, subscription pump, in-flight state, app lock (`tasks/003-app-wiring.md`)
- [x] Task 4: daemon + tray — `OpenApp` dispatch, reaper `SessionEnded`, tray *Open app window* (`tasks/004-daemon-tray.md`)
- [x] Task 5: docs + packaging — desktop Exec revision, ADR-0045, CHANGELOG, TEST_MATRIX, AGENTS.md (`tasks/005-docs-packaging.md`)

Remaining: real-platform TEST_MATRIX cells (L5: launcher entry,
tray *Open app window*, multi-monitor picker; Wayland-session
hide grace). The scripted XV1 pass is green — 18/18 at e5b12a0.
