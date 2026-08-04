# Plan — Phase 3: Pin-to-desktop

> Thin index for the current effort. Requirements live in
> [`spec.md`](spec.md); flat status in [`todo.md`](todo.md); per-task
> acceptance / files / verification / dependencies in
> [`tasks/00N-*.md`](tasks/). For product context see
> [`../../VISION.md`](../../VISION.md); for sequence see
> [`../../ROADMAP.md`](../../ROADMAP.md); for the technical substrate
> see [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) (§Process model,
> §`EditorShell` trait, §IPC protocol).

## Overview

Add pin-to-desktop as the third output path (after Save and Copy). A
pin is a borderless always-on-top floating window showing a composed
image; it persists after the editor closes because it is its own
process. Three entry points converge on one primitive
(`EguiShell::run_pin`, already declared, currently stubbed):

1. `flintshot pin <path>` — real CLI body (load any image format).
2. Editor *Pin* toolbar button — composes the final image, writes it
   to a cache file, spawns `flintshot pin <path>` detached, closes
   the editor.
3. `Request::PinLast` over IPC — the daemon reads its last-tracked
   saved path and spawns `flintshot pin <path>` as a worker.

The third entry point requires a worker→daemon feedback loop (a new
fire-and-forget `Request::ReportSaved { path }`) so the daemon learns
which path the editor saved to. This also fills a Phase 2 gap:
`Event::Saved` is currently never emitted to subscribers (the daemon
emits `EditorOpened` as a process-started proxy).

The COSMIC applet that the roadmap bundles under "Phase 3" is deferred
to `phase-3b-cosmic-applet`, matching the Phase 2 → Phase 2b precedent
— it consumes this phase's `PinLast` + Phase 2's IPC server.

Phase 3 exit criteria (from [`spec.md`](spec.md) §Exit criteria): a
user can pin from the editor, from the CLI, and via `PinLast` over IPC;
the pin is borderless, always-on-top, draggable, Esc-dismissable, and
survives its parent editor/daemon closing; multiple pins coexist; the
abstraction holds (boundary test green).

## Architecture decisions for Phase 3

- **A pin is its own process.** The editor and the daemon both spawn
  `flintshot pin <path>` as a detached child (reusing the Phase 2
  `worker::CommandSpawner` pattern). The pin's event loop never
  contends with the editor's; an editor crash or daemon restart never
  kills an existing pin. Matches Flameshot's architecture and ADR-0019.
  The alternative — a deferred egui viewport inside the editor's eframe
  instance — fails because egui viewports die with their root window;
  the editor closing would close the pin. (Task 001; ADR-0023.)
- **`EditorShell::run_pin` is the single primitive; the CLI is its
  canonical caller.** The editor and daemon both route through
  `flintshot pin <path>` rather than calling `run_pin` in-process.
  This keeps the pin primitive in the editor-egui crate (where eframe
  lives) and the spawn decision in the app crate (where process
  orchestration lives), preserving the crate boundary.
- **`EditorOutcome::Pinned(PathBuf)` carries the cache path, not the
  image.** Images are too large for IPC frames and `RgbaImage` doesn't
  impl `Eq` (so it can't be in `EditorOutcome`, which is `PartialEq +
  Eq`). The editor writes the composed image to a pin-cache file and
  returns the path; the app spawns `flintshot pin <path>` from that
  path; the daemon can later re-pin or clean it. (Task 003.)
- **Worker→daemon outcome reporting is fire-and-forget over the existing
  IPC socket.** A new `Request::ReportSaved { path }` variant (the only
  wire-protocol addition; `#[non_exhaustive]` makes it non-breaking).
  The worker sends it after `run_editor` returns `Saved` or `Pinned`; a
  missing daemon (standalone capture) is logged at `warn` and doesn't
  fail the editor. Chosen over a side-channel file or daemon-side
  directory watch because it reuses the Phase 2 transport and gives
  subscribers the real `Event::Saved` they were promised. (Task 002;
  ADR-0024.)
- **Daemon-side last-saved state is `Arc<Mutex<Option<PathBuf>>>`.**
  Shared between the IPC server (which receives `ReportSaved`) and the
  worker dispatch (which reads it for `PinLast`), same sharing pattern
  as `SubscriberBus`. A per-daemon `AtomicU64` generates monotonic pin
  ids for `Event::Pinned { id }`. (Task 004.)
- **Pin-cache dir helper lives in `flintshot-core::config`.** It's a
  sibling of the existing `config_path()` / `config_dir()` resolver
  family; both use the `dirs` crate (already a workspace dep). Returns
  `<cache_dir>/flintshot/pins/` (platform-correct via `dirs::cache_dir`);
  `None` when the platform has no cache convention (logged, editor Pin
  button falls back to the system temp dir). Extracting a `paths`
  module is deferred until a third resolver appears. (Task 003; spec
  [§Open questions](spec.md#open-questions) Q1.)
- **egui `ViewportCommand::StartDrag` for borderless dragging.** egui
  0.35's `ViewportBuilder` exposes `with_always_on_top()`,
  `with_decorations(false)`, `with_position()`, `with_resizable()`;
  `ViewportCommand::StartDrag` moves the OS window with the left mouse
  button. The pin app calls `StartDrag` on a pointer-down that isn't
  otherwise consumed (no toolbar, no annotations — the pin is pure
  blit). (Task 001.)
- **`run_pin(image, rect)`'s `rect` is honoured only when the caller
  has a real screen position.** For Phase 3 neither caller does: the
  editor has lost the original capture position by the time the user
  clicks Pin, and `flintshot pin <path>` loads a file with no position
  metadata. Both pass a default `Rect`; the WM positions the window.
  The `rect` parameter stays in the trait signature for the Phase 5
  live overlay, which captures in-place and can pass the real origin.
  (Task 001; spec Q2.)

## Dependency graph

```text
001 Pin primitive (run_pin + flintshot pin) ──┬──► 003 Editor Pin button
                                              │       (EditorOutcome::Pinned,
                                              │        pin-cache dir, spawn)
                                              │            │
002 ReportSaved (worker→daemon feedback) ─────┼──► 004 PinLast over IPC
                                              │       (read last_saved,
                                              │        spawn pin worker)
                                              │            │
                                              └────────────┴──► 005 Hardening
                                                                 (boundary,
                                                                  CHANGELOG,
                                                                  ADRs, docs)
```

001 and 002 are independent foundations. 003 depends on 001 (the pin
primitive must exist for the spawn to produce a visible window). 004
depends on 001 (the `flintshot pin` worker) and 002 (the `last_saved`
state that `PinLast` reads). 005 depends on all. 003 does NOT depend on
002 — the editor Pin button writes its own cache file and spawns the pin
directly; it also reports via the 002 mechanism once 002 lands, but the
spawn itself is independent.

The ordering 001 → 002 → 003 → 004 → 005 lets the headline feature (the
pin primitive) land first and de-risk the egui viewport questions before
the IPC plumbing starts.

## Task index

Checkpoints gate phases — do not start the next sub-phase until the
previous checkpoint is signed off. Per-task acceptance criteria,
verification, dependencies, and files live in `tasks/00N-*.md`.

### Phase 3.A — Foundation: the pin primitive
- [ ] Task 001: Pin primitive — `EguiShell::run_pin` real impl + `flintshot pin <path>` real CLI body → [`tasks/001-pin-primitive.md`](tasks/001-pin-primitive.md)

**Checkpoint A** — `flintshot pin <path>` opens a borderless always-on-top window; Esc/close dismisses it. Foundation for all other entry points.

### Phase 3.B — Worker→daemon outcome reporting
- [ ] Task 002: `Request::ReportSaved { path }` — new IPC variant, daemon stores `last_saved` + broadcasts `Event::Saved`, worker sends fire-and-forget on `Saved` → [`tasks/002-report-saved.md`](tasks/002-report-saved.md)

**Checkpoint B** — a daemon-driven capture whose editor saves emits `Event::Saved { path }` to IPC subscribers (closes the Phase 2 gap; unblocks `PinLast`).

### Phase 3.C — Editor Pin button
- [ ] Task 003: `EditorOutcome::Pinned(PathBuf)` + pin-cache dir helper + editor *Pin* toolbar button + spawn handling in `run_editor_for_capture` → [`tasks/003-editor-pin-button.md`](tasks/003-editor-pin-button.md)

**Checkpoint C** — the editor *Pin* button works end-to-end: click Pin → composed image appears as a floating always-on-top window, editor closes, pin persists. (Spec exit steps 1–4.)

### Phase 3.D — PinLast over IPC
- [ ] Task 004: Daemon handles `Request::PinLast` — reads `last_saved`, spawns `flintshot pin <path>` worker, emits `Event::Pinned { id }`; empty `last_saved` → `Error { code: "no-last-capture" }` → [`tasks/004-pin-last-ipc.md`](tasks/004-pin-last-ipc.md)

**Checkpoint D** — full Phase 3 exit criteria: all three pin entry points work; `PinLast` over IPC opens a pin for the most recent save; the subscriber stream observes `Event::Saved` then `Event::Pinned`. (Spec exit steps 5–8.)

### Phase 3.E — Hardening
- [ ] Task 005: Boundary test green, CHANGELOG `[Unreleased]`, ADR-0023 (pin-as-process) + ADR-0024 (ReportSaved), `docs/TEST_MATRIX.md` rows, cross-platform Wayland caveat in README/AGENTS → [`tasks/005-hardening.md`](tasks/005-hardening.md)

**Checkpoint E** — Phase 3 ready for review.

## Risks and mitigations

| Risk                                                                       | Impact | Mitigation                                                                                                                |
| -------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------- |
| egui borderless + always-on-top misbehaves on a target compositor          | High   | Task 001 lands the pin primitive first; manual check on X11 + (best-effort) Wayland before proceeding. Document per-platform behaviour in Task 005. |
| `ViewportCommand::StartDrag` doesn't fire on every platform's borderless window | Med    | Fall back to `OuterPosition(current + drag_delta)` if StartDrag is a no-op on a platform; record in ADR-0023.              |
| Worker→daemon `ReportSaved` arrives before the daemon's `last_saved` state is wired | Med    | Task 002 wires both halves in one task; the daemon handler is the first commit, the worker send is the second.               |
| Pin-cache dir fills up (each editor Pin writes a new file)                 | Low    | Opportunistic cleanup on daemon start (Task 005 or a follow-up); files live in the cache dir, which the OS may reap.       |
| `flintshot-cosmic` stub breaks (it's an opt-in build, not in default-members) | Low    | Task 005 verifies `cargo build -p flintshot-cosmic` still passes; the crate is untouched by Phase 3 but CI gates it.        |
| Pin window renders at wrong scale on HiDPI                                 | Med    | Reuse the editor's `pixels_per_point`-aware blit path (`EguiCanvas::draw_image`); the pin is 1:1 with the source texture.   |
| `EditorOutcome::Pinned` addition breaks the `#[non_exhaustive]` wildcard arm | Low    | The existing wildcard in `run_editor_for_capture` already covers it; Task 003 makes the arm explicit.                       |

## Open questions (resolved to defaults — overridable until the relevant task starts)

1. **Pin-cache dir helper placement.** *(spec Q1)* → **Extend
   `flintshot-core::config`** with `pin_cache_dir()`. It already owns
   `config_path()` and uses the `dirs` crate (a workspace dep);
   extracting a `paths` module is premature with only two resolvers.
   (Task 003.)
2. **`run_pin(image, rect)` rect semantics.** *(spec Q2)* → **Honour
   `rect` when the caller has a real screen position; both Phase 3
   callers pass a default.** The Phase 5 live overlay will pass the
   in-place capture origin. Document in the trait doc. (Task 001.)
3. **Worker→daemon report granularity.** *(spec Q3)* → **Minimal now:
   `Request::ReportSaved { path }` only.** A future `ReportOutcome`
   enum covers `CopiedToClipboard` / `Cancelled` when a consumer needs
   them; `PinLast` only needs the saved path. (Task 002.)
4. **Pin window transparency / rounded corners.** *(spec Q4)* → **No.**
   Keep the pin pixel-rectangular; shadows and rounded corners are
   compositor-dependent and a polish-effort concern. (Task 001.)
5. **macOS always-on-top level.** *(spec Q5)* → **Manual check in
   Task 005.** `with_always_on_top()` maps to `NSWindow.level =
   .floating`; verify it's above fullscreen apps or whether `.statusBar`
   is needed. Document the outcome.
6. **ADRs to draft during build (called out in spec Q6):**
   - **ADR-0023** — pin-as-separate-process (reuses ADR-0019's
     worker-spawn model; records why a deferred viewport doesn't work).
     Drafted in Task 001.
   - **ADR-0024** — worker→daemon outcome reporting via
     `Request::ReportSaved` (fire-and-forget; why it beats a side-channel
     file or directory watch). Drafted in Task 002.
   - The pin-cache dir lifecycle is recorded in `config.rs` doc + a
     CHANGELOG note, not a standalone ADR (too small).

## Verification strategy

Every task clears the standing gate in [`AGENTS.md`](../../../AGENTS.md)
(`fmt --all --check`, `clippy --workspace --all-targets -- -D warnings`,
`test --workspace --all-targets`, doc tests for the two lib crates).
Pin behaviour needing a display is covered headless through:

- pure-helper unit tests (`pin_cache_dir()` resolution, `pin_argv()`
  builder, `ReportSaved`/`PinLast` dispatch tables via
  `RecordingSpawner`, postcard roundtrips);
- `PinApp` construction tests (viewport flags, image-blit cache) — no
  GPU needed, the `ViewportBuilder` fields are public and assertable;

plus `#[ignore]`'d integration tests under `xvfb` for the visual path
(opening a pin, the end-to-end `PinLast` round), mirroring Phase 1/2's
capture-smoke-test convention. Each checkpoint adds a manual run on at
least one platform from [`docs/TEST_MATRIX.md`](../../TEST_MATRIX.md).
Per-task verification steps live in `tasks/00N-*.md`.

## Decisions to land this effort

Lasting technical decisions (full text in [`docs/adrs/`](../../adrs/)):

- **ADR-0023** — pin-as-separate-process (Task 001).
- **ADR-0024** — worker→daemon outcome reporting via `ReportSaved`
  (Task 002).

The `EditorOutcome::Pinned` variant, the pin-cache-dir placement, and
the `run_pin` rect semantics are plan-level refinements recorded above
and in the trait docs, not standalone ADRs.
