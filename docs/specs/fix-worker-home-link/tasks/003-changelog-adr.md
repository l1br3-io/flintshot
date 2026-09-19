# Task 003: CHANGELOG entry + ADR-0065

**Description:** Record the revised window-management stance: the
worker editor's Home breadcrumb link delegates the daemon's
`OpenApp` choreography and then exits, instead of closing bare.
CHANGELOG under `## [Unreleased]` (Fixed category — the link was a
close in navigation clothing); ADR-0065 captures the decision
(Context · Decision · Consequences) per AGENTS.md's "revises a
non-obvious technical choice" rule.

**Acceptance criteria:**
- [x] `CHANGELOG.md` `## [Unreleased]` gains a **Fixed** entry: worker editor's Home link navigates (app window focus/spawn via the daemon) instead of immediately closing the window; references ADR-0065
- [x] `docs/adrs/ADR-0065-worker-home-delegates-to-app-window.md` exists with the repo's Context · Decision · Consequences shape, cross-referencing the single-window spec and the `open_settings_from_worker` precedent
- [x] ADR notes the deliberate edges: a running app window focuses on its *current* route (no force-route); the delegate runs on the UI thread (Settings precedent); hook-absent fallback keeps the Esc contract

**Verification:**
- [x] `python3 .opencode/harness/scripts/check-refs.py` (if it covers docs refs) or manual link check: referenced files/ADR numbers exist
- [x] Full CI gate one last time: fmt, clippy, workspace tests, doc tests

**Dependencies:** Tasks 001 + 002 (the record must describe the shipped behavior)

**Files likely touched:**
- `CHANGELOG.md`
- `docs/adrs/ADR-0065-worker-home-delegates-to-app-window.md` (new)

**Docs:**
- This task *is* the docs.

**Estimated scope:** Small: 2 files
