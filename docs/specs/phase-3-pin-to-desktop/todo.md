# Todo — Phase 3: Pin-to-desktop

Flat status checklist. One line per task, mirroring
[`plan.md`](plan.md)'s task index. Per-task detail in
[`tasks/00N-*.md`](tasks/). This is the single status source the
project resumes from.

## Phase 3.A — Foundation: the pin primitive
- [x] Task 001 — Pin primitive (`EguiShell::run_pin` real impl + `flintshot pin <path>` real CLI body) → [`tasks/001-pin-primitive.md`](tasks/001-pin-primitive.md)

**Checkpoint A** — `flintshot pin <path>` opens a borderless always-on-top window; Esc/close dismisses it.

## Phase 3.B — Worker→daemon outcome reporting
- [x] Task 002 — `Request::ReportSaved { path }` (daemon stores `last_saved` + broadcasts `Event::Saved`; worker sends fire-and-forget on `Saved`) → [`tasks/002-report-saved.md`](tasks/002-report-saved.md)

**Checkpoint B** — daemon-driven capture whose editor saves emits `Event::Saved { path }` to IPC subscribers.

## Phase 3.C — Editor Pin button
- [x] Task 003 — `EditorOutcome::Pinned(PathBuf)` + pin-cache dir helper + editor *Pin* toolbar button + spawn handling → [`tasks/003-editor-pin-button.md`](tasks/003-editor-pin-button.md)

**Checkpoint C** — editor *Pin* button works end-to-end (pin appears, editor closes, pin persists).

## Phase 3.D — PinLast over IPC
- [ ] Task 004 — Daemon handles `Request::PinLast` (reads `last_saved`, spawns pin worker, emits `Event::Pinned { id }`; empty → `Error { code: "no-last-capture" }`) → [`tasks/004-pin-last-ipc.md`](tasks/004-pin-last-ipc.md)

**Checkpoint D** — full Phase 3 exit criteria met (all three pin entry points work).

## Phase 3.E — Hardening
- [ ] Task 005 — Boundary test, CHANGELOG, ADR-0023 + ADR-0024, TEST_MATRIX rows, Wayland caveat → [`tasks/005-hardening.md`](tasks/005-hardening.md)

**Checkpoint E** — Phase 3 ready for review.
