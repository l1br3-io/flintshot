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

## Phase 2 — daemon, tray, hotkeys, multi-monitor

Subcommand: `flintshot daemon`

| Behavior                                                    | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ----------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| `flintshot daemon` starts, tray icon appears                |    |    |    |    |    |    |    |    |    |
| Second `flintshot daemon` exits and signals the first       |    |    |    |    |    |    |    |    |    |
| Tray menu: Region / Fullscreen / Open last / Settings / Quit |    |    |    |    |    |    |    |    |    |
| Global hotkey fires capture on X11 / macOS / Windows        |    | N/A | N/A | N/A | N/A |    |    |    |    |
| Compositor keybind → `flintshot region` (Wayland fallback)  | N/A |    |    |    |    | N/A | N/A | N/A | N/A |
| Multi-monitor capture, combined image, correct geometry     |    |    |    |    |    |    |    |    |    |
| `--monitor N` selects the right monitor                    |    |    |    |    |    |    |    |    |    |
| Ellipse / Line / Marker / Pencil tools                     |    |    |    |    |    |    |    |    |    |
| Move / select existing annotation                          |    |    |    |    |    |    |    |    |    |
| Magnifier on region selection                              |    |    |    |    |    |    |    |    |    |
| Auto-incrementing number bubbles                           |    |    |    |    |    |    |    |    |    |

## Phase 3 — pin-to-desktop + COSMIC applet

Subcommand: `flintshot pin`

| Behavior                                                    | L1 | L2 | L3 | L4 | L5 | M1 | M2 | W1 | W2 |
| ----------------------------------------------------------- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| `flintshot pin <path>` opens borderless always-on-top win  |    |    |    |    |    |    |    |    |    |
| Pin draggable, resizable                                    |    |    |    |    |    |    |    |    |    |
| Multiple pins coexist independently                         |    |    |    |    |    |    |    |    |    |
| "Pin" button from editor works                              |    |    |    |    |    |    |    |    |    |
| COSMIC applet appears in panel (L5 only)                    | N/A | N/A | N/A | N/A |    | N/A | N/A | N/A | N/A |
| Applet popup buttons fire IPC commands to daemon            | N/A | N/A | N/A | N/A |    | N/A | N/A | N/A | N/A |
| Applet localizes (English baseline)                         | N/A | N/A | N/A | N/A |    | N/A | N/A | N/A | N/A |

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
