# Task 007: macOS universal `.dmg` (lipo + `.app` + hdiutil)

**Description:** Custom post-announce job on `macos-latest` that
builds both macOS arches (`rustup target add x86_64-apple-darwin
aarch64-apple-darwin`; tray feature on — macOS needs no GTK),
`lipo`s them into one universal binary, assembles a proper `.app`
bundle (Info.plist with `LSUIElement` so the tray app has no Dock
icon; CFBundleIcon from the SVG → icns), wraps it in a `.dmg` via
`hdiutil`, and uploads with `.sha256`.

**Acceptance criteria:**
- [ ] `packaging/macos/build-dmg.sh`: universal build → lipo →
      `.app` (correct `CFBundleExecutable`, `LSUIElement=true`,
      icns icon) → `hdiutil create` →
      `target/dist/flintshot_<v>_universal.dmg`; honors
      `FLINTSHOT_PACKAGE_NO_UPLOAD=1`
- [ ] Icon conversion step (SVG → icns via `rsvg-convert` +
      `iconutil`, or a pre-generated `assets/icons/...icns`
      committed once — pick the committed-icns route if brew tools
      feel heavy; `png2icns` fallback)
- [ ] `.github/workflows/package-macos-dmg.yml`: `on:
      workflow_call`, `runs-on: macos-latest`, runs the script,
      `.sha256`, `gh release upload` on `github.ref_name`
- [ ] `dist.toml` `post-announce-jobs` gains `"./package-macos-dmg"`;
      release.yml regenerated
- [ ] `packaging-test.yml` gains the macOS no-upload leg

**Verification:**
- [ ] `bash -n` + `shellcheck` (if installed) on the script
- [ ] `just dist-lint` on the new workflow
- [ ] Push branch, dispatch `packaging-test.yml` — macOS leg green,
      artifact contains a valid universal binary (`lipo -info`
      printed in the job log) and a well-formed `.app`
- [ ] Real GUI launch + tray check happens at Task 10 on hardware —
      noted, not blocking here

**Dependencies:** Task 005 (packaging-test.yml pattern) — strictly
only the smoke workflow shape; can start after Task 2 if desired

**Files likely touched:**
- `packaging/macos/build-dmg.sh` (new)
- `.github/workflows/package-macos-dmg.yml` (new)
- `.github/workflows/packaging-test.yml`
- `dist.toml`
- `.github/workflows/release.yml` (regenerated)
- `assets/icons/` (icns, if that route is taken)

**Docs:**
- Unsigned dmg note: Gatekeeper will prompt on first run (right-click
  → Open); goes into release notes at Task 10.

**Estimated scope:** Medium (3-5 files)
