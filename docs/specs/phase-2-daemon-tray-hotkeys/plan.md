# Plan — Phase 2: Daemon, Tray, Hotkeys, IPC

> Thin index for the current effort. Requirements live in
> [`spec.md`](spec.md); flat status in [`todo.md`](todo.md); per-task
> acceptance / files / verification / dependencies in
> [`tasks/00N-*.md`](tasks/). For product context see
> [`../../VISION.md`](../../VISION.md); for sequence see
> [`../../ROADMAP.md`](../../ROADMAP.md); for the technical substrate
> see [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) (§Process model,
> §IPC protocol).

## Overview

Phase 2 makes `flintshot` a resident desktop tool instead of a one-shot
CLI. `flintshot daemon` becomes a long-running process that owns a
system tray (`tray-icon`), global hotkeys (`global-hotkey`) on
X11/macOS/Windows, a single-instance lock (`single`), and a full IPC
server (`interprocess` + `postcard`) exposing the same actions to
external clients (the Phase 3 COSMIC applet, scripts). The daemon never
captures itself — it spawns `flintshot region` / `fullscreen` worker
children (reusing the Phase 1 path verbatim), so an editor crash never
kills the tray and the editor's UI thread never contends with the
daemon's event loop. Config hot-reload (`notify`) re-registers hotkeys
without a restart. Annotation tools, magnifier, and UX polish are
explicitly out — they spin off to `phase-2b-annotation-tools`.

Phase 2 exit criteria (from [`spec.md`](spec.md)):

> A user starts `flintshot daemon` once at login; the tray icon appears
> with per-monitor capture; the configured global hotkey captures and
> opens the editor; a second daemon joins or replaces; config hotkey
> changes apply without restart; an external IPC client can request a
> capture and observe the event stream. The abstraction holds: no
> `tray-icon` / `global-hotkey` / `interprocess` types leak into
> `flintshot-core`.

## Architecture decisions for Phase 2

- **Daemon spawns worker processes, never captures in-process.** On any
  action (tray click, hotkey, IPC request) the daemon spawns a
  `flintshot` child (resolved via `std::env::current_exe`, not `PATH`)
  running the Phase 1 capture+editor CLI. Isolation + reuse; the
  editor's eframe loop and the daemon's event loop never share a
  thread. (ADR-0019; mirrors Flameshot's model and
  ARCHITECTURE.md §Process model.)
- **IPC protocol types in core, transport in app.** `flintshot-core::ipc`
  owns `Request` / `Event` (`#[non_exhaustive]`, postcard-serde) and
  the socket-path resolver; `flintshot-app` owns the `interprocess`
  server + framing. External clients depend on core alone. (Locks
  ADR-0010 transport + ADR-0011 encoding.)
- **Tokio side thread + platform event loop on the main thread, bridged
  by an mpsc channel.** `tray-icon` and `global-hotkey` need
  main-thread event pumping on macOS; the IPC server + config watcher
  are naturally async. The tokio runtime runs in a background thread
  and forwards IPC requests / watcher events to the main loop; tray
  clicks and hotkey fires arrive on the main loop directly. (ADR-0018,
  decided + proven in Task 003's de-risk spike.)
- **Single-instance lock is daemon-role only.** Workers (`flintshot
  region`) never take the lock, so multiple editors can run at once.
  `flintshot daemon --replace` sends `Request::Quit` over IPC, waits
  for the socket to free, then takes the lock. (ADR-0020.)
- **Capture subcommands detect-and-delegate.** `flintshot region` /
  `fullscreen` try the IPC socket first; on connect failure they fall
  back to the existing standalone Phase 1 path, bit-for-bit. Centralises
  the event stream without breaking headless/scripting use.
- **Hotkeys: X11/macOS/Windows now; Wayland deferred to Phase 6.**
  Wayland users bind compositor keys to `flintshot region`; documented
  gap, not a hidden bug.

## Dependency graph

```text
flintshot-core::ipc (types + socket-path)        flintshot-core::config
        │                                                  │ (+ [hotkeys] section)
        ├─► flintshot-app::ipc_client ─► flintshot-app::cli │
        │       (detect-and-delegate)                       │
        │                                                   ▼
        ├─► flintshot-app::ipc_server ─► flintshot-app::hotkeys
        │              │                  (register from config)
        │              ▼
        │      flintshot-app::daemon ◄── flintshot-app::tray
        │              │   ▲   ▲       (menu → handler)
        │              │   │   └── flintshot-app::single (lock)
        │              ▼   │
        │      flintshot-app::worker (spawn child)
        │              │   ▲
        │              │   └── flintshot-app::config_watcher (notify)
        │              ▼
        └────►  spawn `flintshot` child (Phase 1 capture+editor path)
```

Build bottom-up: protocol types + deps first, then the event-loop
de-risk spike (the highest-risk integration), then the IPC server +
worker model, then the user-facing surfaces (tray, hotkeys), then
hot-reload + CLI delegation, then hardening.

## Task index

Checkpoints gate phases — do not start the next sub-phase until the
previous checkpoint is signed off. Per-task acceptance criteria,
verification, dependencies, and files live in `tasks/00N-*.md`.

### Phase 2.A — Foundation
- [`tasks/001-ipc-protocol-types.md`](tasks/001-ipc-protocol-types.md) — IPC protocol types in `flintshot-core::ipc`
- [`tasks/002-workspace-deps.md`](tasks/002-workspace-deps.md) — Workspace deps + MSRV gate

**Checkpoint A** — foundation types + deps in place; workspace builds clean.

### Phase 2.B — Daemon event-loop de-risk (high risk)
- [`tasks/003-daemon-loop-bridge.md`](tasks/003-daemon-loop-bridge.md) — Daemon event-loop bridge spike + single-instance lock

**Checkpoint B** — bridge approach proven; ADR-0018 reviewed before building on it.

### Phase 2.C — IPC server + worker model
- [`tasks/004-ipc-server-transport.md`](tasks/004-ipc-server-transport.md) — IPC server transport + postcard framing
- [`tasks/005-worker-spawn-module.md`](tasks/005-worker-spawn-module.md) — Worker spawn module + dispatch table
- [`tasks/006-wire-capture-requests.md`](tasks/006-wire-capture-requests.md) — Wire capture requests + `--replace`

**Checkpoint C** — IPC end-to-end: external client requests a capture and observes the Event stream; `--replace` swaps daemons.

### Phase 2.D — System UI
- [`tasks/007-system-tray-menu.md`](tasks/007-system-tray-menu.md) — System tray menu
- [`tasks/008-monitor-submenu.md`](tasks/008-monitor-submenu.md) — Multi-monitor tray submenu
- [`tasks/009-global-hotkeys.md`](tasks/009-global-hotkeys.md) — Global hotkeys from `[hotkeys]` config

**Checkpoint D** — full daemon works on X11: tray + hotkeys + IPC + single-instance (spec exit steps 1–3, 5).

### Phase 2.E — Config hot-reload + CLI delegation
- [`tasks/010-config-hot-reload.md`](tasks/010-config-hot-reload.md) — Config hot-reload via `notify`
- [`tasks/011-cli-detect-delegate.md`](tasks/011-cli-detect-delegate.md) — CLI capture subcommands detect-and-delegate

**Checkpoint E** — config edit applies a new hotkey without restart; `flintshot region` delegates when a daemon runs, runs standalone otherwise.

### Phase 2.F — Hardening
- [`tasks/012-hardening.md`](tasks/012-hardening.md) — Boundary test + ADRs + CHANGELOG + cross-platform docs

**Checkpoint F** — Phase 2 exit criteria met; ready for review.

## Risks and mitigations

| Risk                                                                 | Impact | Mitigation                                                                                                                  |
| -------------------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------- |
| tokio runtime + tray/hotkey event loop cannot coexist on one process | High   | Task 003 is a dedicated de-risk spike that proves the bridge before any feature builds on it; ADR-0018 records the approach   |
| `global-hotkey` on macOS triggers Accessibility/Input Monitoring prompt | Med  | Manual check during Task 009; document the outcome in cross-platform notes + TEST_MATRIX                                     |
| `interprocess` named-pipe behaviour on Windows differs from Unix socket | Med  | Abstract behind `socket_path()` (Task 001); framing unit-tested headless; Windows path noted in TEST_MATRIX                  |
| `tray-icon` has no SNI host on some Wayland compositors              | Med    | Tray is best-effort; IPC + hotkey path still works. Document; tray failure is non-fatal (`tracing::warn!`)                   |
| Worker spawn assumes `flintshot` on PATH                             | Med    | Resolve via `std::env::current_exe()` and spawn that binary (Task 005); ADR-0019 notes it                                    |
| Config hot-reload fires mid-write (partial TOML)                     | Low    | Debounce + tolerate parse failure (keep last-known-good hotkeys, log a warning)                                             |
| IPC protocol churn before Phase 3 applet lands                       | Low    | `#[non_exhaustive]` enums; no stability contract until Phase 3 ships                                                        |
| Scope creep from deferred annotation tools                           | Med    | Spec explicitly assigns Ellipse/Line/Marker/Pencil/Number + Magnifier to `phase-2b`; tasks reject out-of-scope additions    |

## Open questions (need human input before the relevant task)

1. **Carried from spec (resolved to defaults).** (a) Multi-monitor UX
   → tray submenu only (Task 008); combined/picker deferred. (b)
   Capture subcommands detect-and-delegate (Task 011) with standalone
   fallback. Both overridable until their task starts.
2. **Event-loop crate choice (Task 003).** Raw channel-polling vs a
   `winit` event loop for tray/hotkey pumping. Decided in the spike;
   recorded in ADR-0018. Recommendation: start with raw polling of the
   `tray-icon` / `global-hotkey` receivers + an mpsc select; only pull
   in winit if a platform demands it.
3. **`single` vs `single-instance` crate (Task 002).** Pick the variant
   whose API + maintenance story fits; both are Tauri-affiliated.
4. **Length-prefix framing detail (Task 004).** u32 little-endian length
   prefix, capped frame size. If non-obvious, split ADR-0021; else
   extend ADR-0011.

## Verification strategy

Every task clears the standing gate in [`AGENTS.md`](../../../AGENTS.md)
(`fmt --all --check`, `clippy --workspace --all-targets -- -D warnings`,
`test --workspace --all-targets`, doc tests for the two lib crates).
Each checkpoint adds a manual run on at least one platform from
[`docs/TEST_MATRIX.md`](../../TEST_MATRIX.md); daemon/tray/hotkey
behaviour that needs a display is covered by `#[ignore]`'d integration
tests (mirroring Phase 1's capture smoke test) plus the manual matrix.
Per-task verification steps live in `tasks/00N-*.md`.

## Decisions to land this effort

Lasting technical decisions (full text in [`docs/adrs/`](../../adrs/)):

- **ADR-0018** — tokio side thread + main-thread event loop, bridged by mpsc (Task 003).
- **ADR-0019** — daemon spawns worker processes, resolves binary via `current_exe` (Tasks 005/006).
- **ADR-0020** — single-instance lock scoped to the daemon role only (Tasks 003/006).
- **ADR-0021** (conditional) — length-prefix postcard framing detail, if it diverges from ADR-0011 (Task 004).
