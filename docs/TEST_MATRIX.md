# Test Matrix

Manual cross-platform verification grid. Used before each phase exit and
before any tagged release. CI covers `fmt`, `clippy`, `cargo test`, and
the MSRV build; **this file covers everything CI can't** — actual
displays, compositors, and permissions.

Phase ownership: this matrix is live as of Phase 1. Rows are added as
features land. Mark each cell with the date and commit hash when
verified, or open an issue and link it in the cell.

Legend:

- **PASS** — verified working
- **PARTIAL** — works with caveats (note them)
- **FAIL** — broken; link the issue
- **N/A** — doesn't apply on this platform
- **(blank)** — not yet tested

## Platforms under test

| ID  | Platform                                          | Display server      | Hardware / VM     |
| --- | ------------------------------------------------- | ------------------- | ----------------- |
| L1  | Ubuntu 24.04 LTS                                  | X11                 | bare metal        |
| L2  | Ubuntu 24.04 LTS                                  | GNOME Wayland       | bare metal        |
| L3  | Fedora 41, Plasma 6                               | Wayland             | bare metal        |
| L4  | Arch, Hyprland                                    | Wayland             | bare metal        |
| L5  | COSMIC alpha                                      | Wayland             | bare metal        |
| M1  | macOS 14 (Sonoma)                                 | Quartz              | Apple Silicon     |
| M2  | macOS 15 (Sequoia)                                | Quartz              | Intel             |
| W1  | Windows 11 23H2                                   | DWM                 | bare metal        |
| W2  | Windows 10 22H2                                   | DWM                 | VM                |

Add a row when you start testing on something new. Don't delete rows
when a platform is dropped — mark **DEPRECATED** and the date.

## Phase 1 — capture + editor window

Subcommand: `flintshot region`, `flintshot fullscreen`

| Behavior                                                | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| `cargo build` succeeds                                  |    |    |    |    |    |    |    |    |    |
| `cargo test --workspace --all-targets` passes         |    |    |    |    |    |    |    |    |    |
| `cargo test -p flintshot-core --test capture -- --ignored smoke_can_grab_primary` returns a non-empty image |    |    |    |    |    |    |    |    |    |
| `flintshot --help` lists all subcommands                |    |    |    |    |    |    |    |    |    |
| Single-monitor capture, no portal prompt                |    |    |    |    |    |    |    |    |    |
| Capture triggers xdg-desktop-portal prompt on first use | N/A |    |    |    |    | N/A | N/A | N/A | N/A |
| macOS: Screen Recording permission flow handled         | N/A | N/A | N/A | N/A | N/A |    |    | N/A | N/A |
| Editor window opens with the screenshot as background   |    |    |    |    |    |    |    |    |    |
| Rectangle tool draws at correct physical-pixel coords   |    |    |    |    |    |    |    |    |    |
| Arrow tool renders head correctly                       |    |    |    |    |    |    |    |    |    |
| Text tool produces readable glyphs (`cosmic-text`)      |    |    |    |    |    |    |    |    |    |
| Blur tool produces uniform output, no seams             |    |    |    |    |    |    |    |    |    |
| Select / crop region works                              |    |    |    |    |    |    |    |    |    |
| Color picker persists to config across restarts         |    |    |    |    |    |    |    |    |    |
| Stroke-width picker persists                            |    |    |    |    |    |    |    |    |    |
| `Ctrl+S` save via `rfd` writes correct PNG              |    |    |    |    |    |    |    |    |    |
| `Ctrl+C` copies image to clipboard                     |    |    |    |    |    |    |    |    |    |
| `Ctrl+Z` / `Ctrl+Y` undo/redo                           |    |    |    |    |    |    |    |    |    |
| `Esc` cancels and closes editor without saving          |    |    |    |    |    |    |    |    |    |
| HiDPI / fractional-scale display captures sharp image   |    |    |    |    |    |    |    |    |    |
| Cold-start-to-capture under 200 ms (commodity HW)       |    |    |    |    |    |    |    |    |    |
| Release binary stripped under 25 MB                     |    |    |    |    |    |    |    |    |    |

### Phase 1 known caveats

- **L2–L5 (Wayland):** first capture per session may show a portal
  prompt. Expected, not a bug (see ADR-0003).
- **L4 (Hyprland):** confirm `flintshot fullscreen` covers the right
  output when the cursor is on a non-primary one.
- **M1/M2:** if Screen Recording permission isn't granted, the editor
  window opens to a black image; verify the detection-and-guide path
  fires.

## Phase 2 — daemon, tray, hotkeys, IPC

Subcommand: `flintshot daemon` (and `flintshot daemon --replace`).
Annotation tools (Ellipse/Line/Marker/Pencil/Number/Magnifier) are
deferred to `phase-2b` and tested there.

| Behavior                                                    | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ----------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| `cargo build --features tray` succeeds                      |    |    |    |    |    |    |    |    |    |
| `flintshot daemon` starts, tray icon appears (`--features tray`) | |  |    |    |    |    |    |    |    |    |
| `flintshot daemon` (no tray feature) still binds IPC + hotkeys |   |    |    |    |    |    |    |    |    |
| Second `flintshot daemon` fails "already running"           |    |    |    |    |    |    |    |    |    |
| `flintshot daemon --replace` swaps daemons cleanly          |    |    |    |    |    |    |    |    |    |
| Tray *Capture region* / *Capture fullscreen* spawn editor   |    |    |    |    |    |    |    |    |    |
| Tray *Fullscreen on monitor →* lists each monitor; each captures the right one | | | | | | | | | |
| Tray *Open config* opens `$EDITOR` on `config.toml`         |    |    |    |    |    |    |    |    |    |
| Tray *Quit* exits the daemon cleanly                        |    |    |    |    |    |    |    |    |    |
| Tray visible with no SNI host → daemon stays alive (warning)|    |    |    |    |    |    |    |    |    |
| Global hotkey fires capture on X11 / macOS / Windows        |    | N/A | N/A | N/A | N/A |    |    |    |    |
| Wayland: hotkey does NOT fire; compositor-keybind workaround logged | N/A |    |    |    |    | N/A | N/A | N/A | N/A |
| Edit `[hotkeys]`, save → new chord works without restart    |    |    |    |    |    |    |    |    |    |
| IPC client `Subscribe` then `broadcast` → receives `Event`  |    |    |    |    |    |    |    |    |    |
| `flintshot region` with daemon running delegates (events centralised) | | | | | | | | | |
| `flintshot region` with no daemon → standalone editor       |    |    |    |    |    |    |    |    |    |
| `flintshot region --standalone` forces in-process capture   |    |    |    |    |    |    |    |    |    |
| Ctrl-C → clean daemon shutdown (socket reclaimed)           |    |    |    |    |    |    |    |    |    |


## Phase 2b — annotation tools + magnifier

Exercised via `flintshot region` → editor. All five tools route through
`flintshot-core::render::render()`, so the saved/copied PNG matches the
live editor (WYSIWYG per ADR-0015). The magnifier is toolkit-owned
(egui-only; no core abstraction, per ARCHITECTURE.md §What stays
toolkit-specific) and reads `EditorState::image` directly. See
ADR-0022 for the Marker/Pencil freehand smoothing + per-pixel-once
compositing decision.

| Behavior                                                    | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ----------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| Ellipse: drag preview → commit → `Ctrl+Z` → click-inside to select | |  |    |    |    |    |    |    |    |
| Line: drag preview → commit → click-near to select (thin-stroke tolerance) | | | | | | | | | |
| Marker: translucent highlight, background text visible through stroke | |  |    |    |    |    |    |    |    |
| Pencil: smooth opaque freehand (Catmull-Rom, no jagged polyline) | |  |    |    |    |    |    |    |    |
| Number: click places `1`,`2`,`3` bubbles; undo decrements counter | |  |    |    |    |    |    |    |    |
| Magnifier: hold `Space` (default) shows 4× inset; release hides |    |    |    |    |    |    |    |    |    |
| Magnifier: `[ui] magnifier-always-on = true` stays up while drawing | |  |    |    |    |    |    |    |    |
| Magnifier: `[ui] magnifier-modifier = "<key>"` swaps the hold key | |  |    |    |    |    |    |    |    |
| Save → re-open PNG: every annotation rasterized identically to the live view (WYSIWYG) | | | | | | | | | |

### Phase 2b known caveats

- **Marker WYSIWYG is near-exact, not bit-exact.** egui's
  `PathShape::line` and `ImageCanvas`'s square-stamp union use different
  cap/join geometry, so a translucent marker's silhouette can differ by
  ±1 px at stroke ends/corners between the live view and the saved PNG.
  The interior translucency (the property the exit criterion names)
  matches. See ADR-0022.
- **Magnifier zoom is fixed at 4×** (config controls only the trigger,
  not the zoom factor). A future config knob could expose it.



## Phase 3 — pin-to-desktop + COSMIC applet

Subcommands: `flintshot pin <path>`, `flintshot open <path>`, editor
*Pin* button, `Request::PinLast` / `Request::OpenLast` over IPC, and
`flintshot-cosmic` panel applet.

A pin is a borderless always-on-top floating window showing a captured
image; each pin runs as its own worker process so a pin crash can't kill
the editor or daemon (ADR-0019), and multiple pins coexist. All three
pin entry points (CLI, editor button, daemon `PinLast`) converge on
`EditorShell::run_pin`. The COSMIC applet is a pure IPC client (depends
on `flintshot-core` + `libcosmic` only — no `egui`, no `flintshot-app`).

### Phase 3.A — pin-to-desktop

| Behavior                                                    | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ----------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| `flintshot pin <path>` opens borderless window              |    |    |    |    |    |    |    |    |    |
| `flintshot pin <path> --at X,Y --size WxH` honours geometry  |    |    |    |    |    |    |    |    |    |
| `flintshot pin <path> --opacity 0.7 --no-clipboard` override |    |    |    |    |    |    |    |    |    |
| Pin window is always-on-top over ordinary windows           |    |    |    |    |    |    |    |    |    |
| Pin draggable (left-drag anywhere moves the window)         |    |    |    |    |    |    |    |    |    |
| Pin draggable across monitors (cross-monitor drag)          |    |    |    |    |    |    |    |    |    |
| Pin closes on `Esc`                                         |    |    |    |    |    |    |    |    |    |
| Pin closes on double-click                                  |    |    |    |    |    |    |    |    |    |
| Hover-opacity: resting at `[pin] default-opacity`, opaque on hover | | | | | | | | | |
| Hover-opacity: returns to resting when cursor leaves        |    |    |    |    |    |    |    |    |    |
| Resize handle (bottom-right): drag resizes window           |    |    |    |    |    |    |    |    |    |
| Resize is aspect-ratio locked (image scales, no crop/distort)|    |    |    |    |    |    |    |    |    |
| Copy-on-close `[pin] copy_on_close = true`: image on clipboard on close | | | | | | | | | |
| Copy-on-close `[pin] copy_on_close = false` (default): no copy | | | | | | | | | |
| Pin persists after the editor exits                         |    |    |    |    |    |    |    |    |    |
| Pin persists after the daemon exits                         |    |    |    |    |    |    |    |    |    |
| Multiple pins coexist as independent windows                |    |    |    |    |    |    |    |    |    |
| Editor *Pin* button: compose → cache file → pin spawns, editor closes | | | | | | | | | |
| Editor *Pin* with active selection pins just the region     |    |    |    |    |    |    |    |    |    |
| `flintshot open <path>` loads existing image into editor    |    |    |    |    |    |    |    |    |    |
| `Request::PinLast` over IPC spawns a pin worker             |    |    |    |    |    |    |    |    |    |
| `Request::OpenLast` over IPC spawns editor on last save     |    |    |    |    |    |    |    |    |    |
| `PinLast` with no prior save → `Error { code: "no-last-capture" }` | | | | | | | | | |
| `Event::Saved { path }` reaches IPC subscribers after editor save | | | | | | | | | |
| `Event::Pinned { id }` reaches IPC subscribers after `PinLast`; ids monotonic | | | | | | | | | |

### Phase 3.B — COSMIC panel applet

Built and exercised only on Linux (`flintshot-cosmic` is an opt-in
crate outside `default-members`; macOS/Windows/standalone-editor cells
are N/A). The applet is a pure IPC client — it sends `Request`s and
reads the `Event` stream; it performs no capture and runs no editor.

| Behavior                                                    | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ----------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| `cargo build -p flintshot-cosmic` succeeds                  |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Applet appears as a COSMIC panel button                     |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Panel button opens popup with full action set               |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Popup *Capture region* sends `Request`, daemon spawns editor |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Popup *Capture fullscreen* sends `Request`, daemon spawns   |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Popup per-monitor submenu lists each monitor (from `monitors()`) | | | | | | N/A | N/A | N/A | N/A |
| Per-monitor entry captures the right monitor                |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Popup *Pin last* sends `Request::PinLast`, pin window spawns |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Popup *Open last* sends `Request::OpenLast`, editor opens   |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Popup *Open config* opens `$EDITOR` on `config.toml`        |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Popup *Quit* sends `Request::Quit`, daemon exits cleanly    |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Event feed: status line updates (*Capturing… → Editor opened → Saved \<path\>*) | | | | | | N/A | N/A | N/A | N/A |
| Event feed: `Event::Pinned { id }` shows *"Pinned #N"*      |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Event feed: `Event::Error` shows tinted-red error line      |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Daemon-absent: popup shows *"daemon not running"* + start hint |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Daemon-absent: daemon-dependent buttons disabled            |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Daemon-absent: *Open config* stays enabled (no daemon needed) |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Reconnect: daemon restart → popup re-subscribes within ~2 s |    |    |    |    |    | N/A | N/A | N/A | N/A |
| `Lagged` / stream error handled without panic               |    |    |    |    |    | N/A | N/A | N/A | N/A |

### Phase 3 known caveats

- **L2–L5 (Wayland):** always-on-top is compositor-dependent.
  `with_always_on_top()` maps to a winit hint that X11/DWM/macOS honour
  but Wayland compositors may ignore (there is no Wayland protocol for
  "always on top" outside layer-shell, which a borderless top-level
  window doesn't use). The pin still opens and is draggable; it just
  may not float above other windows. A layer-shell-based pin is the
  real fix, deferred to a future effort.
- **M1/M2 (macOS):** `with_always_on_top()` maps to
  `NSWindow.level = .floating`. Verify whether `.statusBar` is needed
  for the pin to float above a fullscreen app — record the result.
- **Pin-cache files:** temp files under the OS temp dir
  (`flintshot-pin-<pid>-<n>.png`) are safe to delete at any time — a
  pin reads its image into memory once and holds no file handle. The
  editor→pin handoff writes the temp file, spawns the pin worker, and
  the pin worker reads + holds the image; the temp file may be reaped
  by the OS independently.
- **COSMIC applet (L1–L5 only):** the applet does not auto-start the
  daemon. If no daemon answers, the popup shows *"daemon not running —
  start with `flintshot daemon`"* and disables daemon-dependent
  buttons; the user starts the daemon manually (autostart/packaging is
  post-1.0).
- **COSMIC applet event feed:** the 2 s reconnect poll means a
  daemon restart takes up to 2 s for the popup to notice and
  re-subscribe. Events emitted while disconnected are not retroactively
  fetched (the daemon doesn't buffer for absent subscribers).

## Phase 4 — upload

Subcommand: `flintshot upload`

| Behavior                                                    | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ----------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| Imgur OAuth flow stores creds in OS keychain                |    |    |    |    |    |    |    |    |    |
| Upload from editor surfaces progress via IPC to tray        |    |    |    |    |    |    |    |    |    |
| URL copied to clipboard on success                          |    |    |    |    |    |    |    |    |    |
| Notification fires on success                               |    |    |    |    |    |    |    |    |    |
| Failure surfaces in UI with retry                           |    |    |    |    |    |    |    |    |    |
| Delete from tray using stored delete-hash                   |    |    |    |    |    |    |    |    |    |

## Phase 5 — live overlay

| Behavior                                                    | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ----------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| Hotkey opens transparent fullscreen overlay                 |    | N/A | N/A | N/A | N/A |    |    |    |    |
| Annotation drawn in overlay matches editor-window output    |    | N/A | N/A | N/A | N/A |    |    |    |    |
| Cross-monitor selection drag                                |    | N/A | N/A | N/A | N/A |    |    |    |    |
| Wayland falls back to editor-window cleanly                 | N/A |    |    |    |    | N/A | N/A | N/A | N/A |

## Phase 6 — Wayland global hotkeys via portal

| Behavior                                                    | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ----------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| `ashpd` portal prompts on first bind                        | N/A |    |    |    |    | N/A | N/A | N/A | N/A |
| Hotkey fires capture after bind                             | N/A |    |    |    |    | N/A | N/A | N/A | N/A |
| Graceful fallback when portal unsupported                   | N/A |    |    |    |    | N/A | N/A | N/A | N/A |

## How to verify a cell

1. Note the commit you're testing (`git rev-parse --short HEAD`).
2. `cargo build --release` (or the binary you're validating).
3. Perform the behavior exactly as documented. No creative
   interpretation; if the spec is ambiguous, file an issue and link it.
4. Paste the result into the cell: `PASS <commit> <YYYY-MM-DD>` or
   `FAIL <commit> <YYYY-MM-DD> #issue`.
5. Commit the table update separately from code changes — `chore:
   update test matrix for <phase>`.

## Issue tracking

If a cell turns **FAIL**, open an issue using this template:

```
- Platform: <L1 / M2 / W1 / …>
- Phase: <1 / 2 / …>
- Behavior: <copy the row label>
- Commit: <hash>
- Repro: <exact steps>
- Expected: <from the row label>
- Actual: <what happened>
- Logs: RUST_LOG=flintshot=debug,tokio=warn cargo run -- … output
```

Link the issue from the cell. Don't leave a bare **FAIL**.
