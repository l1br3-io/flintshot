---
status: active
started: 2026-08-04
---

# Spec — Phase 3: Pin-to-desktop

> Draft effort spec. For product context, see
> [`../../VISION.md`](../../VISION.md); for sequencing, see
> [`../../ROADMAP.md`](../../ROADMAP.md); for the technical substrate
> this builds on, see [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md)
> (especially §Process model, §UI abstraction layer, §`EditorShell`
> trait). The shipped Phase 2 spec lives at
> [`../phase-2-daemon-tray-hotkeys/spec.md`](../phase-2-daemon-tray-hotkeys/spec.md);
> Phase 2b at [`../phase-2b-annotation-tools/spec.md`](../phase-2b-annotation-tools/spec.md).

## Objective

After Phase 2, `flintshot` is a resident desktop tool — daemon, tray,
hotkeys, IPC. But its only outputs are Save (write a PNG) and Copy
(clipboard). Phase 3 adds the third output from
[`VISION.md`](../../VISION.md) §Outputs: **pin as a floating
always-on-top window**. A pin lets the user keep a reference capture
visible while they work — the canonical use case is grabbing a snippet
of one window to compare against another, or holding on to a code
snippet from a now-closed dialog.

This effort **only** builds pin-to-desktop. The roadmap's "Phase 3 —
Pin-to-desktop + COSMIC applet" milestone splits in two, matching the
Phase 2 → Phase 2b precedent: this effort (`phase-3-pin-to-desktop`)
lands the pin primitive, the CLI subcommand, the editor Pin button, and
the IPC `PinLast` path. A separate `phase-3b-cosmic-applet` effort will
consume both this phase's pin work and Phase 2's IPC server to build
the COSMIC panel applet. Splitting keeps each effort reviewable and
avoids dragging `libcosmic`'s system deps into every pin-related task.

**Target users:** the same power users from
[`VISION.md`](../../VISION.md) §Target users. Pin-to-desktop is the
feature that distinguishes a "screenshot tool" from a "screenshot +
annotation tool" — Flameshot and Snipaste users specifically.

**Success looks like:**

1. The user captures a region, clicks *Pin* in the editor toolbar
   (alongside *Save* / *Copy*), and the editor closes as a borderless
   always-on-top floating window appears showing exactly the composed
   image (background + all annotations). The pin persists after the
   editor exits.
2. The user drags the pin around the desktop by clicking and dragging
   anywhere on the image. Multiple pins coexist independently.
3. The user closes a pin with `Esc` or the window close button.
4. The user runs `flintshot pin ~/foo.png` from the CLI and the same
   kind of floating window appears for an arbitrary on-disk image.
5. A daemon-driven capture (from the tray or a hotkey) whose editor
   saved the result can be re-pinned without re-opening the editor:
   `PinLast` (over IPC, or from a future tray item / the Phase 3b
   applet) opens a pin for the most recently saved capture.
6. On Wayland, where always-on-top is compositor-dependent, the pin
   still opens as a borderless window; the docs are honest about the
   always-on-top caveat rather than pretending.

### Why this scope, and what's explicitly out

This effort builds the pin primitive once and routes three entry points
through it: `flintshot pin <path>` (CLI), the editor *Pin* button
(GUI), and `Request::PinLast` (IPC, for the daemon / tray / future
applet). Each entry point converges on the same `EguiShell::run_pin`
implementation and (for the latter two) the same worker-spawn model the
daemon already uses for captures. The pin window is its own process —
an editor crash or daemon restart never kills an existing pin, and the
pin's event loop never contends with the editor's.

**Out (deferred):**

- COSMIC panel applet → `phase-3b-cosmic-applet` (consumes this phase's
  `PinLast` + Phase 2's IPC server)
- Layer-shell integration for "true" always-on-top on wlroots compositors
  → documented caveat in this phase, real fix deferred (would pull in
  `smithay-client-toolkit` and is compositor-specific)
- Pin-to-desktop from the tray menu (a *Pin last* tray item) → small
  follow-up once `PinLast` is wired; not blocking
- Per-pin context menu (right-click for close-all, copy, save-as) →
  future polish effort
- Pin window transforms (resize handles, rotate, opacity slider) →
  future polish; resize is supported via the WM, explicit handles aren't
- Upload → Phase 4
- Live overlay → Phase 5

## Scope (in / out)

**In:**

- `EguiShell::run_pin(image, rect)` — real implementation. Borderless,
  always-on-top, decorations-off eframe viewport sized to the image;
  blits the image 1:1; draggable by clicking anywhere on the image;
  `Esc` and the window-close button dismiss it. Multiple concurrent
  pins are independent processes.
- `flintshot pin <path>` — real CLI body. Loads the image, calls
  `EguiShell::run_pin`. The pin's process exits when the window closes.
- Editor *Pin to desktop* toolbar button — fourth output path alongside
  *Save* / *Copy*. Composes the final image (background + all committed
  annotations, same `compose_final` source of truth), writes it to a
  daemon-managed pin-cache file, spawns `flintshot pin <path>` detached,
  and returns a new `EditorOutcome::Pinned(PathBuf)` variant. The
  editor then closes; the pin process persists.
- `Request::PinLast` — daemon resolves its last-tracked saved path,
  spawns `flintshot pin <path>` as a worker, and emits `Event::Pinned
  { id }` with a per-daemon monotonic id. Unsupported when no capture
  has been saved yet (clear `Error { code: "no-last-capture" }`).
- Worker→daemon outcome reporting — the worker (on `EditorOutcome::Saved`
  or `EditorOutcome::Pinned`) sends a new fire-and-forget
  `Request::ReportSaved { path }` to the daemon. The daemon stores the
  last saved/pinned path and broadcasts `Event::Saved { path }` /
  `Event::Pinned { id }` to subscribers. This fills in the currently
  proxy `EditorOpened` → real outcome reporting for the subscriber
  stream, and is the mechanism `PinLast` reads.
- Pin-cache directory — `flintshot` writes pin-bound temp files to a
  well-known cache dir (`$XDG_CACHE_HOME/flintshot/pins/` on Linux,
  platform equivalent elsewhere). Cleaned opportunistically on daemon
  start (best-effort; never blocks).
- The `no_gui_types_in_core` boundary test stays green — no new GUI or
  transport types enter `flintshot-core`'s public API. The pin primitive
  is an `EditorShell` trait method already declared; only its impl moves
  into `flintshot-editor-egui`.

**Out (deferred):**

- COSMIC panel applet → `phase-3b-cosmic-applet`
- Layer-shell / wlroots always-on-top → documented caveat
- Tray *Pin last* menu item → follow-up
- Per-pin right-click context menu → future polish
- Pin resize handles / rotate / opacity → future polish (WM resize is
  supported; explicit handles aren't)

## Deliverables

- [ ] `flintshot-editor-egui::EguiShell::run_pin` real implementation:
      borderless always-on-top eframe viewport, image blit, drag-anywhere,
      `Esc`/close dismiss. New `pin_window` module in
      `flintshot-editor-egui`.
- [ ] `flintshot pin <path>` real CLI body in `cli.rs::run_pin`: load
      image (any format `image` decodes; RGBA-converted), call
      `run_pin(image, Rect::at_origin_with_dims)`. `--help` updated to
      drop the "stub" framing.
- [ ] `EditorOutcome::Pinned(PathBuf)` new variant in
      `flintshot-core::shell`. The path is the pin-cache file the
      editor wrote (so the daemon / future applet can re-pin it).
- [ ] Editor *Pin* button in `flintshot-editor-egui::app`: composes
      final image via `compose_final`, writes to a pin-cache file
      (`pin-cache_dir().join(format!("pin-{timestamp_ns}.png"))`),
      publishes `EditorOutcome::Pinned(path)` to the outcome slot,
      closes the editor. Existing Save/Copy paths unchanged.
- [ ] `flintshot-app::cli::run_editor_for_capture` handles
      `EditorOutcome::Pinned(path)` by spawning `flintshot pin <path>`
      detached (reusing the `worker::Spawner` abstraction / a shared
      `spawn_detached` helper) and logging.
- [ ] `flintshot-core::ipc::Request::ReportSaved { path: PathBuf }`
      new variant (forward-compat; `#[non_exhaustive]` already on the
      enum). Postcard roundtrip test extended.
- [ ] `flintshot-app::worker`: the worker process, after `run_editor`
      returns `Saved(p)` or `Pinned(p)`, fire-and-forget sends
      `Request::ReportSaved { path: p }` to the daemon socket (reusing
      `ipc_client::try_delegate`'s connect path). Failure to report is
      non-fatal (logged at `warn`) — standalone captures (no daemon)
      just skip it.
- [ ] `flintshot-app::daemon` / `ipc_server`: handle
      `Request::ReportSaved` by storing the path in a daemon-owned
      `last_saved: Arc<Mutex<Option<PathBuf>>>` and broadcasting
      `Event::Saved { path }` to subscribers. Handle `Request::PinLast`
      by reading `last_saved`, spawning `flintshot pin <path>` via the
      worker dispatch, and broadcasting `Event::Pinned { id }` (id from
      a per-daemon `AtomicU64`). Empty `last_saved` → broadcast
      `Event::Error { code: "no-last-capture" }`.
- [ ] Pin-cache directory helper in `flintshot-core::config` (or a new
      `flintshot-core::paths` module if that's a cleaner home):
      `pin_cache_dir() -> Option<PathBuf>` resolving to
      `$XDG_CACHE_HOME/flintshot/pins/` (Linux),
      `~/Library/Caches/flintshot/pins/` (macOS),
      `%LOCALAPPDATA%\flintshot\pins\` (Windows). Best-effort create.
- [ ] `no_gui_types_in_core.rs` boundary test still green (no changes
      expected; the pin primitive is a trait method, not a core type).
      Add an assertion that `EditorOutcome::Pinned` is `#[non_exhaustive]`-
      safe (the wildcard arm in `run_editor_for_capture` covers it).
- [ ] Cross-platform notes: Wayland always-on-top caveat (borderless
      works; always-on-top is compositor-dependent — layer-shell is the
      real fix, deferred); macOS borderless-override behavior; Windows
      always-on-top works. Added to README / AGENTS cross-platform
      section + `docs/TEST_MATRIX.md` rows.
- [ ] `CHANGELOG.md` `[Unreleased]` entries; ADR for the
      pin-as-separate-process / worker-spawn model and for the
      worker→daemon outcome-reporting addition (drafted during
      `/planning`).

## Exit criteria

A user can:

1. `flintshot region`, annotate, click *Pin* in the editor — a
   borderless floating window appears showing the composed image, the
   editor closes, and the pin persists.
2. Drag the pin around the desktop; it stays on top of other windows
   (on X11 / macOS / Windows; documented Wayland caveat).
3. Open multiple pins (from multiple captures or repeated `flintshot pin`
   invocations); each is independently draggable and closable.
4. `Esc` or the close button dismisses a pin; the process exits cleanly.
5. `flintshot pin ~/any-image.png` opens the same kind of floating
   window for an on-disk image.
6. With the daemon running: `flintshot region` (delegated), save in the
   editor → an external IPC subscriber observes `Event::Saved { path }`.
   Then a `PinLast` request over IPC opens a pin for that path and the
   subscriber observes `Event::Pinned { id }`.
7. `PinLast` with no prior save returns a clear `Error {
   code: "no-last-capture" }` event, not a silent no-op or a panic.
8. The four-command gate (`fmt`, `clippy`, `test`, doc tests) is green;
   `flintshot-cosmic` still builds (untouched stub).

The abstraction holds: no `eframe` / `egui` types leak into
`flintshot-core`'s public API; the pin primitive is an `EditorShell`
trait method whose impl lives in `flintshot-editor-egui`. The boundary
test stays green.

## Technologies

Following [`ARCHITECTURE.md`](../../ARCHITECTURE.md) §Tech stack — no
new crates. The pin window is a second use of eframe's `ViewportBuilder`
(already the Phase 1 editor's substrate); the worker-spawn model is the
Phase 2 daemon's existing pattern; the IPC additions are one new
`Request` variant on the existing postcard+interprocess transport.

| Concern                      | Source                                 | Notes                                                       |
| ---------------------------- | -------------------------------------- | ----------------------------------------------------------- |
| Pin window                   | `eframe` / `egui::ViewportBuilder`     | `always_on_top(true)`, `decorations(false)`, `resizable(true)`, `inner_size = image dims`. Same crate, same version as the editor. |
| Image blit in pin            | `flintshot-core::render::Canvas`       | Reuse the existing `EguiCanvas::upload_image` / `draw_image` path (no new primitive). The pin renders one image, no annotations. |
| Pin process spawn (editor)   | `std::process::Command` + `current_exe`| Same pattern as `flintshot-app::worker::CommandSpawner`; factored into a shared helper if a second caller appears. |
| Pin process spawn (daemon)   | `flintshot-app::worker::CommandSpawner`| Identical to the capture-worker path; `PinLast` adds a `pin` argv builder. |
| Worker→daemon report         | `flintshot-app::ipc_client` + `Request::ReportSaved` | Fire-and-forget over the existing socket; non-fatal on connect failure. |
| Daemon-side last-saved state | `Arc<std::sync::Mutex<Option<PathBuf>>>` + `AtomicU64` for pin ids | Shared between the IPC server and the tray/hotkey dispatch path (same pattern as `SubscriberBus`). |
| Pin-cache dir                | `flintshot-core::config` (existing `config_dir` resolver family) or new `paths` module | Platform-correct cache dir; opportunistic cleanup. |
| Image decode for `flintshot pin <path>` | `image` crate                | Already a workspace dep; reads any format, converts to `RgbaImage`. |

No new workspace dependencies. No version bumps to existing ones.

## Commands

All run from the repo root. Full reference in
[`AGENTS.md`](../../../AGENTS.md) §Commands; the standing gate is the
source of truth.

```sh
# Build (default-members: flintshot-app + deps)
cargo build

# The Phase 3 entry points
cargo run -- region                         # capture → editor → click *Pin*
cargo run -- pin path/to/image.png          # pin an on-disk image
cargo run -- daemon                         # start the daemon (for PinLast)
# Then, from another shell or an IPC test client:
cargo run -- region                         # delegated; save in editor
# → PinLast arrives over IPC from a test client (no CLI surface for it yet)

# Gate — run in order before declaring done
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

`cargo run -- pin` and `cargo run -- region` (for the Pin button) both
need a display server on Linux. Headless CI exercises pin behaviour
through unit tests of the pure pieces (`run_pin`'s viewport builder
construction, pin-cache path resolution, `ReportSaved` / `PinLast`
dispatch tables, postcard roundtrip) plus `#[ignore]`'d integration
tests that need `xvfb`, mirroring Phase 1 / 2's capture-smoke-test
convention.

## Project structure

New and touched files (no new crates; crate boundaries from
[`ARCHITECTURE.md`](../../ARCHITECTURE.md) hold):

```
crates/flintshot-core/src/
├── shell.rs               # EditorOutcome::Pinned(PathBuf) variant added
├── ipc.rs                 # Request::ReportSaved { path } variant added
├── lib.rs                 # re-export new variants (no new module)
└── config.rs              # pin_cache_dir() helper (or new paths.rs — see below)

crates/flintshot-editor-egui/src/
├── pin_window.rs          # NEW — PinApp (eframe::App): image blit, drag, Esc
├── shell.rs               # run_pin real body (was Phase 3 stub)
├── app.rs                 # Pin toolbar button + outcome publishing
└── lib.rs                 # pub use pin_window::PinApp if externally needed

crates/flintshot-app/src/
├── cli.rs                 # run_pin real body; PinOutcome handling in run_editor_for_capture
├── worker.rs              # pin_argv() builder; ReportSaved send on Saved/Pinned
├── daemon.rs              # last_saved state; PinLast dispatch wiring
├── ipc_server.rs          # handle ReportSaved + PinLast (route via worker::dispatch)
└── main.rs                # unchanged module list

crates/flintshot-core/tests/
└── no_gui_types_in_core.rs   # unchanged (pin is a trait method, not a core type)
```

Rationale: the pin window is a sibling of `EditorApp` in
`flintshot-editor-egui`, not a new crate. It reuses the editor's
`EguiCanvas` for the image blit (upload once, blit every frame — no
annotations, no toolbar, no input state machine). The daemon's pin
dispatch slots into the existing `worker::dispatch` table alongside
`CaptureRegion` / `CaptureFullscreen`. The worker→daemon report reuses
the existing `ipc_client` connect path — no new transport.

The pin-cache-dir helper placement is a minor open question (see
[§Open questions](#open-questions)); the plan will pick between extending
`config.rs` (which already owns `config_path()` / `config_dir()`) and a
new `paths.rs` module (cleaner separation if more path resolvers
appear).

## Code style

Unchanged from [`AGENTS.md`](../../../AGENTS.md) §Conventions and the
`.opencode/tech/rust/` docs (loaded at `/build` time). Two shapes worth
previewing — the new `EditorOutcome` variant (the only new public core
type) and the pin viewport builder (the only non-trivial new egui code):

```rust
// crates/flintshot-core/src/shell.rs
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditorOutcome {
    Saved(PathBuf),
    CopiedToClipboard,
    Uploaded(String),
    /// User pinned the composed image to the desktop; contains the
    /// pin-cache path the editor wrote (so the daemon / future applet
    /// can re-pin or clean it up).
    Pinned(PathBuf),
    Cancelled,
}
```

```rust
// crates/flintshot-editor-egui/src/shell.rs::run_pin
let viewport = egui::ViewportBuilder::default()
    .with_title("flintshot — pin")         // WM class hint; not rendered (decorations off)
    .with_inner_size([img_w as f32, img_h as f32])
    .with_decorations(false)
    .with_always_on_top()
    .with_resizable(true)
    .with_position(screen_rect.min.x, screen_rect.min.y); // open at capture origin
// … PinApp owns the image + cached texture, blits every frame,
// sends ViewportCommand::StartOuterDrag on drag, closes on Esc.
```

Conventions this locks in: `#[non_exhaustive]` on the new
`EditorOutcome` variant (already on the enum), `PathBuf` (not
`RgbaImage`) as the cross-process handle (images are too large for IPC
frames and don't impl `Eq`), the em-dash window-title convention
(`"flintshot — pin"`, matching `"flintshot — annotate"`).

## Testing strategy

Mirrors [`ARCHITECTURE.md`](../../ARCHITECTURE.md) §Testing strategy
and the Phase 1 / 2 split (heavy unit coverage in core + pure pieces;
integration where a display is available).

### Unit — `flintshot-core` (headless, no display)

- `EditorOutcome::Pinned` postcard roundtrip (extend the existing
  `EditorOutcome` serde test if present; add otherwise).
- `Request::ReportSaved` / `Request::PinLast` postcard roundtrip
  (extend `ipc.rs`'s `request_postcard_roundtrip`).
- `pin_cache_dir()` resolver: honours `$XDG_CACHE_HOME` on Linux,
  returns the platform-correct path elsewhere, creates the dir if
  missing.

### Unit — `flintshot-editor-egui` (headless)

- `PinApp` construction from a known `RgbaImage` doesn't panic;
  `ViewportBuilder` carries `decorations(false)` + `always_on_top()`
  (assert on the built viewport's flags — eframe exposes these via
  `ViewportBuilder`'s public fields / builder methods).
- Image blit cache: a second `update` call doesn't re-upload (assert
  via a counter seam or by checking the cached handle is `Some`).

### Unit — `flintshot-app` (headless)

- `worker::pin_argv(path)` builds `["pin", "<path>"]`.
- `worker::dispatch(&Request::PinLast { .. }, RecordingSpawner, …)`
  with a populated `last_saved` spawns a `pin` worker and emits
  `Event::Pinned { id }`; with empty `last_saved` emits
  `Event::Error { code: "no-last-capture" }` and spawns nothing.
- `worker::dispatch(&Request::ReportSaved { path }, …)` stores the
  path in the shared `last_saved` and broadcasts `Event::Saved { path }`.
- The worker→daemon report is fire-and-forget: a missing daemon socket
  is logged at `warn` and doesn't fail the editor outcome.

### Integration — `flintshot-app` (needs `xvfb` on Linux, `#[ignore]`'d)

- Pin window opens and closes: `cargo run -- pin <fixture.png>` under
  `xvfb-run`, send `Esc` via `xdotool` (or a synthetic eframe input),
  assert the process exits 0 within a timeout.
- End-to-end `PinLast`: spin a daemon on a temp socket, send
  `CaptureFullscreen` (worker saves a fixture), observe `Event::Saved`,
  send `PinLast`, observe `Event::Pinned { id }`, assert a `pin` worker
  is alive.
- Worker→daemon report: a worker that saves a fixture triggers an
  `Event::Saved { path }` on the subscriber stream (currently
  unemitted; this phase fills it in).

### Boundary

- `no_gui_types_in_core.rs` unchanged in scope (no new banned imports —
  the pin primitive adds no transport types to core). Re-run to confirm
  green.

### Manual test matrix

Tracked in [`docs/TEST_MATRIX.md`](../../TEST_MATRIX.md); Phase 3 adds
rows for: pin window open / drag / close (X11 / Wayland / macOS /
Windows), always-on-top behaviour per platform, editor *Pin* button,
`flintshot pin <path>` CLI, multi-pin concurrency, `PinLast` over IPC.

## Boundaries

- **Always:**
  - Run the four-command gate (`fmt`, `clippy`, `test`, doc tests)
    before declaring done.
  - The pin primitive is an `EditorShell` trait method; its impl lives
    in `flintshot-editor-egui`. No `eframe` / `egui` types in
    `flintshot-core`'s public API.
  - A pin is its own process. The editor spawns `flintshot pin <path>`
    and exits; the daemon spawns `flintshot pin <path>` and stays
    running. Never block the editor or daemon event loop on a pin
    window's lifetime.
  - Worker→daemon outcome reporting is fire-and-forget. A standalone
    capture (no daemon) skips it and still succeeds; a report failure
    is logged at `warn`, not propagated as an editor error.
  - `flintshot pin <path>` must keep working with no daemon running —
    it's a leaf CLI subcommand, not a daemon client.
  - Preserve the Phase 1 / 2 standalone paths bit-for-bit (the daemon
    delegates to them; the pin path is additive).

- **Ask first:**
  - Adding a workspace dependency not in the ARCHITECTURE.md table
    (none expected for this phase).
  - Changing the IPC wire protocol's framing (length-prefix, max size).
    Adding a new `#[non_exhaustive]` `Request` variant
    (`ReportSaved`) is non-breaking and pre-approved; changing the
    framing is not.
  - Changing CI config (`.github/workflows/ci.yml`) — e.g. adding
    `xdotool` to a job for pin integration tests.
  - New ADRs — draft during `/planning`, but flag for review.
  - Splitting `flintshot-core::config` to extract a `paths` module —
    reasonable refactor, but flag the move.

- **Never:**
  - Introduce `eframe` / `egui` / `interprocess` / `libcosmic` types
    into `flintshot-core`'s public API (the boundary test enforces it).
  - Block the daemon's event loop on `run_pin` or on `xcap`.
  - `unwrap()` / `expect()` outside tests and `const` contexts.
  - Leave temp pin-cache files behind in the repo or in
    `~/.config/flintshot/` (they belong in the cache dir, cleaned
    opportunistically).
  - Silence a clippy warning with `#[allow]` without a justifying
    comment.
  - Pretend Wayland always-on-top works — document the caveat.

## Success criteria

- Editor *Pin* button → pin window visible in < 300 ms on commodity
  hardware (dominated by image composition + process spawn).
- `flintshot pin <path>` cold-start → window visible in < 200 ms.
- A pin survives its parent editor closing and its parent daemon
  restarting (pin is a separate process).
- `PinLast` over IPC opens a pin for the most recent save; the
  subscriber stream observes `Event::Saved` then `Event::Pinned`.
- Multiple concurrent pins are independent (closing one doesn't affect
  others; each is its own process).
- No new GUI/transport types in `flintshot-core` (boundary test green).
- The four-command gate is green on Linux; `cargo build` green on macOS
  and Windows (cross-compile smoke where a native run isn't possible in
  CI); `flintshot-cosmic` still builds unchanged.

## Open questions

1. **Pin-cache dir helper placement.** Extend `flintshot-core::config`
   (which owns `config_path()` / `config_dir()`) or extract a new
   `flintshot-core::paths` module? Recommendation: extend `config` for
   now (one new fn `pin_cache_dir()`); extract `paths` if a third
   resolver appears. Confirm during `/planning`.
2. **Pin opens at capture origin?** The `run_pin(image, rect)` trait
   sig carries a `Rect`. For `flintshot pin <path>` (file source) we
   have no origin — open where the WM places it (or centered). For the
   editor *Pin* button we also don't have a meaningful origin (the
   capture's original screen position was lost when the editor opened).
   Recommendation: `rect` is honoured when the caller has a real screen
   position (future: the Phase 5 live overlay passes the in-place
   capture rect); for Phase 3 both callers pass a default `Rect` and
   the WM positions the window. Document this in the trait doc. Confirm
   during `/planning`.
3. **Worker→daemon report granularity.** `Request::ReportSaved { path }`
   covers the saved case. Should we also report `CopiedToClipboard` /
   `Cancelled` (for a fuller subscriber stream + future analytics), or
   keep the surface minimal (only what `PinLast` needs)? Recommendation:
   minimal now (`ReportSaved` only); a future `ReportOutcome` enum
   covers the rest when a consumer needs it. Confirm during `/planning`.
4. **Pin window transparency / rounded corners.** Borderless
   always-on-top with the image's own alpha is the baseline. Do we want
   a drop shadow or rounded corners (would need `with_transparent(true)`
   + a custom shader / painted rounded rect)? Recommendation: no —
   keep the pin pixel-rectangular; shadows are compositor-dependent
   and a polish-effort concern. Confirm during `/planning`.
5. **macOS always-on-top behaviour.** `ViewportBuilder::with_always_on_top`
   maps to `NSWindow.level = .floating` on macOS. Confirm this keeps
   the pin above fullscreen apps or whether `.statusBar` is needed.
   Needs a manual check; document the outcome.
6. **ADRs to draft during `/planning`:**
   - Pin-as-separate-process (vs. editor viewport or daemon thread).
     Reuses ADR-0019's worker-spawn model; record the pin-specific
     rationale (pin must outlive editor + daemon).
   - Worker→daemon outcome reporting (fire-and-forget `ReportSaved`).
     Extends the Phase 2 wire protocol; record why a new `Request`
     variant beats a side-channel file or a daemon-side directory
     watch.
   - Pin-cache dir lifecycle (where pin-bound temp files live, when
     they're cleaned).
