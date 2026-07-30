---
status: active
started: 2026-07-28
---

# Spec — Phase 2: Daemon, Tray, Hotkeys, IPC

> Draft effort spec. For product context, see
> [`../../VISION.md`](../../VISION.md); for sequencing, see
> [`../../ROADMAP.md`](../../ROADMAP.md); for the technical substrate
> this builds on, see [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md)
> (especially §Process model, §IPC protocol, §Tech stack). The shipped
> Phase 1 spec lives at
> [`../phase-1-capture-editor/spec.md`](../phase-1-capture-editor/spec.md).

## Objective

Make `flintshot` a resident desktop tool, not just a one-shot CLI.
After Phase 1, capture + editor already work via `flintshot region` /
`flintshot fullscreen`. Phase 2 adds the always-on layer that real
screenshot tools run on: a background daemon owning a system tray,
global hotkeys, a single-instance lock, and an IPC server that exposes
the same actions to external clients (the Phase 3 COSMIC applet,
scripts, future shells).

**Target users:** the same power users from
[`VISION.md`](../../VISION.md) §Target users — Linux/Wayland, COSMIC,
cross-platform developers. Phase 2 is what lets them bind a key, hit
it, and get the editor without keeping a terminal open.

**Success looks like:** the user starts `flintshot daemon` once at
login (or via autostart). A tray icon appears with quick actions
including a per-monitor capture submenu. Pressing the configured
global hotkey (e.g. `Ctrl+Shift+Print`) captures and opens the editor.
A second `flintshot daemon` either joins the running one or replaces
it (`--replace`). Editing `[hotkeys]` in the config file is picked up
without a restart. An external process connecting to the IPC socket
can request a capture and observe the event stream. Wayland's
global-hotkey gap is documented, not hidden.

### Why this scope, and what's explicitly out

Phase 1 deferred five annotation tools (Ellipse, Line, Marker, Pencil,
Number) and the Magnifier to "Phase 2". This effort **does not** build
them. Phase 2 is a coherent vertical slice — *system integration* —
that stands on its own and unblocks Phase 3 (COSMIC applet, which needs
the IPC server) and Phase 3's pin-to-desktop. The deferred annotation
tools + magnifier become a separate `phase-2b-annotation-tools`
effort. Keeping them apart keeps each effort reviewable and
independently shippable, and matches how Flameshot-class tools actually
decompose (system layer vs. drawing layer).

## Scope (in / out)

**In:**

- `flintshot daemon` — long-running process owning tray + hotkeys +
  single-instance lock + IPC server + config watcher
- System tray (`tray-icon`) with quick actions: Capture region,
  Capture fullscreen, per-monitor fullscreen submenu, Open config,
  Quit
- Global hotkeys (`global-hotkey`) on X11 / macOS / Windows, read from
  `[hotkeys]` in config
- Single-instance lock (`single`) — daemon role only; workers may run
  concurrently
- Full IPC server (`interprocess` local socket + `postcard` framing)
  with the Request/Event protocol from ARCHITECTURE.md §IPC protocol
- Config hot-reload (`notify` file watcher) — hotkey changes apply
  without restart
- Multi-monitor UX: tray submenu listing monitors for per-monitor
  fullscreen capture (uses Phase 1's `--monitor N` path under the hood)
- CLI integration: `flintshot region` / `fullscreen` detect a running
  daemon and delegate via IPC (centralising the event stream); fall
  back to standalone capture if no daemon answers
- IPC protocol types live in `flintshot-core::ipc`; the daemon/server
  implementation lives in `flintshot-app`

**Out (deferred):**

- Annotation tools: Ellipse, Line, Marker (highlight), Pencil
  (freehand), Number bubbles → `phase-2b-annotation-tools`
- Magnifier → `phase-2b-annotation-tools`
- UX "polish" sweep → its own effort
- Wayland global hotkeys via `ashpd` GlobalShortcuts portal → Phase 6
  (Wayland users configure compositor keybindings that invoke
  `flintshot region`; documented gap)
- Pin-to-desktop → Phase 3
- Upload → Phase 4
- Live overlay → Phase 5
- COSMIC panel applet → Phase 3 (it *consumes* this phase's IPC server)

## Deliverables

- [ ] `flintshot-core::ipc` module: `Request`, `Event` enums
      (`#[non_exhaustive]`, `#[serde(rename_all = "kebab-case")]`),
      postcard roundtrip-tested; socket/pipe path resolver (platform
      default + config override)
- [ ] `flintshot daemon` real implementation: tokio runtime, event
      loop, owns tray + hotkey manager + IPC server + config watcher
- [ ] System tray (`tray-icon`): region, fullscreen, monitor submenu,
      open-config, quit
- [ ] Global hotkeys (`global-hotkey`): registered from `[hotkeys]`
      config; re-registered on config change
- [ ] Single-instance lock (`single`): daemon acquires on start;
      second `flintshot daemon` exits cleanly (or `--replace` tells the
      old one to quit via IPC `Request::Quit` and takes over)
- [ ] IPC server: listens on the resolved socket; handles
      `CaptureRegion` / `CaptureFullscreen` / `Quit` / `Subscribe`;
      emits the `Event` stream to subscribers; `PinLast` /
      `UploadLast` / `OpenLast` defined but return an `Unsupported`
      error until their phases
- [ ] Config hot-reload: `notify` watcher re-reads config and
      re-registers hotkeys on `[hotkeys]` change
- [ ] CLI detect-and-delegate: `flintshot region` / `fullscreen` try
      IPC first; on connect failure fall back to the existing
      standalone capture path (Phase 1 behaviour preserved)
- [ ] Worker-spawn model: daemon never blocks its event loop on
      capture — actions spawn a `flintshot` child worker (reusing the
      Phase 1 capture+editor CLI path) and relay the outcome as
      `Event`s
- [ ] `no_gui_types_in_core` boundary test extended to also reject
      `interprocess`, `global-hotkey`, `tray-icon` types in core (only
      `postcard`/`serde`/std types belong in `flintshot-core::ipc`)
- [ ] Cross-platform notes: macOS hotkey permission expectations;
      Wayland global-hotkey gap + compositor-keybind workaround in
      README/AGENTS
- [ ] `CHANGELOG.md` `[Unreleased]` entries; ADRs for the
      daemon-spawns-worker model and single-instance-on-daemon-only
      (drafted during `/planning`)

## Exit criteria

A user can:

1. Start `flintshot daemon`. A tray icon appears. Clicking *Capture
   region* opens the Phase 1 editor over a fresh capture. Clicking a
   monitor in the submenu captures that monitor.
2. Press the configured global hotkey (default `Ctrl+Shift+Print` on
   Linux) from any app. The editor opens over a region capture — no
   terminal involved.
3. Run a second `flintshot daemon` — it exits with a "already running"
   notice; `flintshot daemon --replace` swaps them cleanly.
4. Edit `[hotkeys]` in `~/.config/flintshot/config.toml`, save, and
   press the new chord — it works without restarting the daemon.
5. From a separate shell, connect to the IPC socket (or run
   `flintshot region`, which delegates) and observe the `Event` stream
   (`CaptureStarted` → `EditorOpened`).
6. On Wayland, the tray + capture work; the global hotkey does **not**
   fire, and the docs explain the compositor-keybind workaround.

The abstraction holds: no `tray-icon` / `global-hotkey` /
`interprocess` types leak into `flintshot-core`'s public API (the
extended boundary test enforces it).

## Technologies

Following [`ARCHITECTURE.md`](../../ARCHITECTURE.md) §Tech stack — no
surprises, all pre-approved choices:

| Concern                  | Crate           | Notes                                                  |
| ------------------------ | --------------- | ------------------------------------------------------ |
| Async runtime (daemon)   | `tokio`         | Already a workspace dep; daemon runs on a multi-thread |
|                          |                 | runtime; the tray/hotkey event loop bridges via        |
|                          |                 | channels                                               |
| System tray              | `tray-icon`     | Tauri; cross-platform SNI                              |
| Global hotkeys           | `global-hotkey` | Tauri; X11/macOS/Windows. Wayland → Phase 6            |
| Single-instance lock     | `single`        | Tauri; lock keyed by app id; daemon role only          |
| IPC transport            | `interprocess`  | Unix domain socket (Linux/macOS), named pipe (Windows) |
| IPC encoding             | `postcard`      | Compact, serde-compatible; length-prefixed framing     |
| Config hot-reload        | `notify`        | File watcher; debounce; re-read on `[hotkeys]` change  |
| Errors                   | `thiserror` (core) / `color_eyre` (app) | Per existing convention       |
| CLI                      | `clap`          | `DaemonArgs` already stubbed                           |

New workspace deps to add (all from established authors, all in the
architecture table): `tray-icon`, `global-hotkey`, `single`,
`interprocess`, `postcard`, `notify`. Exact versions pinned during
`/planning`; MSRV 1.95 compatibility verified before pinning.

## Commands

All run from the repo root. Full reference in
[`AGENTS.md`](../../../AGENTS.md) §Commands; the standing gate is the
source of truth.

```sh
# Build (default-members: flintshot-app + deps)
cargo build

# The Phase 2 entry point
cargo run -- daemon                    # start the daemon (tray + hotkeys + IPC)
cargo run -- daemon --replace          # replace a running daemon
cargo run -- region                    # standalone capture (or delegates to daemon)
cargo run -- fullscreen                # standalone capture (or delegates to daemon)
cargo run -- config                    # print resolved config (incl. [hotkeys])

# Gate — run in order before declaring done
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

`cargo run -- daemon` needs a display server on Linux (tray + capture).
For headless CI, daemon-specific behaviour is exercised through unit
tests of the pure pieces (protocol roundtrip, hotkey-config parsing,
dispatch table) plus `#[ignore]`'d integration tests that need `xvfb`,
mirroring how Phase 1 handles the capture smoke test.

## Project structure

New and touched files (crate boundaries from
[`ARCHITECTURE.md`](../../ARCHITECTURE.md) hold — no new crates):

```
crates/flintshot-core/src/
├── ipc.rs                 # NEW — Request, Event enums, socket-path resolver
├── lib.rs                 # re-export ipc
└── error.rs               # new variant: Ipc { detail }, SingleInstance { detail }

crates/flintshot-app/src/
├── main.rs                # unchanged dispatch shape; Daemon now real
├── cli.rs                 # run_daemon body; capture subcommands gain daemon-detect
├── daemon.rs              # NEW — event loop, owns tray+hotkeys+ipc+config-watcher
├── tray.rs                # NEW — tray-icon setup, menu, monitor submenu
├── hotkeys.rs             # NEW — config → global-hotkey registration
├── ipc_server.rs          # NEW — interprocess socket server, postcard framing
├── ipc_client.rs          # NEW — connect-to-daemon helper (used by capture subs)
├── single.rs              # NEW — single-instance lock (daemon role only)
└── worker.rs              # NEW — spawn `flintshot` child for capture+editor

crates/flintshot-core/tests/
└── no_gui_types_in_core.rs   # EXTENDED — also ban interprocess/global-hotkey/tray-icon
```

Rationale for the split: the daemon is a process orchestrator, not a
capture site. It spawns worker children (reusing the Phase 1 CLI) so an
editor crash never kills the tray, and the editor's UI thread never
contends with the daemon's event loop. This is the Flameshot model and
matches ARCHITECTURE.md §Process model.

## Code style

Unchanged from [`AGENTS.md`](../../../AGENTS.md) §Conventions and the
`.opencode/tech/rust/` docs (loaded at `/build` time). The one shape
worth previewing — the IPC protocol types, which are the only new
public surface in `flintshot-core`:

```rust
// crates/flintshot-core/src/ipc.rs
//! IPC protocol types. Transport (interprocess) and encoding (postcard)
//! live in flintshot-app; only the schema is here so external clients
//! (the Phase 3 applet, scripts) can depend on core alone.

use std::path::PathBuf;

/// A client → daemon request. Length-prefixed postcard on the wire.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Request {
    CaptureRegion { monitor: Option<u32>, delay_ms: u32 },
    CaptureFullscreen { monitor: Option<u32> },
    Quit,
    Subscribe,
    // Defined for forward compatibility; daemon returns Unsupported
    // until their phases land:
    OpenLast,
    PinLast,
    UploadLast { backend: String },
}

/// A daemon → client event. Subscribers receive a stream of these.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Event {
    CaptureStarted,
    EditorOpened,
    Saved { path: PathBuf },
    Error { code: String, message: String },
    // Phase 3/4 variants defined now, emitted later:
    Pinned { id: u64 },
    Uploaded { url: String, delete_handle: Option<String> },
}
```

Conventions this locks in: `#[non_exhaustive]` on every public enum,
`#[serde(rename_all = "kebab-case")]`, doc comment on the module, no
transport types in core.

## Testing strategy

Mirrors [`ARCHITECTURE.md`](../../ARCHITECTURE.md) §Testing strategy
and the Phase 1 split (heavy unit coverage in core; integration where a
display is available).

### Unit — `flintshot-core` (headless, no display)

- `ipc` postcard roundtrip: encode every `Request` / `Event` variant,
  decode, assert equality. Catches `#[non_exhaustive]` / serde tag
  regressions.
- Socket-path resolver: `$XDG_RUNTIME_DIR` honoured on Linux, named
  pipe on Windows, config override wins over default.
- `Config` `[hotkeys]` parse: known chord strings → accelerator tokens.

### Unit — `flintshot-app` (headless)

- Hotkey config → `global-hotkey` accelerator mapping (pure function;
  no actual OS registration).
- Dispatch table: given a `Request`, assert the expected worker-spawn
  command line / event emission. The spawner is behind a trait so the
  table is testable without forking.
- Daemon-detect fallback: `ipc_client` returns "no daemon" when the
  socket is absent; capture subcommands then take the standalone path.

### Integration — `flintshot-app` (needs `xvfb` on Linux, `#[ignore]`'d)

- Two-process IPC: spin a daemon on a temp socket, connect a client,
  send `CaptureFullscreen`, assert the `CaptureStarted` → `EditorOpened`
  event sequence arrives on a `Subscribe` stream.
- Single-instance: launch two `flintshot daemon` against the same lock;
  the second exits non-zero with an "already running" message;
  `--replace` swaps them.

### Boundary

- `no_gui_types_in_core.rs` extended to scan for `use interprocess`,
  `use global_hotkey`, `use tray_icon` in `flintshot-core`. Only
  `postcard` + `serde` + std types are permitted in `ipc.rs`.

### Manual test matrix

Tracked in [`docs/TEST_MATRIX.md`](../../TEST_MATRIX.md); Phase 2
adds rows for: tray menu (X11/Wayland/macOS/Windows), hotkey
registration (X11/macOS/Windows — and the Wayland gap), single-instance
behaviour, config hot-reload.

## Boundaries

- **Always:**
  - Run the four-command gate (`fmt`, `clippy`, `test`, doc tests)
    before declaring done.
  - Keep IPC protocol *types* in `flintshot-core::ipc`, transport /
    server *implementation* in `flintshot-app`. Core stays GUI- and
    transport-agnostic.
  - Acquire the single-instance lock **only** in the daemon role.
    Workers (`flintshot region`) never take it.
  - Spawn a worker for capture+editor — **never** block the daemon's
    event loop on `xcap` or eframe.
  - Fall back to standalone capture if the daemon is unreachable;
    `flintshot region` must keep working with no daemon running.
  - Preserve the Phase 1 standalone path bit-for-bit (the daemon
    delegates to it).

- **Ask first:**
  - Adding a workspace dependency not in the ARCHITECTURE.md table.
  - Changing the IPC wire protocol once Phase 3 has shipped (stability
    contract begins then).
  - Changing CI config (`.github/workflows/ci.yml`) — e.g. adding
    `xvfb` to a job.
  - New ADRs — draft during `/planning`, but flag for review.

- **Never:**
  - Introduce `tray-icon` / `global-hotkey` / `interprocess` /
    `libcosmic` / `egui` types into `flintshot-core`'s public API.
  - `unwrap()` / `expect()` outside tests and `const` contexts.
  - Commit the daemon's runtime socket file or any lock artefact.
  - Silence a clippy warning with `#[allow]` without a justifying
    comment.
  - Pretend Wayland global hotkeys work — document the gap.

## Success criteria

- Cold daemon start → tray visible in < 500 ms on commodity hardware.
- Hotkey press → editor open latency dominated by capture (Phase 1's
  path), not by daemon overhead (< 50 ms added).
- An external IPC client can request a capture and receive the full
  `Event` sequence for it.
- Config hot-reload applies a new hotkey without restarting the daemon.
- No new GUI/transport types in `flintshot-core` (boundary test green).
- The four-command gate is green on Linux; `cargo build` green on
  macOS and Windows (cross-compile smoke where a native run isn't
  possible in CI).

## Open questions

1. **Multi-monitor UX (unanswered).** Defaulting to the tray monitor
   submenu. Want combined/stitched "capture all desktops" or an
   interactive monitor-picker overlay folded in, or both deferred?
   (The picker overlaps with Phase 5's live overlay; recommend defer.)
2. **Daemon-detect in capture subcommands.** Spec'd as "try IPC, fall
   back to standalone." Alternative: capture subcommands always run
   standalone (they *are* the workers), and IPC is purely for external
   clients. Confirm the detect-and-delegate behaviour is worth the
   centralised event stream.
3. **macOS hotkey permissions.** Does `global-hotkey` (CGEventTap)
   trigger an Input Monitoring / Accessibility prompt on first use?
   Needs a manual check; document the outcome in cross-platform notes.
4. **ADRs to draft during `/planning`:**
   - Daemon-spawns-worker-processes model (vs. in-process capture).
   - Single-instance lock scoped to the daemon role only.
   - Postcard length-prefix framing details (endianness, max size).
5. **Tokio vs. the tray event loop.** `tray-icon` and `global-hotkey`
   want a winit-style event loop on some platforms; tokio wants its
   own runtime. Confirm the bridge strategy (likely: tokio runtime in
   a background thread, tray/hotkey on the platform event loop,
   channels between) during `/planning` and record in an ADR.
