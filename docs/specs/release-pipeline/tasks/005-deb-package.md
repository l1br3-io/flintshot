# Task 005: `.deb` packaging + custom Linux job + packaging-test smoke

**Description:** Build the `.deb` with cargo-deb inside a custom
post-announce job. Derive runtime `Depends` from `ldd` of the
release binary (GTK3, appindicator, xcap's X11/Wayland/PipeWire
libs), include the desktop entry + icon as deb assets. The packaging
script is shared: CI runs it in `package-linux.yml`, locals run it
via `just dist-build`, and `packaging-test.yml` (workflow_dispatch)
runs it with `FLINTSHOT_PACKAGE_NO_UPLOAD=1` for branch-time smoke
without a release.

**Acceptance criteria:**
- [ ] `packaging/linux/build-packages.sh` builds the tray release
      binary and produces `target/packages/flintshot_<v>_amd64.deb`;
      installs cargo-deb pinned+locked; honors
      `FLINTSHOT_PACKAGE_NO_UPLOAD=1`; `set -euo pipefail`, clear
      failure messages
- [ ] `[package.metadata.deb]` in `crates/flintshot-app/Cargo.toml`:
      ldd-derived `depends`, desktop entry + icon assets (from
      `assets/applications/`, `assets/icons/`), maintainer, section
- [ ] `.github/workflows/package-linux.yml`: `on: workflow_call`,
      job in a **`debian:12` container** on `ubuntu-latest` (glibc
      2.36 + PipeWire 0.3.65 = the release floor; 22.04's 0.3.48
      headers cannot compile the pipewire stack), installs xcap+GTK+
      xdo apt
      deps, runs the script, generates `.sha256`, uploads to the
      GitHub Release via `gh release upload` (tag from
      `github.ref_name`)
- [ ] `dist-workspace.toml`: `post-announce-jobs = ["./package-linux"]`;
      release.yml regenerated
- [ ] `.github/workflows/packaging-test.yml`: `on:
      workflow_dispatch` (+ `pull_request` paths filter on
      `packaging/**`, the workflow files, and the deb metadata) —
      Linux job runs the script no-upload
- [ ] Local: script runs on this machine; produced deb installs and
      runs in a clean container

**Verification:**
- [ ] `FLINTSHOT_PACKAGE_NO_UPLOAD=1 bash packaging/linux/build-packages.sh` locally, exit 0
- [ ] `docker run --rm -v "$PWD/target/packages:/a:ro" debian:12 bash -c
      "apt-get update -qq && apt-get install -y -qq /a/*.deb &&
      flintshot --version"` — exit 0 (deps auto-resolve)
- [ ] Same on `ubuntu:24.04`
- [ ] `bash -n` + `shellcheck` (if installed) on the script
- [ ] `just dist-lint` (actionlint) on new workflows
- [ ] Push branch, dispatch `packaging-test.yml`, green

**Dependencies:** Task 002 (regeneration, recipes)

**Files likely touched:**
- `packaging/linux/build-packages.sh` (new)
- `.github/workflows/package-linux.yml` (new)
- `.github/workflows/packaging-test.yml` (new)
- `crates/flintshot-app/Cargo.toml`
- `dist-workspace.toml`
- `.github/workflows/release.yml` (regenerated)

**Docs:**
- GOTCHA candidate: deb `Depends` names differ across Debian
  releases (`libgtk-3-0` vs `libgtk-3-0t64` on 24.04+) — if both
  can't be satisfied with one field, use the `t64`-compatible
  alternative or a `|` dependency and note it here.

**Estimated scope:** Medium-Large (6 files — single coherent slice;
the script + workflow are the templates Tasks 6/8 extend)
