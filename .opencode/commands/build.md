---
description: Implement tasks incrementally — build, test, verify, commit. "auto" runs the whole approved task list in one unattended run.
---

Load the `incremental-implementation` skill alongside `test-driven-development` with the `skill` tool, then discover the task's tech stack and load every additional skill the `Intent → skill mapping` in AGENTS.md calls for (see below).

## Artifact model (see AGENTS.md "Git as docs")

There are no chunk files. The approved spec and task list come from the
session (or the PR body they were recorded in). The durable record is git
itself: one commit per task with a scope-prefixed subject, and the effort's
completion commit carries the Context / Decisions / Deviations / Outcome
block defined in AGENTS.md. Look up any effort's trail with
`git log --grep='<effort>'`.

## Step 0 — select the mode from the invocation

Arguments received: `$ARGUMENTS`

| Invocation | Mode |
|---|---|
| *(empty)* | next task of the approved task list |
| `auto` | autonomous run of every remaining task of the approved list |

The task list is the approved plan from `/plan` (in-session or PR body). If
no approved task list exists, stop and point at `/spec` / `/plan` — never
invent tasks from `ROADMAP.md`, the codebase, or anywhere else.

## Tech-stack discovery → skill mapping

Before writing any code, work out what the task actually involves, then let
the `Intent → skill mapping` in AGENTS.md decide which skills to load on top
of `incremental-implementation` + `test-driven-development`.

1. **Detect the stack.** Read `Cargo.toml` and the files the task touches to pin
   down what it involves — plain Rust logic, libcosmic / COSMIC APIs, Wayland
   layer-shell, config parsing, or the PTY/terminal engine.
2. **Map the intent.** Consult the `Intent → skill mapping` in AGENTS.md and load
   every skill the task's intent matches. For this repo that means:
   - Rust code / review → `rust-best-practices` (load for every Rust task)
   - New module boundary or public interface → `api-and-interface-design`
   - Untrusted input, auth, secrets, config → `security-and-hardening`
   - Performance-sensitive path or suspected regression → `performance-optimization`
3. **Verify unfamiliar APIs.** When the task calls a libcosmic / COSMIC API you'd
   otherwise write from memory, ground it in official docs
   (`source-driven-development`) rather than trusting training data.

The mapping — not this file — is the authority on which skill governs an
intent. If the task matches an entry listed there, load it.

## Single task (default, `/build`)

1. Pick the next pending task from the approved task list.
2. Load its context: the spec from the session/PR, the files it touches, existing patterns.
3. Discover the task's tech stack and consult the `Intent → skill mapping` in AGENTS.md, loading every skill the task's intent matches (see "Tech-stack discovery → skill mapping").
4. Write a failing test for the expected behavior (RED).
5. Implement the minimum code to pass the test (GREEN).
6. Run the full test suite to check for regressions.
7. Run the build to verify compilation.
8. Commit with a descriptive, scope-prefixed subject; when this task
   completes the effort, append the Context / Decisions / Deviations /
   Outcome block (AGENTS.md "Git as docs") to the commit message.
9. Stop and report.

## Autonomous: the whole list (`/build auto`)

Use this once the task list is approved, to collapse plan + build into one
run. Every task still earns a passing test and its own commit.

1. **Require an approved task list** (in-session or PR body). Otherwise stop
   and point at `/spec` / `/plan` — do not invent requirements.
2. **Establish a clean baseline.** Run `git status --porcelain`. If there are
   uncommitted changes, stop and ask the user to commit, stash, or confirm.
   Autonomous per-task commits must not absorb unrelated local work.
3. **Announce the run, then start immediately.** State the tasks once so the
   run is visible in the transcript, then begin the task loop without
   waiting for approval. The stop-conditions in step 5 are the only thing
   that may interrupt the run.
4. **Task loop — repeat until the list is exhausted.** Pick the next pending
   task and run the full single-task procedure on it (discover stack → map
   skills → RED → GREEN → regression suite → build → commit). After each
   commit, move to the next task and **immediately continue** — do not stop
   between tasks, do not ask for approval, do not summarize early. Stage
   only the files that task touched — never `git add -A` blindly — one
   commit per task so any point is a clean rollback. The last task's commit
   carries the effort's context block.
5. **Stop and ask the user** (do not push through) when:
   - a test can't be made to pass or the build breaks without an obvious fix → follow `debugging-and-error-recovery`
   - the spec is ambiguous, or a task needs a decision the spec doesn't cover
   - a task is high-risk or irreversible — anything touching secrets, destructive operations, or that you can't undo with `git revert` → follow `doubt-driven-development` and get explicit sign-off

   After the user resolves a blocker, they re-invoke `/build auto` — it
   resumes from the next pending task.
6. **Summarize at the end:** tasks completed, tests added, commits made, anything skipped, flagged, or left for the user.

If any step fails, follow the `debugging-and-error-recovery` skill.
