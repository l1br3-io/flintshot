# Task 010: First release `v0.1.0` + post-release checklist

**Description:** Execute the real end-to-end release with the
maintainer: confirm `0.1.0`, ensure the tap repo + secret exist, run
`just release minor`, watch the pipeline, then walk the post-release
checklist from the spec. This is the acceptance run for the whole
effort — human-gated at the tag push.

**Acceptance criteria:**
- [ ] Maintainer confirmed `0.1.0` + tap repo `l1br3-io/
      homebrew-tap` exists with `HOMEBREW_TAP_TOKEN` set
- [ ] CHANGELOG `## [Unreleased]` contains release-worthy notes
      (effort summary)
- [ ] `just release minor` ran: CI gate → plan → bump → changelog →
      tag `v0.1.0` → push; release.yml + both custom jobs green
- [ ] GitHub Release `v0.1.0` contains: 2 macOS tarballs, universal
      dmg, Windows zip + msi, Linux tarball + deb + rpm (+ AppImage
      unless dropped) — each with `.sha256`; plus
      `dist-manifest.json`, shell/powershell installers, homebrew
      formula pushed to the tap
- [ ] Checklist (record results in this file when done):
  - [ ] Every `.sha256` verifies (`sha256sum -c`)
  - [ ] Released `.deb` installs + `flintshot --version` in
        `debian:12` container
  - [ ] Released `.rpm` installs in `fedora:39` container
  - [ ] `cargo binstall flintshot@0.1.0` resolves + installs
  - [ ] `brew install l1br3-io/tap/flintshot` on macOS
  - [ ] `.dmg` `.app` launches on macOS, tray icon appears
        (Intel or AS; both if available)
  - [ ] `.msi` installs/uninstalls on Windows, Start-menu entry
  - [ ] One capture round-trip on at least one Linux + one
        macOS/Windows machine
- [ ] Any failures → fixes land as `v0.1.1` follow-ups, never
      silent artifact swaps (spec boundary)

**Verification:**
- [ ] The checklist above, executed and checked off
- [ ] `spec.md` success criteria re-read; met or explicitly deferred
      with sign-off

**Dependencies:** Tasks 1-9

**Files likely touched:**
- `CHANGELOG.md` (notes polish, pre-tag)
- This task file (checklist results)

**Docs:**
- After merge to main, the merge commit flips `spec.md` frontmatter
  to `status: shipped` + `shipped: YYYY-MM-DD` (effort lifecycle).

**Estimated scope:** Execution run (human-gated; ~0 code files)
