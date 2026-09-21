# Task 006: `.rpm` packaging (cargo-generate-rpm)

**Description:** Extend the Task-5 Linux packaging path with
`.rpm` via cargo-generate-rpm: Fedora-named runtime `requires`,
desktop entry + icon payloads, same shared script and CI job —
no new workflows.

**Acceptance criteria:**
- [ ] `packaging/linux/build-packages.sh` also produces
      `target/packages/flintshot-<v>.x86_64.rpm` (installs
      cargo-generate-rpm pinned+locked)
- [ ] `[package.metadata.generate-rpm]` in
      `crates/flintshot-app/Cargo.toml`: Fedora package `requires`
      (gtk3, libayatana-appindicator-gtk3, pipewire-libs, libxcb,
      libXrandr, mesa-libEGL, mesa-libgbm, libwayland-client,
      libxkbcommon — finalize from `ldd` + Fedora package names),
      desktop entry + icon payload files
- [ ] `.sha256` generated and uploaded by the existing
      `package-linux.yml` (script emits both formats — no workflow
      change beyond picking up the new file)

**Verification:**
- [ ] Local no-upload script run, exit 0
- [ ] `docker run --rm -v "$PWD/target/dist:/a:ro" fedora:39 bash -c
      "dnf -y install /a/*.rpm && flintshot --version"` — exit 0
- [ ] `bash -n` on the script; `just dist-lint`
- [ ] Dispatch `packaging-test.yml`, green

**Dependencies:** Task 005

**Files likely touched:**
- `packaging/linux/build-packages.sh`
- `crates/flintshot-app/Cargo.toml`

**Docs:**
- GOTCHA candidate: Fedora renames (`libappindicator` vs
  `libayatana-appindicator-gtk3`) — record the resolved names.

**Estimated scope:** Small-Medium (2 files)
