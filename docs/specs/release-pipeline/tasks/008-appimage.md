# Task 008: `.AppImage` (timeboxed, fallback: drop)

**Description:** Add a tray-enabled `.AppImage` to the Linux
packaging script via linuxdeploy (AppDir with the existing `.desktop`
+ icons; GTK3/appindicator libraries pulled in by linuxdeploy's
dependency tracing or the GTK plugin). This is the flagged-risk
artifact: **timebox to one focused session on the bundling**. If
GTK-in-AppImage proves unstable, drop the format (decided fallback —
see spec Open Questions) and record it; do not ship a no-tray
AppImage.

**Outcome (attempt path taken, succeeded):**
- linuxdeploy `1-alpha-20251107-1`, `--appimage-extract-and-run`
  (no FUSE in CI), `OUTPUT` env for the final name.
- GOTCHA: `--deploy-deps-only` requires the lib *inside* the AppDir
  first — copy the dlopen'd `libayatana-appindicator3.so.1` into
  `usr/lib/`, then deploy its deps.
- GOTCHA: appimagetool requires the `file` utility — added to the
  job's apt list.
- GOTCHA: libpipewire is dlopen'd (not NEEDED-linked) and is
  deliberately NOT bundled — a bundled client newer than the
  session's daemon breaks the protocol; rely on the host's
  (standard AppImage practice). Same discovery added
  `libpipewire-0.3-0` to the deb's Depends.
- Verified: built + runs via extract-and-run in the debian:12
  container (floor); 62 bundled libs incl. gtk-3 + appindicator.

**Acceptance criteria (attempt path):**
- [x] `packaging/linux/build-packages.sh` produces
      `target/packages/flintshot_<v>_x86_64.AppImage` (linuxdeploy
      downloaded pinned by SHA256 or version-pinned URL;
      `--appimage-extract-and-run` used so no FUSE needed in CI)
- [x] AppDir carries `flintshot.desktop` + hicolor icons
- [x] `.sha256` + upload handled by the existing job

**Acceptance criteria (fallback path — if timebox expires):**
- [ ] AppImage dropped from spec artifact table + CHANGELOG note +
      `docs/gotchas.md` entry explaining what failed (which
      bundling step, which library)

**Verification:**
- [ ] Local no-upload script run produces the AppImage; `./flintshot
      *.AppImage --version` runs via extract-and-run on this
      machine (X11/Wayland session permitting — else dispatch smoke
      only)
- [ ] Dispatch `packaging-test.yml` green
- [ ] `bash -n`, `just dist-lint`

**Dependencies:** Task 005

**Files likely touched:**
- `packaging/linux/build-packages.sh`
- `docs/specs/release-pipeline/spec.md` (artifact table, if fallback)

**Docs:**
- GOTCHA: whatever breaks GTK bundling goes in
  `docs/gotchas.md` with the exact error — this is the
  institutional knowledge that makes a future retry cheap.

**Estimated scope:** Medium (1-2 files, high-uncertainty work)
