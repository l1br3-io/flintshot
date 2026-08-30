# Test Matrix

Manual cross-platform verification guide. CI covers `fmt`, `clippy`,
`cargo test`, and the MSRV build; **this file covers everything CI
can't** — actual displays, compositors, permissions, keychains, and
network paths.

Each phase has three parts:

1. **Test setup** — what you need before testing that phase.
2. **Test script** — ordered, step-by-step with exact commands and the
   expected output you should see. Run top-to-bottom in one session.
3. **Matrix** — the recording grid. Paste your result into each cell.

Phase ownership: this matrix is live as of Phase 1. Rows are added as
features land. Mark each cell with the date and commit hash when
verified, or open an issue and link it in the cell.

## Legend

- **PASS** — verified working
- **PARTIAL** — works with caveats (note them in the cell)
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

> **Focus on your own platform first.** You don't need all nine filled
> to ship — one fully-verified platform per phase is the minimum bar.
> Leave others blank for contributors to pick up.

## Getting started

### Build a release binary

All manual testing uses a release build (faster startup, closer to what
users run):

```sh
cargo build --release
```

The binary lands at `./target/release/flintshot`. Either copy it onto
`$PATH` or invoke it by path. The examples below assume it's on
`$PATH` as `flintshot`; substitute `./target/release/flintshot` if not.

To run from source without copying:

```sh
cargo run --release -- <subcommand> [args]
```

### Configuration

flintshot reads its config from (in priority order):

| Platform | Path                                                    |
| -------- | ------------------------------------------------------- |
| Linux    | `~/.config/flintshot/config.toml`                       |
| macOS    | `~/Library/Application Support/flintshot/config.toml`   |
| Windows  | `%APPDATA%\flintshot\config.toml`                       |

If the file doesn't exist, defaults apply. Print the resolved config
(the merged defaults + your overrides) with:

```sh
flintshot config
```

Key defaults you'll be testing against:

| Section    | Key                     | Default                     |
| ---------- | ----------------------- | --------------------------- |
| `[hotkeys]`| `region`                | `Ctrl+Shift+PrintScreen`    |
| `[hotkeys]`| `fullscreen`            | `Ctrl+Alt+PrintScreen`      |
| `[pin]`    | `default-opacity`       | `1.0`                       |
| `[pin]`    | `copy-on-close`         | `false`                     |

### Credentials (Phase 4 only)

Upload testing needs an Imgur API Client-ID for the authenticated path.
Get one (free, no app review needed):

1. Go to <https://api.imgur.com/oauth2/addclient>.
2. Choose **OAuth 2 authorization without a callback URL**.
3. Copy the **Client-ID** (not the secret).

Store it before testing the authenticated rows:

```sh
flintshot auth imgur          # prompts "Imgur Client-ID: " (hidden input)
# or, non-interactive:
IMGUR_CLIENT_ID=xxxx cargo run --release -- auth imgur
```

The anonymous path needs no credentials at all.

### Logging

When something fails, re-run with verbose logging to capture diagnostics
for the issue template:

```sh
RUST_LOG=flintshot=debug,tokio=warn flintshot <command>
```

### What goes where

flintshot is a daemon + worker-process architecture (ADR-0019). For
testing it helps to know which process owns which UX:

| You invoked…                    | Process that runs it            | Notifies?       |
| ------------------------------- | ------------------------------- | --------------- |
| `flintshot region`/`fullscreen` | daemon spawns a worker          | tracing log     |
| `flintshot daemon`              | the daemon itself               | —               |
| `flintshot pin <path>`          | standalone pin worker           | —               |
| `flintshot upload <path>`       | standalone upload worker        | desktop notify  |
| `flintshot auth imgur`          | inline in the CLI process       | —               |

**Only upload fires a desktop notification.** Save, copy, and pin use
`tracing` log lines only (no toast), so keep a terminal open to watch
for `RUST_LOG=flintshot=info` confirms.

---

## Phase 1 — capture + editor window

### Test setup

```sh
cargo build --release
cargo test --workspace --all-targets          # all green before manual
cargo test -p flintshot-core --test capture -- --ignored smoke_can_grab_primary
```

The smoke test grabs the primary monitor. On Wayland it may pop an
`xdg-desktop-portal` permission dialog; on macOS it may prompt for
Screen Recording permission. Both are expected on first run.

### Test script

Run in order. Each step lists the **command**, the **expected** result,
and the **matrix row** it satisfies.

**1. Build + gate (rows 1–3).**

```sh
cargo build --release
cargo test --workspace --all-targets
cargo test -p flintshot-core --test capture -- --ignored smoke_can_grab_primary
```

Expected: all pass. The smoke test asserts `img.width() > 0 &&
img.height() > 0`. On macOS without permission it prints
`SKIP: capture permission denied (…)` and exits 0 — that's a soft-skip,
not a failure.

**2. Help surface (row 4).**

```sh
flintshot --help
```

Expected: lists `daemon`, `region`, `fullscreen`, `config`, `pin`,
`open`, `upload`, `auth` subcommands.

**3. Region capture → editor (rows 5–8).**

```sh
flintshot region
```

Expected: editor window opens with the screen capture as its
background. On Wayland the first capture per session may show a portal
prompt (expected, not a bug). On macOS the first run triggers a Screen
Recording permission prompt.

**4. Annotation tools (rows 9–13).**

In the editor, with each tool selected from the toolbar:

| Tool      | Toolbar label | Action                              | Expected                       |
| --------- | ------------- | ----------------------------------- | ------------------------------ |
| Rectangle | `▭ Rectangle` | drag a box                          | filled rectangle at cursor     |
| Arrow     | `→ Arrow`     | drag                                | line with arrowhead at end     |
| Text      | `T Text`      | click, type, `Enter`                | readable glyphs render         |
| Blur      | `░ Blur`      | drag over text (try `Gaussian`/`Pixelate`) | uniform blur, no seams   |
| Select    | `⬚ Select`    | drag, then `✂ Crop to selection`    | canvas crops to the rect       |

**5. Persistence (rows 14–15).**

Change the color picker and the `Width` slider. Close the editor,
re-open (`flintshot region` again), confirm both persisted:

```sh
flintshot config | grep -A2 '\[ui\]'
```

Expected: the `[ui]` section reflects your last pick.

**6. Save / copy / undo (rows 16–19).**

| Shortcut  | Expected                                       |
| --------- | ---------------------------------------------- |
| `Ctrl+S`  | native file dialog (`rfd`); saved PNG decodes  |
| `Ctrl+C`  | image on clipboard (paste into an image app)   |
| `Ctrl+Z`  | last annotation removed                        |
| `Ctrl+Y`  | removed annotation restored                    |
| `Esc`     | editor closes, no save                         |

**7. HiDPI / performance (rows 20–22).**

On a HiDPI or fractional-scale display, capture and zoom into the saved
PNG — it should be sharp, not blurry. Time cold start to editor-open;
target under 200 ms on commodity hardware. Check the release binary
size: `ls -lh ./target/release/flintshot` — target under 25 MB.

### Matrix

| Behavior                                                | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| `cargo build` succeeds                                  |    |    |    |    |    |    |    |    |    |
| `cargo test --workspace --all-targets` passes           |    |    |    |    |    |    |    |    |    |
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

---

## Phase 2 — daemon, tray, hotkeys, IPC

### Test setup

```sh
cargo build --release --features tray
flintshot daemon &        # background it; you'll kill it at the end
```

If your desktop has no SNI host (StatusNotifierItem), the tray icon
won't appear but the daemon should still bind IPC + hotkeys.

### Test script

**1. Build + single-instance (rows 1–4).**

```sh
cargo build --release --features tray
flintshot daemon &            # tray icon "flintshot" appears
flintshot daemon              # expected: non-zero exit, stderr:
#   another flintshot daemon is already running
#   (single-instance lock "flintshot-daemon" held)
flintshot daemon --replace    # old daemon quits, new one takes over
```

**2. Tray menu (rows 5–8).**

Click the tray icon. Expected menu items (exact labels):

- `Capture region` → spawns editor
- `Capture fullscreen` → spawns editor on primary monitor
- `Fullscreen on monitor →` submenu → one entry per monitor, formatted
  `"<index> · <name>"` (or `"<index> · <name> (primary)"`)
- `Open config` → opens `$EDITOR` on `config.toml`
- `Quit` → daemon exits cleanly

**3. Tray resilience (row 9).**

Kill any SNI host (or test on a desktop without one). The daemon should
stay alive and log a tray-unavailable warning.

**4. Global hotkeys (rows 10–12).**

Press the defaults (X11/macOS/Windows only — Wayland has no global
hotkey support, see caveat):

- `Ctrl+Shift+PrintScreen` → region capture
- `Ctrl+Alt+PrintScreen` → fullscreen capture

On Wayland, confirm the hotkey does **not** fire (expected) and that
the compositor-keybind workaround is the documented path.

**5. Hot-reload (row 13).**

Edit `config.toml`, change `[hotkeys] region` to something new (e.g.
`"Ctrl+Shift+P"`), save. Without restarting the daemon, press the new
chord — it should capture.

**6. IPC subscribe (row 14).**

In a second terminal, subscribe to the event stream:

```sh
flintshot daemon   # already running; the CLI delegates…
```

Actually, the IPC is postcard-framed binary on the socket at
`$XDG_RUNTIME_DIR/flintshot.sock` (Linux). There's no built-in
`subscribe` CLI subcommand; the COSMIC applet (Phase 3.B) is the
subscriber to verify against. Trigger a capture and watch the applet's
event feed for `CaptureStarted → EditorOpened → Saved <path>`.

**7. Delegation vs standalone (rows 15–17).**

```sh
# with daemon running — CLI delegates:
flintshot region
# with daemon stopped — CLI runs standalone:
flintshot daemon --replace  # then Ctrl-C to stop it
flintshot region
# force standalone even with a daemon up:
flintshot region --standalone
```

Expected: all three open the editor; only the delegated path
centralises events for subscribers.

**8. Clean shutdown (row 18).**

```sh
kill %1          # or Ctrl-C the backgrounded daemon
```

Expected: clean exit, socket file at
`$XDG_RUNTIME_DIR/flintshot.sock` is reclaimed.

### Matrix

| Behavior                                                    | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ----------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| `cargo build --features tray` succeeds                      |    |    |    |    |    |    |    |    |    |
| `flintshot daemon` starts, tray icon appears (`--features tray`) | |  |    |    | pending-maintainer-L5 — fix-tray-sni runbook step 2 (fix landed 5c78497+8ac5c4f 2026-08-29, finding 3) |    |    |    |    |    |
| `flintshot daemon` (no tray feature) still binds IPC + hotkeys |   |    |    |    |    |    |    |    |    |
| Second `flintshot daemon` fails "already running"           |    |    |    |    |    |    |    |    |    |
| `flintshot daemon --replace` swaps daemons cleanly          |    |    |    |    |    |    |    |    |    |
| Tray *Capture region* / *Capture fullscreen* spawn editor   |    |    |    |    | pending-maintainer-L5 — fix-tray-sni runbook step 4 (never run on L5 — finding 1; blocked by finding 3) |    |    |    |    |
| Tray *Fullscreen on monitor →* lists each monitor; each captures the right one | | | | | pending-maintainer-L5 — fix-tray-sni runbook step 4 (never run on L5 — finding 1; blocked by finding 3) | | | | |
| Tray *Open config* opens `$EDITOR` on `config.toml`         |    |    |    |    | pending-maintainer-L5 — fix-tray-sni runbook step 4 (never run on L5 — finding 1; blocked by finding 3) |    |    |    |    |
| Tray *Quit* exits the daemon cleanly                        |    |    |    |    | pending-maintainer-L5 — fix-tray-sni runbook step 4 (never run on L5 — finding 1; blocked by finding 3) |    |    |    |    |
| Tray visible with no SNI host → daemon stays alive (warning)|    |    |    |    | pending-maintainer-L5 — fix-tray-sni runbook step 7 (warn-and-continue path untouched by the fix) |    |    |    |    |
| Watcher killed mid-session → icon returns within ~30 s (daemon running) | N/A | N/A | N/A | N/A | pending-maintainer-L5 — fix-tray-watcher-resurrect runbook step 2 (fix landed 547753a 2026-08-30) | N/A | N/A | N/A | N/A |
| Fresh daemon with watcher dead → icon appears at startup | N/A | N/A | N/A | N/A | pending-maintainer-L5 — fix-tray-watcher-resurrect runbook step 3 (fix landed 547753a 2026-08-30) | N/A | N/A | N/A | N/A |
| Global hotkey fires capture on X11 / macOS / Windows        |    | N/A | N/A | N/A | N/A |    |    |    |    |
| Wayland: hotkey does NOT fire; compositor-keybind workaround logged | N/A |    |    |    |    | N/A | N/A | N/A | N/A |
| Edit `[hotkeys]`, save → new chord works without restart    |    |    |    |    |    |    |    |    |    |
| IPC client `Subscribe` then `broadcast` → receives `Event`  |    |    |    |    |    |    |    |    |    |
| `flintshot region` with daemon running delegates (events centralised) |    |    |    |    | fix landed 9ce5497+54d0bf1 2026-08-28 (fork-loop guard + ambient runtime); regression tests green, manual L5 pass pending — L5 findings 1–2 |    |    |    |    |
| `flintshot region` with no daemon → standalone editor       |    |    |    |    | fix landed 54d0bf1 2026-08-28 (ambient runtime); regression tests green, manual L5 pass pending — L5 finding 2 |    |    |    |    |
| `flintshot region --standalone` forces in-process capture   |    |    |    |    | fix landed 54d0bf1 2026-08-28 (ambient runtime); regression tests green, manual L5 pass pending — L5 finding 2 |    |    |    |    |
| Ctrl-C → clean daemon shutdown (socket reclaimed)           |    |    |    |    |    |    |    |    |    |

---

## Phase 2b — annotation tools + magnifier

### Test setup

```sh
cargo build --release
flintshot region
```

All five tools route through `flintshot-core::render::render()`, so the
saved/copied PNG matches the live editor (WYSIWYG per ADR-0015). The
magnifier is toolkit-owned (egui-only; no core abstraction, per
ARCHITECTURE.md §What stays toolkit-specific) and reads
`EditorState::image` directly. See ADR-0022 for the Marker/Pencil
freehand smoothing + per-pixel-once compositing decision.

### Test script

For each tool: select it from the toolbar, draw on the canvas, then
**save (`Ctrl+S`) and re-open the PNG** to verify WYSIWYG (last row).

| Tool     | Label        | Steps                                          | Expected                                          |
| -------- | ------------ | ---------------------------------------------- | ------------------------------------------------- |
| Ellipse  | `◯ Ellipse`  | drag preview → release → `Ctrl+Z` → click inside | outline ellipse; undo removes; click re-selects   |
| Line     | `／ Line`    | drag → release → click near the line           | thin line; click-near selects (tolerance-based)   |
| Marker   | `🖍 Marker`  | drag over text                                 | translucent highlight; text visible through stroke |
| Pencil   | `✏ Pencil`   | draw a freehand squiggle                       | smooth opaque curve (Catmull-Rom), no jaggies     |
| Number   | `# Number`   | click 3× in different spots → `Ctrl+Z`         | `1`, `2`, `3` bubbles; undo decrements to `2`     |

**Magnifier** (hold-trigger, toolkit-owned):

1. Hold `Space` — a 4× inset magnifier appears centred on the cursor.
2. Release — it hides.
3. Set `[ui] magnifier-always-on = true` in config, restart editor —
   the magnifier stays up while drawing (no hold needed).
4. Set `[ui] magnifier-modifier = "Alt"` — now `Alt` is the hold key.

**WYSIWYG verification (final row):**

```sh
flintshot region
# draw annotations with several tools
# Ctrl+S → save to /tmp/wysiwyg.png
# Esc to close
flintshot open /tmp/wysiwyg.png
```

Re-open the saved PNG: every annotation should rasterize identically to
what you saw live.

### Matrix

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

---

## Phase 3 — pin-to-desktop + COSMIC applet

### Test setup

```sh
cargo build --release
flintshot region           # capture something, Ctrl+S → /tmp/shot.png
flintshot daemon &         # needed for PinLast/OpenLast IPC rows
```

A pin is a borderless always-on-top floating window showing a captured
image; each pin runs as its own worker process so a pin crash can't kill
the editor or daemon (ADR-0019), and multiple pins coexist. All three
pin entry points (CLI, editor button, daemon `PinLast`) converge on
`EditorShell::run_pin`. The COSMIC applet is a pure IPC client (depends
on `flintshot-core` + `libcosmic` only — no `egui`, no `flintshot-app`).

### Phase 3.A — pin-to-desktop test script

**1. Basic pin (rows 1–3).**

```sh
flintshot pin /tmp/shot.png
flintshot pin /tmp/shot.png --at 100,100 --size 400x300
flintshot pin /tmp/shot.png --opacity 0.7 --no-clipboard
```

Expected: borderless window opens at the given position/size; the
opacity override dims it; `--no-clipboard` suppresses close-copy.

**2. Window behavior (rows 4–8).**

- The pin floats above ordinary windows (always-on-top).
- Left-drag anywhere inside → moves the window.
- Drag across a second monitor → window follows.
- `Esc` → closes.
- Double-click → closes.

**3. Hover-opacity (rows 9–10).**

With `[pin] default-opacity = 0.5`: the pin rests dimmed; moving the
cursor over it makes it fully opaque; leaving returns it to 0.5.

**4. Resize (rows 11–12).**

Drag the bottom-right resize handle. The image scales — aspect ratio
locked, no crop or distortion.

**5. Copy-on-close (rows 13–14).**

```sh
# default: no copy
flintshot pin /tmp/shot.png          # close it → paste in an image app → nothing
# opt in:
# (set [pin] copy-on-close = true in config, or:)
flintshot pin /tmp/shot.png --no-clipboard  # --no-clipboard overrides even if config says true
```

To test `copy-on-close = true`: set it in config, pin, close, then paste
into an image app — the pinned image should be on the clipboard.

**6. Persistence + multiplicity (rows 15–17).**

```sh
flintshot pin /tmp/shot.png &
flintshot pin /tmp/shot.png &
flintshot pin /tmp/shot.png &
```

Close the editor and the daemon — all three pins stay open as
independent windows.

**7. Editor pin button (rows 18–19).**

```sh
flintshot region
# draw a selection box with ⬚ Select
# click 📌 Pin
```

Expected: the editor composes the final image to a temp file
(`flintshot-pin-<pid>-<n>.png` under the OS temp dir), spawns a pin
worker, and closes. With an active selection, only the selected region
is pinned.

**8. `open` + IPC commands (rows 20–25).**

```sh
flintshot open /tmp/shot.png        # loads existing image into editor
# via the daemon (PinLast / OpenLast) — exercised through the tray or
# COSMIC applet, or via a raw IPC client. PinLast with no prior save
# returns: Event::Error { code: "no-last-capture",
#   message: "no capture has been saved yet" }
```

**9. Events (rows 26–27).**

After an editor save, subscribers see
`Event::Saved { path: "<path>" }`. After `PinLast`, they see
`Event::Pinned { id: <N> }` — ids are monotonic per daemon session.

### Phase 3.A matrix

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

#### Test setup

```sh
cargo build --release -p flintshot-cosmic
flintshot daemon &
# launch the COSMIC applet (via your panel / cosmic-panel)
```

#### Test script

Click the panel button → popup opens with: *Capture region*,
*Capture fullscreen*, *Fullscreen on monitor* submenu, *Pin last*,
*Open last*, *Open config*, *Quit*. Trigger each and confirm the daemon
acts. Watch the event-feed status line update through
*Capturing… → Editor opened → Saved \<path\>*.

For the daemon-absent path: stop the daemon, open the popup — it shows
*"daemon not running — start with `flintshot daemon`"*, daemon-dependent
buttons grey out, and *Open config* stays enabled. Restart the daemon;
the popup re-subscribes within ~2 s.

### Phase 3.B matrix

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

---

## Phase 4 — upload

### Test setup

```sh
cargo build --release
flintshot region           # capture, Ctrl+S → /tmp/shot.png (your test image)
flintshot auth imgur       # optional: store a Client-ID for authenticated rows
# (see "Credentials" in Getting Started above)
```

Every entry point converges on the same `flintshot upload` worker
process (ADR-0019); `ImgurBackend` is the only backend — anonymous by
default, Client-ID authenticated via keychain opt-in (ADR-0024,
ADR-0025, ADR-0026).

### Test script

**1. Anonymous upload — the happy path (rows 3–6).**

```sh
flintshot upload /tmp/shot.png
```

Expected output:

- **stdout:** the Imgur URL on its own line (e.g.
  `https://imgur.com/xxxxxxx`). This is the *only* stdout line — pipe
  with `| tail -1` to extract it.
- **clipboard:** the URL is copied immediately (paste into a browser to
  confirm).
- **desktop notification:** summary `flintshot upload`, body
  `Uploaded to <url>`.
- **IPC event:** if a daemon is running with subscribers, they see
  `Event::Uploaded { url, delete-handle }`. The COSMIC applet feed shows
  `Uploaded <url>`.

```sh
flintshot upload /tmp/shot.png | tail -1    # just the URL
```

**2. Editor Upload button (rows 1–2).**

```sh
flintshot region
# (optional) draw a selection box with ⬚ Select
# click ☁ Upload
```

Expected: editor composes the image to a temp file
(`flintshot-upload-<pid>-<n>.png`), spawns the upload worker, then
closes. With an active selection, only the selected region is uploaded.
The URL arrives via the worker's notification + clipboard — not in the
editor (see caveat: `EditorOutcome::Uploaded` carries an empty
placeholder).

**3. Authenticated upload (row 8).**

```sh
flintshot auth imgur          # enter your Client-ID at the hidden prompt
flintshot upload /tmp/shot.png
```

Expected: same UX as anonymous, but the request carries a `Client-ID`
header (raises the rate limit). Confirm in the daemon log
(`RUST_LOG=flintshot=debug`) that the upload used the stored credential.

**4. Auth lifecycle (rows 14–17).**

```sh
flintshot auth imgur            # set: stderr → "Imgur Client-ID stored in keychain."
flintshot auth imgur            # re-set: overwrites; same confirmation
flintshot auth imgur --clear    # stderr → "Imgur Client-ID removed from keychain."
flintshot auth imgur --clear    # no entry: stderr → "No Imgur Client-ID stored." exit 0
```

Each step should trigger the platform keychain unlock prompt (macOS
keychain, Windows Credential Manager, Linux Secret Service / GNOME
Keyring).

**5. Error paths (rows 9–11).**

```sh
flintshot upload /nonexistent.png
# expected: stderr error, non-zero exit, no stdout URL

# network failure: disconnect Wi-Fi / block imgur.com, then:
flintshot upload /tmp/shot.png
# expected: stderr error, notification body "Upload failed: …",
#   non-zero exit, no hang
```

Anonymous rate limit (429) is per-IP (~50/hr), so hitting a real 429 is
impractical manually. The mapping is covered headless by the mockito
test `rate_limit_maps_to_backend_error`; a local mock server can stand
in if you want to exercise the manual cell.

**6. UploadLast over IPC (rows 12–13).**

With a daemon running and a prior save:

```sh
# trigger Request::UploadLast via the COSMIC applet or a raw IPC client
```

Expected: upload worker spawns, subscribers see
`Event::UploadStarted { backend: "imgur" }` immediately, then
`Event::Uploaded { url, delete-handle }` on success. With no prior
save, the daemon replies `Event::Error { code: "no-last-capture",
message: "no capture has been saved yet" }`.

### Matrix

| Behavior                                                    | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ----------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| Editor *Upload*: compose → temp PNG → worker spawns, editor closes | | | | | | | | | |
| Editor *Upload* with active selection uploads just the region | | | | | | | | | |
| Upload success: URL is stdout's last line (`flintshot upload x.png \| tail -1`) | | | | | | | | | |
| Upload success: URL on the clipboard immediately             |    |    |    |    |    |    |    |    |    |
| Upload success: "Uploaded to \<url\>" desktop notification   |    |    |    |    |    |    |    |    |    |
| Upload success: `Event::Uploaded` reaches IPC subscribers (applet: "Uploaded \<url\>") | | | | | | | | | |
| Anonymous upload succeeds zero-config (no keyring entry)     |    |    |    |    |    |    |    |    |    |
| Authenticated: `flintshot auth imgur` set → uploads send Client-ID (rate-limit lift) | | | | | | | | | |
| `flintshot upload /nonexistent` → clear stderr error, non-zero exit | | | | | | | | | |
| Network failure mid-upload → stderr error, "Upload failed: …" notification, non-zero exit, no hang | | | | | | | | | |
| Anonymous rate limit (429) → clear backend error, non-zero exit | | | | | | | | | |
| `Request::UploadLast` over IPC spawns upload worker + emits `Event::UploadStarted` | | | | | | | | | |
| `UploadLast` with no prior save → `Error { code: "no-last-capture" }` | | | | | | | | | |
| `flintshot auth imgur` set: hidden prompt, keychain entry stored, confirmation on stderr | | | | | | | | | |
| `flintshot auth imgur` re-set overwrites the stored Client-ID |    |    |    |    |    |    |    |    |    |
| `flintshot auth imgur --clear`: entry removed; uploads revert to anonymous (log confirms) | | | | | | | | | |
| `flintshot auth imgur --clear` with no entry → "No Imgur Client-ID stored.", exit 0 | | | | | | | | | |
| Keychain unlock prompt handled (macOS keychain / Windows credential manager / Linux Secret Service) | | | | | | | | | |

### Phase 4 known caveats

- **No tray / COSMIC-applet *Upload last* menu item shipped.**
  `Request::UploadLast` is reachable over IPC (scripts / custom
  clients); the spec draft's applet/tray button is a follow-up.
- **Upload failures don't reach daemon subscribers.** The worker owns
  failure UX (notification + stderr + non-zero exit); subscribers see
  `UploadStarted` with no matching `Uploaded`. See ADR-0025.
- **Anonymous rate limit is per-IP** (~50 uploads/hr), so manually
  hitting a real 429 is impractical; the mapping is covered headless
  by the mockito test (`rate_limit_maps_to_backend_error`), and a
  mock server can stand in for the manual cell.
- **The editor's `EditorOutcome::Uploaded` carries an empty
  placeholder URL** — the outcome signals "upload initiated"; the real
  URL arrives via the worker's daemon report, not the editor.

---

## Phase 5 — live overlay

### Test setup

```sh
cargo build --release
flintshot region --overlay
```

The overlay is **capture-then-overlay** (ADR-0028): `xcap` grabs the
screen first, then a fullscreen borderless viewport shows the frozen
pixels — the desktop is never visible beneath it. All annotation runs
through the unchanged `EditorState` dispatcher (the same engine as the
editor window), so tool behaviour is pinned by the ten-tools parity
test; the rows below verify the *overlay shell* around it.

Overlay-proper rows are **N/A on L2–L5**: on Wayland a fullscreen
borderless window cannot reliably be raised above other windows or
grab input, so the overlay is demoted to the Phase 1 editor window
(logged, never silent). Wayland platforms test the fallback rows
instead.

### Test script

**1. Mode resolution (rows 1–4, 21).**

```sh
flintshot region --overlay        # overlay opens (log: mode source "flag")
flintshot region --editor         # editor window, byte-identical to Phase 1
# set [ui] interaction-mode = "overlay" in config, then:
flintshot region                  # overlay opens (log: mode source "config")
# set interaction-mode = "hologram" (unknown), then:
flintshot region                  # warn + editor window (fallback)
flintshot region --overlay --editor   # clap error: mutually exclusive
```

**2. Overlay shell (rows 1, 18).**

The overlay opens fullscreen showing the frozen capture. Press `Esc`:
the process exits 0 with no file written, no clipboard write, and no
worker spawned (check `RUST_LOG=flintshot=info` + paste into an image
app to confirm nothing changed).

**3. Selection chrome (rows 5–9).**

Drag anywhere → a region selection appears; everything outside dims.
Hover each of the 8 handles (corners + edge midpoints) → cursor
changes per direction; drag to resize. Drag inside the region → it
moves, clamped to the screen edges. Watch the `W × H` badge track the
live size near the selection (it flips below near the top screen
edge). Rest the cursor (no modifier) → the pixel-zoom magnifier
appears; move away → it hides.

**4. Floating toolbar (row 10).**

With a selection present, the two-row toolbar floats below its
bottom-left; drag the selection near the bottom edge → it flips
above; near any edge → it clamps into the viewport. It hides during
fresh region drags and follows resize/move gestures.

**5. Annotation inside the selection (rows 11–12).**

Activate each of the ten tools (chip click or single-key
`S R E A L M P T B N`) and draw inside the region: strokes are clipped
at the region edge — exactly where the region-cropped output cuts.
`Ctrl+Z` / `Ctrl+Y` undo/redo through the toolbar's arrows. Text /
Number / Blur / Marker render identically to the editor window (the
parity is test-pinned; this verifies it live).

**6. Outputs (rows 13–17).**

| Action                     | Expected                                                    |
| -------------------------- | ----------------------------------------------------------- |
| `Enter` (or ✓ / Copy chip) | region copied to clipboard, overlay closes; paste an image app |
| Save chip                  | native `rfd` dialog; saved PNG is the region-cropped shot    |
| Pin chip                   | overlay closes; pin worker floats the region                 |
| Upload chip                | overlay closes; upload worker → URL notification + clipboard |

With the daemon running, subscribers see the same `Saved` /
`Pinned` / `Uploaded` events as the editor window produces.

**7. Wayland fallback (rows 19–20).**

With `[ui] interaction-mode = "overlay"` on a Wayland session:
`flintshot region` opens the **editor window**; the log names the
fallback (`RUST_LOG=flintshot=info`). An explicit
`flintshot region --overlay` is honored best-effort with a warning
naming the risk — expect compositor-dependent raise/behaviour.

**8. Idle repaint + fullscreen (rows 22–23).**

Leave the overlay open, hands off: CPU use drops to ~0 (on-demand
repaint — no 60 fps burn). Also run `flintshot fullscreen --overlay`:
the same overlay flow over the full-monitor frozen capture — the
region is drawn by the first drag, exactly like `region` (nothing is
pre-selected).

### Matrix

| Behavior                                                    | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ----------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| `flintshot region --overlay` opens fullscreen frozen-frame overlay | | N/A | N/A | N/A | N/A |    |    |    |    |
| `[ui] interaction-mode = "overlay"` (no flag) opens overlay |    | N/A | N/A | N/A | N/A |    |    |    |    |
| Unknown `interaction-mode` value → warn + editor window     |    |    |    |    |    |    |    |    |    |
| `--overlay --editor` together → clap mutual-exclusion error |    |    |    |    |    |    |    |    |    |
| Drag draws region; dim mask outside the region              |    | N/A | N/A | N/A | N/A |    |    |    |    |
| 8 resize handles resize; per-direction resize cursors       |    | N/A | N/A | N/A | N/A |    |    |    |    |
| Move-drag inside region moves it, clamped to the screen     |    | N/A | N/A | N/A | N/A |    |    |    |    |
| `W × H` badge tracks live size; flips near the top edge     |    | N/A | N/A | N/A | N/A |    |    |    |    |
| Magnifier shows on hover (no modifier hold), hides away     |    | N/A | N/A | N/A | N/A |    |    |    |    |
| Toolbar anchors to selection; flips above / clamps to viewport | | N/A | N/A | N/A | N/A |    |    |    |    |
| All ten tools annotate inside the selection, clipped at edges |  | N/A | N/A | N/A | N/A |    |    |    |    |
| Single-key tool switching + `Ctrl+Z`/`Ctrl+Y` work in overlay |  | N/A | N/A | N/A | N/A |    |    |    |    |
| `Enter` (or ✓ / Copy chip) copies region; paste verified    |    | N/A | N/A | N/A | N/A |    |    |    |    |
| X11: process lingers ≤60 s post-copy (clipboard owner rule) |    | N/A | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| Save chip → `rfd` dialog writes region-cropped PNG          |    | N/A | N/A | N/A | N/A |    |    |    |    |
| Pin chip → pin worker floats the region                     |    | N/A | N/A | N/A | N/A |    |    |    |    |
| Upload chip → upload worker; URL notification + clipboard   |    | N/A | N/A | N/A | N/A |    |    |    |    |
| `Esc` cancels: exit 0, no clipboard/file/worker side effects |   | N/A | N/A | N/A | N/A |    |    |    |    |
| Wayland: config `overlay` demoted to editor window; log names fallback | N/A |    |    |    |    | N/A | N/A | N/A | N/A |
| Wayland: explicit `--overlay` honored best-effort with warning | N/A |    |    |    |    | N/A | N/A | N/A | N/A |
| `--editor` / `interaction-mode = "editor"`: editor window unregressed | | | | | | | | | |
| Overlay idle CPU ≈ 0 (on-demand repaint, no 60 fps burn)    |    | N/A | N/A | N/A | N/A |    |    |    |    |
| `flintshot fullscreen --overlay`: same overlay flow, full-monitor capture | | N/A | N/A | N/A | N/A |    |    |    |    |

### Phase 5 known caveats

- **Frozen frame (by design, ADR-0028):** the desktop is not live
  behind the overlay — animations stop and anything changing behind
  it is invisible until accept/cancel. This is the price of a
  deterministic, portable, WYSIWYG-by-construction selection surface.
- **Primary monitor only.** The overlay captures the monitor `xcap`
  flags primary (the `flintshot region` default; `--monitor N`
  overrides); multi-monitor spanning is deferred.
- **Wayland `--overlay` is best-effort.** A plain fullscreen window
  may not raise above other windows or grab input on every
  compositor; the warning says so. The default path (config-set
  overlay) is demoted to the editor window instead — honest UX, not a
  hidden bug.
- **X11 clipboard linger.** After an accept-with-copy the worker
  process stays alive up to 60 s holding the `arboard` handle (X11's
  owner-based clipboard), then exits — `ps` will show it outlive the
  window close. macOS / Windows own the clipboard at the OS level and
  exit immediately.

## Phase 6 — Wayland global hotkeys via portal

### Test setup

```sh
cargo build --release
cargo test --workspace --all-targets      # X11 regression gate: existing tests pass unmodified
pgrep -af xdg-desktop-portal              # confirm which frontends run
cat /usr/share/xdg-desktop-portal/portals/*.portal   # does any list GlobalShortcuts?
RUST_LOG=flintshot=info ./target/release/flintshot daemon &
```

Happy-path rows need a desktop whose portal frontend implements
`org.freedesktop.impl.portal.GlobalShortcuts` — KDE Plasma, or a
COSMIC build whose `xdg-desktop-portal-cosmic` lists it (see the
caveat below; the L5 machine's packaged build does not). Fallback
rows are verifiable anywhere, including a mocked no-bus run.

### Test script

**1. Gates (rows 1–2).**

```sh
cargo test --workspace --all-targets        # HotkeyRegistry/X11 units unmodified + green
./target/release/flintshot hotkeys bind     # no daemon yet → error names the fix, exit ≠ 0
```

**2. Fresh daemon start (rows 3–4).**

Start the daemon on Wayland. Expected log: `Wayland session: global
hotkeys via xdg-desktop-portal GlobalShortcuts`, then either the
silent attach line (`portal session attached; bound shortcuts …` —
**no dialog at startup**, by design) or a classified fallback notice
(see step 5). Ctrl-C: `daemon exited cleanly`, exit 0, socket at
`$XDG_RUNTIME_DIR/flintshot.sock` reclaimed.

**3. Bind flow (rows 5–8, portal-capable desktop only).**

```sh
flintshot daemon &
flintshot hotkeys bind
```

Dialog lists `region` / `fullscreen` with the `[hotkeys]` chords as
suggested bindings. Bind, then press the chord → region capture fires
(same dispatch as tray). Restart the daemon → attach line reports the
bound set, no dialog. Re-run `hotkeys bind`, pick different keys →
only the new set fires. Dismiss the dialog instead → daemon healthy,
CLI reports `bind-cancelled`.

**4. Hot-reload (row 9).**

With the daemon running, edit `[hotkeys] region` in `config.toml`.
Expected: info notice "hotkey suggestions changed; re-run
`flintshot hotkeys bind`", no dialog, no crash, previously bound
shortcuts keep firing.

**5. Fallback (rows 10–11).**

On a desktop without GlobalShortcuts (GNOME, sway, or the L5 COSMIC
package build): daemon logs the classified warn notice naming the
compositor-keybind workaround; tray + IPC still serve; Ctrl-C exit 0.
For the no-bus variant:

```sh
RUST_LOG=flintshot=info DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent \
  ./target/release/flintshot daemon
# expect: "no D-Bus session bus reachable…" at info, IPC bound, Ctrl-C → exit 0
```

**6. X11 regression (row 12, L1).**

Re-run the Phase 2 hotkey + tray + hot-reload rows on an X11 session:
menu has no *Bind shortcuts…* item, `[hotkeys]` edit re-registers as
before.

### Matrix

| Behavior                                                    | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ----------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| Workspace tests green; existing X11 hotkey units unmodified  |    | N/A | N/A | N/A | PASS 1da727a 2026-08-20 | N/A | N/A | N/A | N/A |
| `hotkeys bind` with no daemon → clear error, non-zero exit   | N/A |    |    |    | PASS 1da727a 2026-08-20 | N/A | N/A | N/A | N/A |
| Daemon start on Wayland selects portal path (log line)      | N/A |    |    |    | PASS 1da727a 2026-08-20 | N/A | N/A | N/A | N/A |
| Portal attach logs bound set; **no dialog at startup**      | N/A |    |    |    |    | N/A | N/A | N/A | N/A |
| `hotkeys bind` dialog lists both shortcuts w/ suggested chords | N/A |    |    |    |    | N/A | N/A | N/A | N/A |
| Bound chord fires capture through the shared worker path    | N/A |    |    |    |    | N/A | N/A | N/A | N/A |
| Daemon restart re-attaches bound shortcuts, no dialog       | N/A |    |    |    |    | N/A | N/A | N/A | N/A |
| Second `bind` replaces bindings; only new set fires         | N/A |    |    |    |    | N/A | N/A | N/A | N/A |
| `[hotkeys]` edit mid-daemon: suggestions notice, no dialog/crash, old bindings keep firing | N/A |    |    |    | PARTIAL 1da727a 2026-08-20 (notice + no-dialog verified; "keep firing" needs a bound portal) | N/A | N/A | N/A | N/A |
| Fallback, frontend lacks GlobalShortcuts: classified warn, tray/IPC alive, Ctrl-C exit 0 | N/A |    |    |    | PASS 1da727a 2026-08-20 (real: packaged cosmic portal lacks the interface) | N/A | N/A | N/A | N/A |
| Fallback, no session bus: classified info notice, IPC alive, exit 0 | N/A |    |    |    | PASS 1da727a 2026-08-20 (mocked `DBUS_SESSION_BUS_ADDRESS`) | N/A | N/A | N/A | N/A |
| X11: Phase 2 hotkey/tray/reload flows unchanged (no bind tray item) |    | N/A | N/A | N/A | N/A | N/A | N/A | N/A | N/A |

### Phase 6 known caveats

- **"COSMIC ships it" is package-dependent.** The L5 machine's
  `xdg-desktop-portal-cosmic 0.1.0pop1` (Ubuntu 24.04-based) does
  **not** list GlobalShortcuts in its Interfaces, so the daemon
  correctly falls back there. Upstream COSMIC implements it — check
  the installed `*.portal` files before expecting the happy path.
  Happy-path cells (rows 4–8) await a KDE (L3) or
  GlobalShortcuts-capable COSMIC pass; see ADR-0029.
- **X11 code path re-verified via an env-spoofed XWayland run**
  (56e783f, 2026-08-20): with `WAYLAND_DISPLAY` /
  `XDG_SESSION_TYPE` unset, the daemon takes the `global-hotkey`
  branch — both default chords register, a `[hotkeys]` edit
  re-registers with the new chord (`re-registered global hotkeys
  after a config change`), and Ctrl-C exits 0 with the socket
  reclaimed. Registration + reload semantics are unchanged by
  Phase 6; actual key *firing* and the tray-menu check still need
  a real L1 (X11 session) pass — XWayland grabs register but
  never fire.
- **App-id in the dialog may be generic** for a windowless daemon
  (portal matches bus credentials against installed `.desktop`
  files). Cosmetic; `flintshot.desktop` matching the binary name
  mitigates. Record what the dialog actually shows when filling row 5.
- **Portal-owned persistence:** bindings outlive the daemon in the
  portal's store (per-app); `hotkeys bind` re-binds the session's set
  rather than accumulating.

---

## Launcher desktop entry — app-menu integration

Effort: `launcher-desktop-entry`. Linux-only by design (XDG desktop
entries + hicolor icons); macOS / Windows cells are N/A pending the
packaging stretch goal. The entry, its desktop actions, and the
autostart variant are validated headless by
`crates/flintshot-app/tests/desktop_files.rs` (grammar + `Exec=`
↔ CLI sync); the rows below cover what a test can't — the actual
launcher, session, and login flows.

### Test setup

```sh
just install-user
```

Expected: release build + three files land under `~/.local` —

```sh
ls ~/.local/bin/flintshot \
   ~/.local/share/applications/flintshot.desktop \
   ~/.local/share/icons/hicolor/scalable/apps/flintshot.svg
```

`just install-user` builds with the `tray` feature on by default
(the launcher promise: daemon + tray), so the GTK dev libs
(`libgtk-3-dev`, `libayatana-appindicator3-dev` on Debian/Ubuntu)
must be present at build time; the installed binary links
`libgtk-3` (`ldd ~/.local/bin/flintshot | grep libgtk-3`). The
appindicator backend itself is dlopened at runtime — a missing SNI
host degrades to no-tray, daemon still runs. `FEATURES=''` selects
the minimal no-tray build (row 12).

Daemon-absent verification: `pgrep -af flintshot` before / after each
step.

### Test script

**1. Launcher discovery + activation (rows 1–5).**

Open the app menu / launcher, type "Flintshot". Expected: the entry
appears with the flint/spark SVG icon. Activate it → the daemon
process starts (`pgrep -af 'flintshot daemon'`); with a tray build,
the tray icon appears. Activate it again → no second daemon and no
second tray icon (single-instance lock; the second process exits).

**2. Desktop actions (rows 6–7).**

Right-click / long-press the launcher entry. Expected menu actions:
*Capture region*, *Capture fullscreen*. Each triggers the standard
capture flow — same editor window, portal prompts, and fallbacks as
the tray / CLI paths on your platform (Phase 1 / Phase 5 caveats
apply unchanged).

**3. Autostart (rows 8–9).**

```sh
AUTOSTART=1 just install-user
ls ~/.config/autostart/flintshot-autostart.desktop
```

Log out and back in. Expected: the daemon is running with no manual
action (tray icon present on a tray build).

**4. Uninstall + idempotent reinstall (rows 10–11).**

```sh
just uninstall            # remove the autostart entry too if installed
pgrep -af flintshot       # daemon (if any) unaffected — uninstall removes files only
ls ~/.local/bin/flintshot 2>&1   # expected: No such file or directory
```

Then re-run `just install-user` over the removed install, and once
more over the existing install — launcher entry works identically
after both (recipes are idempotent; `cp --remove-destination` handles
pre-existing files/symlinks).

**5. Minimal-build path (row 12).**

```sh
FEATURES='' just install-user
ldd ~/.local/bin/flintshot | grep -c libgtk-3   # expected: 0
```

Activate the launcher entry: daemon starts (IPC + hotkeys + desktop
actions work) but no tray icon. Re-run plain `just install-user`
afterwards to restore the tray build.

### Matrix

| Behavior                                                    | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ----------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| `just install-user`: binary + `.desktop` + icon land under `~/.local` |    |    |    |    | PASS 54383b1 2026-08-28 (tray build; `ldd` links libgtk-3) | N/A | N/A | N/A | N/A |
| Launcher lists "Flintshot" with the SVG icon                 |    |    |    |    |    | N/A | N/A | N/A | N/A |
| Activating the entry starts the daemon (no terminal)         |    |    |    |    | PASS 54383b1 2026-08-28 (via `gtk-launch` / `g_desktop_app_info_launch_action`, the XDG activation path; daemon detaches, lock + IPC bound) | N/A | N/A | N/A | N/A |
| Tray icon appears after activation (tray-default build)    |    |    |    |    | FAIL 54383b1 2026-08-28 — SNI item never registers with COSMIC's StatusNotifierWatcher; fix landed 5c78497+8ac5c4f 2026-08-29 (GLib context pump in the daemon loop); regression tests green, manual L5 pass pending — L5 finding 3; re-run via the fix-tray-sni runbook on launcher task 007 | N/A | N/A | N/A | N/A |
| Second activation: no second daemon / tray icon              |    |    |    |    | PASS 54383b1 2026-08-28 (second process refuses via single-instance lock; exactly one daemon) | N/A | N/A | N/A | N/A |
| Desktop action *Capture region* opens the standard capture flow | |    |    |    | FAIL 54383b1 2026-08-28 — delegated capture fork-loops, no editor (see caveats; issue pending) | N/A | N/A | N/A | N/A |
| Desktop action *Capture fullscreen* ditto                    |    |    |    |    | FAIL 54383b1 2026-08-28 (same dispatch path as *Capture region*) | N/A | N/A | N/A | N/A |
| `AUTOSTART=1 just install-user` writes the autostart entry   |    |    |    |    | PASS 54383b1 2026-08-28 | N/A | N/A | N/A | N/A |
| Re-login after autostart install → daemon running, no manual action | | | | | | N/A | N/A | N/A | N/A |
| `just uninstall` removes every installed file (bin, entries, icon, autostart) | | | | | PASS 54383b1 2026-08-28 (no leftovers) | N/A | N/A | N/A | N/A |
| Reinstall after uninstall, and install over existing, both idempotent | | | | | PASS 54383b1 2026-08-28 (entry still activates the daemon after both) | N/A | N/A | N/A | N/A |
| `FEATURES='' just install-user`: minimal binary (no libgtk-3), daemon runs, no tray | | | | | PASS 54383b1 2026-08-28 (0 libgtk-3 links; daemon runs; owns no bus names — no SNI object at all. Desktop actions broken by the fork loop above, orthogonal to the build) | N/A | N/A | N/A | N/A |

### Known caveats

- **Tray build needs GTK dev libs.** `just install` /
  `install-user` build with `--features tray` by default, which
  needs `libgtk-3-dev` + `libayatana-appindicator3-dev` at build
  time (Debian/Ubuntu names). On failure the recipe prints the
  prerequisite and the `FEATURES=''` escape hatch (minimal no-tray
  binary; IPC, hotkeys, and desktop actions still work).
  *Clean-container verification of the failure hint deferred*
  (2026-08-28) — re-ride the next task-007 re-run after the
  blockers below are fixed.
- **System install needs elevation externally** — the recipes never
  sudo. `sudo env "PATH=$PATH" just install` (rustup's cargo path
  must survive sudo's `secure_path`).
- **Launcher caching.** If the icon or entry doesn't refresh
  immediately, the recipes already run `update-desktop-database` /
  `gtk-update-icon-cache` when present; otherwise re-login or run
  them manually. GNOME has been known to need a session restart to
  pick up new icons.

#### L5 pass findings (2026-08-28, commit 54383b1) — three blockers, all pre-existing product bugs outside this effort's scope

Findings 1–2 are **fixed** by the `fix-delegated-capture` effort
(`9ce5497`, `54d0bf1`) and finding 3 by `fix-tray-sni`
(`5c78497`, `8ac5c4f`); automated regression tests are green for
all three, manual L5 re-verification pending — see the annotations
below.

1. **Delegated captures fork-loop; no editor ever opens** (fails
   rows 6–7, exit criteria 1–2). `worker.rs::region_argv` /
   `fullscreen_argv` spawn `flintshot region` **without
   `--standalone`**, and `cli.rs::run_region` re-delegates
   whenever the daemon is up — so the daemon-spawned worker
   delegates right back, and the daemon spawns the next worker.
   Observed: ~470 worker processes in 8 s (~60–100/s) from a
   single desktop action; each generation connects, writes one
   frame, exits; the daemon log fills with
   `IPC connection ended … early eof` (one per generation) and no
   editor window appears. Affects **every** daemon-mediated
   capture path (tray menu, desktop actions, CLI-with-daemon,
   hotkeys) since Phase 2's detect-and-delegate; never caught
   because the Phase 2 tray-capture matrix rows were never run.
   Fix shape: mark daemon-spawned capture workers
   `--standalone` (or equivalent env/argv guard) + a
   no-re-delegation regression test.

   **Fixed** in `9ce5497` (2026-08-28, `fix-delegated-capture`
   Task 1): `region_argv` / `fullscreen_argv` append
   `--standalone`; regression test
   `capture_workers_spawn_standalone_to_break_delegation_loop`
   plus the updated `RecordingSpawner` dispatch assertions
   (`crates/flintshot-app/src/worker.rs`) pin the argv contract.
   **Manual L5 re-verification pending** — no desktop session
   exists in the dev container; the maintainer runs spec exit
   criteria 1–3 before merge.
2. **Standalone editor workers crash on at-spi sessions**
   (blocks the fallback path, and pin/upload windows share it).
   `flintshot region --standalone` (or with no daemon running)
   panics before any window opens:
   `there is no reactor running, must be called from the context
   of a Tokio 1.x runtime` at `zbus-5.18.0/src/abstractions/
   executor.rs`. Chain: eframe → egui-winit → accesskit_winit →
   accesskit_unix → atspi → zbus 5. zbus 5 (tokio feature)
   requires an ambient tokio reactor; window-opening workers have
   none (the CLI's one-shot delegate runtime is already dropped).
   zbus 5 entered the tree with Phase 6's `ashpd` (commit 23d85b6)
   and unified accesskit onto it — so this likely affects **every
   desktop with at-spi running** (GNOME/KDE included), not just
   COSMIC. Sessions without `org.a11y.Bus` skip the path and work.
   Fix shape: give window workers a dropped-in ambient tokio
   runtime (or pin accesskit's zbus back to 4.x / async-io
   backend).

   **Fixed** in `54d0bf1` (2026-08-28, `fix-delegated-capture`
   Task 2): new `flintshot-app::ambient` module enters a
   multi-thread tokio runtime around every window path
   (`run_editor_with_config`, `run_overlay_with_config`,
   `run_pin_window`) for exactly the duration of the window loop;
   regression tests in `crates/flintshot-app/src/ambient.rs`
   proxy the exact zbus-5 panic condition via
   `tokio::runtime::Handle::try_current()` (no handle on a plain
   thread, visible inside `with_ambient_runtime`, gone after).
   **Manual L5 re-verification pending** — as above.
3. **Tray SNI item never registers with COSMIC's watcher** (fails
   row 4, exit criterion 1). The daemon (tray build) publishes
   `/StatusNotifierItem` + `/org/ayatana/NotificationItem` on its
   bus name, but `RegisteredStatusNotifierItems` on
   `org.kde.StatusNotifierWatcher` (cosmic-applet-status-area)
   never includes it — no tray icon appears. Other apps
   (syncthing-gtk, pcloud) register fine on the same watcher.
   Needs hands-on debugging in the tray-icon/appindicator init
   path (registration call ordering vs watcher availability).

   **Fixed** in `5c78497` + `8ac5c4f` (2026-08-29, `fix-tray-sni`
   Tasks 1–2): root cause was not registration ordering — the
   daemon's platform loop never iterated the GLib main context
   libappindicator/muda drive from, so the
   `RegisterStatusNotifierItem` call (and menu-event delivery)
   queued on sources nobody dispatched. The loop now pumps the
   default GLib context every iteration before draining tray
   events (bounded: ≤64 non-blocking `iteration(false)` calls,
   early-out when idle). Regression tests in
   `crates/flintshot-app/src/daemon.rs` (`glib_pump` module) pin
   the seam headless: idle source never fires unpumped, fires
   after the pump, a self-re-arming source is bounded by the cap,
   and the real `run_event_loop` dispatches only via its own
   pumping. The no-SNI-host warn-and-continue path is untouched
   (expected behavior unchanged). **Manual L5 pass pending** — no
   desktop session exists in the dev container; the maintainer
   runs the spec's exit criteria (icon registers, every menu
   action dispatches, watcher-started-after-daemon race,
   no-SNI-host + X11 unregressed) before merge — the runbook
   below is that protocol, step by step.

#### L5 runbook — fix-tray-sni (protocol only; every row it names is pending-maintainer-L5)

Recorded by `fix-tray-sni` task 005. This block is the exact
verification protocol for the three fixes above — **it records no
results and predicts none**. Execute it in one sitting on the L5
box, paste each command's output next to its step, and fill the
matrix cells the steps name (Phase 2 rows 2 and 6–10, L5 column).
Launcher rows 4/6/7 are **not** re-run here — they re-run on the
`launcher-desktop-entry` effort's task 007
(`docs/specs/launcher-desktop-entry/tasks/007-manual-pass-closeout.md`)
once this effort lands.

Build the tray binary and keep a log tail open (plain cargo keeps
`tray` opt-in; `just install-user` builds it by default if you
want the launcher path instead):

```sh
cargo build --release --features tray
RUST_LOG=flintshot=info ./target/release/flintshot daemon
```

**Step 1 — baseline probe (daemon not yet started).**

```sh
pgrep -af flintshot    # expected: no flintshot processes
gdbus call --session --dest org.kde.StatusNotifierWatcher \
  --object-path /StatusNotifierWatcher \
  --method org.kde.StatusNotifierWatcher.GetRegisteredStatusNotifierItems
```

Expected: a list **without** a flintshot entry (other apps such
as syncthing-gtk / pcloud may legitimately appear). Save the
output for comparison.

**Step 2 — start the daemon → item registers + icon renders**
(Phase 2 row 2, L5).

```sh
RUST_LOG=flintshot=info ./target/release/flintshot daemon &
sleep 2
gdbus call --session --dest org.kde.StatusNotifierWatcher \
  --object-path /StatusNotifierWatcher \
  --method org.kde.StatusNotifierWatcher.GetRegisteredStatusNotifierItems
```

Expected: the list now includes a flintshot entry (a unique bus
name such as `:1.NN` plus the object path the watcher consumed),
and the icon is visible in the status area
(cosmic-applet-status-area). If the entry is absent, capture
`RUST_LOG=flintshot=debug` daemon output and the probe result for
the issue — do not debug blind.

**Step 3 — which SNI interface did the watcher consume?**
(answers `fix-tray-sni` spec open question 1). Read the flintshot
entry's object-path suffix from step 2: `/StatusNotifierItem` =
KDE interface, `/org/ayatana/NotificationItem` = Ayatana. The
daemon exports both (finding 3); this records which one the
watcher actually registered. If the suffix is ambiguous, inspect:

```sh
gdbus introspect --session --dest <flintshot bus name from step 2> \
  --object-path /StatusNotifierItem
```

Paste the answer into the results alongside this runbook — no
code change is expected either way.

**Step 4 — menu sweep** (Phase 2 rows 6–9, L5). Click the tray
icon and walk every item:

| Menu item                        | Expected                                            |
| -------------------------------- | --------------------------------------------------- |
| `Capture region`                 | exactly **one** editor window opens (findings 1–2 ride the same dispatch); `pgrep -af flintshot` shows one `region --standalone` worker, no fork loop |
| `Capture fullscreen`             | one editor on the primary monitor                   |
| `Fullscreen on monitor →`        | one entry per monitor, `"<index> · <name>"` (`" (primary)"` on the primary); each entry captures the right monitor |
| `Bind shortcuts…`                | portal bind dialog on a GlobalShortcuts-capable desktop; on the L5 packaged portal (lacks the interface — Phase 6 caveat) the classified fallback notice is the expected outcome |
| `Open config`                    | `$EDITOR` opens `config.toml`                       |
| `Quit`                           | daemon exits; `pgrep -af flintshot` empty; `$XDG_RUNTIME_DIR/flintshot.sock` reclaimed |

**Step 5 — restart ordering: watcher comes up after the daemon**
(spec success criterion 3). Find the watcher host, take it down,
run the daemon watcher-less, then restore the watcher:

```sh
gdbus call --session --dest org.freedesktop.DBus \
  --object-path /org/freedesktop/DBus \
  --method org.freedesktop.DBus.GetNameOwner org.kde.StatusNotifierWatcher
# maps to the cosmic-applet-status-area process; then:
pkill -f cosmic-applet-status-area        # or stop the panel your way
./target/release/flintshot daemon --replace   # daemon starts with no watcher
pgrep -af cosmic-applet-status-area || echo "watcher still down"
# re-launch the status area / panel the way your session does
# (re-login also works), then repeat the step-2 probe
```

Expected: while the watcher is down the daemon stays alive and
error-free (appindicator retries internally, silently); once the
watcher returns the flintshot entry appears in the probe and the
icon renders — **without** restarting the daemon.

**Step 6 — X11 session sanity** (spec success criterion 4). Log
into an X11 session (any machine) and repeat steps 2 and 4 with
the same tray build. Expected: the icon appears in an XEmbed
system tray and the menu works — record against Phase 2 row 2 in
the column matching the session used (L1 for an Ubuntu X11 box).

**Step 7 — no-SNI-host sanity** (Phase 2 row 10, L5; spec
success criterion 4). With no SNI host reachable the daemon must
warn and continue. The no-bus variant is fully scriptable:

```sh
RUST_LOG=flintshot=info DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent \
  ./target/release/flintshot daemon
# expected: tray-unavailable warning, IPC binds, Ctrl-C → exit 0
```

Expected: warn-and-continue, daemon functional, clean exit. (A
desktop that simply has no SNI host works equally well as the
test environment.)

Minor notes (non-blocking):

- `gtk-launch` from a terminal stays attached to the daemon's
  output (it doesn't fork); real launchers detach properly. Use
  `setsid gtk-launch flintshot` when scripting.
- The daemon takes >1–2 s after SIGTERM to release the
  single-instance lock; rapid `pkill` → restart cycles fail with
  "already running" (the `--replace` path handles this via
  `wait_for_socket_to_free`; scripts should sleep ~2 s).
- Daemon startup logs ~100 × `Ignoring a wl_output with version
  < 4.` on COSMIC (wayland-client noise from monitor
  enumeration). Cosmetic; pre-existing.

---

## L5 runbook — fix-tray-watcher-resurrect

Recorded by `fix-tray-watcher-resurrect` task 005 (fix landed
`547753a` 2026-08-30). Protocol only — **it records no results and
predicts none**. Execute in one sitting on the L5 box (COSMIC,
Wayland) with debug logging so watchdog ticks are visible, and fill
the two Phase 2 matrix rows this block names (L5 column):

```sh
cargo build --release --features tray
RUST_LOG=flintshot=debug ./target/release/flintshot daemon
```

**Step 1 — baseline probe (daemon not yet started).**

```sh
pgrep -af flintshot    # expected: no flintshot processes
gdbus call --session --dest org.kde.StatusNotifierWatcher \
  --object-path /StatusNotifierWatcher \
  --method org.kde.StatusNotifierWatcher.GetRegisteredStatusNotifierItems
```

Expected: a list **without** a flintshot entry. Save for
comparison; this probe is the assertion tool for every step
below.

**Step 2 — kill the watcher mid-session → icon returns within
~30 s** (row "Watcher killed mid-session", L5). With the daemon
running and the icon visible, kill the tray host and wait one
watchdog interval:

```sh
pkill -f cosmic-applet-status-area
# icon vanishes; the daemon must stay alive and quiet
sleep 35
gdbus call --session --dest org.kde.StatusNotifierWatcher \
  --object-path /StatusNotifierWatcher \
  --method org.kde.StatusNotifierWatcher.GetRegisteredStatusNotifierItems
```

Expected: the flintshot entry is back in the list and the icon
renders again — with **no** manual watcher restart and **no**
daemon restart. The debug log shows the probe going unowned and
the COSMIC poke succeeding (`poke reached the SNI watcher`). If
the entry is absent after ~35 s, capture the daemon's debug
output (it names both poke attempts) for the issue.

**Step 3 — fresh daemon with the watcher dead → icon appears at
startup** (row "Fresh daemon with watcher dead", L5). Keep the
watcher dead, restart the daemon, and check the startup poke:

```sh
pkill -f flintshot; sleep 2          # let the single-instance lock go
pgrep -af cosmic-applet-status-area || echo "watcher still down"
RUST_LOG=flintshot=debug ./target/release/flintshot daemon &
sleep 5
# repeat the step-1 probe
```

Expected: the flintshot entry appears without any manual busctl —
the startup barrier (`ensure_watcher`) probes, pokes, and waits
(≤ 2 s) for the watcher **before** building the tray, so
registration is deterministic (amended in task 006 after the
fire-and-forget variant lost this exact race live). Note the entry
typically lands ~1 s after activation (libappindicator's own
re-registration).

**Step 4 — healthy-session regression + OQ2 residency** (spec
exit criteria 3 + open question 2; no matrix row — pass/fail note
here). With the watcher alive and the daemon restarted, watch two
intervals:

```sh
sleep 65
grep -c "SNI watcher" <the daemon's debug log>
```

Expected: the daemon logs exactly two `NameHasOwner`-shaped quiet
cycles — one probe per 30 s, **no pokes, no output above
`debug`**, no extra processes (`pgrep -af cosmic-applet-status-area`
still shows exactly one). Then check OQ2: the watcher must still
be the *same* PID as after step 3 — if the COSMIC watcher
idle-exits with zero items and the watchdog's poll creates a
spawn/exit cycle (new PID per interval), record it here; the
back-off guard is a fenced follow-up, not this effort.

**Step 5 — X11 sanity** (no matrix row; regression note). On any
X11 session, run the daemon with the same build for a couple of
minutes. Expected: probes succeed (an X11 SNI host or none at
all), failed pokes — if there is no host — stay at `debug`, and
the daemon's behavior is otherwise unchanged.

---

## How to verify a cell

1. Note the commit you're testing (`git rev-parse --short HEAD`).
2. `cargo build --release` (or the binary you're validating).
3. Follow the **Test script** for that phase — it lists the exact
   command and the expected output. If what you see doesn't match, it's
   a FAIL; file an issue (below).
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
- Expected: <from the Test script>
- Actual: <what happened>
- Logs: RUST_LOG=flintshot=debug,tokio=warn cargo run -- … output
```

Link the issue from the cell. Don't leave a bare **FAIL**.
