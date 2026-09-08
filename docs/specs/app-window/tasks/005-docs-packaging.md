# Task 005 — Docs + packaging revision

## What

- `assets/applications/flintshot.desktop`: primary
  `Exec=flintshot app` (was `flintshot daemon`); `StartupWMClass`
  updated to the home window's verified WM class; desktop actions
  (region/fullscreen) unchanged. Autostart entry stays
  `flintshot daemon` (login → tray, no window).
- `crates/flintshot-app/tests/desktop_files.rs`: Exec-target sync
  test keeps passing (`app` must exist as a subcommand).
- `docs/adrs/ADR-0045-app-window.md`: client-process window,
  reaper-based `SessionEnded`, hide-grace, focus ping, desktop
  Exec revision (supersedes the launcher spec's "daemon is the
  app" *primary entry* decision — context, decision,
  consequences). Index `docs/adrs/README.md`.
- `CHANGELOG.md` under `## [Unreleased]`: the subcommand, tray
  item, protocol additions, desktop entry revision.
- `docs/TEST_MATRIX.md`: rows — auto-start flow, region/fullscreen
  round-trip (X11 + Wayland), crash restore, focus ping,
  launcher entry.
- `AGENTS.md`: `cargo run -- app` in the run-commands table.

## Where

- `assets/`, `docs/`, `CHANGELOG.md`, `AGENTS.md`, desktop-files
  test.

## Verify

- Desktop-files test green; standing gates; manual TEST_MATRIX
  pass on L5.
