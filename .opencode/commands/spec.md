---
description: Start spec-driven development — write a structured specification before writing code
---

Load the `spec-driven-development` skill with the `skill` tool.

Begin by understanding what the user wants to build. Ask clarifying questions about:
1. The objective and target users
2. Core features and acceptance criteria
3. Tech stack preferences and constraints
4. Known boundaries (what to always do, ask first about, and never do)

Then generate a structured spec covering all six core areas: objective, commands, project structure, code style, testing strategy, and boundaries.

If the request bundles several independently testable capabilities, first propose a capability map (module ids, dependency direction, build order) per the skill's Phase 0 and get it approved, then spec each module in dependency order.

No spec files are written. Present the spec (and the capability map, if any) in the session for approval; it is recorded when the work lands — the effort's completion commit carries the Context / Decisions / Deviations / Outcome block defined in AGENTS.md "Git as docs", and the trail is looked up with `git log --grep='<effort>'`. Confirm with the user before proceeding to `/plan`.
