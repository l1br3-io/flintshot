# Task 010: ADR, CHANGELOG, docs

**Description:** Record the single-window decision and close the effort's
paper trail. (1) New `docs/adrs/ADR-00NN-single-window-app.md` (Context ·
Decision · Consequences): one routed window per process, page epochs over
runtime resizing, in-process grab for the app window, daemon forwarding to
a present window, workers as the no-window fallback — and how it builds on
ADR-0046/0047's close-before-grab while retiring their multi-window
stance. (2) `CHANGELOG.md` under `## [Unreleased]`: user-visible changes
(single window, paste, stay-in-editor outcomes, external-capture routing).
(3) `docs/ARCHITECTURE.md`: window/process model section refresh.
(4) `AGENTS.md` "Where things live": add `capture_flow.rs` and the router
row; keep it to the table. (5) Spec frontmatter check: `status: active`
while building (flipped by `/build`), never left stale after merge.

**Acceptance criteria:**
- [ ] ADR follows the numbered format and cites ADR-0046/0047 (and
      supersedes the app-window spec's multi-window stance explicitly)
- [ ] CHANGELOG entry covers all ten spec acceptance criteria's user
      stories in plain language
- [ ] No doc contradicts another (window model described once, linked)

**Verification:**
- [ ] `python3 .opencode/harness/scripts/check-refs.py` (if it validates
      docs links) + manual read-through
- [ ] Full gate green (docs-only change; proves nothing broke)

**Dependencies:** Tasks 001–009 (it records what they decided)

**Files likely touched:**
- `docs/adrs/ADR-00NN-single-window-app.md` (new)
- `CHANGELOG.md`, `docs/ARCHITECTURE.md`, `AGENTS.md`

**Docs:** This task is the docs.

**Estimated scope:** Small: 4 files, no code
