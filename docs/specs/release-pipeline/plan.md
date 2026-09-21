# Plan: Cross-platform release pipeline (flintshot)

## Overview

Add a tag-driven release pipeline to flintshot: cargo-dist generates the
CI workflow and native artifacts (tarballs, .msi, shell/powershell
installers, checksums, binstall manifest, Homebrew formula);
cargo-release automates version/changelog/tag; custom post-announce
jobs build the non-native formats (.deb via cargo-deb, .rpm via
cargo-generate-rpm, universal .dmg via lipo+hdiutil, .AppImage via
linuxdeploy); `just` recipes tie it together (`just release patch` is
the whole maintainer flow). Full detail: `spec.md` in this directory.

## Architecture Decisions

1. **cargo-dist 0.32.0 (pinned), actively maintained** — verified Sep
   2026. Config in root `dist-workspace.toml`; `release.yml` is generated and
   checked in; regeneration only, never hand-edit.
2. **Native vs custom artifacts** — cargo-dist builds: per-arch macOS
   tarballs, Windows zip+msi, Linux tarball, installers, checksums,
   dist-manifest.json. `extra-artifacts` are arch-independent-only
   (verified against cargo-dist #613) so .deb/.rpm/.AppImage/.dmg are
   built by custom `post-announce-jobs` (reusable `workflow_call`
   workflows invoked from release.yml), each uploading its own
   `.sha256`.
3. **No universal-apple-darwin** (cargo-dist #77, open) — per-arch
   tarballs are the norm; the dmg job lipos both arches into one
   universal `.app`/`.dmg`.
4. **Linux floor: Debian 12 / Ubuntu 24.04+** — xcap 0.9's
   pipewire-rs bindgen-compiles against system PipeWire headers;
   22.04's 0.3.48 cannot compile (verified in container; user
   approved the floor). Native job pinned `ubuntu-24.04`; deb/rpm/
   AppImage built in a `debian:12` container (glibc 2.36, PipeWire
   0.3.65). Ubuntu 22.04 users build from source.
5. **`precise-builds = true` + `packages = ["flintshot-app"]`** — the
   default `--workspace` build would compile `flintshot-cosmic`
   (libcosmic system deps absent in the release job).
6. **Tray on all platforms** — `features = ["tray"]` in dist-workspace.toml;
   deb/rpm declare GTK/appindicator/xcap runtime deps (ldd-derived);
   Homebrew formula needs none (tray's GTK deps are Linux-only).
7. **Packaging scripts are shared** — `packaging/{linux,macos}/*.sh`
   run identically in CI jobs and `just dist-build` (env-gated
   `FLINTSHOT_PACKAGE_NO_UPLOAD=1`), so local smoke covers CI paths.
8. **Signing deferred** — structure leaves `ssldotcom-windows-sign` /
   `azure-windows-sign` (native cargo-dist hooks) as later drop-ins.

## Task Index

### Phase 1: Foundation (native cargo-dist matrix)
- [ ] Task 1: Author `dist-workspace.toml` (pinned config, plan green) → `tasks/001-dist-toml.md`
- [ ] Task 2: Generate `release.yml`, PR plan-check job, `just` recipes → `tasks/002-workflow-and-recipes.md`

### Checkpoint A: Foundation
- [ ] `cargo dist plan` green; `just dist-plan` works; PR job green
- [ ] Review with human before proceeding

### Phase 2: One-command release + channels
- [ ] Task 3: cargo-release wiring (`release.toml`, `just release*`) → `tasks/003-cargo-release.md`
- [ ] Task 4: Homebrew tap channel → `tasks/004-homebrew-tap.md`

### Checkpoint B: Release path
- [ ] `just release-dry` shows correct bump/tag/changelog preview
- [ ] Review with human before proceeding

### Phase 3: Extended formats (custom jobs)
- [ ] Task 5: `.deb` packaging + custom Linux job + smoke workflow → `tasks/005-deb-package.md`
- [ ] Task 6: `.rpm` packaging → `tasks/006-rpm-package.md`
- [ ] Task 7: macOS universal `.dmg` → `tasks/007-macos-dmg.md`
- [ ] Task 8: `.AppImage` (timeboxed, fallback: drop) → `tasks/008-appimage.md`

### Checkpoint C: Full artifact matrix
- [ ] `packaging-test.yml` dispatch smoke green (Linux + macOS)
- [ ] Container install tests pass (Debian 12, Fedora)
- [ ] Review with human before proceeding

### Phase 4: Docs + first release
- [ ] Task 9: ADR, AGENTS.md commands, CHANGELOG → `tasks/009-docs.md`
- [ ] Task 10: Execute first release `v0.1.0` + checklist → `tasks/010-first-release.md`

### Checkpoint D: Complete
- [ ] All spec success criteria met (or explicitly deferred with user sign-off)
- [ ] Ready for review / PR

## Risks and Mitigations

See `spec.md` § Risks & Mitigations — verified and updated at planning
time. Top residual risks: AppImage+GTK bundling fragility (fallback:
drop the format), cargo-release workspace-version bump (verify in dry
run before any execute).

## Open Questions

- First release version — `0.1.0` assumed; confirm at Task 10.
- Tap repo `l1br3-io/homebrew-tap` + `HOMEBREW_TAP_TOKEN` secret —
  user action needed before Task 4 verification / first release.
