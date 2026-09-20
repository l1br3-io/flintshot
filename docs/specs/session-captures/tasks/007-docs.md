# Task 007: Docs — CHANGELOG, ADR, AGENTS row

**Description:** Record the effort. `CHANGELOG.md` under
`## [Unreleased]` (user-visible: session captures survive navigation;
NewCapture stashes instead of discarding). New
`docs/adrs/ADR-0067-session-capture-store.md` (Context · Decision ·
Consequences — the temp-dir store, app-window ownership,
flush-at-leave over snapshots, removal semantics). One row in
`AGENTS.md`'s "Where things live" table for
`crates/flintshot-core/src/session.rs`. Spec frontmatter stays
`active` until merge flips it.

**Acceptance criteria:**
- [ ] CHANGELOG entry present and user-framed
- [ ] ADR-0067 follows the numbered Context/Decision/Consequences format of its siblings
- [ ] AGENTS.md table row added; no other AGENTS bloat

**Verification:**
- [ ] `python3 .opencode/harness/scripts/check-refs.py` (or manual scan) — no dangling doc references
- [ ] Full gate commands one final time

**Dependencies:** Tasks 001-006

**Files likely touched:**
- `CHANGELOG.md`
- `docs/adrs/ADR-0067-session-capture-store.md` (new)
- `AGENTS.md`

**Docs:**
- This task *is* docs

**Estimated scope:** Small
