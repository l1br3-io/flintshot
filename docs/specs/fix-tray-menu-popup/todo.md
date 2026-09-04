# fix-tray-menu-popup — task checklist

Single status source; mirrors `plan.md`'s task index.

- [x] Task 1: `DaemonTray` rebuildable — `Option<TrayIcon>` + session flavor; Linux hide = unregister, restore = `tray::build` behind the ADR-0032 watcher barrier (`tasks/001-daemon-tray-rebuildable.md`)
- [x] Task 2: tray-gate semantics retune — `TrayAction::Hide` doc'd as SNI unregister on Linux; `MENU_GRACE` rationale updated (`tasks/002-tray-gate-semantics.md`)
- [x] Task 3: docs — ADR-0038 + `adrs/README` index, CHANGELOG `[Unreleased]`, TEST_MATRIX runbook rows retuned (`tasks/003-docs-adr-0038.md`)
