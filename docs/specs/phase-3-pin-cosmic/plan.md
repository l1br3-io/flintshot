# Plan — Phase 3: Pin-to-desktop + COSMIC Applet

> Thin index for the current effort. Requirements live in
> [`spec.md`](spec.md); flat status in [`todo.md`](todo.md); per-task
> acceptance / files / verification / dependencies in
> [`tasks/00N-*.md`](tasks/). For product context see
> [`../../VISION.md`](../../VISION.md); for sequence see
> [`../../ROADMAP.md`](../../ROADMAP.md); for the technical substrate
> see [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) (§Process model,
> §IPC protocol, §`EditorShell` trait, §Crate responsibilities).

## Overview

Two "keep the result visible" deliverables that follow the editor:

1. **Pin-to-desktop** — float the captured image as a borderless
   always-on-top eframe viewport (`flintshot pin <path>`), reachable
   from three entry points: the CLI, the editor toolbar, and the daemon
   (`Request::PinLast`).
2. **COSMIC applet** — a real `flintshot-cosmic` panel applet that
   talks IPC to the running daemon (capture actions, per-monitor
   submenu, Pin/Open last, config, quit) and shows a live event feed.

The scaffolding is already in place: `EditorShell::run_pin` is declared
(`crates/flintshot-core/src/shell.rs:44`) but returns
`FlintshotError::NotImplemented` (`shell.rs:42`); `Request::PinLast` /
`OpenLast` and `Event::Pinned` are in the IPC schema
(`crates/flintshot-core/src/ipc.rs:58-60,101`); `flintshot pin` is a
path-touching stub (`crates/flintshot-app/src/cli.rs:322`);
`flintshot-cosmic` is a stub that links `libcosmic`
(`crates/flintshot-cosmic/src/main.rs:13`). The IPC server already
converts a `Subscribe` connection into a pure `Event` sink
(`crates/flintshot-app/src/ipc_server.rs:272`) — the applet reuses it
verbatim.

Phase 3 exit criteria (from [`spec.md`](spec.md)): a user pins from the
editor, drags/resizes/hovers the floating window, dismisses it; from the
COSMIC popup they capture, watch the live status, then Pin/Open last;
multiple pins coexist; the applet degrades gracefully when the daemon is
absent. The abstraction holds: pin rendering routes through
`flintshot-core::render`; the applet touches only `flintshot-core::ipc`;
`no_gui_types_in_core.rs` stays green.

## Architecture decisions for Phase 3

- **Pin is one-worker-per-window (follows ADR-0019).** Each pin is a
  `flintshot pin <path>` child process owning one eframe viewport. A pin
  crash can't kill the editor or daemon; multiple pins coexist as
  independent processes. No new ADR — this is a direct application of
  ADR-0019 (the worker-spawn model), recorded here as the resolution of
  spec Q2.
- **Worker reports its save result back to the daemon (new ADR-0023).**
  Today the worker is fire-and-forget — the daemon emits
  `EditorOpened` optimistically on spawn and never learns the saved
  path. `PinLast`/`OpenLast` need that path. Add a worker-originated
  `Request::ReportSaved { path }` variant: after the editor closes with
  `Saved(path)`, the worker connects to the daemon socket and reports
  it (skipped naturally when no daemon answers — reuses
  `ipc_client`'s `NoDaemon` detection). The daemon keeps an
  `Arc<Mutex<Option<PathBuf>>>` as the last-saved state. This closes
  the Phase 2 reporting gap generally (also unblocks Phase 4
  upload-from-last). Spec Q1, resolved → (a).
- **Pin window geometry = the `Rect` arg.** `EditorShell::run_pin(image,
  rect)` treats `rect.min` as the window's top-left position and
  `rect.max - rect.min` as its size, both in physical px; the whole
  image is shown scaled to that size. Signature unchanged (spec Q6).
- **Pin defaults to the active monitor's center; `--at X,Y` overrides.**
  Capture-origin placement requires threading the source rect through to
  the pin, which the editor doesn't track today — defer. CLI, editor
  button, and daemon `PinLast` all center by default (spec Q3,
  simplified).
- **Hover-opacity is instant on enter/leave (Flameshot-style).** Pointer
  enters → full opaque; leaves → back to `[pin] default-opacity`
  (default `0.5`). No delay (spec Q4).
- **Applet shells out to `$EDITOR` for "Open config" itself.** The
  daemon's `open_config_file()` (`daemon.rs:237`) is `#[cfg(feature =
  "tray")]` and lives on the daemon's main thread — not reachable over
  IPC without a new `Request` variant. The applet instead calls
  `flintshot_core::config_path()` (already public, `config.rs:377`) and
  spawns `$EDITOR` directly (~10 LOC, mirroring `open_config_file`).
  Avoids touching the IPC schema for a minor feature; keeps the applet a
  pure client (spec Q5, resolved → shell-out).
- **COSMIC applet duplicates ~30 LOC of IPC framing.** `read_frame` /
  `write_frame` / `encode_msg` live in `flintshot-app::ipc_server` as
  `pub(crate)`. Moving them to `flintshot-core` would violate ADR-0010
  ("schema in core, transport in bins"). The applet gets its own small
  `ipc` module (mirror of `ipc_client.rs`'s pattern). Ask-first if
  `/planning`-equivalent review wants to reconsider.
- **Applet is a pure IPC client (no `flintshot-app`/`egui` dep).** It
  depends on `flintshot-core` (for `ipc` types + `monitors()` +
  `config_path()`) and `libcosmic` only. Enforced by the extended
  `no_gui_types_in_core.rs` (which gains a `use cosmic`/`libcosmic`/
  `iced`/`slint` check) and by the applet's own `Cargo.toml` dep list.
- **Pin and COSMIC tracks are independently shippable.** The pin track
  (Tasks 001–005) lands first; if `libcosmic` `master` breaks during
  the applet track (Tasks 006–008), pin is already shippable on its
  own. Checkpoint C (pin complete) precedes Checkpoint D (applet).

## Dependency graph

```text
  001 PinConfig + pin CLI scaffold (core config + cli arg parse + load img)
     │
     ▼
  002 Pin window end-to-end (run_pin real + PinApp, all 5 UX features)
     │                                              │
     │                              003 Worker ReportSaved + last-saved state
     │                                              │
     │                                              ▼
     │                              004 Daemon PinLast/OpenLast dispatch
     │                                              │
     ▼                                              │
  005 Editor Pin button (compose → tempfile → spawn pin → close) ◄──┘
     │
     │              006 COSMIC IPC client (framing + Request + Subscribe + absent)
     │                     │
     │                     ▼
     │              007 COSMIC applet shell + popup menu (actions, monitor submenu)
     │                     │
     │                     ▼
     │              008 COSMIC event stream + daemon-absent state
     │                     │
     ▼─────────────────────┴──► 009 Hardening (boundary test, CHANGELOG, ADR-0023, TEST_MATRIX)
```

- 001 → 002 (the window needs `PinConfig`).
- 002 → 005 (the editor button spawns the `flintshot pin` CLI built in
  001/002).
- 003 → 004 (`PinLast`/`OpenLast` read the last-saved state 003 writes).
- 004 → 005 soft (the editor button is the primary pin path; daemon
  `PinLast` is the secondary — both land before the applet exercises
  them).
- 006 → 007 → 008 (applet UI layers on the IPC client; the event stream
  layers on the popup shell). 006 is independent of the pin track and
  could start in parallel from Task 003 onward.
- 009 depends on all.

## Task index

Checkpoints gate phases — do not start the next sub-phase until the
previous checkpoint is signed off. Per-task acceptance criteria,
verification, dependencies, and files live in `tasks/00N-*.md`.

### Phase 3.A — Pin foundation
- [ ] Task 1: PinConfig (`[pin]` section) + `flintshot pin` CLI scaffolding → `tasks/001-pin-config-and-cli.md`

**Checkpoint A** — `flintshot pin <path>` loads an image and reaches `run_pin` (still a stub returning `NotImplemented`); the `[pin]` config round-trips. Plumbing proven; window lands next.

- [ ] Task 2: Pin window end-to-end — `run_pin` real impl + `PinApp` with all 5 UX features → `tasks/002-pin-window.md`

**Checkpoint B** — `flintshot pin <path>` floats a borderless always-on-top window; drag, Esc/double-click close, hover-opacity, resize, copy-on-close all work.

### Phase 3.B — Daemon last-saved + PinLast/OpenLast
- [ ] Task 3: Worker result reporting (`Request::ReportSaved`) + daemon last-saved state → `tasks/003-worker-report-saved.md`
- [ ] Task 4: Daemon `PinLast` / `OpenLast` dispatch (spawn worker, emit `Event::Pinned`) → `tasks/004-daemon-pin-last-open-last.md`

**Checkpoint C** — daemon responds to `PinLast`/`OpenLast` over IPC (verifiable via a test client); `Event::Pinned { id }` broadcasts with monotonic ids.

### Phase 3.C — Editor Pin button
- [ ] Task 5: Editor toolbar Pin button (compose → tempfile → spawn pin → close) → `tasks/005-editor-pin-button.md`

**Checkpoint D** — pin works from all three entry points: CLI, daemon, editor toolbar. Pin-to-desktop is feature-complete and independently shippable.

### Phase 3.D — COSMIC applet
- [ ] Task 6: COSMIC IPC client (framing + Request send + Subscribe read + daemon-absent) → `tasks/006-cosmic-ipc-client.md`
- [ ] Task 7: COSMIC applet shell + popup menu (panel button, actions, monitor submenu, Open config/Quit) → `tasks/007-cosmic-applet-popup.md`
- [ ] Task 8: COSMIC event stream + daemon-absent state (Subscribe display, status line) → `tasks/008-cosmic-event-stream.md`

**Checkpoint E** — COSMIC applet works end-to-end against a running daemon; popup shows live status and degrades gracefully when the daemon is absent.

### Phase 3.E — Hardening
- [ ] Task 9: Boundary test extension (reject cosmic/libcosmic/iced/slint) + CHANGELOG + ADR-0023 + TEST_MATRIX → `tasks/009-hardening.md`

**Checkpoint F** — Phase 3 exit criteria met; ready for review.

## Risks and mitigations

| Risk                                                                 | Impact | Mitigation                                                                                                                       |
| -------------------------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------- |
| `libcosmic` git `master` breaks mid-effort                          | High   | Applet track (Tasks 6–8) is after pin track (1–5); pin is independently shippable at Checkpoint D. Pin `master` to a commit if it breaks; flag in Task 7. |
| eframe `always_on_top`/`transparent`/`decorations(false)` differ per-OS | Med    | Task 2 verifies on Linux first; macOS `NSWindow` level + Windows `WS_EX_TOPMOST` via eframe are well-trodden. `#[ignore]`'d tests cover non-Linux in CI. |
| `Request::ReportSaved` blurs the client/worker distinction in IPC    | Low    | It's a client→daemon message like any other; the daemon treats it as state-update, not an action. ADR-0023 records the rationale. |
| Worker can't reach daemon to report (daemon quit mid-edit)           | Low    | `NoDaemon` detection reused (`ipc_client.rs:71`); report silently skips. Last-saved state simply isn't updated.                  |
| Applet popup built at panel-open vs. per-open (monitor hot-plug)     | Low    | Mirror the tray's documented limitation (`tray.rs:85`): build per-open is fine for v1; hot-plug needs a reopen. Document in TEST_MATRIX. |
| Pin copy-on-close races process exit (X11 owner-based clipboard)     | Med    | Keep the `arboard::Clipboard` handle alive until the event loop drains (mirror the editor Copy fix, `app.rs:86`); optionally a short grace delay before exit. |
| Scope creep from upload/live-overlay/polish features                 | Med    | Spec explicitly excludes them; tasks reject out-of-scope additions.                                                              |

## Open questions (resolved to defaults — overridable until the relevant task starts)

1. **Last-saved-path tracking** *(spec Q1)* → **(a) worker reports back.**
   New `Request::ReportSaved { path }`; daemon keeps
   `Arc<Mutex<Option<PathBuf>>>`. (Task 3; ADR-0023.)
2. **One-worker-per-pin vs multi-viewport** *(spec Q2)* → **One worker
   per pin.** Direct application of ADR-0019; no new ADR. (Task 2.)
3. **Pin positioning default** *(spec Q3)* → **Active-monitor center;
   `--at X,Y` overrides.** Capture-origin placement deferred (editor
   doesn't track source rect today). (Tasks 1, 2.)
4. **Hover-opacity UX** *(spec Q4)* → **Instant on enter/leave.**
   (Task 2.)
5. **Applet "Open config"** *(spec Q5)* → **Applet shells out to
   `$EDITOR`** via `flintshot_core::config_path()`. No new IPC variant.
   (Task 7.)
6. **Pin `rect` semantics** *(spec Q6)* → **Window geometry** (`min` =
   position, `max - min` = size, physical px). Signature unchanged.
   (Task 2.)

## Verification strategy

Every task clears the standing gate in [`AGENTS.md`](../../../AGENTS.md)
(`fmt --all --check`, `clippy --workspace --all-targets -- -D warnings`,
`test --workspace --all-targets`, doc tests for the two lib crates).
Window/applet behaviour needing a display is covered headless through
pure-core tests (config parsing, dispatch-table unit tests with
`RecordingSpawner`, IPC client against an in-process `IpcServer`);
`#[ignore]`'d integration tests cover the visual pin + live COSMIC popup
under `xvfb`, mirroring Phase 1's capture smoke test. Each checkpoint
adds a manual run on at least one platform from
[`docs/TEST_MATRIX.md`](../../TEST_MATRIX.md). Per-task verification
steps live in `tasks/00N-*.md`.

## Decisions to land this effort

Lasting technical decisions (full text in [`docs/adrs/`](../../adrs/)):

- **ADR-0023** — worker→daemon result reporting (`Request::ReportSaved`)
  for last-saved-path tracking (Task 3).

The one-worker-per-pin choice is recorded above as an application of
ADR-0019 (not a new ADR). The applet-as-pure-IPC-client and
framing-duplication choices follow ADR-0005/ADR-0010 (not new ADRs).
