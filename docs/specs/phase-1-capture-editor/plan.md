# Plan — Phase 1: Capture + Editor Window

> Thin index for the current effort. Requirements live in
> [`spec.md`](spec.md); flat status in [`todo.md`](todo.md); per-task
> acceptance / files / verification / dependencies in
> [`tasks/00N-*.md`](tasks/). For product context see
> [`../../VISION.md`](../../VISION.md); for sequence see
> [`../../ROADMAP.md`](../../ROADMAP.md).

## Overview

Phase 1 makes `flintshot region` and `flintshot fullscreen` do real work:
capture the screen, open an egui editor window with the screenshot as
background, let the user annotate with a focused toolset (Select, Rect,
Arrow, Text, Blur), then save the result as PNG or copy it to the
clipboard. The architecture-abstraction (`Canvas` / `EditorShell` traits,
no GUI types in `flintshot-core`) established in Phase 0 stays intact and
is exercised by real code on every task.

Phase 1 exit criteria (from [`spec.md`](spec.md)):

> A user can run `flintshot region`, drag-select a region, draw a red
> rectangle with an arrow pointing at it, add a text label, save the
> result as a PNG, and have it land in their clipboard too. The
> abstraction holds: no egui types leak into `flintshot-core`.

## Architecture decisions for Phase 1

- **All annotation drawing routes through `Canvas`.** No direct
  `egui::Painter` calls in the editor for annotations. The shared
  `flintshot-core::render::render()` function walks `EditorState` and
  emits `Canvas` calls — egui just implements the primitives. (Already
  locked in by ADR-0012; reiterated here because Phase 1 is the first
  real test.)
- **Blur/pixelate computed in core.** `imageproc` lives in
  `flintshot-core`; the resulting `RgbaImage` is emitted to `Canvas` via
  a new `draw_image_region` (or reused `draw_image` on a pre-cropped
  sub-image — decided in Task 013). Output is toolkit-independent.
- **In-flight annotation lives in `EditorState`.** A new field
  `pending: Option<Annotation>` holds the annotation being drawn so the
  render path stays read-only over `state`. The shell feeds pointer
  events in; core decides when to commit.
- **Snapshot undo/redo.** `EditorState::commit` pushes the pre-commit
  `anns` onto `undo` and clears `redo`. Simple, correct, sufficient for
  Phase 1's toolset; revisit if annotation counts get large.
- **Config is read at editor open, written on change.** A
  `Config::load()` at app start, a `Config::save()` after the user
  changes a default. No live reload yet (Phase 2 daemon adds that).
- **CLI one-shots capture → editor → save.** No daemon in Phase 1; the
  `flintshot region` process captures, opens the editor, exits when the
  editor closes.

## Dependency graph

```text
flintshot-core::config ─┐
                        ├─► flintshot-app::cli (region/fullscreen)
flintshot-core::capture ┘         │
                                  ▼
                          flintshot-editor-egui (EguiShell + EguiCanvas)
                                  │
                                  ▼
                          flintshot-core::render::render()  (per-frame)
                                  │
                                  ▼
                          flintshot-core::editor  (state machine,
                                                   undo/redo,
                                                   hit-testing)
                                  │
                                  ▼
                          flintshot-core::image_ops  (blur/pixelate,
                                                      crop)
```

Build bottom-up: config + capture first, then shell skeleton, then one
annotation tool end-to-end (which forces the state machine + render
dispatch + MockCanvas tests to be designed correctly), then outputs,
then the remaining tools, then region-crop + pickers.

## Task index

Checkpoints gate phases — do not start the next sub-phase until the
previous checkpoint is signed off. Per-task acceptance criteria,
verification, dependencies, and files live in `tasks/00N-*.md`.

### Phase 1.A — Capture path (CLI writes a real PNG)
- [`tasks/001-config-module.md`](tasks/001-config-module.md) — Config module
- [`tasks/002-capture-module.md`](tasks/002-capture-module.md) — Capture module
- [`tasks/003-cli-wiring-png.md`](tasks/003-cli-wiring-png.md) — CLI wiring

**Checkpoint A** — capture stack proven end-to-end. ✅

### Phase 1.B — Editor skeleton (window opens with screenshot)
- [`tasks/004-eguicanvas-background.md`](tasks/004-eguicanvas-background.md) — EguiCanvas background primitives
- [`tasks/005-eguishell-skeleton.md`](tasks/005-eguishell-skeleton.md) — EguiShell skeleton
- [`tasks/006-cli-routes-editor.md`](tasks/006-cli-routes-editor.md) — CLI routes capture → editor

**Checkpoint B** — editor opens over a real screenshot. ✅

### Phase 1.C — First annotation tool (Rect, end-to-end vertical slice)
- [`tasks/007-editor-state-machine.md`](tasks/007-editor-state-machine.md) — Core editor state machine + MockCanvas
- [`tasks/008-rectangle-tool.md`](tasks/008-rectangle-tool.md) — Rectangle tool end-to-end

**Checkpoint C** — Rectangle draws, persists, renders correctly. ✅

### Phase 1.D — Outputs
- [`tasks/009-save-copy-outputs.md`](tasks/009-save-copy-outputs.md) — Save (rfd + PNG) and Copy (arboard)
- [`tasks/010-keyboard-shortcuts.md`](tasks/010-keyboard-shortcuts.md) — Keyboard shortcuts

**Checkpoint D** — user can save PNG and copy to clipboard. ✅ _(closed
via interactive testing; see commit `46bb05f` "Phase 1 hardening: editor
bug fixes from interactive testing".)_

### Phase 1.E — Remaining annotation tools
- [`tasks/011-arrow-tool.md`](tasks/011-arrow-tool.md) — Arrow tool
- [`tasks/012-text-tool.md`](tasks/012-text-tool.md) — Text tool (cosmic-text)
- [`tasks/013-blur-tool.md`](tasks/013-blur-tool.md) — Blur tool (imageproc, computed in core)

**Checkpoint E** — all annotation tools work. ✅

### Phase 1.F — Region select & polish
- [`tasks/014-select-crop-tool.md`](tasks/014-select-crop-tool.md) — Select / crop region tool
- [`tasks/015-color-stroke-pickers.md`](tasks/015-color-stroke-pickers.md) — Color & stroke-width pickers + config

**Checkpoint F** — Phase 1 exit criteria met; ready for Phase 2. ✅

## Risks and mitigations

| Risk                                                                                  | Impact | Mitigation                                                                                                                                  |
| ------------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `xcap` Wayland portal prompt breaks headless CI                                      | Med    | Tests for `capture` are `#[ignore]`'d and listed in `TEST_MATRIX.md`; CI asserts only that the module compiles and the API is callable      |
| eframe + egui 0.35 viewport API drift (multi-window story still young)                | Med    | Phase 1 uses single-window editor only; defer viewports to Phase 3 (pin). Lock egui/eframe versions at workspace root                       |
| `cosmic-text` integration is non-trivial (font loading, shaping pipeline)             | High   | Task 012 is sized M and isolated; if it slips, ship Phase 1 with `ab_glyph` fallback and ADR the swap. Text is in exit criteria, can't drop |
| HiDPI rounding mismatch between `xcap` physical px and egui logical px                | Med    | Pin a single rule: core works in physical px; shell converts at the event boundary. Test with `Point::lerp`-based assertions in core         |
| EditorState grows unbounded as tools land                                             | Low    | Snapshot undo/redo is correct, just memory-heavy; cap at 100 snapshots with ring buffer in Phase 2 if profiling demands                     |
| Scope creep from Phase 2 tools (Ellipse, Line, Marker, Pencil)                        | Med    | ROADMAP explicitly assigns those to Phase 2; tasks here reject out-of-scope additions                                                       |
| `render()` becomes a giant match as tools land                                        | Low    | Split per-variant render helpers into a `flintshot-core::render::ann` submodule by Task 013 if the match exceeds ~150 lines                 |
| Config schema change after pickers ship                                               | Low    | Schema is `#[non_exhaustive]`; additive only. Missing fields default via `serde`                                                            |

## Open questions (need human input before the relevant task)

1. **Default capture path for Task 003's smoke test.** Should `flintshot
   region` with no editor wired yet (a) save to
   `$XDG_RUNTIME_DIR/flintshot-capture.png`, (b) print base64 to stdout,
   or (c) take an explicit `--out PATH` flag that becomes the Phase 2
   default? **Recommendation: (c) — `--out PATH`, defaults to
   `/tmp/flintshot-<timestamp>.png`.** _(Resolved — shipped as (c).)_
2. **Text tool font source.** Bundle a single font (e.g. DejaVu Sans) in
   the binary, or rely on system fontconfig? **Recommendation: fontconfig
   via `cosmic-text` default; add a bundled-font feature flag only if
   CI/headless tests need it.** _(Open — decides in Task 012.)_
3. **`EguiShell::run_editor` blocking semantics.** Phase 0 signature
   returns `Result<EditorOutcome>` and is documented as "block until
   done". Confirm: eframe's event loop runs on the calling thread, which
   means `flintshot region` cannot service other RPC during editing.
   Phase 2 daemon will spawn editor windows as separate processes —
   agree? **Recommendation: yes; document in ADR.** _(Open — Phase 2
   concern.)_

## Verification strategy

Every task clears the standing gate in [`AGENTS.md`](../../../AGENTS.md)
(`fmt --check`, `clippy --workspace --all-targets -- -D warnings`,
`test --workspace --all-targets`, doc tests for the two lib crates). Each
checkpoint adds a manual run on at least one platform from
[`docs/TEST_MATRIX.md`](../../TEST_MATRIX.md). Per-task verification
steps live in `tasks/00N-*.md`.

## Decisions landed this effort

Lasting technical decisions (full text in [`docs/adrs/`](../../adrs/)):

- [ADR-0013](../../adrs/ADR-0013-error-field-detail-not-source.md) — `FlintshotError` variants use a `detail` field, never `source` (thiserror rule).
- [ADR-0014](../../adrs/ADR-0014-synchronous-rfd-filedialog.md) — Synchronous `rfd::FileDialog` over the async API in the blocking editor.
- [ADR-0015](../../adrs/ADR-0015-compose-final-separate-render-path.md) — `compose_final` renders annotations into the image, separate from live `render()`.
- [ADR-0016](../../adrs/ADR-0016-workspace-scoped-standing-gate.md) — Standing gate uses `--workspace` + scoped `--doc` selectors.
