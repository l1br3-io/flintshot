# Implementation Plan: Phase 1 — Capture + Editor Window

> Rolling plan for the current phase. Overwritten at the start of each
> phase; never appended across phases. For product context see
> [`VISION.md`](VISION.md); for sequence see [`ROADMAP.md`](ROADMAP.md);
> for per-task detail see [`TODO.md`](TODO.md).

## Overview

Phase 1 makes `flintshot region` and `flintshot fullscreen` do real work:
capture the screen, open an egui editor window with the screenshot as
background, let the user annotate with a focused toolset (Select, Rect,
Arrow, Text, Blur), then save the result as PNG or copy it to the
clipboard. The architecture-abstraction (`Canvas` / `EditorShell` traits,
no GUI types in `flintshot-core`) established in Phase 0 stays intact
and is exercised by real code on every task.

Phase 1 **exit criteria** (from `ROADMAP.md`):

> A user can run `flintshot region`, drag-select a region, draw a red
> rectangle with an arrow pointing at it, add a text label, save the
> result as a PNG, and have it land in their clipboard too. The
> abstraction holds: no egui types leak into `flintshot-core`.

## Scope (in / out)

**In:**
- Single-monitor capture via `xcap`
- Editor window via `eframe` (implements `EditorShell`)
- Tools: Select (region crop), Rect, Arrow, Text, Blur
- Color & stroke-width pickers, persisted to TOML config
- Outputs: Save (`rfd`), Copy to clipboard (`arboard`)
- Undo/redo via snapshot history
- Keyboard shortcuts: `Esc`, `Ctrl+S`, `Ctrl+C`, `Ctrl+Z`, `Ctrl+Y`
- `cosmic-text` for text shaping
- HiDPI-aware coordinates (physical px in core, logical px at the shell)

**Out (deferred):**
- Multi-monitor capture (Phase 2)
- Daemon, tray, hotkeys, IPC (Phase 2)
- Ellipse / Line / Marker / Pencil / Number tools (Phase 2)
- Magnifier (Phase 2)
- Pin-to-desktop (Phase 3)
- Upload (Phase 4)
- Live overlay (Phase 5)

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
  sub-image — decided in Task 13). Output is toolkit-independent.
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

```
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

## Task list (summary)

Per-task acceptance criteria, verification, and dependencies live in
[`TODO.md`](TODO.md). Checkpoints gate phases — do not start the next
phase until the previous checkpoint is signed off.

### Phase 1.A — Capture path (CLI writes a real PNG)
- [x] Task 1 — Config module (`flintshot-core::config`)
- [x] Task 2 — Capture module (`flintshot-core::capture`)
- [x] Task 3 — CLI wiring: `region` / `fullscreen` → PNG to `/tmp`

**Checkpoint A** — capture stack proven end-to-end

### Phase 1.B — Editor skeleton (window opens with screenshot)
- [x] Task 4 — `EguiCanvas` background primitives
- [x] Task 5 — `EguiShell::run_editor` skeleton (eframe window)
- [x] Task 6 — CLI routes capture → editor

**Checkpoint B** — editor opens over a real screenshot

### Phase 1.C — First annotation tool (Rect, end-to-end vertical slice)
- [ ] Task 7 — Core editor state machine + `MockCanvas` test infra
- [ ] Task 8 — Rectangle tool end-to-end

**Checkpoint C** — Rectangle draws, persists, renders correctly

### Phase 1.D — Outputs
- [ ] Task 9 — Save (rfd + PNG encode) and Copy (arboard)
- [ ] Task 10 — Keyboard shortcuts

**Checkpoint D** — user can save PNG and copy to clipboard

### Phase 1.E — Remaining annotation tools
- [ ] Task 11 — Arrow tool
- [ ] Task 12 — Text tool (cosmic-text)
- [ ] Task 13 — Blur tool (imageproc, computed in core)

**Checkpoint E** — all annotation tools work

### Phase 1.F — Region select & polish
- [ ] Task 14 — Select / crop region tool
- [ ] Task 15 — Color & stroke-width pickers + config persistence

**Checkpoint F** — Phase 1 exit criteria met; ready for Phase 2

## Risks and mitigations

| Risk                                                                                  | Impact | Mitigation                                                                                                                                  |
| ------------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `xcap` Wayland portal prompt breaks headless CI                                      | Med    | Tests for `capture` are `#[ignore]`'d and listed in `TEST_MATRIX.md`; CI asserts only that the module compiles and the API is callable      |
| eframe + egui 0.35 viewport API drift (multi-window story still young)                | Med    | Phase 1 uses single-window editor only; defer viewports to Phase 3 (pin). Lock egui/eframe versions at workspace root                       |
| `cosmic-text` integration is non-trivial (font loading, shaping pipeline)             | High   | Task 12 is sized M and isolated; if it slips, ship Phase 1 with `ab_glyph` fallback and ADR the swap. Text is in exit criteria, can't drop   |
| HiDPI rounding mismatch between `xcap` physical px and egui logical px                | Med    | Pin a single rule: core works in physical px; shell converts at the event boundary. Test with `Point::lerp`-based assertions in core         |
| EditorState grows unbounded as tools land                                             | Low    | Snapshot undo/redo is correct, just memory-heavy; cap at 100 snapshots with ring buffer in Phase 2 if profiling demands                     |
| Scope creep from Phase 2 tools (Ellipse, Line, Marker, Pencil)                        | Med    | ROADMAP explicitly assigns those to Phase 2; tasks here reject out-of-scope additions                                                       |
| `render()` becomes a giant match as tools land                                        | Low    | Split per-variant render helpers into a `flintshot-core::render::ann` submodule by Task 13 if the match exceeds ~150 lines                   |
| Config schema change after pickers ship                                               | Low    | Schema is `#[non_exhaustive]`; additive only. Missing fields default via `serde`                                                            |

## Open questions (need human input before the relevant task)

1. **Default capture path for Task 3's smoke test.** Should `flintshot
   region` with no editor wired yet (a) save to
   `$XDG_RUNTIME_DIR/flintshot-capture.png`, (b) print base64 to
   stdout, or (c) take an explicit `--out PATH` flag that becomes the
   Phase 2 default? **Recommendation: (c) — `--out PATH`, defaults to
   `/tmp/flintshot-<timestamp>.png`.**
2. **Text tool font source.** Bundle a single font (e.g. DejaVu Sans)
   in the binary, or rely on system fontconfig? **Recommendation:
   fontconfig via `cosmic-text` default; add a bundled-font feature
   flag only if CI/headless tests need it.**
3. **`EguiShell::run_editor` blocking semantics.** Phase 0 signature
   returns `Result<EditorOutcome>` and is documented as "block until
   done". Confirm: eframe's event loop runs on the calling thread,
   which means `flintshot region` cannot service other RPC during
   editing. Phase 2 daemon will spawn editor windows as separate
   processes — agree? **Recommendation: yes; document in ADR.**

## Verification strategy

Per the project's `definition-of-done.md`:

- **Every task** clears Correctness + Quality (tests pass, no
  regressions, runtime-verified, edge cases handled).
- **Each checkpoint** adds Integration (cross-module behavior works)
  and a manual run on at least one platform from `TEST_MATRIX.md`.
- **Phase exit** adds Documentation (`CHANGELOG.md` updated, ADRs
  written for non-obvious choices, `TEST_MATRIX.md` rows filled) and
  Ship-readiness (security review of new file I/O and clipboard paths).

Standing gate (must be green before any task is marked done):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

## Session notes

_None yet. Updated when a task deviates from this plan during
execution; surfaced by the `/status` command._
