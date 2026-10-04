---
description: Break work into small verifiable tasks with acceptance criteria and dependency ordering
---

Load the `planning-and-task-breakdown` skill with the `skill` tool.

Read the approved spec from the session (or the PR body it was recorded in) and the relevant codebase sections. Then:

1. Enter plan mode — read only, no code changes
2. Identify the dependency graph between components
3. Slice work vertically (one complete path per task, not horizontal layers)
4. Write tasks with acceptance criteria and verification steps
5. Add checkpoints between phases
6. Present the plan for human review

No plan files are written. The plan and its task list are presented in the session and recorded as the work lands: one commit per task with a scope-prefixed subject, and the effort's completion commit carries the Context / Decisions / Deviations / Outcome block defined in AGENTS.md "Git as docs". Look up any effort's trail with `git log --grep='<effort>'`.
