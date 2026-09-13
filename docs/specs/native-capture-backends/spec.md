---
status: active
started: 2026-09-13
---

# Spec: Native capture backends

## Objective

Abstract all capture functionality behind a `Capturer` trait in
`flintshot-core`, mirroring the existing `Canvas` / `EditorShell` /
`UploadBackend` trait pattern, and layer **OS-native screenshot
services** on top of `xcap` (which remains the default capture provider
and universal fallback).

Why now:

- **Wayland pain.** `xcap`'s Wayland path goes through the ScreenCast
  portal with per-session prompts and the latency/quirks ADR-0003
  flagged for re-evaluation. The OS's own screenshot services (GNOME
  Shell Screenshot D-Bus API, the `org.freedesktop.portal.Screenshot`
  portal) are the sanctioned native paths on those sessions.
- **Architecture.** Capture is the last direct-dependency surface in
   `flintshot-core` with no trait boundary. Every other pluggable
   concern already has one.

User stories:

- As a GNOME Wayland user, `flintshot region` captures without a
  portal re-prompt and without ScreenCast session setup when the
  shell's screenshot service is callable.
- As an X11 / macOS / Windows user, my behavior is byte-for-byte
  unchanged (xcap path).
- As a power user, I can pin a backend via config
  (`[capture] backend = "portal"`) or debug what auto-detection
  picks (`flintshot backends`).
- As a contributor, I can add a capture backend without touching any
  call site.

## Scope

**In scope (Linux-first per decision):**

1. `Capturer` trait + shared logic (rect validation, cropping,
   monitor-selection) in `flintshot-core`.
2. `XcapCapturer` — wraps today's behavior; default provider and
   fallback.
3. ~~`GnomeShellCapturer`~~ — **dropped by Spike A** (service
   allowlist-restricted since GNOME 41; see
   `tasks/001-spike-gnome-shell.md`).
4. `PortalScreenshotCapturer` — `org.freedesktop.portal.Screenshot`
   via `zbus` (the transport `ashpd` already links; see
   **Technologies**); the cross-desktop native service (KDE, COSMIC,
   wlroots).
5. Runtime auto-detection with config override.
6. `flintshot backends` debug subcommand.

**Out of scope (explicitly):**

- macOS `screencapture` CLI / ScreenCaptureKit, Windows
  `Windows.Graphics.Capture` backends (macOS/Windows stay on xcap
  behind the trait — future effort).
- Server-side region grabs (GNOME `ScreenshotArea`): flintshot's UX
  stays capture-monitor → select region in overlay/editor. Native
  region capture is a future effort.
- ScreenCast-portal *persistent session* capture (video/GIF path).
- Removing the `xcap` dependency.

## Technologies

| Concern                  | Crate / mechanism                                  | Notes |
| ------------------------ | -------------------------------------------------- | ----- |
| Trait + shared logic     | `flintshot-core::capture` (module, not new crate)  | Keeps `flintshot_core::monitors()` API stable for `flintshot-app` / `flintshot-cosmic` |
| Fallback backend         | `xcap` (existing)                                  | Unchanged behavior |
| Portal screenshot        | `zbus` (raw portal request/response)               | Hand-rolled `Screenshot` call; see note below |
| Auto-detection           | Bus-name probes + `XDG_SESSION_TYPE` / env         | Injectable seam for tests (`select_backend_with` / `backend_report_with`) |
| Config override          | `[capture] backend = "auto"`                       | Follows existing string-tolerant config convention |

**zbus, not ashpd.** `no_gui_types_in_core.rs` deliberately bans
`use ashpd` from `flintshot-core` (Phase 6 decision: the portal client
lives in `flintshot-app`). That ban stands. The portal backend talks
D-Bus directly via `zbus` — the transport layer, not a GUI
toolkit, riding the same exemption as `keyring` (ADR-0024 precedent).
`zbus` lands as a
`[target.'cfg(target_os = "linux")'.dependencies]` entry so
macOS/Windows builds compile no D-Bus code, and the no-GUI guard test
gains a path-scoped allowance: `use zbus` is legal **only** under
`src/capture/`. (The portal request/response dance — handle token,
`Request::Response` signal, timeout — is hand-rolled; ashpd already
links zbus, so no new native stack enters the binary.)

## Commands

```sh
cargo build                                   # default members
cargo build --workspace                       # all crates
cargo test --workspace --all-targets          # unit + integration
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -- region                           # manual: default backend path
cargo run -- backends                         # manual: list probe results
cargo run -- region --backend portal      # manual: pinned backend (CLI override)
XDG_SESSION_TYPE=wayland cargo run -- backends  # manual: force detection inputs
```

New system deps: none. (`zbus` is pure Rust; `ashpd` is already in
the tree for Phase 6.)

## Project structure

```
crates/flintshot-core/src/capture/
├── mod.rs          # Capturer trait, CaptureOptions, registry + auto-detect, compat wrappers
├── xcap_backend.rs # XcapCapturer (moved from capture.rs, behavior unchanged)
└── portal.rs       # PortalScreenshotCapturer (zbus, linux-only)
```

- `capture.rs` free functions (`monitors()`, `capture_fullscreen()`,
  `capture_region()`) remain as **delegating wrappers** over the
  selected backend so `flintshot-app`, `flintshot-cosmic`, and tests
  keep compiling unchanged. Wrappers are `#[doc(hidden)]`-grade
  compatibility surface; new code goes through the trait.
- Backend files compile behind `#[cfg(target_os = "linux")]`.
- `flintshot-app/src/cli.rs` gains `--backend` on `region` /
  `fullscreen` and the `backends` subcommand; `worker.rs` /
  `app_window.rs` call sites switch to the registry with config
  preference passed through.

## Trait sketch

Sync methods — every current capture call site is synchronous worker
code; D-Bus backends use the blocking facades internally.

```rust
#[non_exhaustive]
pub struct CaptureOptions {
    pub include_cursor: bool,
}

pub trait Capturer: Send + Sync {
    /// Stable machine id ("xcap", "portal") — config/CLI key.
    fn id(&self) -> &'static str;
    /// Human-readable name for `flintshot backends` output.
    fn name(&self) -> &'static str;
    /// Probe availability without capturing (bus-name check, env check).
    /// `Ok(())` = usable; `Err(detail)` feeds the `backends` report.
    fn available(&self) -> Result<()>;
    /// Enumerate monitors in virtual-desktop order (physical px).
    fn monitors(&self) -> Result<Vec<MonitorInfo>>;
    /// Grab a whole monitor; `None` = primary. Physical px, BGRA→RGBA
    /// normalized by the backend.
    fn capture_monitor(&self, monitor: Option<u32>, opts: &CaptureOptions)
        -> Result<image::RgbaImage>;
}

/// Resolve the backend: config override → auto-detect → xcap.
/// Auto never fails: if probes all fail, XcapCapturer is returned.
pub fn select_backend(preference: Option<&str>) -> Result<Box<dyn Capturer>>;
```

Region capture stays a free function (validate → capture_monitor →
crop); it is not on the trait.

## Auto-detection precedence (amended by Spike A; Spike B may reorder)

GNOME Shell's screenshot D-Bus service is allowlist-restricted for
unprivileged callers since GNOME 41 (Spike A verdict:
`drop-to-portal`), so no `gnome-shell` backend exists; GNOME Wayland
rides the general portal row.

| Session                    | Precedence                                        |
| -------------------------- | ------------------------------------------------- |
| X11                        | `xcap` (direct grab is fastest; services add overhead) |
| Wayland (any compositor)   | `portal` → `xcap`                                 |
| macOS / Windows            | `xcap`                                            |

Config `backend = "auto"` (default) or explicit pin; unknown id →
`tracing::warn!` + auto. CLI `--backend` beats config.

## Code style

Follows repo conventions verbatim (pedantic clippy, `#![forbid(unsafe_code)]`,
doc comments on all public items, `#[non_exhaustive]`, thiserror in
core, no comments restating code). The dropped GNOME-Shell
illustration lived here before Spike A; see `portal.rs` for the
shipped backend's shape (probe → dance → decode → crop, every phase
time-bounded).

## Testing strategy

- **Registry / precedence** — unit tests with `FakeCapturer`s
  (in-crate test impl): override wins, auto falls through failed
  probes, xcap terminal fallback, unknown-id warning path. All
  headless; no bus needed.
- **Probe seam** — selection/report logic is pure over an injected
  backend list (`select_backend_with` / `backend_report_with`), so
  tests simulate X11 / Wayland+portal / Wayland-bare sessions
  headless with fakes.
- **Config** — `backend` string roundtrips, survives re-saves
  verbatim, unknown values load + persist untouched (same policy as
  `interaction-mode`).
- **Backends** — D-Bus backends get: decode-path unit tests against
  fixture files (PNG bytes → `RgbaImage`), error-mapping tests;
  real-service tests are `#[ignore]`'d like the existing
  `tests/capture.rs` smoke test (run manually per desktop).
- **Existing suites** — `capture.rs` unit tests, the smoke
  integration test, and `no_gui_types_in_core.rs` all stay green;
  the no-GUI guard resolved as a path-scoped `use zbus` allowance
  under `src/capture/` only (regression-tested both directions),
  with the `ashpd` ban kept global.
- **Manual matrix** — `TEST_MATRIX.md` gains a backends × session
  section (X11, Wayland+GNOME, Wayland+COSMIC).

## Boundaries

- **Always**: keep `xcap` working on every platform; keep the public
  `flintshot-core` capture API source-compatible (delegating
  wrappers); every backend failure falls back to `xcap` at selection
  time — a capture must never hard-fail because a native backend was
  miscalibrated; run the full finish gate (fmt, clippy, tests) before
  declaring done.
- **Ask first**: new external crates beyond `zbus` reuse (ashpd must
  stay within its existing feature set); changes to
  `CaptureGuard`/supersede handshake semantics; touching IPC schema;
  reordering auto precedence after Spike B.
- **Never**: no `unsafe`; no GUI toolkit types in `flintshot-core`'s
  public API; no removal of the xcap backend; no server-side region
  capture in this effort; no per-backend user-visible UX beyond a
  `tracing` line + `flintshot backends` output.

## Success criteria

1. `cargo test --workspace --all-targets` green; full finish gate
   green; no new system deps on any platform.
2. `flintshot backends` on a Linux session prints each backend's
   availability and what `auto` selects — deterministic given the
   same environment inputs (verified via the probe seam unit tests).
3. GNOME Wayland: auto-selected native backend produces a capture
   with correct dimensions/monitor selection; if Spike A finds the
   shell service restricted, `portal` takes its slot and the spec's
   precedence table is amended accordingly.
4. X11: capture bytes identical (same monitor, same options) to
   pre-refactor within toolchain nondeterminism — spot-checked via
   the existing smoke test path.
5. `[capture] backend = "xcap"` on any session reproduces
   pre-refactor behavior exactly.
6. `flintshot-cosmic` and all `flintshot-app` call sites compile
   without behavior change (they consume the compat wrappers /
   registry).
7. ADR written: supersedes-extends ADR-0003 ("`xcap` for screen
   capture" → "`xcap` as default provider behind `Capturer`").

## Open questions (spikes — resolved in /planning before tasks are cut)

- **Spike A — GNOME Shell service callable-ness.** GNOME 41+
  reshaped `org.gnome.Shell.Screenshot` and may restrict direct
  callers (portal-only). Verify on current GNOME: method surface
  (`Screenshot` vs `ScreenshotMonitor`), cursor/flash args, whether a
  plain session-bus client may call it, and file-write location.
  Fallback if restricted: drop `GnomeShellCapturer` from scope, keep
  portal backend as the Wayland-native path.
- **Spike B — portal Screenshot UX.** For each portal impl available
  to us (GNOME, KDE, COSMIC, xdg-desktop-portal-wlr): does the
  one-shot Screenshot call prompt? Does it honor `include_cursor` /
  `interactive`? Does it return a URI we can decode fast enough
  (<250ms budget end-to-end)? Results amend the precedence table.
- **Monitor enumeration on Wayland without prompting.** `monitors()`
  currently rides xcap enumeration. Confirm enumeration is prompt-free
  on all target Wayland sessions; if not, portal/OccupancyMonitor
  alternatives get specced.
- **Blocking D-Bus inside worker processes.** Confirm
  `zbus::blocking` + `ashpd` blocking facade fit the worker's
  sync context without dead-locking the CaptureGuard handshake
  window (the grab happens while surfaces are hidden — latency here
  is user-visible).
