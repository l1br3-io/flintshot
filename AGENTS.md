# FlintShot — Agent Skills

This project uses the [addyosmani/agent-skills](https://github.com/addyosmani/agent-skills) pack, installed under `.agents/` and `.opencode/`. Reusable workflows (skills), shared checklists (references), lifecycle slash commands, and specialist personas are all wired up and auto-discovered.

## Locations

| Asset | Path |
|-------|------|
| Skills | `.agents/skills/<name>/SKILL.md` |
| Shared checklists | `.agents/references/*.md` |
| Slash commands | `.opencode/commands/*.md` |
| Personas (subagents) | `.opencode/agents/*.md` |

## Core rules

- If a task matches a skill, load it with the `skill` tool before acting.
- Follow the skill workflow strictly — do not partially apply it.
- Never skip required steps (spec, plan, test) a skill demands.
- Every change clears the project-wide Definition of Done in `.agents/references/definition-of-done.md`.
- Commands are silent on success and announce on failure: justfile recipes echo nothing on success (see `set quiet` in the justfile) and fail loudly; slash commands give a concise result on success and surface failures prominently instead of burying them.

## Intent → skill mapping

- New feature / significant change → `spec-driven-development`, then `incremental-implementation` + `test-driven-development`
- Planning / breakdown → `planning-and-task-breakdown`
- Bug / failure / unexpected behavior → `debugging-and-error-recovery`
- Code review → `code-review-and-quality`
- Refactoring / simplification → `code-simplification`
- API or module boundary design → `api-and-interface-design`
- Rust code / review → `rust-best-practices`
- Security-sensitive input, auth, secrets, dependencies → `security-and-hardening`
- Performance requirements or suspected regressions → `performance-optimization`
- Making a change (always) → `git-workflow-and-versioning`

## Slash commands

`/spec`, `/plan`, `/build [auto]`, `/test`, `/review`, `/constraints`, `/code-simplify`, `/ship`, `/webperf`. Run `/ship` before merge for a parallel fan-out review.

## Personas

`code-reviewer`, `security-auditor`, `test-engineer`, `web-performance-auditor` — single-role subagents invoked by `/ship` / `/webperf` (or directly). **Personas do not invoke other personas**; orchestration belongs to slash commands and the main session (see `.agents/references/orchestration-patterns.md`).

## Project notes

- Run `cargo clippy` and `cargo fmt --check` before a change counts as done; `cargo test` is the verification gate.
- Screen capture and editor behavior are only partially testable headless; unit tests cover what can be tested and the rest follows the manual checklist in the README.

## Git as docs (the spec trail)

There are no spec, plan, or task files — git history is the documentation.
Every effort's context, decisions, and outcome live in enriched commit
messages; every commit carries a scope-prefixed subject
(`feat(<scope>)`, `fix(<scope>)`), so any effort's full trail is:

    git log --grep='<effort>'        # e.g. git log --grep=fix-tray-sni

The completion commit of each effort carries a context block in this
shape (this is THE place context/decisions/deviations get recorded):

    ---
    Context: why this work existed (1-3 lines)
    Decisions:
    - key choices and their reasons
    Deviations:
    - where implementation diverged from plan (omit if none)
    Outcome: what shipped and how it was verified

Rules:

- Plan in the session (`/spec`, `/plan`); do not write planning files.
  PRs carry the effort's success criteria in the PR body.
- One commit per task; interim commits get normal scope-prefixed
  subjects, the completion commit gets the context block above.
- Debugging root causes go in the Deviations lines of the fixing
  effort's block — they are the project's war stories.
- `.gitignore` still blocks `SPEC*.md` and `/tasks` defensively; never
  commit planning artifacts.
