---
description: Break work into small verifiable tasks with acceptance criteria and dependency ordering
---

# /planning

Invoke the planning-and-task-breakdown skill.

Read the existing spec (SPEC.md or equivalent) and the relevant codebase sections. Then:

1. Enter plan mode — read only, no code changes
2. Identify the dependency graph between components
3. Slice work vertically (one complete path per task, not horizontal layers)
4. Write tasks with acceptance criteria and verification steps
5. Add checkpoints between phases
6. Present the plan for human review

Save the plan to docs/PLAN.md and task list to docs/TODO.md. Both are
rolling — overwritten at the start of each phase, never appended across
phases. Include a `## Session notes` section if any task deviates from
the plan during execution (the `/status` command surfaces it).