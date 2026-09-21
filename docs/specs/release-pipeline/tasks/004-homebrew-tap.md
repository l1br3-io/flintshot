# Task 004: Homebrew tap channel

**Description:** Enable cargo-dist's Homebrew installer: add
`"homebrew"` to `installers`, set `tap = "anomalyco/homebrew-tap"`
and `publish-jobs = ["homebrew"]` in `dist.toml`. The tap repo and
its `HOMEBREW_TAP_TOKEN` secret are user actions — this task ships
the config plus the exact setup instructions.

**Acceptance criteria:**
- [ ] `dist.toml`: `installers` includes `"homebrew"`,
      `tap = "anomalyco/homebrew-tap"`,
      `publish-jobs = ["homebrew"`
- [ ] `cargo dist plan` exit 0; plan shows the homebrew publish job
      and the formula artifact
- [ ] `release.yml` regenerated (homebrew publish job present)
- [ ] Setup instructions for the maintainer written into the task
      file / release docs (create empty repo `anomalyco/
      homebrew-tap`, PAT with repo scope → repo secret
      `HOMEBREW_TAP_TOKEN` on `anomalyco/flintshot`)
- [ ] No `[dist.dependencies.homebrew]` entries needed (tray's GTK
      deps are Linux-only — verified: `gtk`/`zbus` are
      target-scoped in flintshot-app/Cargo.toml)

**Verification:**
- [ ] `just dist-plan` exit 0
- [ ] `cargo dist generate ci` no diff after regeneration commit
- [ ] Manual (user-gated): tap repo exists + secret set — real
      formula publish verified at Task 10, not here

**Dependencies:** Task 002 (regenerating release.yml)

**Files likely touched:**
- `dist.toml`
- `.github/workflows/release.yml` (regenerated)

**Docs:**
- Formula name defaults to package name (`flintshot-app`?) — if the
  formula/binary naming looks off in the plan (`flintshot` expected),
  set `formula = "flintshot"` and note why.

**Estimated scope:** Small (1-2 files + human setup)

## Maintainer setup (required before first release — Task 010)

The tap repo and token are user-side actions; the pipeline cannot
create them:

1. **Create the tap repo**: <https://github.com/new> → owner
   `l1br3-io`, name `homebrew-tap`, **public**, no README/license
   (cargo-dist pushes `Formula/flintshot.rb` itself; an empty repo
   keeps the first push conflict-free).
2. **Create a PAT**: <https://github.com/settings/tokens> →
   "Fine-grained" or classic with `repo` scope (classic) / "Contents:
   read and write" on `l1br3-io/homebrew-tap` only (fine-grained).
3. **Add the secret** on the main repo:
   <https://github.com/l1br3-io/flintshot/settings/secrets/actions> →
   `HOMEBREW_TAP_TOKEN` = the PAT.
4. After the first release: `brew tap l1br3-io/tap
   https://github.com/l1br3-io/homebrew-tap && brew install flintshot`
   (or `brew install l1br3-io/tap/flintshot`) verifies the channel.

Note: `tap` in dist-workspace.toml uses `l1br3-io` (the real git
remote), replacing the spec's assumed `anomalyco` org.
