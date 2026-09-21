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
