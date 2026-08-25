# Plan: Launcher desktop entry

## Overview

Make Flintshot discoverable in desktop app launchers: a `.desktop`
entry that starts the daemon (tray appears), desktop actions for
quick captures, an opt-in autostart variant, an SVG icon, and
idempotent `just install` / `install-user` / `uninstall` recipes —
guarded by a validation test that keeps `Exec=` targets in sync with
the CLI surface. Spec:
[`spec.md`](spec.md). No product code; Linux XDG only.

## Architecture Decisions

1. **Daemon is the app** — main entry `Exec=flintshot daemon`;
   single-instance lock makes relaunches safe. Quick captures are
   Desktop Actions, not top-level entries (spec decision).
2. **Sync test over linter dep** — validation is an integration test
   in `flintshot-app` using `env!("CARGO_BIN_EXE_flintshot")` to
   assert each `Exec=` subcommand exists in the binary's real
   `--help` output, plus a spec-compliant parse of the `.desktop`
   grammar. Zero new dependencies; CI stays hermetic.
3. **Assets live in-repo** — `assets/applications/*.desktop` +
   `assets/icons/hicolor/scalable/apps/flintshot.svg`; recipes
   `cp` them into XDG locations (no build.rs, no build-time
   templating).
4. **Recipes follow justfile house style** — `#!/usr/bin/env bash`
   with `set -euo pipefail`, graceful skip when optional tools
   (`desktop-file-install`, `update-desktop-database`) are absent,
   idempotent re-runs, no sudo inside recipes (system install
   requires the user to elevate externally, documented in the recipe
   header comment).

## Task Index

### Phase 1 — Artifacts
- [ ] Task 1: Icon + desktop entries → `tasks/001-assets-icon-desktop.md`

### Checkpoint: Artifacts
- [ ] Files parse by eye + `desktop-file-validate` if available locally

### Phase 2 — Guardrail
- [ ] Task 2: Desktop-file validation test → `tasks/002-validation-test.md`

### Checkpoint: Guardrail
- [ ] `cargo test -p flintshot-app` green; test demonstrably fails on a broken `Exec=`

### Phase 3 — Installation
- [ ] Task 3: `just install` / `install-user` / `uninstall` recipes → `tasks/003-install-recipes.md`

### Checkpoint: Installation
- [ ] Manual: `install-user` → launcher entry appears, launches daemon, actions work; uninstall removes all; re-install idempotent

### Phase 4 — Docs
- [ ] Task 4: CHANGELOG + TEST_MATRIX + final gates → `tasks/004-docs-closeout.md`

### Checkpoint: Complete
- [ ] All spec exit criteria verified; standing gates green; ready for review

## Risks and Mitigations

| Risk                                                             | Impact | Mitigation                                                                  |
| ---------------------------------------------------------------- | ------ | --------------------------------------------------------------------------- |
| `StartupWMClass` wrong → window not grouped with launcher entry  | Low    | Verify with `xprop` during Task 1; recorded in TEST_MATRIX (Task 4)         |
| Icon unrenderable in some launchers (SVG quirks)                 | Low    | Keep the SVG geometrically simple; placeholder accepted now per spec OQ-1   |
| Desktop-environment quirks (actions hidden, autostart ignored)   | Med    | TEST_MATRIX rows for GNOME/KDE/COSMIC on both X11 + Wayland; document finds |
| Recipe drift from XDG spec                                       | Low    | Task 2's parse test also validates installed-file content at install time?  |

(The last row's mitigation is: the test validates the *source*
artifacts; recipe correctness is covered by Task 3's manual
checkpoint — kept honest here rather than over-engineered.)

## Open Questions

- Icon placeholder vs. designed artwork — spec recommends
  placeholder; Task 1 proceeds with a simple geometric mark unless
  overridden.
