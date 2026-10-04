# ADR-0068: Release pipeline — cargo-dist core, custom jobs for non-native formats

**Context.** flintshot is a cross-platform desktop app with CI that
only linted/tested on Ubuntu; there was no way to ship it. The
release needs macOS (Intel + Apple Silicon), Windows (x64), and
Linux (deb + rpm + AppImage + tarball) artifacts, checksums, and
install channels (`curl | sh`, `cargo binstall`, Homebrew tap) —
cut with one command from a maintainer. Investigation at plan time
established hard constraints: cargo-dist (0.32, crates.io max) has
no `.deb`/`.rpm`/`.dmg`/AppImage artifact kinds and no
`universal-apple-darwin` target (issue #77); its `extra-artifacts`
are arch-independent-only (single copy per release, PR #613); and
`libspa` 0.10 (xcap 0.9's pipewire stack) bindgen-compiles against
the *system* PipeWire headers, which put a hard floor on build
environments.

**Decision.**

1. **cargo-dist generates the core**: per-arch macOS tarballs,
   Windows zip + msi, Linux tarball, shell/powershell installers,
   sha256 checksums, `dist-manifest.json` (cargo-binstall),
   Homebrew formula (`tap = "l1br3-io/homebrew-tap"`,
   `formula = "flintshot"`). Config: `dist-workspace.toml`, scoped
   with `packages = ["flintshot-app"]` + `precise-builds = true`
   so `flintshot-cosmic` is never compiled. `release.yml` is
   generated and checked in — regenerate only, never hand-edit
   (`dist generate --check` runs in CI).
2. **Custom `post-announce-jobs` for everything cargo-dist can't
   build**: reusable `workflow_call` workflows invoked from the
   generated release.yml after the GitHub Release exists.
   `package-linux` builds deb (cargo-deb 3.8.0), rpm
   (cargo-generate-rpm 0.21.0), and AppImage (linuxdeploy
   `1-alpha-20251107-1`) in a **`debian:12` container**;
   `package-macos-dmg` lipo-merges both arches into one universal
   `.app`/`.dmg`. Each uploads its own `.sha256` (bare filenames,
   so `sha256sum -c` works in the download dir). The packaging
   scripts are shared with `just dist-build` and a
   `packaging-test.yml` dispatch workflow, so branch-time smoke
   exercises the exact CI paths without a release.
3. **Linux floor: Debian 12 / Ubuntu 24.04+ / Fedora 39+**
   (user-approved). Ubuntu 22.04's PipeWire 0.3.48 headers cannot
   compile the capture stack (missing SPA symbols; 0.3.65 and 1.0.4
   verified to compile) — 22.04 users build from source. The native
   Linux job pins `ubuntu-24.04` (not `-latest`, so runner bumps
   can't silently change the environment); deb/rpm/AppImage build
   in the debian:12 container (glibc 2.36). Verified by container
   install tests: deb on debian:12 + ubuntu:24.04, rpm on fedora:39.
4. **dlopen'd libraries** (invisible to `ldd`-based tooling):
   `libayatana-appindicator3` is copied into the AppImage's AppDir
   then deployed with `--deploy-deps-only`; `libpipewire-0.3-0` is
   a deb `Depends` but deliberately NOT bundled in the AppImage
   (a bundled client newer than the session's daemon breaks the
   protocol — rely on the host's).
5. **One-command release**: `just release [patch|minor|major]` =
   CI gate → `dist plan` gate → cargo-release (workspace version
   bump, CHANGELOG `## [Unreleased]` rewrite, `v*` tag, push); the
   tag push triggers release.yml. `just release-dry` is the human
   checkpoint. Signing/notarization deferred; cargo-dist's native
   hooks (`ssldotcom-windows-sign`, `azure-windows-sign`) are the
   future drop-ins.

**Consequences.** Release artifact surface is fully covered but
split across generated and owned code — the boundary is explicit
(never hand-edit release.yml; regenerate). The Linux support floor
is a stated, tested contract rather than an accident of whatever
the runner had installed. Adding a format means extending the
shared script + a reusable workflow, not fighting cargo-dist.
Ubuntu 22.04 desktops are out of scope for prebuilt binaries.
AppImage runtime relies on the host's pipewire. First release
(`v0.1.0`) is the end-to-end acceptance run (spec checklist).

See also: `git log --grep='release-pipeline'` (spec + tasks with the
verification evidence), `justfile` (`dist-*`, `release*` recipes).
