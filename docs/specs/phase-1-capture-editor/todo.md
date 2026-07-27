# TODO — Phase 1: Capture + Editor Window

> Flat status index — the single status source. One line per task +
> checkpoint; mirrors [`plan.md`](plan.md). Per-task acceptance criteria,
> verification, dependencies, and files live in
> [`tasks/00N-*.md`](tasks/). Check off by completing the acceptance
> criteria in the task file, not by writing code.

Status reflects the maintainer's "code-complete through 1.D" call (see
commit history); granular unchecked items — particularly the interactive
verification debt on Tasks 9 + 10 and Checkpoint D — are preserved in
the task files.

## Phase 1.A — Capture path

- [x] 001 — Config module (`flintshot-core::config`) → [`tasks/001-config-module.md`](tasks/001-config-module.md)
- [x] 002 — Capture module (`flintshot-core::capture`) → [`tasks/002-capture-module.md`](tasks/002-capture-module.md)
- [x] 003 — CLI wiring: `region` / `fullscreen` → PNG → [`tasks/003-cli-wiring-png.md`](tasks/003-cli-wiring-png.md)
- [x] **Checkpoint A** — capture stack proven end-to-end

## Phase 1.B — Editor skeleton

- [x] 004 — `EguiCanvas` background primitives + `render()` body → [`tasks/004-eguicanvas-background.md`](tasks/004-eguicanvas-background.md)
- [x] 005 — `EguiShell::run_editor` skeleton (eframe window) → [`tasks/005-eguishell-skeleton.md`](tasks/005-eguishell-skeleton.md)
- [x] 006 — CLI routes `region` / `fullscreen` through `EguiShell` → [`tasks/006-cli-routes-editor.md`](tasks/006-cli-routes-editor.md)
- [x] **Checkpoint B** — editor opens over a real screenshot

## Phase 1.C — First annotation tool (vertical slice)

- [x] 007 — Core editor state machine + `MockCanvas` test infra → [`tasks/007-editor-state-machine.md`](tasks/007-editor-state-machine.md)
- [x] 008 — Rectangle tool end-to-end → [`tasks/008-rectangle-tool.md`](tasks/008-rectangle-tool.md)
- [x] **Checkpoint C** — Rectangle draws, persists, renders correctly

## Phase 1.D — Outputs

- [x] 009 — Save (`rfd` + PNG encode) and Copy (`arboard`) → [`tasks/009-save-copy-outputs.md`](tasks/009-save-copy-outputs.md)
- [x] 010 — Keyboard shortcuts → [`tasks/010-keyboard-shortcuts.md`](tasks/010-keyboard-shortcuts.md)
- [ ] **Checkpoint D** — user can save PNG and copy to clipboard _(interactive close-out owed)_

## Phase 1.E — Remaining annotation tools

- [x] 011 — Arrow tool end-to-end → [`tasks/011-arrow-tool.md`](tasks/011-arrow-tool.md)
- [x] 012 — Text tool (`cosmic-text`) → [`tasks/012-text-tool.md`](tasks/012-text-tool.md)
- [x] 013 — Blur tool (`imageproc`, computed in core) → [`tasks/013-blur-tool.md`](tasks/013-blur-tool.md)
- [ ] **Checkpoint E** — all annotation tools work

## Phase 1.F — Region select & polish

- [x] 014 — Select / crop region tool → [`tasks/014-select-crop-tool.md`](tasks/014-select-crop-tool.md)
- [x] 015 — Color & stroke-width pickers + config persistence → [`tasks/015-color-stroke-pickers.md`](tasks/015-color-stroke-pickers.md)
- [ ] **Checkpoint F** — Phase 1 exit criteria met; ready for Phase 2
