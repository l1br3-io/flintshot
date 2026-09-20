# Plan: tray-show-hide-toggle — one tray item, both directions

## Overview

Replace the tray's **Open app window** item with a single static
**Show/Hide flintshot** toggle. One new IPC pair
(`Request::ToggleWindow` / `Event::AppToggle`, appended per the
postcard variant-index rule); the daemon resolves the direction with
the existing app-window **presence oracle** — present → forward the
toggle to the window (spawn nothing), absent → the `OpenApp`
choreography spawns it. The window resolves a forwarded toggle from
its own surface state: visible → graceful exit (X-close semantics),
capture-hidden → reopen. The retired `flintshot.app` menu id is
pinned dead by test.

## Architecture decisions

- **Presence-gated forward-or-spawn, not broadcast-always.** The
  `OpenApp` pattern (broadcast + unconditional spawn) would race the
  exiting window's app-lock release and can resurrect a window the
  user just asked to hide. `ToggleWindow` instead joins
  `forwards_to_app_window()` (worker.rs) and rides the same
  connection-layer routing (`ipc_server.rs`) and
  `CaptureDispatch::dispatch_open_blocking` seam that
  `OpenImage`/`OpenLast` already use: presence → broadcast
  `Event::AppToggle` only; no presence → fall through to the plain
  `dispatch` arm (broadcast `AppFocus` + spawn `app --standalone`).
  No new probe, no connection-layer code changes, no `single.rs`
  changes — the spec's lock-probe idea was superseded by this
  existing mechanism (ADR-0052 presence coverage).
- **The window owns visibility; the daemon never tracks it.** A
  forwarded `AppToggle` is resolved by `HomeCore::on_event` from
  state it already keeps: `hidden` (capture-hidden) → the existing
  `app_focus()` show path; visible → a new `HomeDirective::Quit`
  that closes the viewport with X-close semantics (process exits,
  daemon unaffected, mid-editor session discards — identical to the
  window's X / Alt+F4, per ADR-0047's "no true Wayland hide").
- **`Quit` is a directive + a `Shared` flag, mirroring `Hide`.**
  `push_directive(Quit)` arms `SharedInner::quit_requested`; the
  open window's directive consumers (home-route host
  `editor-egui/window.rs`, editor-route host `editor-egui/app.rs`)
  translate it to `ViewportCommand::Close` exactly as `Hide`;
  `close_reason` classifies it as a user quit, ordered **after**
  pending-capture and `hide_requested` (a capture in progress owns
  the screen and the close).
- **Tray ids retire, never recycle**: `flintshot.app` is pinned dead
  by test (the `legacy_capture_ids_are_gone` precedent);
  `flintshot.toggle-window` is the new item, static label, and the
  helper stays alive on its click (`exits_after_action == false`).
- **No COSMIC applet change** (spec open question, decided): the
  applet has no window item today and keeps Capture / Open last /
  Settings / Quit.

## Task index

### Phase 1: Foundation (wire types)

- [ ] Task 1: IPC append — `Request::ToggleWindow` + `Event::AppToggle` → `tasks/001-ipc-toggle-window.md`

### Checkpoint: Foundation
- [ ] `cargo test -p flintshot-core` green; workspace builds

### Phase 2: The toggle flow (daemon + window, two independent halves)

- [ ] Task 2: Daemon-side resolution — presence forward-or-spawn + spawn fallback arm → `tasks/002-worker-toggle-dispatch.md`
- [ ] Task 3: Window-side resolution — `HomeDirective::Quit` + `AppToggle` handling → `tasks/003-window-toggle-handling.md`

### Checkpoint: Toggle flow end-to-end (no tray yet)
- [ ] IPC `ToggleWindow` from a test client: window visible → exits; window absent → spawns; capture-hidden → reopens
- [ ] `cargo test --workspace --all-targets` green

### Phase 3: The tray item (user-visible swap)

- [ ] Task 4: Tray menu swap — Show/Hide flintshot replaces Open app window → `tasks/004-tray-toggle-item.md`

### Checkpoint: User flow
- [ ] Live daemon + tray: toggle spawns, toggle hides, Capture/Settings/Quit unchanged

### Phase 4: Closeout

- [ ] Task 5: ADR-0067 + CHANGELOG + full finish gates → `tasks/005-changelog-adr-closeout.md`

## Risks and mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Presence race: window exits just as the toggle arrives → forward lands on nobody, click lost | Low | Benign by design (user re-clicks); the spawn side's `--standalone` child exits quietly on a held lock, so the mirror race (spawn while a window is starting) cannot duplicate windows |
| Toggle during a capture is suppressed (`app_focus` gate) — user perceives a dead click | Low | Intended: a capture owns the screen; the window reopens on `SessionEnded` anyway. Documented in the `on_event` arm |
| `HomeDirective` lives in `flintshot-editor-egui`, so the variant touches three crates | Low | Task 3 does all three in one pass; both host consumers are `Hide`-precedent copies with headless tests |
| A future hotkey binding for `ToggleWindow` would route through plain `dispatch` (spawn-only, no presence gate) in `drain_hotkeys` | Low | No such binding exists; noted in ADR-0067 as the known asymmetry with a follow-up if hotkeys grow open intents |
| Wildcard-arm exhaustiveness: `Request`/`Event`/`HomeDirective` are `#[non_exhaustive]` — a missed arm compiles | Med | Every task pins its arm with tests (`variant_name`, `action_for`, `on_event`, directive-consumer headless tests) |

## Open questions

- None blocking. Label wording (*Show/Hide flintshot*) and variant
  names (`ToggleWindow` / `AppToggle`) are decided here; both are
  local, reversible choices.
