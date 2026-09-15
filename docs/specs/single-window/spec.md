---
status: active
started: 2026-09-15
---

# Spec: Single-window app (routed home → editor)

## Objective

Refactor flintshot from a multi-window, multi-process UX (home window +
daemon-spawned worker editor windows) into a **single window** driven by
page routing, in the spirit of the Win11 Snipping Tool / CleanShot X:

- On first load the window shows three centered actions: **Capture**,
  **Open image**, **Paste**.
- A capture routes the *same window* (same process) to the **Editor
  page**.
- External triggers (hotkey, tray, CLI `flintshot capture` delegation)
  also route into the running app window when one exists; the
  worker-with-own-editor remains the fallback when none does.
- After an editor outcome (Save / Copy / Upload) the window **stays in
  the editor** — the editor chrome exposes the same three actions
  (capture, open, paste) so new content can be loaded without going
  home. `Esc` returns to the three-button home (the "empty state").

**Why:** managing multiple windows across two processes (home window +
worker editor + `flintshot open` child) is friction; one window that
routes is how the industry tools behave.

### Decisions (from brainstorm, 2026-09-15)

| Fork | Decision |
| ---- | -------- |
| Who grabs for app-window captures | **In-process** — the app window's pump thread runs the grab; no worker spawn, no image relay. Grab flow extracted from `cli.rs` into a shared module the worker path also calls. |
| External captures (hotkey/tray/CLI) | **Route into the running app window.** Daemon forwards the intent; worker-editor stays the fallback when no window answers. |
| After editor outcome | **Stay in editor.** Editor chrome gains Capture / Open / Paste actions. Home page = empty state. |
| Paste button | **Ship it** — clipboard image → Editor page, straight in-process. |

### Future, explicitly out of scope

- **Tabs / workspaces per capture** (multiple images open at once) —
  brainstormed later; the routing model here must not preclude it
  (page state stays a value, not baked-in window assumptions).
- Overlay interaction mode (`--overlay`) — unchanged, separate shell.

### User stories

1. As a user, I open `flintshot app` and get one window with three
   centered buttons; nothing else opens unless I act.
2. Capture: the window closes (not-in-the-shot, ADR-0047), the OS
   selection UI (or fallback monitor grab) runs, and the **same
   window** reopens as the editor with my capture.
3. Open image: file picker → editor page. No child process.
4. Paste: clipboard image → editor page. Empty clipboard → status
   message on home, no route.
5. Hotkey/tray capture while the app window is running: the same
   window closes, captures, and reopens on the editor page.
6. After Save/Copy/Upload the editor stays put; I can annotate again,
   or fire any of the three actions from the editor chrome.
7. `Esc` in the editor returns to the three-button home.
8. With no app window running, hotkey/tray/CLI captures behave exactly
   as today (worker process opens its own editor window).

### Acceptance criteria

1. `flintshot app` shows exactly one window at all times; no flow in
   this spec ever opens a second flintshot window in parallel.
2. App-window Capture: window closes **before** the grab (unmap grace
   preserved), grab runs in-process, editor page opens with the
   capture. No worker process is spawned for this flow.
3. Open: picked image loads into the editor page in-process; the
   `flintshot open` child spawn from the home window is gone.
4. Paste: clipboard image routes to the editor page; a non-image or
   empty clipboard is a status-line message, not an error dialog.
5. External capture intent with a registered app window: the daemon
   forwards it to the window (exact seam below), which runs the same
   close → grab → editor-page flow. Without an app window: today's
   worker-editor path, byte-for-byte.
6. CLI `capture --out`, `--standalone`, and explicit `--after` /
   interaction flags never route into the app window (the IPC request
   cannot carry them — existing rule, now also guarding the window).
7. Editor outcomes apply their side effects (file saved, clipboard
   copy, upload + URL, daemon report) and the page remains on the
   image. `Esc` routes to home; window close (X / Alt+F4) exits the
   app process (daemon unaffected).
8. Editor chrome exposes Capture / Open / Paste actions with the same
   semantics as the home buttons (including the close-before-grab
   dance for capture).
9. Supersede/Replace policy preserved: a newer capture intent replaces
   an in-flight grab or the editor's current image (unsaved
   annotations are lost — same semantics as today's editor recapture).
10. The prepare handshake (tray hide + app-hidden ack, ADR-0036/0047)
    wraps in-process grabs exactly as it wraps worker grabs.

## Technologies

No new dependencies. Pure Rust, existing crates:

- `flintshot-app` — routed window shell, in-process capture flow,
  daemon forwarding.
- `flintshot-editor-egui` — `HomeApp` (empty state), `EditorApp`
  (page mode + chrome actions), page router entry on `EguiShell`.
- `flintshot-core` — `EditorState` / `EditorOutcome` reused; new IPC
  seam (event or request) for capture forwarding, kebab-case serde.
- `arboard` (already a dep) for Paste.

## Commands

From the repo root (see `AGENTS.md` for the full set):

```sh
cargo build                                  # default members
cargo run -- app                             # the single window
cargo run -- capture                         # CLI capture (worker fallback / routing)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

## Project structure

Where the work lands (files may shift during planning; boundaries hold):

| Concern | Location |
| ------ | -------- |
| Routed outer loop (one eframe epoch per page) | `crates/flintshot-app/src/app_window.rs` |
| Shared grab flow (extracted from `cli.rs`) | `crates/flintshot-app/src/capture_flow.rs` (new) |
| Page definitions + router entry | `crates/flintshot-editor-egui/src/shell.rs` (`run_app(page)`) |
| Home page (three buttons, empty state) | `crates/flintshot-editor-egui/src/home.rs` |
| Editor page mode + chrome actions | `crates/flintshot-editor-egui/src/{app,chrome,toolbar}.rs` |
| Capture-forwarding seam (daemon → window) | `crates/flintshot-core/src/ipc.rs` + `crates/flintshot-app/src/{daemon,ipc_server,ipc_client}.rs` |
| Worker fallback path (unchanged behavior) | `crates/flintshot-app/src/{cli,worker}.rs` |

## Code style

Repo conventions (see `AGENTS.md`, `.opencode/tech/rust/coding-style.md`):
`#![forbid(unsafe_code)]`, pedantic clippy, no `unwrap`/`expect` outside
tests, doc comments on public items in core, `#[non_exhaustive]` on
plausible-extension types, thiserror in libs / color_eyre in bins.

The routing model in one snippet (the outer loop picks the next page
per window epoch — no runtime window mutation):

```rust
/// What the single window shows for its next lifetime.
#[derive(Debug, Clone, PartialEq)]
pub enum Page {
    /// Empty state: three centered actions.
    Home,
    /// Editor session over one image.
    Editor(EditorState),
}
```

## Testing strategy

Same levels as the repo today:

- **Unit (pure):** page-routing state machine, capture-forwarding
  decision (daemon side: window-present vs worker), paste's
  clipboard-image decode path, outcome handling that keeps the page
  alive.
- **Headless egui:** the existing `Context::default()` pattern for
  directive/accelerator/chrome tests (see `home.rs` tests).
- **Existing suites keep passing:** `HomeCore` close/reopen tests adapt
  (editor-page epochs replace force-show-on-`SessionEnded` where the
  in-process flow owns the transition), the worker fallback tests are
  untouched.

## Boundaries

- **Always:** preserve the close-before-grab invariant (ADR-0046/0047)
  for every capture entry point; keep the worker fallback path working
  when no app window exists; run fmt + clippy + tests before declaring
  done; `flintshot-core` public API stays GUI-free (enforced test).
- **Ask first:** new IPC events/requests or any protocol change;
  touching daemon session/supersede accounting; deleting or repurposing
  any `docs/specs/*` machinery; config-key additions.
- **Never:** a second parallel flintshot window in any flow covered
  here; `unsafe`; GUI types in core; losing an editor outcome's side
  effects (save/copy/upload) to a route change.

## Success criteria

1. All ten acceptance criteria demonstrable by hand on a Wayland
   session (`cargo run -- app`): three-button home → capture → editor
   page → copy → still in editor → Esc → home.
2. Hotkey capture mid-editor routes the window through the capture
   dance and lands on the new image (old unsaved image replaced).
3. `pkill` the app window; hotkey capture still opens the worker
   editor exactly as today.
4. Full gate green: fmt, clippy `-D warnings`, workspace tests, doc
   tests.
5. `CHANGELOG.md` updated under `## [Unreleased]`; an ADR recorded for
   the single-window model (supersedes the multi-window stance of the
   app-window spec; references ADR-0046/0047 invariants).

## Open questions

1. **Forwarding seam:** daemon → app-window capture forwarding rides
   the existing presence connection (`announce_app_window`) or a new
   subscription event? Decide in plan.md.
2. **`flintshot open` / tray `OpenLast`:** route into the running app
   window too (proposed: yes, same rule as captures), or keep spawning
   the worker editor?
3. **Esc in editor:** proposed `Esc → home` (empty state). Alternative:
   `Esc` closes the app outright. Confirm during spec review.
4. **Window title/WM class per page:** home is `flintshot`, editor is
   `flintshot — annotate` today. Per-epoch titles keep `.desktop`
   `StartupWMClass` routing working — confirm during implementation.
5. **`HomeCore` reuse:** how much of the `SessionEnded`-driven restore
   survives once in-process captures own their transitions (editor
   epochs may make hide/show bookkeeping mostly worker-fallback-only)?
   Answer in plan.md.
