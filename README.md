# flintshot

Cross-platform screenshot capture and annotation tool, written in pure Rust.
Inspired by [Flameshot](https://flameshot.org), designed to be fast, modular,
and friendly to the [COSMIC desktop](https://system76.com/cosmic).

> **Status:** Phase 1 (capture + editor window) in progress. Basic capture works;
> annotation tools and save/copy coming soon. See the
> [Phase 1 spec](docs/specs/phase-1-capture-editor/spec.md), or
> [`docs/ROADMAP.md`](docs/ROADMAP.md) for the milestone index.

## At a glance

- **Pure-Rust stack**: eframe/egui for UI, xcap for capture, tokio for async
- **Cross-platform**: Linux (X11 + Wayland), macOS, Windows
- **Two interaction models**: editor-window (Satty-style, ships first) +
  live overlay (Flameshot-style, Phase 5)
- **COSMIC panel applet** as an alternate front-end (Phase 3)
- **Pluggable upload backends**: Imgur ships first; `Backend` trait designed
  for extension

## Getting started

### Prerequisites

- **Rust 1.95+** (auto-selected via `rust-toolchain.toml`)
- **Linux:** X11/Wayland dev libraries for screen capture:
  ```bash
  sudo apt-get install -y pkg-config libclang-dev libxcb1-dev libxrandr-dev \
    libdbus-1-dev libpipewire-0.3-dev libwayland-dev libegl-dev libgbm-dev
  ```
- **macOS / Windows:** No extra deps needed

### Run the app

```bash
# Build
cargo build

# Capture fullscreen and open editor
cargo run -- fullscreen

# Capture with delay (useful for setting up the screen)
cargo run -- region --delay-ms 2000

# Capture and save directly (skip editor)
cargo run -- region --out /tmp/screenshot.png

# See all options
cargo run -- --help
cargo run -- region --help
```

### Daemon, tray & hotkeys (Phase 2)

Run the resident daemon for the system tray, global hotkeys, and IPC
(the tray needs `--features tray`, which pulls GTK on Linux):

```sh
cargo run -- daemon                 # tray* + hotkeys + IPC server
cargo run -- daemon --replace       # take over from a running daemon
cargo run --features tray -- daemon # with the system tray enabled
```

`flintshot region` / `fullscreen` delegate to a running daemon when one
is present (centralising the event stream) and fall back to standalone
otherwise; `--standalone` forces the in-process path.

> **Wayland:** `global-hotkey` has no Wayland backend yet (Phase 6,
> via the `ashpd` `GlobalShortcuts` portal). On Wayland the daemon logs
> a notice and you bind a compositor keybinding to `flintshot region` /
> `flintshot fullscreen`. See
> [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) §Cross-platform concerns.

> **Pin always-on-top (Wayland):** the pin window's always-on-top hint
> is honoured on X11, macOS, and Windows, but Wayland has no
> always-on-top protocol for ordinary top-level windows — some
> compositors ignore it. The pin still opens and is draggable; it just
> may not float above other windows. A layer-shell-based pin is the real
> fix, deferred to a future effort. See
> [`docs/TEST_MATRIX.md`](docs/TEST_MATRIX.md) §Phase 3 known caveats.

### Development

For contributors. The `justfile` wraps the common loop; see
[`CONTRIBUTING.md`](CONTRIBUTING.md) for the full workflow.

```bash
just bootstrap     # install cargo-watch + cargo-nextest (one-time)
just dev           # auto-rebuild + re-run on file changes
just ci            # run the same checks CI runs, locally
just test          # tests (prefers nextest when installed)
```

Dev shortcuts exist for capture subcommands too — e.g.
`just fullscreen`, `just region --delay-ms 500`. See
`just --list` for the full surface.

### What works right now

- Capture fullscreen/region (via xcap)
- Editor window with annotation tools (rectangle, arrow, text, blur, ellipse, line, marker, pencil, number)
- Save to file / copy to clipboard / pin to desktop
- Config file at `~/.config/flintshot/config.toml`
- Daemon with tray, hotkeys, and IPC server
- Pin-to-desktop (`flintshot pin`, editor Pin button, `Request::PinLast`)

### Coming next (Phase 4)

- Upload backends (Imgur, …)

See [`docs/ROADMAP.md`](docs/ROADMAP.md) for the milestone index.

## COSMIC panel applet

`flintshot-cosmic` is an alternate front-end for COSMIC desktop users: a
panel button + popup that talks IPC to a running `flintshot daemon` (no
capture or editing in-process — it mirrors the tray's actions and adds a
live status line the tray can't show). It is **Linux-only** (libcosmic is
Linux/COSMIC-only); macOS and Windows users keep the tray.

> **N/A on macOS / Windows.** The applet crate is not in `default-members`
> and won't build there. Those platforms use the daemon + tray surface
> (see [§Daemon, tray & hotkeys](#daemon-tray--hotkeys-phase-2)).

### Prerequisites (Linux)

The applet needs the libcosmic system deps on top of the xcap deps listed
above. On Debian / Ubuntu (full list in
[`AGENTS.md`](AGENTS.md) §COSMIC crate system deps):

```bash
sudo apt-get install -y cmake libxkbcommon-dev libfontconfig-dev \
  libfreetype6-dev libglib2.0-dev libgfxbuffer-dev pkg-config
```

### Build

```bash
cargo build -p flintshot-cosmic
```

(The crate is opt-in — it's deliberately not in `default-members` because
libcosmic pulls extra system libraries a default build shouldn't require.)

### Install the `.desktop` (one-time, per machine)

`dist/io.github.anomalyco.Flintshot.desktop` is the discovery descriptor
COSMIC's panel-settings picker scans for. Copy it where COSMIC looks for
applets — either a user install or system-wide:

```bash
# User install (no root):
mkdir -p ~/.local/share/cosmic/applets
cp dist/io.github.anomalyco.Flintshot.desktop ~/.local/share/cosmic/applets/

# System-wide:
sudo cp dist/io.github.anomalyco.Flintshot.desktop /usr/share/cosmic/applets/
```

The filename is the app id (`io.github.anomalyco.Flintshot`) — that's how
`cosmic-panel` correlates a running applet with its config slot, so don't
rename it. The `.desktop` carries `Categories=COSMIC;` +
`X-CosmicApplet=true` (the markers the picker requires); its header
comment records the `pop-os/cosmic-panel` + `cosmic-applets` revisions
the markers were confirmed against, in case a future COSMIC release
drifts.

### Add it from COSMIC panel settings

1. Start the daemon: `flintshot daemon` (the applet is a client — it does
   nothing useful without one).
2. Open **COSMIC Settings → Panel** and add "flintshot". The panel button
   (a camera-photo-symbolic icon) appears.
3. Click the button → the popup opens with *Capture region* / *Capture
   fullscreen* / per-monitor entries / *Pin last* / *Open config* /
   *Quit*, plus a live status line that updates through a capture
   ("capturing…" → "editor open" → "saved: \<path\>" → "pinned #N").

### Daemon-absent flow (*Start daemon*)

When no daemon answers, the popup disables every capture/pin action and
shows a single *Start daemon* button. Clicking it spawns
`flintshot daemon` resolved on `PATH` — so **a production install must
put `flintshot` on `PATH`** (`cargo install --path crates/flintshot-app`,
or your distro's package). Running the applet via `cargo run -p
flintshot-cosmic` won't find `flintshot` on `PATH`; launch the daemon
directly in that case. Once the daemon's socket answers, the applet
self-heals (the subscriber reconnects with bounded backoff and the
actions re-enable) — no applet restart needed.

### Tray coexistence

On COSMIC both the tray and the applet appear when the daemon runs. Both
are harmless IPC clients; neither interferes with the other. Hiding the
tray when a COSMIC session is detected is future polish.

<!-- TODO: replace the placeholder Icon=camera-photo-symbolic with a real
     flintshot symbolic asset (io.github.anomalyco.Flintshot-symbolic)
     in dist/io.github.anomalyco.Flintshot.desktop + an installed theme.
     Polish effort, not gated on Phase 3b. -->

## Documentation

Product & design:

- [`docs/VISION.md`](docs/VISION.md) — product vision, scope, non-goals
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — milestone index (thin)
- [`docs/specs/phase-1-capture-editor/`](docs/specs/phase-1-capture-editor/) — current effort spec, plan, and task checklist (Phase 1)
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — workspace, crates, dependencies, data model
- [`docs/adrs/`](docs/adrs/) — technical decision records (ADRs)
- [`docs/TEST_MATRIX.md`](docs/TEST_MATRIX.md) — cross-platform manual QA grid

Working in the repo:

- [`CONTRIBUTING.md`](CONTRIBUTING.md) — dev setup, workflow, code style
- [`AGENTS.md`](AGENTS.md) — operational guide for AI agents (build/test/lint commands)
- [`CHANGELOG.md`](CHANGELOG.md) — release history

## Project name vs directory

The project is **`flintshot`** (the binary you'll install and the crates you'll
depend on). The local directory may still be named `capture` from initial
scaffolding — that's incidental; cargo package names are independent of the
directory.

## License

Dual-licensed under `MIT OR Apache-2.0`, at your option.
