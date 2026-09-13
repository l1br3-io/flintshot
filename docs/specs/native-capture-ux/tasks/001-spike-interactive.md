# Task 001: Spike C — live-verify interactive portal

**Description:** Research task resolving the spec's open question
#1 before anything builds on interactive mode. Re-run the spike-B
scratch zbus client (`/tmp/opencode/spike-b-portal/portal-shot`, it
already has an `--interactive` flag) against the live COSMIC portal
and record: (a) does the Response carry `uri` + code 0 after
select-and-Enter, (b) **is the returned image exactly the selection**
(dims vs a known-size selection) or a composite needing a crop,
(c) Escape/dismiss → code 1 with no file, (d) latency
call→handle→Response, (e) any dialog/prompt behavior. Amends the
spec's Open Questions with the answer and feeds Task 2's
implementation choice (passthrough vs crop reuse).

**Acceptance criteria:**
- [ ] Interactive grab measured on the live session: Enter path and
      Escape path both exercised, findings recorded under
      `## Findings` here
- [ ] Return-shape question answered explicitly: selection-only vs
      composite (with dims evidence)
- [ ] Latency numbers recorded (call→handle, handle→Response)
- [ ] `spec.md` Open Question #1 amended with the verdict

**Verification:**
- [ ] Findings contain concrete measurements (no repo code changes;
      scratch stays under `/tmp/opencode/`)

**Dependencies:** None (the merged portal backend is the substrate)

**Files likely touched:**
- `docs/specs/native-capture-ux/tasks/001-spike-interactive.md` (findings)
- `docs/specs/native-capture-ux/spec.md` (open-question verdict)

**Docs:**
- Findings feed Task 2 and Task 10's ADR.

**Estimated scope:** Small (2 files; scratch probe outside the tree)
