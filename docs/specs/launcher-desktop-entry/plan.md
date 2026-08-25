# Plan: Launcher desktop entry

## Overview

Make Flintshot discoverable in desktop app launchers: a `.desktop`
entry that starts the daemon (tray appears), desktop actions for
quick captures, an opt-in autostart variant, an SVG icon, and
idempotent `just install` / `install-user` / `uninstall` recipes —
guarded by a validation test that keeps `Exec=` targets in sync with
the CLI surface. Spec:
[`spec.md`](spec.md). No product code; Linux XDG only.

**Revised 2026-08-26:** Tasks 1–4 shipped but closeout found the
default install builds a no-tray binary, breaking the "launch it →
tray appears" promise. Tasks 5–7 close the gap (recipe-level tray
default), add a CI job compiling the tray path, and finish the
manual verification pass. Note: the uncommitted spec-revision edits
are on `flatpak-packages` — before `/build`, switch to the
`launcher-desktop-entry` branch (checkout carries uncommitted
changes).

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
5. **Tray on by default at the recipe level** *(revision)* — install
   recipes build `--features tray` unless `FEATURES=` overrides;
   cargo-level defaults stay minimal so CI and bare checkouts remain
   hermetic. `${FEATURES-tray}` expansion so `FEATURES=""` selects
   the minimal build; failure path prints the GTK-dev-libs
   prerequisite and escape hatch. See ADR candidate: none — recorded
   in spec Decisions.
6. **CI compiles the tray path** *(revision)* — a dedicated
   `tray-build` job (clippy with `--features tray`, `-D warnings`)
   because the main lint job never enables the feature and the code
   path could rot silently.

## Task Index

### Phase 1 — Artifacts
- [x] Task 1: Icon + desktop entries → `tasks/001-assets-icon-desktop.md`

### Checkpoint: Artifacts
- [x] Files parse by eye + `desktop-file-validate` if available locally

### Phase 2 — Guardrail
- [x] Task 2: Desktop-file validation test → `tasks/002-validation-test.md`

### Checkpoint: Guardrail
- [x] `cargo test -p flintshot-app` green; test demonstrably fails on a broken `Exec=`

### Phase 3 — Installation
- [x] Task 3: `just install` / `install-user` / `uninstall` recipes → `tasks/003-install-recipes.md`

### Checkpoint: Installation
- [x] Recipe mechanics exercised in task 003; desktop-session half rolled into task 007

### Phase 4 — Docs
- [x] Task 4: CHANGELOG + TEST_MATRIX + final gates → `tasks/004-docs-closeout.md`

### Checkpoint: Complete (tasks 1–4)
- [x] Shipped to `main` (c9c7ff6); manual criteria 1–4 carried as
      pending — reopened by the revision below

### Phase 5 — Revision: tray-default install (spec revised 2026-08-26)
- [ ] Task 5: Tray-default install builds (`FEATURES=` passthrough) → `tasks/005-tray-default-install.md`
- [ ] Task 6: CI `tray-build` job → `tasks/006-ci-tray-build.md` (independent of 5; parallelizable)

### Checkpoint: Revision build
- [ ] Local: default install-user produces a tray binary; `FEATURES=""` minimal path works
- [ ] CI: `tray-build` job green on push; standing gates green
- [ ] TEST_MATRIX workaround removed without losing coverage

### Phase 6 — Closeout
- [ ] Task 7: Manual TEST_MATRIX pass + exit-criteria sweep → `tasks/007-manual-pass-closeout.md`

### Checkpoint: Complete (effort)
- [ ] All spec exit criteria verified (1–6); ready for `/review` → `/ship`

## Risks and Mitigations

| Risk                                                             | Impact | Mitigation                                                                  |
| ---------------------------------------------------------------- | ------ | --------------------------------------------------------------------------- |
| `StartupWMClass` wrong → window not grouped with launcher entry  | Low    | Verify with `xprop` during Task 1; recorded in TEST_MATRIX (Task 4)         |
| Icon unrenderable in some launchers (SVG quirks)                 | Low    | Keep the SVG geometrically simple; placeholder accepted now per spec OQ-1   |
| Desktop-environment quirks (actions hidden, autostart ignored)   | Med    | TEST_MATRIX rows for GNOME/KDE/COSMIC on both X11 + Wayland; document finds |
| Recipe drift from XDG spec                                       | Low    | Task 2's parse test also validates installed-file content at install time?  |
| Tray build pulls system deps into every install                  | Med    | `FEATURES=""` escape hatch + actionable hint; cargo defaults untouched (spec decision #5) |
| CI runner GTK package name wrong → tray-build job red            | Low    | `libayatana-appindicator3-dev` on Debian/Ubuntu (verified present locally via pkg-config `ayatana-appindicator3-0.1`); job is isolated, doesn't gate others |
| Manual pass blocked (no second session type available)           | Med    | Fill available session columns, mark the rest N/A-with-reason in the sweep; effort ships when no row is silently empty |

(The *Recipe drift* row's mitigation, precisely: the test validates
the *source* artifacts; installed-file correctness is covered by the
recipes' own idempotent `cp` steps + task 007's manual pass — kept
honest here rather than over-engineered.)

## Open Questions

- ~~Icon placeholder vs. designed artwork~~ — resolved 2026-08-25:
  placeholder shipped (task 001); refinement is post-1.0 branding.
- ~~CI `--features tray` build job?~~ — resolved 2026-08-26: yes,
  task 006 (user-approved; CI changes are ask-first and were asked).
