# Session handoff — Phase 1, Task 10 done; Phase 1.E (Arrow) next

> **Last session:** Tue Jul 21 2026. Task 10 landed (uncommitted in
> the working tree): pure `EditorState::dispatch_input` lives in a
> new `flintshot-core::input` module; `EditorApp::logic` is now a
> thin translator; `Esc` / `Ctrl+Z` / `Ctrl+Y` / `Ctrl+Shift+Z` /
> `Ctrl+S` / `Ctrl+C` all route through the dispatcher. The Task 5 /
> 7 / 8 close-outs' forward-looking extraction is done — input
> dispatch is unit-testable without a window.
> **Next:** Task 11 (Arrow tool end-to-end). Same drag input model
> as Rect (the dispatcher already handles it), so Task 11 is mostly:
> (a) real `EguiCanvas::draw_arrow` impl, (b) Arrow toolbar button,
> (c) `MockCanvas` snapshot test for the Arrow variant.
> Replace this file at the start of each session; the permanent
> record lives in [`TODO.md`](TODO.md) under "Session notes".

## Where we are

**Phase 1 — Capture + editor window.** Phase 1.A through 1.D are
code-complete. Phase 1.D's Task 10 implementation is done in the
working tree (uncommitted); the only remaining Phase 1.D debt is
the interactive verification of Task 9's Save / Copy buttons AND
Task 10's keyboard shortcuts — both clearable in one
`cargo run -- region` pass.

```
Phase 1.A — Capture path             Phase 1.B — Editor skeleton
  Task 1  config module     [x]        Task 4  EguiCanvas bg     [x]
  Task 2  capture module    [x]        Task 5  EguiShell run     [x]
  Task 3  CLI wiring        [x]        Task 6  CLI → editor      [x]
  ▓▓▓ Checkpoint A closed              ▓▓▓ Checkpoint B closed

Phase 1.C — First annotation tool (vertical slice)
  Task 7  state machine + MockCanvas  [x]
  Task 8  Rectangle end-to-end        [x]
  ▓▓▓ Checkpoint C closed

Phase 1.D — Outputs
  Task 9  Save + Copy                 [x]*   (* manual verification still open)
  Task 10 keyboard shortcuts          [x]*   (* manual verification still open)
  ▓▓▓ Checkpoint D (0/1)                     ← one interactive pass closes it

Phase 1.E — Remaining annotation tools
  Task 11 Arrow                       [ ]    ← next
  Task 12 Text (cosmic-text)          [ ]
  Task 13 Blur (imageproc)            [ ]
  ▓▓▓ Checkpoint E (0/1)

Phase 1.F — Region select & polish
  Task 14 Select / crop               [ ]
  Task 15 Color & stroke pickers      [ ]
  ▓▓▓ Checkpoint F (Phase 1 exit)
```

## What landed last session

- **Task 10 — pure dispatcher extraction.** New
  `flintshot-core::input` module with `Modifiers`, `InputEvent`,
  `DispatchOutcome`. `EditorState::dispatch_input` owns every
  keyboard + pointer decision. `EditorApp::logic` is now a thin
  translator: egui modifier state + pressed keys + pointer
  transitions → `InputEvent`, dispatch, `if matches!` on outcome.
  Cross-platform modifier mapping lives at the shell boundary
  (egui's `command` → our `ctrl`).

- **Standing gate green at 74 pass / 1 ignored / 0 fail** (was 57).
  +13 dispatcher tests + 4 Modifiers tests. `no_gui_types_in_core`
  integration test still green.

## Standing gate (last green run, 2026-07-21)

```sh
cargo fmt --all --check                                          # clean
cargo clippy --workspace --all-targets -- -D warnings            # clean
cargo clippy --no-default-features -p flintshot-app \
  --all-targets -- -D warnings                                   # clean
cargo test --workspace --all-targets                             # 74 pass, 1 ignored, 0 fail
cargo test --doc -p flintshot-core -p flintshot-editor-egui      # 0 doc tests
```

## Open verification debt

Tasks 9 + 10 share the same interactive check; one run closes both,
plus Checkpoint D:

```sh
cargo run -- region
```

What to verify:
- Draw a rect with the Rect tool.
- `Ctrl+Z` — rect disappears (undo).
- `Ctrl+Y` or `Ctrl+Shift+Z` — rect reappears (redo).
- `Ctrl+S` — native save dialog opens; pick a path; PNG has the
  rect baked in (opens in image viewer).
- `Ctrl+C` — image on clipboard; paste in GIMP / image viewer; rect
  visible.
- `Esc` — closes cleanly with `EditorOutcome::Cancelled`.

If any shortcut fails, capture the symptom and add a session note
before debugging. The dispatcher's unit tests cover the logic; an
interactive failure would mean the egui-side translation
(`translate_modifiers`, key capture) is off.

## Next: Task 11 — Arrow tool end-to-end

**Task 11** ([`TODO.md`](TODO.md) §"Task 11"). Goal: `Tool::Arrow`
becomes interactive. Same drag input model as Rect; the dispatcher
already handles it (`begin_annotation` constructs an `Arrow`
annotation for `Tool::Arrow`, `update_annotation` sets the head
endpoint). The work is in three pieces:

1. **Real `EguiCanvas::draw_arrow` impl** — line + two barbs at
   ±30° from the shaft direction, length = `head_size`. Currently
   a stub emitting `tracing::warn!`.
2. **Arrow toolbar button** in `EditorApp::ui`'s toolbar closure —
   `selectable_value(&mut self.state.tool, Tool::Arrow, "→ Arrow")`.
3. **`MockCanvas` snapshot test** for the Arrow variant — assert
   exact `draw_arrow` call arguments.

**Acceptance criteria** (from [`TODO.md`](TODO.md) §"Task 11"):
- [ ] Drag with Arrow tool active commits an `Annotation::Arrow`
      with tail at the down point and head at the up point.
- [ ] `head_size` defaults to 10 physical px; subsequent arrows
      inherit `state.stroke_width` for the shaft.
- [ ] `EguiCanvas::draw_arrow` renders barbs at ±30° from the shaft
      direction, length = `head_size`.
- [ ] Toolbar adds an Arrow button; clicking switches `state.tool`.
- [ ] Undo/redo works for arrows (no extra plumbing — Task 7 covers
      it).

**Verification:**
- [ ] Standing gate passes.
- [ ] `MockCanvas` snapshot test for an `EditorState` with one Arrow
      ann: assert exact `draw_arrow` call arguments.
- [ ] Manual: draw an arrow over a screenshot, verify head/tail
      orientation; save and reopen.

**Files likely touched:**
- `crates/flintshot-core/src/render.rs` (Arrow dispatch — already
  exists, may need adjustment if the MockCanvas needs to record
  `head_size`).
- `crates/flintshot-editor-egui/src/canvas.rs` (`draw_arrow` impl).
- `crates/flintshot-editor-egui/src/app.rs` (Arrow toolbar button).

**Watch out for:**
- `Annotation::Arrow` already exists with `{ a, b, stroke,
  head_size }`. The dispatch path `render_ann` already calls
  `canvas.draw_arrow(*a, *b, stroke, *head_size)`. The `MockCanvas`
  arm for `draw_arrow` already records the call. So Task 11 might
  be smaller than the spec suggests — verify by running the
  existing tests and reading the relevant `render_ann` arm before
  estimating.
- Barb geometry: at ±30° from the shaft direction means the barbs
  form a 60° angle at the head. Length is `head_size` (physical
  px). Some flame-shot-style tools use a fixed barb angle and
  scale length with stroke width; the TODO spec fixes length at
  `head_size`. Keep it spec-compliant.
- HiDPI: arrow geometry is in canvas-physical px (like everything
  else). The canvas converts to logical at the painter boundary.
- `ARROW_DEFAULT_HEAD_SIZE = 10.0` already lives in
  `editor.rs:475`; `begin_annotation` uses it. No new constant
  needed.
- Phase 2 may want a configurable head size (Marker / Pencil
  inherit stroke width; Arrow is the only one with a separate
  head length). Don't add a config knob yet — Phase 1's
  `head_size = 10.0` constant is fine.

## How to resume in a new session

1. **Start opencode in this repo:**
   ```sh
   cd ~/src/oss/l1br3/flintshot
   opencode
   ```

2. **Commit the uncommitted Task 10 work first** — see the
   "Uncommitted state" section below for the suggested pathspec.
   Then run the interactive Checkpoint D verification.

3. **If the interactive check passes**, tick Checkpoint D and start
   Task 11. If it fails, capture the symptom as a session note
   before debugging.

4. **Read order for context-loading** (opencode does most of this
   automatically, but for manual context-checking):
   - [`AGENTS.md`](../AGENTS.md) — operational guide + commands.
   - [`docs/TODO.md`](TODO.md) §"Session notes" — Tasks 3–10
     close-outs + the Checkpoint B + C close-out (deviations +
     downstream impact).
   - [`docs/PLAN.md`](PLAN.md) — phase plan + risks; especially the
     Phase 1.E task list.
   - This file — for the resume checklist.

5. **Run the standing gate first** as a sanity check that nothing
   drifted between sessions:
   ```sh
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo clippy --no-default-features -p flintshot-app --all-targets -- -D warnings
   cargo test --workspace --all-targets
   cargo test --doc -p flintshot-core -p flintshot-editor-egui
   ```

## Uncommitted state

Task 10 work is unstaged. Survey:

```sh
git status
```

Suggested commit (do this **before** starting Task 11 so the working
tree is clean):

```sh
git add crates/flintshot-core/src/input.rs \
        crates/flintshot-core/src/editor.rs \
        crates/flintshot-core/src/lib.rs \
        crates/flintshot-editor-egui/src/app.rs \
        CHANGELOG.md docs/TODO.md docs/SESSION_HANDOFF.md
git commit -m "Task 10: keyboard shortcuts + pure dispatch_input helper"
```

The `README.md` change in `git status` is the maintainer's earlier
"Getting started" expansion; include it in the same commit or split
into a separate `docs: expand README getting started` commit as
preferred.

Adjust the pathspec against the actual `git status` output. If the
extraction touched additional files (it shouldn't — Task 10 was
scoped to the four files above plus docs), include them.
