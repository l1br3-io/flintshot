---
description: Snapshot of project state — phase, task progress, working tree, with the next task front and center. Add "verify" to also run cargo fmt + clippy + test + build.
model: opencode/big-pickle
---

# /status

Produce a read-only snapshot of where the Flintshot project is right now. The
**next task** is the hero of the output — everything else is context for it.

## Modes

- `/status` — parse docs + git only. Fast (~1s). Default.
- `/status verify` — additionally run the four Rust quality commands:
  `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace --all-targets`, `cargo build`. Slower, but exposes stale
  "done" checkboxes. Treat `verify` (canonical) as the verify mode; anything
  else (or empty) is the default.

## What to read, in order

1. `docs/ROADMAP.md` → identify the **current phase** (the one whose
   deliverable boxes are partially `[x]`, or the first fully-`[ ]` phase)
   and its theme, for the headline. A phase is "in flight" if any of its
   boxes are checked but not all. If the prior phase is fully checked and
   the next phase has zero checks, the project is between phases — say so
   and point at the next phase instead of printing a next task.
2. `docs/TODO.md` → the live checklist:
   - The first `[ ]` task in the **current batch** is **the next task** —
     promote it to the top.
   - Count `[x]` vs total within the current batch for progress.
   - Note checkpoint state (✅/⬜) — checkpoints are batch boundaries.
   - If the current batch is fully `[x]`, say so and point at the next
     batch. If all batches are done, point at `docs/ROADMAP.md` for the
     next phase.
3. `docs/PLAN.md` → look up the next task's full entry (description,
   dependencies, files likely touched) for the detail line. The plan is
   **rolling** — rewritten per phase. If it contains a
   `## Session notes` section, summarize recorded deviations and mention
   the count.
4. `git status --porcelain` → working-tree cleanliness.
5. `git log --oneline -1` → the most recent commit only (hash + subject).

## Output template

```markdown
## Flintshot status — <Phase N: title> (<theme>)

### ▶ Next task
**<slice.task#> — <title>**
<one-line description from docs/PLAN.md>

**To proceed:** `/build`        **Re-plan:** `/planning`

---

### Progress
- Batch <N>: <done>/<total> tasks done
- Checkpoint <last>: <✅/⬜>

### Session notes (from docs/PLAN.md)
- <as-of line, if a Session notes section exists>
- <N> deviations on record — read before continuing.
*(omit this block entirely if docs/PLAN.md has no Session notes section)*

### Working tree
- <clean | N uncommitted files>
- last: <hash> <subject>
```

When `/status verify` is invoked, append a `### Verification` block after the
template with a one-line pass/fail for each command:

```markdown
### Verification
- `cargo fmt --all --check`: ✅
- `cargo clippy --workspace --all-targets -- -D warnings`: ✅
- `cargo test --workspace --all-targets`: ✅ (N tests)
- `cargo build`: ✅
```

## Rules

1. **Read-only.** Never edit files, never commit, never push. `/status`
   observes and reports.
2. **The next task leads.** If the output had to be one line, it would be
   the next task's title and the `/build` suggestion. Everything else is
   supporting context.
3. **Graceful degradation.** If `docs/PLAN.md` has no `## Session notes`
   section, omit that block. If `docs/TODO.md` has no pending task in the
   current batch, replace the Next-task block with a batch-complete notice
   pointing at the next batch. If all batches are complete, point at
   `docs/ROADMAP.md` for the next phase. Never error on a missing file —
   note it and continue.
4. **Verify never blocks.** If a verify-mode command fails, report the
   failure inline; do not abort the snapshot. Failures are signal, not
   exceptions.
5. **No speculation.** If a source is ambiguous (e.g. two phases look
   in-flight), say so and ask the user rather than guessing the next task.
6. **Suggested next action is always the footer of the Next-task block:**
   `/build` to implement, or `/planning` to re-plan. Nothing else.
7. **Milestones are phases.** Flintshot uses "Phase N" terminology, not
   "Milestone N". The roadmap defines six phases; the current phase is the
   one with `Phase N — <title>` whose deliverables are partially checked.
