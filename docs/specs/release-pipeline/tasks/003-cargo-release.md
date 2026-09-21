# Task 003: cargo-release wiring (`release.toml`, `just release` / `release-dry`)

**Description:** Configure cargo-release for consolidated workspace
releases: bump `[workspace.package] version` (all crates inherit),
rewrite CHANGELOG.md `## [Unreleased]` → `## [vX.Y.Z] - YYYY-MM-DD`
while inserting a fresh empty Unreleased section, create tag `vX.Y.Z`,
push. No crates.io publish. Add the `release` / `release-dry` just
recipes (`release` gates on `just ci` + `just dist-plan` first).

**Acceptance criteria:**
- [ ] `release.toml` at repo root: `publish = false`, consolidated
      commits/pushes, tag prefix `v`, verify disabled (CI gate covers
      it), `pre-release-replacements` for CHANGELOG.md as above
- [ ] `just release-dry` (→ `cargo release --workspace patch
      --no-execute`) previews: version `0.0.0` → `0.0.1`, CHANGELOG
      replacement applied to the preview, tag `v0.0.1` — and makes
      **zero** working-tree changes afterwards
- [ ] `just release-dry minor` previews `0.1.0` / `v0.1.0` (the
      intended first-release path)
- [ ] `just release level="patch"` recipe chains `just ci` →
      `just dist-plan` → `cargo release <level> --workspace
      --execute`
- [ ] Workspace version actually bumps on the dry-run preview — i.e.
      cargo-release handles `version.workspace = true` inheritance
      (if not, stop and record the fallback: a small bump script in
      the recipe + plain cargo-release tagging; decide with
      maintainer)

**Verification:**
- [ ] `just release-dry minor` — read preview, then `git status
      --porcelain` + `git diff` empty, exit 0
- [ ] Follow the cargo-dist book's cargo-release guide
      (book/workspaces/cargo-release-guide.html); only deviate where
      it conflicts with this repo's Keep-a-Changelog format

**Dependencies:** Task 001 (soft — `release` recipe calls
`just dist-plan`); can be built in parallel with Task 2

**Files likely touched:**
- `release.toml` (new)
- `justfile`

**Docs:**
- GOTCHA if hit: cargo-release + workspace-inherited versions —
  record the exact behavior in a `release.toml` comment.

**Estimated scope:** Small (2 files; requires `cargo install
cargo-release --locked`)
