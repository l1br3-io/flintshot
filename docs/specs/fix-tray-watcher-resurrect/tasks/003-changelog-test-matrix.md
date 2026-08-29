# Task 003: CHANGELOG + TEST_MATRIX updates

**Description:** Record the fix in the standing docs. CHANGELOG
gains a `Fixed` entry under `## [Unreleased]`: the daemon no longer
waits passively for a StatusNotifierWatcher — it pokes the watcher's
well-known names at startup and every 30 s when unowned, so a dead
or late tray host self-heals (bounded, best-effort, `tray` feature +
Linux only). TEST_MATRIX's Phase 2 matrix gains two rows mirroring
the spec's exit criteria 1–2: icon returns within ~30 s of the
watcher being killed (daemon left running), and a fresh daemon on a
watcher-dead session shows the icon at startup. Both start
`pending-maintainer-L5` pointing at the Task 5 runbook — honest
pending, no simulated results.

**Acceptance criteria:**

- [ ] CHANGELOG `## [Unreleased]` → `### Fixed` entry (Keep-a-Changelog
      voice, matches existing entries' density)
- [ ] TEST_MATRIX Phase 2 matrix: two new rows (watcher killed
      mid-session → icon returns ≤ ~30 s; fresh daemon with watcher
      dead → icon at startup), columns blank, notes column marked
      `pending-maintainer-L5 — fix-tray-watcher-resurrect runbook
      steps N–M` with the landing commit hash
- [ ] No existing rows or findings edited

**Verification:**

- [ ] `grep -n "StatusNotifierWatcher" CHANGELOG.md docs/TEST_MATRIX.md`
      shows the new entry + rows
- [ ] Rows reference the runbook that Task 5 will add (steps land
      before any L5 pass is attempted)

**Dependencies:** Task 2 (commit hash to cite) — or lands in the same
series with the code commits' hashes amended in at Task 5 closeout
if preferred; cite the Task 2 commit.

**Files likely touched:**

- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`

**Docs:**

- This task *is* docs.

**Estimated scope:** Small: 2 files
