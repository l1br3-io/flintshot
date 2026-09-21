---
status: active
started: 2026-09-21
---

# Spec: Cross-platform release pipeline (flintshot)

## Objective

flintshot is a pre-1.0 desktop app (version `0.0.0`) with CI that only
lints/tests on Ubuntu. There is no way to ship it. This effort adds a
**tag-driven release pipeline** that builds, packages, and distributes
flintshot for macOS, Windows, and Linux, plus `just` recipes so a
maintainer can cut a release with one command.

"Deployment" for this project = **release**: git tag → CI build matrix
→ packaged artifacts → GitHub Release → install channels. There is no
server deployment.

**User stories:**

- As a maintainer, I run `just release patch` (or minor/major) and the
  pipeline bumps the version, updates the changelog, tags, builds all
  platforms, and publishes a GitHub Release with checksums — no manual
  steps beyond confirming the dry run.
- As a macOS user, I download a `.dmg` (with a proper `.app` bundle) or
  a universal `.tar.gz` from the GitHub Release; it runs natively on
  Intel and Apple Silicon.
- As a Windows user, I download an `.msi` (or portable `.zip`) and get
  a Start-menu entry and clean uninstall.
- As a Linux user, I install the `.deb` (Debian/Ubuntu, tray deps
  auto-resolved) or `.rpm` (Fedora/RHEL/openSUSE), grab the
  `.AppImage`, or use the plain `.tar.gz`.
- As any developer, I can `cargo binstall flintshot` (prebuilt binary
  from the GitHub Release) or `brew install` from the project tap.

**Non-goals (this effort):**

- Code signing / notarization (macOS notarization, Windows Authenticode)
  — deferred; the pipeline is structured so secrets slot in later.
- Self-update inside the app (updater UX).
- `flintshot-cosmic` in release artifacts — it stays an opt-in
  CI-build-only crate.
- Flatpak / Snap packaging.
- crates.io publishing (source installs).

## Technologies

| Tool             | Role                                                            |
| ---------------- | --------------------------------------------------------------- |
| **cargo-dist** (pinned) | Generates the release workflow, cross-platform build matrix, native artifacts (`.tar.gz`/`.zip`, `.msi`, shell/powershell installers), SHA256 checksums, `dist-manifest.json` (what makes `cargo binstall` work), and the Homebrew formula. Config: `dist.toml`. Extended via custom `post-announce-jobs` for the non-native formats. |
| **cargo-release** | Version bump (workspace-consolidated), CHANGELOG `## [Unreleased]` → version header, git tag `vX.Y.Z`, push. Config: `release.toml`. |
| **cargo-deb** | `.deb` construction — invoked by our custom Linux packaging job/script. Runtime deps + desktop entry + icon declared in `[package.metadata.deb]`. |
| **cargo-generate-rpm** | `.rpm` construction — invoked by the same job. Runtime deps in `[package.metadata.generate-rpm]`. |
| WiX (via cargo-dist) | Windows `.msi` packaging — installed automatically by the generated workflow. |
| `hdiutil` + `lipo` + custom `.app` bundling script | macOS **universal** `.dmg` (cargo-dist has no native dmg and no `universal-apple-darwin` target — issue #77 — so the dmg job builds both arches and `lipo`s them itself). |
| linuxdeploy / appimagetool | `.AppImage` built by the custom Linux packaging job. |
| actionlint (optional) | Lint GitHub workflow syntax in `just dist-lint`. |
| GitHub Actions | Runners: `macos-latest`, `windows-latest`, Linux x64 pinned to `ubuntu-22.04` via `[dist.github-custom-runners]` (glibc 2.35 floor → runs on Debian 12+, Ubuntu 22.04+, Fedora 39+). |

Pinning rule: `cargo-dist-version` and `cargo-release` versions are
pinned exact in `dist.toml` / `release.toml` so releases are
reproducible. Versions are chosen at plan time after verifying current
maintenance status (see Risks — cargo-dist maintenance).

## Commands

### Existing (unchanged gates, reused by release flow)

```sh
just ci            # fmt --check + clippy + tests (the pre-release gate)
cargo dist plan    # validate the release plan without building
```

### New just recipes (the deliverable)

Recipes follow justfile conventions: `set positional-arguments`,
`#!/usr/bin/env bash` + `set -euo pipefail` for anything non-trivial,
`[private]` helpers, quiet-on-success output, comments explain "why"
not "what".

```just
# Validate the cargo-dist release plan (no build, no network).
# Fast sanity check that dist.toml + Cargo.toml produce the expected
# artifact set. Run before merging any release-config change.
dist-plan:
    cargo dist plan -q

# Build host-platform artifacts locally without publishing — smoke
# test the packaging config (deb/rpm/AppImage on Linux, msi/zip on Windows,
# tar.gz + dmg on macOS). Output under target/dist/.
dist-build:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo dist build --all -q
    # Custom formats share the same scripts CI uses (no upload).
    if [[ "$(uname -s)" == "Linux" ]]; then
        FLINTSHOT_PACKAGE_NO_UPLOAD=1 bash packaging/linux/build-packages.sh
    elif [[ "$(uname -s)" == "Darwin" ]]; then
        FLINTSHOT_PACKAGE_NO_UPLOAD=1 bash packaging/macos/build-dmg.sh
    fi

# Lint GitHub workflow files. Uses actionlint when installed;
# otherwise prints a skip note (not a failure — matches the
# graceful-fallback convention of `bootstrap` recipes).
dist-lint:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v actionlint >/dev/null 2>&1; then
        actionlint
    else
        echo "ok: actionlint not on PATH; skipping (cargo install actionlint)"
    fi

# Dry-run the release: show what cargo-release would do (version bump,
# changelog edit, tag, push) without executing anything.
release-dry level="patch":
    cargo release {{ level }} --workspace --no-execute

# Cut a release. Runs the full CI gate first (never release red),
# then `just dist-plan` (never release a broken plan), then
# cargo-release with --execute. cargo-release pushes the tag; the
# tag push triggers .github/workflows/release.yml which builds the
# matrix and publishes the GitHub Release.
#   just release          # 0.x.y -> 0.x.(y+1)
#   just release minor    # 0.x.* -> 0.(x+1).0
release level="patch":
    #!/usr/bin/env bash
    set -euo pipefail
    just ci
    just dist-plan
    cargo release {{ level }} --workspace --execute
```

(`release-dry` before `release` is the intended human checkpoint —
cargo-release also prompts for confirmation on `--execute`.)

### Underlying full commands (for reference / CI)

```sh
cargo dist plan                                   # plan validation
cargo dist build --all                            # local artifact build
cargo release patch --workspace --no-execute      # dry run
cargo release patch --workspace --execute         # real release
```

The generated `.github/workflows/release.yml` (by cargo-dist) handles
the rest: trigger on tag push matching `v*`, build matrix, checksums,
GitHub Release upload, Homebrew formula push to the tap repo.

## Project Structure

```
dist.toml                       # NEW — cargo-dist config: targets, packages
                                #   = ["flintshot-app"], features = ["tray"],
                                #   precise-builds, apt build deps, custom
                                #   runners, installers, post-announce-jobs
release.toml                    # NEW — cargo-release config: workspace-
                                #   consolidated versions, changelog
                                #   replacements, no publish, tag prefix "v"
.github/workflows/release.yml   # NEW — generated by cargo-dist, checked in;
                                #   regenerate via `cargo dist init`, never
                                #   hand-edit beyond regeneration
.github/workflows/package-linux.yml   # NEW — custom job (workflow_call):
                                #   deb + rpm (+ AppImage) on ubuntu-22.04,
                                #   uploads to the release with .sha256
.github/workflows/package-macos-dmg.yml # NEW — custom job (workflow_call):
                                #   universal .app + .dmg, uploads with .sha256
.github/workflows/packaging-test.yml   # NEW — workflow_dispatch smoke: runs
                                #   both packaging jobs' scripts with upload
                                #   disabled, on a branch, without a release
packaging/                      # NEW — custom packaging helpers, shared by
├── macos/                      #   CI jobs and `just dist-build`
│   └── build-dmg.sh            #   universal lipo build, .app assembly
│                               #   (Info.plist with LSUIElement), hdiutil
└── linux/
    └── build-packages.sh       #   cargo-deb + cargo-generate-rpm (+ AppImage)
docs/specs/release-pipeline/    # this spec
```

Artifact set per release:

| Platform | Artifacts                                    |
| -------- | -------------------------------------------- |
| macOS    | `flintshot_<v>_x86_64-apple-darwin.tar.gz`, `flintshot_<v>_aarch64-apple-darwin.tar.gz`, `flintshot_<v>_universal.dmg` (custom job) |
| Windows  | `flintshot_<v>_x86_64-pc-windows-msvc.zip`, `.msi` |
| Linux    | `flintshot_<v>_x86_64-unknown-linux-gnu.tar.gz`, `.deb` (custom job), `.rpm` (custom job), `.AppImage` (custom job) |

Native cargo-dist artifacts get `.sha256` files + `dist-manifest.json`
(consumed by `cargo binstall`). Custom-job artifacts ship with
`.sha256` files generated and uploaded by their own jobs.

## Code Style

Config files follow repo conventions: TOML keys documented by
comments only when the "why" is non-obvious, no dead config, exact
version pins. Example — the shape of `dist.toml` (exact keys verified
against cargo-dist docs at plan time):

```toml
[dist]
cargo-dist-version = "0.33.0"       # pinned exact
packages = ["flintshot-app"]        # never package flintshot-cosmic
features = ["tray"]                 # release = the launcher promise
precise-builds = true               # --package builds only; the workspace
                                    # build would drag in flintshot-cosmic
                                    # (needs libcosmic system deps)
targets = [
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
]
installers = ["shell", "powershell", "msi"]
pr-run-mode = "upload"              # PRs build artifacts to validate the plan
post-announce-jobs = ["./package-linux", "./package-macos-dmg"]

[dist.github-custom-runners]
# glibc 2.35 floor: binary runs on Debian 12+, Ubuntu 22.04+, Fedora 39+
x86_64-unknown-linux-gnu = "ubuntu-22.04"

[dist.dependencies.apt]             # xcap + tray build deps for the Linux job
pkg-config = "*"
libclang-dev = "*"
libxcb1-dev = "*"
libxrandr-dev = "*"
libdbus-1-dev = "*"
libpipewire-0.3-dev = "*"
libwayland-dev = "*"
libegl-dev = "*"
libgbm-dev = "*"
libgtk-3-dev = "*"
libayatana-appindicator3-dev = "*"
```

Justfile additions follow the existing file's style (shown in
Commands). Generated workflow YAML is cargo-dist's — not hand-styled.

## Testing Strategy

A release pipeline can't be unit-tested; verification is layered:

1. **Plan validation (fast, every PR):** a new CI job runs
   `cargo dist plan` so a broken `dist.toml`/feature/target combo
   fails before merge. Locally: `just dist-plan`.
2. **PR artifact build (medium, every PR touching release config):**
   `pr-run-mode = "upload"` makes release.yml build (but not publish)
   artifacts on PRs, proving the matrix stays green.
3. **Local smoke (per maintainer, pre-release):** `just dist-build`
   then inspect `target/dist/` — install the `.deb` in a fresh
   container (`docker run --rm -v target/dist:/a debian:bookworm` …
   `apt install /a/*.deb && flintshot --version`), mount the `.msi` /
   `.dmg` steps are manual.
4. **Post-release checklist (per release, manual):** download each
   artifact from the GitHub Release, verify its `.sha256`, run
   `flintshot --version` + one capture on at least one Linux and one
   macOS/Windows machine, confirm `cargo binstall flintshot@<v>`
   resolves, confirm the Homebrew formula updated.
5. **Workflow lint:** `just dist-lint` (actionlint) in the optional
   toolchain; CI runs actionlint on workflow changes when available.

No unit tests are added by this effort; packaging scripts under
`packaging/` carry `set -euo pipefail` + explicit failure messages
instead.

## Boundaries

**Always:**
- Version bumps go through `just release` / cargo-release — never a
  manual `git tag` or manual version edit.
- Run `just ci` before any release (enforced by the `release` recipe).
- Keep `cargo-dist` / `cargo-release` versions pinned exactly.
- Ship `.sha256` checksums for every artifact (cargo-dist default —
  never disable).
- Keep the workspace version consolidated (all crates share
  `workspace.package` version).
- Update `CHANGELOG.md` via cargo-release replacements.

**Ask first:**
- Adding/removing a target, artifact format, or distribution channel.
- Adding repo secrets (signing keys, `HOMEBREW_TAP_TOKEN`).
- Changing the tag pattern (`v*`) or trigger model.
- Enabling crates.io publishing.
- Editing the generated `release.yml` in any way other than
  regenerating it with cargo-dist.

**Never:**
- Commit secrets or tokens.
- Hand-edit a published GitHub Release's binaries (a bad artifact is
  deleted and the release re-cut as `vX.Y.Z-1` patch… or the tag
  deleted and re-pushed before anyone installs it — decide at the
  time; never silently swap files).
- Ship a release that skips the CI gate.
- Package `flintshot-cosmic` into release artifacts.

## Success Criteria

- [ ] `cargo dist plan` succeeds locally and lists the native artifact
      set (per-arch macOS + Windows zip/msi + Linux tarball, checksums,
      shell/powershell/msi installers) — no `flintshot-cosmic`.
- [ ] Pushing a `v*` tag produces a green matrix build in
      `.github/workflows/release.yml` and a GitHub Release containing
      all native artifacts + `.sha256` files + `dist-manifest.json`,
      plus the custom-job artifacts (`.deb`, `.rpm`, `.AppImage`,
      universal `.dmg`) each with `.sha256`.
- [ ] `just release-dry` previews, and `just release patch` performs:
      CI gate → plan validation → version bump → changelog edit →
      tag → push, with no manual steps between confirm and Release.
- [ ] `.deb` installs on a clean Debian 12 / Ubuntu 22.04 container
      and `flintshot daemon` starts with a working tray (GTK +
      appindicator runtime deps auto-resolved by apt).
- [ ] `.rpm` installs on a clean Fedora container likewise.
- [ ] `.dmg` contains a universal `.app` bundle that launches on
      macOS (Intel and Apple Silicon), tray icon appears.
- [ ] `.msi` installs on Windows with Start-menu entry and uninstalls
      cleanly.
- [ ] `cargo binstall flintshot@<first-version>` installs the
      prebuilt binary from the GitHub Release.
- [ ] The Homebrew tap repo's formula updates on release and
      `brew install <tap>/flintshot` succeeds on macOS.
- [ ] First release version is `0.1.0` (assumption; confirm below).

## Risks & Mitigations

| Risk | Mitigation |
| ---- | ---------- |
| ~~cargo-dist maintenance status~~ | **Resolved at planning**: actively maintained (v0.33.0, Sep 2026). |
| `extra-artifacts` are **arch-independent only** (single copy per release, runs on the global ubuntu job) — not usable for deb/rpm/dmg/AppImage. | Custom `post-announce-jobs` (reusable `workflow_call` workflows) on pinned runners: `package-linux` (deb/rpm/AppImage, ubuntu-22.04), `package-macos-dmg` (universal lipo dmg). Each uploads its own `.sha256`. |
| No `universal-apple-darwin` target (cargo-dist #77, open). | Ship per-arch macOS tarballs natively (standard practice — matches rustup/cargo-dist itself); the custom dmg job builds both arches and `lipo`s them into one universal `.app`/`.dmg`. |
| `.deb`/`.rpm` are not native cargo-dist artifact kinds. | cargo-deb + cargo-generate-rpm with runtime deps derived from `ldd` of the built binary (GTK/appindicator/xcap libs); verified by container install tests (Debian 12, Fedora 39+). |
| AppImage + GTK3/appindicator bundling is fragile (linuxdeploy GTK plugin maturity). | Timeboxed attempt; fallback decision point: drop the AppImage format for v1 of this effort (documented here + CHANGELOG). |
| Linux glibc portability — `ubuntu-latest` (24.04) links against glibc 2.39. | `[dist.github-custom-runners]` pins the Linux target job to `ubuntu-22.04` (glibc 2.35); custom packaging job uses the same runner. Verified by the Debian 12 smoke test. |
| cargo-dist `--workspace` build would compile `flintshot-cosmic` (needs libcosmic system deps not installed in the release job). | `precise-builds = true` + `packages = ["flintshot-app"]`; caught immediately by `cargo dist plan`. |
| cargo-release with workspace-inherited versions (`version.workspace = true`). | Follow the cargo-dist book's cargo-release guide; verify bump applies to `[workspace.package] version` in the dry run before any `--execute`. |
| Homebrew formula runtime deps. | None needed — the `tray` feature's GTK/appindicator deps are Linux-only (`gtk`/`zbus` are target-scoped in flintshot-app's Cargo.toml); on macOS tray-icon uses native APIs. Formula stays dependency-free. |

## Open Questions

1. **First release version** — `0.1.0`? (assumed)
2. **Tap repo name** — `anomalyco/homebrew-tap`? Needs creating +
   `HOMEBREW_TAP_TOKEN` secret before first release.
3. **AppImage fallback** — if GTK bundling proves unstable: drop the
   format? (decided: drop, revisit — not the no-tray variant, to keep
   the "tray everywhere" release promise)
4. ~~glibc baseline~~ — **Resolved at planning**: `ubuntu-22.04`
   runner pin → glibc 2.35 floor (Debian 12+, Ubuntu 22.04+,
   Fedora 39+).
