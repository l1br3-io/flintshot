---
status: active
started: 2026-09-14
---

# Spec: Native capture UX

## Objective

Collapse flintshot's capture UX to a **single Capture action** on
every surface, and make the **OS's own screenshot UI** do the
selecting wherever it exists. On Wayland the XDG Screenshot Portal's
*interactive* mode (the compositor's selection overlay — verified on
COSMIC by Spike B) replaces flintshot's region-selection windows; the
editor remains the default post-grab destination and flintshot's
value-add. `xcap` fallback keeps today's flow where no native UI
exists (X11, portal down, or an explicit pin).

Built on the shipped `native-capture-backends` foundation
(`Capturer` trait, portal backend, `select_backend`,
`--backend`/`[capture] backend`).

User stories:

- As a Wayland user, hitting Capture (hotkey / tray / app window /
  CLI) opens my compositor's own select-and-confirm UI; Enter returns
  exactly what I selected to flintshot.
- As any user, the same single Capture action works everywhere — one
  button in the home window, one tray item, one CLI subcommand.
- As a user who annotates, the editor still opens by default with
  the grab; `[capture] after_grab = "clipboard" | "upload"` opts into
  a window-less flow.
- As an X11 / portal-down user, nothing regresses: today's
  monitor-grab → editor/overlay flow runs, selected by the same
  single Capture action.
- As a contributor, `region` vs `fullscreen` no longer forks the
  codebase — one request, one worker path, one menu handler.

## Scope

**In scope:**

1. **Portal interactive mode**: `interactive: true` on the Screenshot
   call in the portal backend; Escape/dismiss maps to the existing
   `"cancelled"` capture error; the returned image feeds post-grab
   routing. Native path only (Wayland, portal selected).
2. **Post-grab routing**: `[capture] after_grab = "editor"
   (default) | "clipboard" | "upload"`, CLI-overridable
   (e.g. `--after clipboard`). Native path only — the fallback path
   always routes into today's editor/overlay flow (selection happens
   there; a raw monitor dump to clipboard is never right).
3. **`flintshot capture` subcommand**: replaces `region` and
   `fullscreen`, which are **purged** (clean break, per decision).
   Carries `--delay`, `--backend`, `--out`, `--standalone`, and the
   interaction flags; `--monitor` is dropped (selection is the OS
   UI's job; fallback uses the primary monitor).
4. **IPC fold**: `Request::Capture { delay_ms }` replaces
   `CaptureRegion`/`CaptureFullscreen` (schema is 0.x and all
   consumers ship together); `worker.rs` gains one `capture_argv`
   builder.
5. **Home window**: controls become **Capture** and **Open** (file
   dialog → editor, the `flintshot open` path); **Paste** is built
   as a placeholder hidden behind a compile-time const for a future
   clipboard→editor feature.
6. **Tray menu**: *Capture region*, *Capture fullscreen*, and the
   per-monitor submenu collapse to a single **Capture** item; the
   rest (open last, config, quit, status) unchanged.
7. **COSMIC applet**: same single-Capture collapse (drops its
   per-monitor entries too).
8. **Hotkeys**: `[hotkeys]` collapses to a single `capture` binding
   (X11/macOS/Windows via `global-hotkey`; the daemon's Wayland
   advice text becomes "bind a compositor keybinding to `flintshot
   capture`"). Legacy `region`/`fullscreen` keys in existing configs
   follow the string-tolerant policy: warn, don't bind.
9. **Docs**: AGENTS.md run examples, ARCHITECTURE.md (subcommand
   list, IPC schema, home-window description), TEST_MATRIX command
   sweep, CHANGELOG. ADR recording the UX decision.

**Out of scope (explicitly):**

- The **Paste** feature itself (placeholder only — later effort).
- Windows/macOS native backends (they stay xcap behind the trait).
- Any change to the editor's tool set, the upload backends, or the
  daemon lifecycle.
- `flintshot open` / `upload` / `auth` / `hotkeys` CLI semantics.
- Live-window capture (portal window picking) — interactive region
  selection only, whatever the compositor's UI itself offers.

## Technologies

| Concern                  | Mechanism                                        | Notes |
| ------------------------ | ------------------------------------------------ | ----- |
| Native selection         | Portal `Screenshot` with `interactive: true`     | Spike B verified on COSMIC: layer-surface UI, Enter=capture, Escape=`Cancelled` (code 1) — the existing error mapping already carries the `"cancelled"` substring |
| Backend selection        | `select_backend` (shipped)                       | Precedence unchanged: CLI > config > auto (Wayland: portal → xcap) |
| Post-grab routing        | Pure dispatch on `[capture] after_grab`          | editor / clipboard (arboard) / upload (existing upload path) |
| CLI                      | clap — one `capture` subcommand                  | Old subcommands removed, no aliases (decision) |
| IPC                      | `Request::Capture { delay_ms }`                  | Breaking schema change, accepted |
| Surfaces                 | egui home (`home.rs`), tray menu, COSMIC applet  | All collapse to one Capture action |

No new crates. arboard/image are already dependencies.

## Commands

```sh
cargo build                                   # default members
cargo build -p flintshot-cosmic               # applet (opt-in, system deps)
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -- capture                          # manual: native flow (Wayland)
cargo run -- capture --backend xcap           # manual: fallback flow pinned
cargo run -- capture --after clipboard        # manual: window-less flow
cargo run -- backends                         # manual: backend diagnostics
cargo test -p flintshot-core --test capture -- --ignored   # live smoke incl. interactive
```

No new system deps.

## Project structure

Primary touch points (details in the plan):

```
crates/flintshot-core/src/capture/portal.rs   # interactive option + response
crates/flintshot-core/src/config.rs           # after_grab, [hotkeys] capture
crates/flintshot-core/src/ipc.rs              # Request::Capture fold
crates/flintshot-app/src/cli.rs               # capture subcommand, after_grab dispatch
crates/flintshot-app/src/worker.rs            # capture_argv, dispatch arm
crates/flintshot-app/src/daemon.rs            # hotkey binding, notice text
crates/flintshot-app/src/app_window.rs        # bridge: request_capture
crates/flintshot-app/src/tray*.rs             # single Capture item
crates/flintshot-editor-egui/src/home.rs      # Capture/Open/(hidden Paste)
crates/flintshot-cosmic/src/app.rs            # single Capture action
```

## Flow

```
Capture (any surface)
  └─ select_backend(CLI --backend > [capture] backend > auto)
       ├─ portal (Wayland, up)          → interactive grab
       │     ├─ user selects, Enter     → image
       │     │    └─ after_grab: editor (default) | clipboard | upload
       │     └─ Escape / dismissed      → quiet exit (log, no error toast)
       └─ xcap (X11, portal down, pin)  → today's flow unchanged:
             monitor grab (primary) → overlay/editor selection → editor
```

Cancellation is not an error UX: the `"cancelled"` capture error
propagates from the backend, and the worker exits quietly (one log
line, no notification, exit 0).

## Config / CLI / IPC sketches

```toml
[capture]
backend = "auto"          # shipped
after_grab = "editor"     # editor | clipboard | upload  (new)

[hotkeys]
capture = "Ctrl+Shift+PrintScreen"   # single binding (region/fullscreen keys gone)
```

```rust
// cli.rs
Capture(CaptureArgs);   // --delay, --backend, --after, --out, --standalone, interaction flags

// ipc.rs
pub enum Request {
    Capture { delay_ms: u32 },
    OpenLast,
    UploadLast { backend: String },
    ReportSaved { path: PathBuf },
    Quit,
    Subscribe,
}
```

## Code style

Repo conventions verbatim (pedantic clippy, `#![forbid(unsafe_code)]`,
doc comments on public items, `#[non_exhaustive]`, thiserror in
core, no comments restating code). See
`docs/specs/native-capture-backends/spec.md` §Code style and
`.opencode/tech/rust/coding-style.md`.

## Testing strategy

- **Portal interactive option** — unit-test the options map carries
  `interactive: true` on the native path (pure seam over the request
  body builder); `#[ignore]`'d live test extends
  `portal_smoke_grabs_primary` with an interactive grab
  (manual: select, Enter; and Escape → `"cancelled"`).
- **Post-grab routing** — pure dispatch fn unit tests (editor /
  clipboard / upload / unknown-warns-and-editor).
- **Config** — `after_grab` round-trips; unknown value warns +
  defaults; `[hotkeys] capture` round-trip; legacy
  `region`/`fullscreen` keys warn and don't bind.
- **CLI** — parse tests for `capture` and its flags; `region` /
  `fullscreen` now fail to parse (pinned by test — the purge is
  intentional); `--after` validation mirrors `--backend`.
- **IPC/worker** — `Request::Capture` postcard round-trip;
  `capture_argv` builder tests (backend elision at default carried
  over).
- **Surfaces** — home window: button set + hidden Paste const;
  tray/applet menu structure tests where they exist today.
- **Manual matrix** — TEST_MATRIX section: native flow per Wayland
  desktop (COSMIC first-class), fallback flow on X11, cancellation
  paths, after_grab variants.

## Boundaries

- **Always**: keep `xcap` fallback byte-identical to today's flow;
  cancellation exits quietly; run the full finish gate before
  declaring done; update CHANGELOG (breaking CLI/IPC/config changes
  under a **Changed** heading).
- **Ask first**: touching the CaptureGuard/supersede handshake;
  portal call-shape changes beyond adding the `interactive` option;
  any new dependency.
- **Never**: no `unsafe`; no GUI types in core's public API; no
  removal of the editor; no aliases for `region`/`fullscreen`
  (decided: clean break); no Paste feature beyond the hidden
  placeholder.

## Success criteria

1. Full finish gate green on all crates (incl. `flintshot-cosmic`).
2. On the live COSMIC session: `flintshot capture` opens the OS
   selection UI; Enter → editor (default) with the exact selection;
   Escape → quiet exit; `--after clipboard` copies without windows.
3. `flintshot capture --backend xcap` reproduces today's fallback
   flow; `flintshot region` / `flintshot fullscreen` fail to parse.
4. Home window shows Capture + Open (Paste hidden); tray and COSMIC
   applet show a single Capture item; per-monitor entries gone.
5. `Request::Capture` round-trips; old variants don't compile
   (removed, not deprecated).
6. Existing configs with `[hotkeys] region = "…"` load with a
   warning and no binding; `[hotkeys] capture` binds and fires.
7. Docs sweep complete: AGENTS, ARCHITECTURE, TEST_MATRIX,
   CHANGELOG, ADR (UX decision + supersede of the "we have our own
   editor" Spike B note).

## Open questions

- ~~**Interactive return shape**~~ **Resolved (Spike C, 2026-09-14):**
  the portal returns exactly the selected region (dims track the
  drag; passthrough, no composite crop). Caveats recorded in
  `tasks/001-spike-interactive.md`: on the installed
  `xdg-desktop-portal-cosmic` (0.1.0pop1~abf258b) Escape is
  unreliable — sometimes code 1 (proper cancel), sometimes a
  spurious code 0 capture of a stale rect, sometimes no Response at
  all (hang). The interactive path therefore needs a long Response
  budget + hard outer bound with quiet timeout-exit, and
  read-then-delete for files that land in `~/Pictures/Screenshots`
  (not `/tmp`).
- **GNOME/KDE/wlr interactive behavior**: unmeasured; their UIs and
  prompt behavior may differ (TEST_MATRIX rows). Auto keeps xcap as
  the terminal fallback; pinning remains the escape hatch.
- **`--delay` on the native path**: keep (sleep before the
  interactive call) or reject with a clear error? Lean: keep —
  harmless, and scripts use it.
