# Task 003: CHANGELOG + TEST_MATRIX updates

**Description:** Record both fixes in the user-facing changelog and
the test matrix. TEST_MATRIX findings 1–2 get marked as fixed with
commit refs and their automated regression tests named — the manual
L5 re-verification is explicitly marked pending (no desktop session
exists in the dev container; the maintainer runs exit criteria 1–3
before merge). Fill the Phase 2 delegated-capture rows with the same
honest pending-manual status.

**Acceptance criteria:**

- [ ] `CHANGELOG.md` `## [Unreleased]` → **Fixed**: two entries
      (fork loop; at-spi standalone crash), one per failure mode
- [ ] `docs/TEST_MATRIX.md` "L5 pass findings" 1–2 annotated as
      fixed with commit refs + regression-test names, manual L5
      re-run marked pending
- [ ] No predicted-passes recorded as real results

**Verification:**

- [ ] Claims cross-checked against the actual commits (`git log`)
- [ ] Doc build not applicable; markdown reviewed by reading

**Dependencies:** Tasks 1, 2 (needs their commit hashes)

**Files likely touched:**

- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`

**Docs:**

- This task *is* docs.

**Estimated scope:** Small: 2 files
