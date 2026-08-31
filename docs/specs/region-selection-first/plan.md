# Plan: Region selection-first editor

> Thin index. Requirements live in [`spec.md`](spec.md). Task files
> under `tasks/` are authored when each task starts (per-task:
> description, acceptance criteria, verification commands).

## Overview

Port the spotlight into the editor window: a selection-first phase on
`EditorApp` (Select pre-armed, dim mask outside the selection, hint
line, Esc-cancel contract, phase ends on first crop), requested by the
app crate only on the `region` → editor-window path (default editor
mode everywhere + the Wayland `overlay` demotion). Zero core changes,
zero new dependencies, no IPC/daemon surface changes.

## Architecture decisions

1. **Phase state is shell-owned on `EditorApp`** (`selection_first:
   bool` + a `phase_ended` latch), exactly like the overlay's chrome
   is shell-owned. `EditorState` is only *seeded* (`tool =
   Tool::Select`) — no core API grows a concept of the phase. → Task
   001.
2. **`EguiShell::with_selection_first()` builder method** carries the
   request from the app crate (mirrors `with_pin_config`; no
   `EditorShell` trait change — the trait's `run_editor(image)`
   signature already passes everything the state needs via the
   builder-held flag). → Task 001.
3. **Mask math is extracted beside `overlay.rs`** (module-private
   helper or a small shared fn in `flintshot-editor-egui`) so the
   editor and overlay render the same four-rect mask from the same
   `MASK_ALPHA`; no second constant to drift. → Task 001.
4. **App-crate wiring is a single decision point**: `run_region`'s
   editor path and the Wayland fallback in `resolve_interaction_mode`
   set selection-first; `run_fullscreen` / `run_open` do not, pinned
   by tests. Daemon-spawned workers need nothing — they already run
   bare `region` argv. → Task 002.
5. **Cancel contract mirrors the overlay**: Esc with no selection
   during the phase exits the process (exit 0, no side effects);
   anything later keeps the editor's existing Esc behaviour. → Task
   003.
6. **Docs last, in one task**: ADR-0035 (revising ADR-0028's Wayland
   consequence), CHANGELOG `[Unreleased]`, TEST_MATRIX rows, spec
   status flip. → Task 004.

## Task index

### Stage 1 — Editor shell phase
- [ ] Task 001: `EditorApp` selection-first phase — Select pre-arm, dim mask (shared mask helper), hint line, phase-end-on-crop latch
  - *Checkpoint: `EguiShell::with_selection_first().run_editor(state)` opens masked + Select-armed on a test image; unit tests cover phase transitions + mask math reuse; gates green.*

### Stage 2 — App wiring
- [ ] Task 002: region-only wiring in `flintshot-app` — editor path + Wayland demotion set the flag; fullscreen/open pinned unregressed
  - *Checkpoint: `cargo run -- region` lands selection-first; `cargo run -- fullscreen` / `open` byte-identical; CLI dispatch unit tests pin the asymmetry.*

### Stage 3 — Contract hardening
- [ ] Task 003: Esc-cancel (no selection → exit 0, no side effects) + outputs-with-live-selection verification (Copy/Save honour the selection pre-crop)
  - *Checkpoint: full manual flow on Wayland + X11 editor-mode; exit-code and no-side-effect checks.*

### Stage 4 — Docs
- [ ] Task 004: ADR-0035, CHANGELOG, TEST_MATRIX rows, spec → shipped
  - *Checkpoint: standing gates + docs review.*
