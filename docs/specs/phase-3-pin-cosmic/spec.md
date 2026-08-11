---
status: shipped
started: 2026-08-09
shipped: 2026-08-11
---

# Spec — Phase 3: Pin-to-desktop + COSMIC Applet

> Draft effort spec. For product context, see
> [`../../VISION.md`](../../VISION.md); for sequencing, see
> [`../../ROADMAP.md`](../../ROADMAP.md); for the technical substrate
> this builds on, see [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md)
> (especially §Process model, §IPC protocol, §UI abstraction layer,
> §`EditorShell` trait). The shipped phases this effort stands on:
> [Phase 1](../phase-1-capture-editor/spec.md) (capture + editor),
> [Phase 2](../phase-2-daemon-tray-hotkeys/spec.md) (daemon, tray,
> hotkeys, IPC), [Phase 2b](../phase-2b-annotation-tools/spec.md)
> (annotation tools + magnifier).

## Objective

Make `flintshot` a tool you *keep using* after the editor closes. After
Phases 1–2b, a capture flows: daemon spawns worker → editor opens → user
annotates → Save/Copy → window closes. Two Flameshot-grade affordances
are still missing, and this phase lands both:

1. **Pin-to-desktop** — float the captured image as a borderless,
   always-on-top, draggable window that stays visible while you work in
   other windows. The classic "I took a screenshot of the error, now I
   need to re-type it into the bug report" workflow.

2. **COSMIC panel applet** — a `flintshot-cosmic` binary that runs as a
   COSMIC panel button, opening a popup with the same quick actions as
   the tray (capture, per-monitor capture, pin/open last, config, quit)
   plus a *live event feed* so the user sees capture/save/pin feedback
   without a terminal. It talks IPC to the running daemon — it is not a
   capture/editor host.

**Target users:** the same power users from
[`VISION.md`](../../VISION.md) §Target users. Pin-to-desktop is the
single most-requested Flameshot feature among developers juggling
multiple windows; the COSMIC applet is what makes `flintshot` feel
native on System76's desktop instead of "that GTK tray icon."

**Success looks like:**

- A user captures a region, annotates it, clicks **Pin** in the editor
  toolbar (or presses the chord), and the composed image floats as an
  always-on-top window over their editor. They drag it to a second
  monitor, type the relevant snippet into a chat, double-click the pin
  (or press `Esc`) to dismiss it, and optionally have it copied to the
  clipboard on close.
- From the COSMIC panel, the user clicks the `flintshot` button, picks
  *Fullscreen on monitor → 1*, watches the popup show *"Capturing… →
  Editor opened → Saved /tmp/x.png"*, then clicks *Pin last* and the
  saved screenshot floats as a pin window seconds later.
- A second pin from the tray coexists with the first — each pin is an
  independent floating window.

### Why this scope, and what's explicitly out

ROADMAP groups these two deliverables under "Phase 3" because both are
the "keep the result visible" layer that follows the editor. The
scaffolding is already in place: `EditorShell::run_pin` is declared
(`crates/flintshot-core/src/shell.rs:44`), `Request::PinLast` /
`Request::OpenLast` / `Event::Pinned` are in the IPC schema
(`crates/flintshot-core/src/ipc.rs:58-60,101`), `flintshot pin` is a
stub subcommand (`crates/flintshot-app/src/cli.rs:322`), and
`flintshot-cosmic` is a stub binary that already links `libcosmic`
(`crates/flintshot-cosmic/src/main.rs:13`). This effort fills in the
behavior behind those stubs.

**Out of scope** — kept tight so the two deliverables land together:

- **Upload** (Imgur et al.) → Phase 4. The `UploadBackend` trait and
  `Request::UploadLast` already exist; this effort does not implement
  any backend.
- **Live overlay** (Flameshot-style fullscreen draw-in-place) → Phase 5.
- **Wayland global hotkeys** (`ashpd` portal) → Phase 6.
- **Move/resize of committed annotations, tool keyboard shortcuts,
  palette polish** → the deferred "polish" effort (Phase 2b spec line 67).
- **`cosmic-text` migration** → own effort; the editor stays on
  `ab_glyph` per ADR-0017.
- **macOS/Windows packaging** — pin-to-desktop *works* cross-platform,
  but installer/signing/notarization is post-1.0.

## Scope (in / out)

### Pin-to-desktop

**In:**

- **`EguiShell::run_pin` real implementation** (`flintshot-editor-egui`):
  a borderless, always-on-top, transparent eframe viewport that blits
  the image at 1:1 and supports the UX features below. Replaces the
  current `FlintshotError::NotImplemented` stub
  (`crates/flintshot-editor-egui/src/shell.rs:41`).
- **Pin window UX** (all five, per spec confirmation):
  1. *Always-on-top + draggable* — floats above other windows; drag
     anywhere on the image to move it.
  2. *Close on click/double-click/Esc* — double-click anywhere, or
     press `Esc`, closes the pin.
  3. *Hover-opacity* — semi-transparent (configurable, default `0.5`)
     until the cursor enters the pin, then fully opaque. Matches
     Flameshot.
  4. *Resize handle* — drag a corner/edge to resize (aspect-ratio
     locked); the image scales, not crops.
  5. *Copy-on-close* — when enabled (config flag, default `off`), the
     pinned image is placed on the clipboard as the window closes.
- **`flintshot pin <path>` CLI** — real body: load the image, place the
  pin window (centered by default; `--at X,Y` / `--size WxH` optional),
  run the pin event loop. Replaces the path-touching stub
  (`cli.rs:322`).
- **Editor "Pin" button** — new toolbar button in the editor that
  composes the final image (background + annotations, via the existing
  `compose_final`), writes it to a temp file, and spawns `flintshot pin
  <tmpfile>` as a detached child; the editor then closes. This reuses
  the CLI worker path end-to-end.
- **Daemon support for `Request::PinLast` / `Request::OpenLast`** — the
  daemon remembers the last-saved path (see Open Questions #1 for the
  worker→daemon reporting mechanism) and spawns `flintshot pin
  <last-path>` / re-opens it in the editor. Emits `Event::Pinned { id }`
  with a daemon-assigned monotonic id.
- **`[pin]` config section** — `default-opacity` (`0.5`), `copy-on-close`
  (`false`), `default-position` (`"center"`), read alongside the
  existing `[ui]`/`[capture]`/`[hotkeys]` sections.
- **`Event::Pinned { id }` broadcast** on pin-worker spawn (mirrors the
  Phase 2 `CaptureStarted`/`EditorOpened` optimistic-emit pattern).

### COSMIC applet

**In:**

- **`flintshot-cosmic` real applet** — replaces the stub `main()`
  (`crates/flintshot-cosmic/src/main.rs:13`) with a `cosmic::applet`
  panel button that opens a popup. Built on `libcosmic`'s applet
  template (`cosmic::app::Daemon` / `cosmic::Application`).
- **Popup menu mirroring the tray** — *Capture region*, *Capture
  fullscreen*, *Fullscreen on monitor →* (per-monitor submenu via
  `flintshot_core::monitors()`), *Pin last*, *Open last*, *Open config*,
  *Quit*. Each sends the matching `Request` over IPC.
- **Live event stream in popup** — a persistent `Request::Subscribe`
  connection reads `Event` frames; the popup shows the latest status
  line (e.g. *"Idle"*, *"Capturing…"*, *"Saved /tmp/x.png"*, *"Pinned
  #3"*). Decouples cleanly because the IPC server already converts a
  `Subscribe` connection into a pure `Event` sink
  (`crates/flintshot-app/src/ipc_server.rs:272`).
- **Applet-as-IPC-client** — the applet is a pure client: it connects
  to the daemon socket, sends `Request`s, and reads the `Event` stream.
  It performs no capture, runs no editor. If no daemon answers, the
  popup shows a clear *"daemon not running"* state with a *Start
  daemon* hint (it does not auto-start one — out of scope).

### Out (deferred):

- Upload (Phase 4), live overlay (Phase 5), Wayland hotkeys (Phase 6).
- Annotation move/resize, tool shortcuts, palette polish.
- `cosmic-text` migration (stays `ab_glyph`).
- Auto-starting the daemon from the applet (systemd/cosmic-autostart
  integration is packaging work, post-1.0).
- Pin window *annotation editing* — a pin is a read-only float; to edit
  you re-open in the editor (*Open last*).

## Deliverables

### Pin-to-desktop

- [ ] `EguiShell::run_pin` opens a borderless always-on-top transparent
      viewport; blits the image 1:1; supports drag-move,
      double-click/Esc close, hover-opacity, aspect-locked resize,
      copy-on-close (gated on config).
- [ ] `flintshot pin <path>` CLI loads the image and runs the pin
      window; `--at X,Y` / `--size WxH` optional; `--no-clipboard` /
      `--opacity N` overrides.
- [ ] Editor toolbar gains a **Pin** button that composes the final
      image and spawns `flintshot pin <tmpfile>`; the editor closes
      after spawning.
- [ ] Daemon handles `Request::PinLast` (spawn `flintshot pin
      <last-saved>`, emit `Event::Pinned { id }`) and `Request::OpenLast`
      (re-open the last-saved image in the editor).
- [ ] Daemon tracks the last-saved path (mechanism decided in
      `/planning`; see Open Questions #1).
- [ ] `[pin]` config section wired into `Config::load` / `Config::save`
      (comment-preserving, like `[ui]`/`[hotkeys]`).
- [ ] `no_gui_types_in_core.rs` stays green; the pin *window* logic
      lives in `flintshot-editor-egui`, the *dispatch* in
      `flintshot-app`, the *types* in `flintshot-core`.

### COSMIC applet

- [ ] `flintshot-cosmic` builds as a COSMIC panel applet (not a stub);
      `cosmic-build` CI job stays green on Linux.
- [ ] Panel button opens a popup with the full action set (capture,
      per-monitor, pin/open last, config, quit).
- [ ] Popup shows the live `Event` stream from a `Subscribe`
      connection; handles `Lagged`/disconnect gracefully.
- [ ] Popup shows a clear *"daemon not running"* state when the socket
      is absent / connection refused.
- [ ] Applet depends only on `flintshot-core` (for `ipc` types +
      `monitors()`) + `libcosmic` — no `egui`, no `flintshot-app`.

### Shared

- [ ] `CHANGELOG.md` `[Unreleased]` entries for both deliverables.
- [ ] ADR for the pin window-architecture decision (one-worker-per-pin
      vs. multi-viewport) and the last-saved-path tracking mechanism.
- [ ] `docs/TEST_MATRIX.md` rows for pin (drag, close, opacity, resize,
      copy-on-close) and the COSMIC applet (each action, event feed,
      daemon-absent state).

## Exit criteria

A user, in a single session with `flintshot daemon` running:

1. Runs `flintshot region`, annotates, clicks **Pin** in the editor
   toolbar; the editor closes and the composed image floats as an
   always-on-top, semi-transparent window.
2. Drags the pin to a corner of the screen; it moves. Hovers it; it
   goes fully opaque. Resizes it via the corner handle; the image
   scales (no crop). Double-clicks; it closes (and, with
   `copy-on-close = true`, is on the clipboard).
3. From the COSMIC panel `flintshot` button, clicks *Capture fullscreen
   → monitor 2*; the popup shows *"Capturing… → Editor opened"*. They
   save, and the popup updates to *"Saved /tmp/flintshot-…png"*.
4. From the popup, clicks *Pin last*; the just-saved image floats as a
   second pin window, coexisting with any earlier pin.
5. From the popup, clicks *Open last*; the last-saved image re-opens in
   the editor for further annotation.
6. Stops the daemon (`flintshot daemon --replace` or tray/applet
   *Quit*); the COSMIC popup shows *"daemon not running"* without
   crashing.

The abstraction holds: pin rendering routes through
`flintshot-core::render` (the pin window is "render one image, no
annotations"); the COSMIC applet touches only `flintshot-core::ipc`;
`no_gui_types_in_core.rs` stays green.

## Technologies

No new crates. Both deliverables fit the existing workspace. The
substrate already present:

| Concern                        | Crate / type                                      | Status                                             |
| ------------------------------ | ------------------------------------------------- | -------------------------------------------------- |
| Pin window lifecycle           | `EditorShell::run_pin` (`flintshot-core::shell`)  | Trait method declared; impl is the stub to replace |
| Pin rendering                  | `flintshot-core::render::render` + `Canvas`       | Already blits a background image (Phase 1 Task 4)  |
| Image composition              | `flintshot-core::compose_final`                   | Already bakes annotations (Phase 1 Task 9)         |
| Pin viewport                   | `eframe` 0.35 `ViewportBuilder`                   | `always_on_top`/`decorations`/`transparent`/`with_position` available |
| Clipboard (copy-on-close)      | `arboard` (already a workspace dep)               | Editor Copy path uses it (Phase 1 Task 9)          |
| Temp file for editor→pin handoff | `tempfile` (already a dev-dep; promote to dep)  | —                                                  |
| IPC client (COSMIC applet)     | `interprocess` + `postcard` + `flintshot-core::ipc` | Daemon + CLI already use this stack               |
| Subscribe stream               | `Request::Subscribe` → `Event` sink               | Already implemented (`ipc_server.rs:272`)          |
| Monitor enumeration            | `flintshot_core::monitors()` / `MonitorInfo`      | Tray submenu already uses it (`tray.rs:114`)       |
| COSMIC toolkit                 | `libcosmic` (git `master`, `tokio`+`wayland`+`multi-window`) | Already a dep of `flintshot-cosmic`         |
| Errors                         | `thiserror` (core) / `color_eyre` (app)           | Existing convention                                |

If `/planning` finds the last-saved-path tracking needs a new
workspace dep (it should not — see Open Questions #1), surface it then
(ask-first boundary). `tempfile` moving from `[dev-dependencies]` to
`[dependencies]` in `flintshot-app` is pre-approved (it is already in
the workspace dep set).

## Commands

All run from the repo root. Full reference in
[`AGENTS.md`](../../../AGENTS.md) §Commands; the standing gate is the
source of truth.

```sh
# Build (default-members: flintshot-app + deps)
cargo build

# The Phase 1 entry points — exercise pin via the editor toolbar
cargo run -- region
cargo run -- fullscreen

# Direct pin (after Task lands the CLI body)
cargo run -- pin /path/to/image.png
cargo run -- pin /path/to/image.png --at 100,200 --opacity 0.7

# COSMIC applet (Linux only; needs libcosmic system deps)
cargo build -p flintshot-cosmic
cargo run -p flintshot-cosmic

# Gate — run in order before declaring done
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

Pin window testing needs a display server. Headless CI exercises pin
logic through pure-core tests (image load, config parsing,
dispatch-table unit tests with `RecordingSpawner`) and the COSMIC applet
through IPC-client unit tests against an in-process `IpcServer` (no
libcosmic UI in the loop). `#[ignore]`'d integration tests cover the
visual pin + the live COSMIC popup under `xvfb`, mirroring Phase 1's
capture smoke-test handling.

## Project structure

No new crates, no workspace-shape changes. Crate boundaries from
[`ARCHITECTURE.md`](../../ARCHITECTURE.md) hold. Touched files:

```
crates/flintshot-core/src/
├── shell.rs          # UNCHANGED — run_pin signature already correct
├── config.rs         # EXTEND — new [pin] section (PinConfig)
├── ipc.rs            # UNCHANGED — PinLast/OpenLast/Pinned already declared
└── lib.rs            # re-export PinConfig

crates/flintshot-editor-egui/src/
├── shell.rs          # EXTEND — real run_pin; spawns the pin viewport
├── pin.rs            # NEW — PinApp (eframe::App): blit, drag, hover-opacity,
│                     #         resize handle, Esc/double-click close, copy-on-close
└── lib.rs            # pub use pin module

crates/flintshot-app/src/
├── cli.rs            # EXTEND — real run_pin body; PinArgs gains --at/--size/…
├── worker.rs         # EXTEND — PinLast/OpenLast dispatch (spawn `pin`/editor);
│                     #         last-saved-path tracking
├── daemon.rs         # EXTEND — track last-saved; thread the path to PinLast/OpenLast
├── ipc_server.rs     # EXTEND if worker-result-reporting needs a server-side change
└── editor pin spawn  # editor toolbar Pin button → compose_final → temp file → spawn

crates/flintshot-cosmic/src/
├── main.rs           # REWRITE — cosmic applet entry (cosmic::applet template)
├── app.rs            # NEW — cosmic Application: popup menu, action dispatch
├── ipc.rs            # NEW — IPC client: send Request, read Event stream
└── (popup widgets)   # NEW — status line, monitor submenu, daemon-absent state

crates/flintshot-core/tests/
└── no_gui_types_in_core.rs   # UNCHANGED but must stay green; extend to also
                              # reject `use libcosmic` / `use cosmic` if not already
```

Rationale: pin-to-desktop is the third `EditorShell` impl method (after
`run_editor`); it slots into the existing `flintshot-editor-egui` three-
module layout as a sibling `pin.rs`. The COSMIC applet is an entirely
separate binary that is a *client* of the daemon — it depends on
`flintshot-core` for types and on `libcosmic` for UI, and nothing else
from this workspace.

## Code style

Unchanged from [`AGENTS.md`](../../../AGENTS.md) §Conventions and the
`.opencode/tech/rust/` docs (loaded at `/build` time). Two shapes worth
previewing — the surfaces this effort adds:

**Pin viewport construction** — mirrors the editor's
`run_editor_blocking` but with a different `ViewportBuilder`:

```rust
// crates/flintshot-editor-egui/src/shell.rs (illustrative)
// Pin: borderless, always-on-top, transparent, undecorated, positioned.
let viewport = egui::ViewportBuilder::default()
    .with_title("flintshot — pin")
    .with_decorations(false)
    .with_always_on_top()
    .with_transparent(true)
    .with_resizable(true)
    .with_inner_size(logical_size)
    .with_position(position);
```

**COSMIC applet IPC client** — the applet is a pure client; it never
imports `flintshot-app`. The framing helpers (`read_frame`/`write_frame`)
currently live in `flintshot-app::ipc_server` and are `pub(crate)`; the
COSMIC applet needs its own (small) framing copy or a shared helper.
`/planning` decides: duplicate ~30 LOC in `flintshot-cosmic::ipc`, or
promote the framing helpers to a `flintshot-core::ipc::wire` module.
Recommend duplicate-in-applet (keeps core transport-free per ADR-0010's
"schema in core, transport in bins" split) — flag in `/planning`.

Conventions this effort locks in (all already standing): every public
type `#[non_exhaustive]`-safe, doc comments on all public items, no
`unwrap()`/`expect()` outside tests, comments only when the "why" is
non-obvious, errors via `thiserror` (core) / `color_eyre` (app).

## Testing strategy

Mirrors [`ARCHITECTURE.md`](../../ARCHITECTURE.md) §Testing strategy and
the Phase 1/2 split: heavy headless unit coverage in core + app dispatch
(`RecordingSpawner`); toolkit-specific concerns in
`flintshot-editor-egui` and `flintshot-cosmic`.

### Unit — `flintshot-app` (headless, no display)

- **Worker dispatch for `PinLast`/`OpenLast`** — extend the
  `RecordingSpawner` tests in `worker.rs`: `PinLast` spawns `pin
  <last-path>` and emits `Event::Pinned { id }`; `OpenLast` spawns the
  editor on the last path. Ids are monotonic across consecutive pins.
- **Last-saved-path tracking** — whatever mechanism `/planning` picks,
  unit-test the state machine: `Saved { path }` observed → last path
  updated; `PinLast` before any save → `Event::Error { code:
  "no-last-capture" }`.
- **`flintshot pin` arg parsing** — `--at`, `--size`, `--opacity`,
  `--no-clipboard` parse to the expected config; defaults apply when
  absent.

### Unit — `flintshot-editor-egui` (headless where possible)

- **`PinApp` construction** — given an image + rect + `PinConfig`, the
  `eframe::App` builds without panicking; `hover_opacity` /
  `copy_on_close` flags flow through.
- **Pin close logic** — Esc / double-click set the close flag; the
  copy-on-close path is exercised via the existing `arboard` seam
  (mirror the editor Copy test).

### Unit — `flintshot-cosmic` (IPC client, no UI)

- **IPC client framing** — `Request` round-trips through the client's
  `read_frame`/`write_frame` against an in-process `IpcServer` (the
  existing test harness in `ipc_server.rs` is the template).
- **Event stream handling** — a `Subscribe` connection receives the
  expected `Event`s in order; `Lagged` is handled without panicking;
  disconnect reconnects.
- **Daemon-absent detection** — connection-refused / missing-socket
  surfaces the *"daemon not running"* state (mirror
  `ipc_client::is_no_daemon`).

### Unit — `flintshot-core`

- **`PinConfig` (de)serialization** — `[pin]` section round-trips
  through `Config::load`/`save`; missing section uses defaults;
  comment-preserving write leaves unrelated sections intact (mirror the
  Phase 2 `HotkeyConfig` tests).
- **IPC roundtrip** — `Request::PinLast` / `OpenLast` and
  `Event::Pinned { id }` already round-trip (Phase 2 tests); ensure no
  regression.

### Boundary

- `no_gui_types_in_core.rs` unchanged and green. Extend the scanner to
  also reject `use cosmic` / `use libcosmic` / `use iced` if it doesn't
  already (it currently checks `egui`/`interprocess`/`global_hotkey`/
  `tray_icon`/`notify`). The COSMIC applet is a *binary* crate — its
  libcosmic use never reaches core.

### Manual test matrix

Tracked in [`docs/TEST_MATRIX.md`](../../TEST_MATRIX.md); Phase 3 adds
rows for pin (drag, hover-opacity, resize, double-click/Esc close,
copy-on-close, multi-pin coexistence, cross-monitor drag on Linux/macOS/
Windows) and the COSMIC applet (each popup action, event feed ordering,
daemon-absent state, per-monitor submenu correctness).

## Boundaries

- **Always:**
  - Run the four-command gate (`fmt`, `clippy`, `test`, doc tests)
    before declaring done.
  - Route pin image rendering through `flintshot-core::render` — the
    pin window is "render one image, no annotations", reusing the
    Phase 1 background-blit path. **No direct `egui::Painter` calls
    that bypass `render()` for image content** (locked by ADR-0012).
  - Keep the COSMIC applet a pure IPC client — it must not import
    `flintshot-app`, `egui`, or `flintshot-editor-egui`. It depends on
    `flintshot-core` (types) + `libcosmic` only.
  - Preserve Phase 1/2/2b behavior bit-for-bit (the editor, the daemon,
    tray, hotkeys, all annotation tools).
  - Spawn each pin as its own worker process (`flintshot pin <path>`)
    per ADR-0019 — a pin crash must never kill the editor or daemon,
    and multiple pins coexist as independent processes.
  - Keep `flintshot-cosmic` out of `default-members` (its libcosmic
    system deps would break the default CI build); it stays in its own
    `cosmic-build` CI job.

- **Ask first:**
  - Promoting IPC framing helpers (`read_frame`/`write_frame`) out of
    `flintshot-app::ipc_server` into `flintshot-core` — touches the
    core boundary. Default is to duplicate ~30 LOC in the applet.
  - Adding any workspace dependency not already present (e.g. a
    temp-file helper beyond `tempfile`). `tempfile` moving from
    dev-dep to dep in `flintshot-app` is pre-approved.
  - Changing the `EditorShell` trait shape (e.g. `run_pin` signature) —
    the current `(image, rect)` is presumed sufficient; surface during
    `/planning` if not.
  - Changing CI config (`.github/workflows/ci.yml`) — e.g. adding
    libcosmic system deps to a new job.
  - Pinning `libcosmic` to a specific git commit (vs. tracking
    `master`) — flag if `master` breaks during the effort.
  - New ADRs — draft during `/planning`, flag for review.

- **Never:**
  - Introduce `egui` / `libcosmic` / `iced` / `slint` types into
    `flintshot-core`'s public API (boundary test enforces; extend it to
    cover `cosmic`/`libcosmic`).
  - `unwrap()` / `expect()` outside tests and `const` contexts.
  - Silence a clippy warning with `#[allow]` without a justifying
    comment.
  - Add upload, live-overlay, or annotation-polish features under this
    effort's banner — keep the slice honest.
  - Make the COSMIC applet auto-start the daemon — packaging/autostart
    is post-1.0.
  - Have the daemon perform capture/editor/pin work in-process —
    ADR-0019 (worker spawn) is non-negotiable.

## Success criteria

- **Pin-to-desktop** works end-to-end from three entry points: editor
  Pin button, `flintshot pin <path>` CLI, and daemon `PinLast` (tray /
  COSMIC applet). All five UX features (always-on-top+drag,
  click/Esc/double-click close, hover-opacity, resize,
  copy-on-close) behave as specified.
- **Multiple pins coexist** — each is an independent floating window in
  its own worker process.
- **COSMIC applet** builds as a panel applet, opens a popup with the
  full action set, shows the live event feed, and degrades gracefully
  when the daemon is absent. It is a pure IPC client (no `flintshot-app`
  / `egui` dependency).
- **Daemon `PinLast`/`OpenLast`** resolve the last-saved capture and
  spawn the right worker; `Event::Pinned { id }` broadcasts with a
  monotonic id.
- **Abstraction holds**: pin rendering routes through
  `flintshot-core::render`; the COSMIC applet touches only
  `flintshot-core::ipc`; `no_gui_types_in_core.rs` stays green (extended
  to reject `cosmic`/`libcosmic`).
- The four-command gate is green on Linux; `cargo build` green on macOS
  and Windows for the default-members; `cargo build -p flintshot-cosmic`
  green on Linux (cross-compile smoke where a native run isn't possible
  in CI).

## Open questions

1. **Last-saved-path tracking mechanism.** `Request::PinLast` /
   `OpenLast` need the daemon to know the last-saved path. Today the
   worker is fire-and-forget — it never reports `EditorOutcome::Saved`
   back over IPC (the daemon emits `EditorOpened` optimistically on
   spawn). Options:
   - **(a)** Worker reports back: after the editor closes, the worker
     connects to the daemon socket and sends a result message (new
     `Request` variant, e.g. `Request::WorkerResult { saved:
     Option<PathBuf> }`, or a dedicated worker-only message). Daemon
     keeps the last path. Clean, but adds a worker→daemon channel.
   - **(b)** Well-known cache path: every save also writes
     `~/.cache/flintshot/last.png`; `PinLast`/`OpenLast` read from
     there. Simple, but loses the user's chosen filename for *Open
     last* and doubles disk writes.
   - **(c)** Editor Pin button is the primary path (bypasses daemon
     tracking); `PinLast` uses (a) or (b) for the daemon-initiated path.
   Recommend **(a)** — it closes the Phase 2 "worker never reports back"
     gap generally (also unblocks Phase 4 upload-from-last) and is the
     most honest model. Decide during `/planning`.

2. **Pin window: one-worker-per-pin vs. multi-viewport.** The simplest
   model (and the one that matches ADR-0019) is one worker process per
   pin window — `flintshot pin <path>` opens exactly one viewport.
   Alternative: a single long-lived "pin manager" worker opens multiple
   eframe viewports. Recommend **one-worker-per-pin** — simpler, a pin
   crash is isolated, and eframe multi-viewport is younger than single.
   Confirm during `/planning`.

3. **Pin positioning default.** `flintshot pin <path>` with no `--at`:
   center of the active monitor, or the cursor position, or the
   top-left of the source capture? Flameshot pins at the capture's
   original location. Recommend **center of the active monitor** for
   CLI invocation (no capture-origin context) and **capture origin**
   for the editor Pin button + daemon PinLast (where the origin is
   known). Decide during `/planning`.

4. **Hover-opacity UX detail.** Does the pin snap to full opacity on
   *any* pointer entry (Flameshot), or only when the user is about to
   interact (e.g. on a deliberate hover delay)? Recommend **instant on
   pointer-enter, instant on leave** — matches Flameshot, simplest to
   reason about. The opacity value is already configurable
   (`[pin] default-opacity`). Confirm during `/planning`.

5. **COSMIC applet: tray-feature-parity for *Open config*.** The tray
   opens `config.toml` in `$EDITOR` via the daemon's main thread. The
   COSMIC applet is a separate process; should it shell out to
   `$EDITOR` itself, or send a new `Request::OpenConfig` that the
   daemon services? Recommend **send a request** (centralises the
   behaviour; the daemon already knows how to open config for the
   tray). This may add one new `Request` variant — flag in `/planning`.

6. **Pin window `rect` parameter semantics.** `EditorShell::run_pin(
   image, rect)` — is `rect` the window geometry (position + size) or
   a sub-rect of the source image? ARCHITECTURE.md says "showing `image`
   at `rect`", implying *window geometry*. Recommend **window geometry
   in physical px** (`min` = position, `max - min` = size); the pin
   shows the whole image scaled to that size. Confirm during `/planning`
   — if it needs to be a sub-rect, the signature may widen.
