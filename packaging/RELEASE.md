# Releasing flintshot — the fire-the-release runbook

Everything here is **yours to fire** — the agent stages artifacts but
never pushes tags, never publishes, never submits PRs.

**Status:** the first release went out as **v0.1.1** on 2026-10-17
via `just release` (patch bump from the release-ring-staged 0.1.0
tree, which was never tagged — hence the version skip). §4–§8 below
activate the channels for v0.1.1. For every future release:
`just release [patch|minor]` automates §1–§3 (bump, changelog flip,
tag, push, matrix build) — then repeat §4–§6 at the new version.

All commands run from the repo root on `main`.

## 0. Preflight

```bash
just ci                                   # fmt + clippy + tests
just dist-plan                            # cargo-dist plan is coherent
git status --porcelain                    # clean tree, on main
```

- **`HOMEBREW_TAP_TOKEN` must exist** or the tap publish job fails
  (surfaced, not fixed, per the release-ring spec):

  ```bash
  gh secret list --repo l1br3-io/flintshot | grep HOMEBREW_TAP_TOKEN
  ```

  Missing → GitHub → Settings → Secrets and variables → Actions → new
  PAT with `repo` scope for the `l1br3-io/homebrew-tap` repo.

- Optional nice-to-have, **not a blocker**: the archived
  `vision-validate` Task 8 live manual pass (one hosted provider:
  save-banner + real transform; the Ollama `llama3:8b`
  silent-ignore repro) — `tasks/archive/vision-validate.md`.

## 1. Flip the changelog (fallback — `just release` automates this)

This is the rewrite `[package.metadata.release]` in
`crates/flintshot-app/Cargo.toml` performs on `just release`-driven
releases; hand-run it only when tagging by hand. Anchored,
exactly-once, and it leaves a fresh `## [Unreleased]` above the new
release section:

```bash
VER=0.1.2
sed -i "s/^## \[Unreleased\]$/## [Unreleased]\n\n## [$VER] - $(date +%F)/" CHANGELOG.md
grep -n "## \[$VER\]" CHANGELOG.md         # one new header, today's date
grep -c '^## \[Unreleased\]' CHANGELOG.md   # must print 1
git add CHANGELOG.md && git commit -m "chore(release): flintshot $VER"
```

## 2. Tag + push (the signed tag triggers everything)

```bash
git tag -s "v$VER" -m "flintshot $VER"   # signed, per the release cheat sheet
git push origin main "v$VER"
```

## 3. Watch release.yml build the matrix

```bash
gh run watch --repo l1br3-io/flintshot     # the run the tag just queued
```

Expect: 4 targets (x86_64-linux, x64-windows msi+zip, x64 + aarch64
macOS) with `.sha256` files, shell/powershell installers, the Homebrew
formula push to `l1br3-io/homebrew-tap` (needs the §0 secret), then
the post-announce deb/rpm/AppImage jobs (debian:12 container) and the
universal macOS dmg. Every artifact lands on the GitHub Release.

## 4. Publish the crates (this unlocks cargo-binstall)

`cargo login` once with a crates.io token if you haven't. Order is
load-bearing — each crate's registry requirement on the previous one
resolves only after it exists:

```bash
cargo publish -p flintshot-core
cargo publish --dry-run -p flintshot-editor-egui   # now resolvable; was impossible pre-core
cargo publish -p flintshot-editor-egui
cargo publish --dry-run -p flintshot-app
cargo publish -p flintshot-app
```

(`flintshot-cosmic` is never published — dist-workspace excludes it
and the crate is `publish = false`.)

## 5. Activate winget (submit the manifest triple)

1. Fill the placeholder checksum in
   `packaging/winget/l1br3.flintshot.installer.yaml` —
   `InstallerSha256` ← the Release asset's
   `flintshot-app-x86_64-pc-windows-msvc.msi.sha256`.
2. Fork [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs),
   branch, and copy the triple to
   `manifests/l/l1br3/flintshot/0.1.1/` (same three filenames).
3. On Windows: `winget validate --manifest manifests/l/l1br3/flintshot/0.1.1`
   (the repo's PR pipeline validates too).
4. Open the PR — title `New package: l1br3.flintshot version 0.1.0`;
   the submission guide is
   <https://github.com/microsoft/winget-pkgs/tree/master/doc/manual/PackageSubmission.md>.

## 6. Activate AUR (push flintshot-bin)

```bash
git clone ssh://aur@aur.archlinux.org/flintshot-bin.git   # first push creates the repo
cp packaging/aur/flintshot-bin/PKGBUILD packaging/aur/flintshot-bin/.SRCINFO flintshot-bin/
cd flintshot-bin
updpkgsums                          # pacman-contrib; fills all three sha256s from the tag-pinned sources
makepkg --printsrcinfo > .SRCINFO   # regenerate after the sums changed
git add PKGBUILD .SRCINFO && git commit -m "flintshot-bin 0.1.0" && git push
```

The `libxdo3` dependency is AUR — note that in the first AUR comment
(users of plain `makepkg -si` must install it first; the PKGBUILD's
header comment explains why it exists).

## 7. Homebrew — automatic, just verify

§3's run pushed the formula to the tap (job `publish-homebrew`).

```bash
brew install l1br3-io/tap/flintshot
```

## 7b. Flathub — the COSMIC Store channel (manual submission)

Artifacts live in-repo: `packaging/flatpak/io.l1br3.Flintshot.yaml`
+ `assets/metainfo/io.l1br3.Flintshot.metainfo.xml`. The v1
sandbox scope is daemon-less/tray-less (see SPEC-flathub.md).

1. **Screenshots first** — Flathub requires real ones, and the
   metainfo's URLs point at tag-pinned raws: capture
   `docs/screenshots/{editor,ai,home}.png` **with flintshot itself**,
   commit them, and land them in a tagged release (the next `just
   release`); retarget the metainfo URLs at that tag.
2. Re-validate: `appstreamcli validate assets/metainfo/*.metainfo.xml`
   — only the inherent `cid-rdns-contains-hyphen` warning (the
   `l1br3-io` org name) may remain.
3. Local proof (what the flathub effort ran):
   `flatpak-builder --force-clean --user --install-deps-from=flathub --repo=/tmp/fsh-repo /tmp/fsh-build packaging/flatpak/io.l1br3.Flintshot.yaml`
4. Submit via a [Flathub](https://flathub.org) account (new-app
   submission, carrying the manifest + metainfo); their build + lint
   runs the matrix — fix findings, wait for review.
5. Post-acceptance: `flatpak install flathub io.l1br3.Flintshot`
   works and the COSMIC Store lists it (it surfaces Flathub).
6. Per release: bump the manifest's git `tag:` and add a
   `<release>` row to the metainfo.

Manual sandbox checklist (record in the submission PR): app window
opens, portal capture round-trips (region select → editor), save lands
in `~/Pictures/Flintshot`, clipboard Copy behaves per the spike
verdict, AI + imgur reach the network, keyring reads work.

## 8. Smoke every channel

```bash
cargo binstall flintshot-app   # CRATE name (installs the `flintshot` bin; reads §4's manifests)
winget install --id l1br3.flintshot --source winget     # after §5 merges
yay -S flintshot-bin           # after §6
flintshot --version            # everywhere: prints 0.1.0
```

## 9. Post-release fill-ins + next release

- Fill the real `sha256sums` in `packaging/aur/flintshot-bin/PKGBUILD`
  and the real `InstallerSha256` in the winget installer manifest, and
  commit — keeps the repo templates truthful for diffing.
- Keep each release's changelog section concise: the GitHub Release
  body IS the `## [x.y.z]` section, and it passes through an
  argv-limited step — the first v0.1.1 run died with "Argument list
  too long" (~180 KB of first-release history) until the detail moved
  under the non-version `## Pre-release history` heading. Never let a
  single section approach ~100 KB.
- Next release: bump `PackageVersion` (winget ×3 files) and `pkgver`
  (AUR) alongside the version, re-templatize the URLs/sums, and walk
  this runbook again; §5–§6 are the same flow at the new version.
