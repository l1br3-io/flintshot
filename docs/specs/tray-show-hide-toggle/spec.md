# Spec: tray-show-hide-toggle

---
status: active
started: 2026-09-20
---

## Objective

Replace the tray menu's **Open app window** item with a single static
**Show/Hide flintshot** toggle — the Telegram/Discord-style tray
convention — and make the daemon the authority that resolves the
toggle safely.

Today *Open app window* (`Request::OpenApp`) only ever opens or
focuses; there is no tray path that dismisses the window. This effort:

1. **Tray menu becomes** `Show/Hide flintshot / Capture / Settings /
   — / Quit`. One static label (no live "Show"/"Hide" flipping — that
   would need a daemon→helper reverse feed, explicitly out of scope).
2. **New IPC**: `Request::ToggleWindow` (client → daemon) and a
   broadcast event the running window resolves (daemon → window).
   Both **appended** to the postcard enums (variant-index rule).
3. **Daemon resolves the toggle from real liveness**, not
   broadcast-always: the existing app-window **presence oracle** —
   `AnnounceAppWindow` connections plus the daemon-spawned process
   registry (`CaptureDispatch::app_window_present`, ADR-0052) —
   decides. Window present → broadcast the toggle event and
   **spawn nothing** — the race pin. (An unconditional spawn, the
   `OpenApp` pattern, would race the exiting window's lock release
   and could resurrect a window the user just asked to hide.) No
   window → the existing `OpenApp` choreography (broadcast
   `AppFocus` + spawn the app worker). This reuses the exact
   forward-or-spawn seam `OpenImage`/`OpenLast` already ride
   (`forwards_to_app_window` → `dispatch_open_blocking`), so no new
   probe and no connection-layer changes.
4. **Window-side semantics**: hide = the same graceful exit as the
   window's X / Alt+F4 — the window process exits, daemon unaffected,
   a mid-editor session discards exactly as X-close does today (per
   ADR-0047, a true Wayland surface-hide is not an option, so
   toggle = close/respawn and the spawn cost on show is accepted).
   The one refinement: while the window is **capture-hidden**
   (surface closed, pump process waiting on a `Show` directive), the
   toggle event means *show* — the existing process reopens instead
   of exiting. The window resolves the event from its own surface
   state; the daemon never tracks visibility.

**Users**: Linux desktop users who keep the daemon in the tray and
expect the tray to toggle the app window, as other tray apps do.

## Technologies

Unchanged from the workspace: pure-Rust Cargo workspace, egui/eframe
app window, `tray-icon`/muda tray (helper process, ADR-0042),
postcard IPC (`flintshot-core::ipc`), std abstract-socket
single-instance locks (`single.rs`). No new dependencies.

## Commands

From the repo root (full list in `AGENTS.md`):

```sh
cargo build                                   # default members
cargo run -- daemon                           # tray + IPC
cargo test --workspace --all-targets          # unit + integration
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Project Structure

Touch points (no new crates):

```
crates/flintshot-core/src/ipc.rs               # append Request::ToggleWindow + Event::AppToggle
crates/flintshot-app/src/tray.rs               # menu item swap; legacy id pin
crates/flintshot-app/src/tray_helper.rs        # ToggleWindow keeps helper alive
crates/flintshot-app/src/worker.rs             # ToggleWindow dispatch + forward-or-spawn
crates/flintshot-app/src/app_window.rs         # AppToggle handler + quit-requested flag
crates/flintshot-editor-egui/src/home.rs       # HomeDirective::Quit variant
crates/flintshot-editor-egui/src/{window,app}.rs # Quit consumption (home/editor hosts)
docs/specs/tray-show-hide-toggle/              # this effort
```

## Code Style

Repo conventions (`AGENTS.md` + `.opencode/tech/rust/coding-style.md`)
apply: `#![forbid(unsafe_code)]`, `clippy::pedantic` clean, doc
comments on public items in core, `#[non_exhaustive]`, no
`unwrap()`/`expect()` outside tests, comments only for non-obvious
*why*.

Representative snippets — IPC variants **appended** (postcard
variant-index rule); dispatch stays pure by taking the probe result
through `DispatchState` (the `capture_backend` precedent), keeping the
`Spawner` seam testable:

```rust
// ipc.rs — appended (postcard variant-index rule)
/// Client → daemon: toggle the app window (show when down, close
/// when up). Pairs with [`Event::AppToggle`].
ToggleWindow,

/// Broadcast on [`Request::ToggleWindow`] when a window is running;
/// the window resolves it from its own surface state: visible →
/// graceful exit, capture-hidden → reopen.
AppToggle,
```

```rust
// worker.rs — the presence oracle gates forward-vs-spawn; the
// connection layer already routes forwards_to_app_window requests
// here (the OpenImage/OpenLast precedent)
if self.app_window_present() {
    sink(Event::AppToggle);   // window resolves; NO spawn (race pin)
    return Ok(DispatchOutcome::Continue);
}
dispatch(request, spawner, state, sink)  // falls to the OpenApp choreography
```

Tray ids stay short + namespaced; the retired id is pinned dead by
test (the `legacy_capture_ids_are_gone` precedent):

```rust
const ID_TOGGLE_WINDOW: &str = "flintshot.toggle-window";
// "flintshot.app" must resolve to None — pinned in tests, never reused.
```

## Testing Strategy

Same levels as the workspace: unit tests in-crate (`#[cfg(test)]`),
integration tests in `flintshot-core/tests/`. Key pins:

- **Tray**: `action_for` maps `flintshot.toggle-window` →
  `Request::ToggleWindow`; `flintshot.app` resolves to `None`
  (legacy-id pin); the item label is one static string.
- **Helper**: `exits_after_action(ToggleWindow) == false` (pinned
  next to `OpenApp`/`OpenSettings`).
- **Dispatch**: presence armed → `dispatch_open_blocking(ToggleWindow)`
  emits exactly one `Event::AppToggle` and **zero** spawns (the
  hide-then-reshow race pin); presence absent → the plain
  `dispatch` arm emits `Event::AppFocus` + one app-worker spawn
  (the `OpenApp` argv/table-name pins); `forwards_to_app_window
  (ToggleWindow) == true`; `spawns_capture_worker(ToggleWindow) ==
  false`; `variant_name(ToggleWindow) == "toggle-window"`.
- **App window**: `AppToggle` on a visible window arms the
  quit-requested flag (`HomeCore::on_event`); `close_reason`
  classifies it with user-quit semantics (process exits), ordered
  after pending-capture and hide; `AppToggle` on a capture-hidden
  window yields `HomeDirective::Show` (reopen, no quit); a pending
  capture-intent still outranks the toggle (the
  `close_reason_capture_beats_hide` precedent for ordering).
- **Editor hosts**: both directive consumers (the home-route host
  in `window.rs`, the editor-route host in `app.rs`) translate
  `HomeDirective::Quit` into `ViewportCommand::Close` (the `Hide`
  precedent, headless-pinned).
- **IPC**: round-trip encode/decode for the new variants; existing
  variant wire bytes unchanged (append-only).

## Boundaries

- **Always**: append IPC variants (never insert/reorder); keep
  `flintshot-core`'s public API free of GUI types
  (`tests/no_gui_types_in_core.rs` stays green); run the four finish
  commands before declaring done; update `CHANGELOG.md` under
  `## [Unreleased]`; record the toggle-resolution choice as an ADR
  (presence-gated forward-or-spawn vs broadcast-always) since it is a
  non-obvious, load-bearing decision.
- **Ask first**: new dependencies (none expected); CI changes; any
  change to the tray helper's exit choreography (ADR-0036/0037/0042
  handshake) beyond adding the non-exiting `ToggleWindow` arm.
- **Never**: `unsafe`; `#[allow]` without a justifying comment;
  reuse of retired menu ids (`flintshot.app`, `flintshot.open-config`,
  `flintshot.bind-shortcuts`); breaking postcard wire bytes of
  existing variants; a dynamic/flipping menu label; left-click-on-
  tray-icon toggling; changes to the window's X-close behavior.

## Success Criteria

1. Tray menu offers exactly *Show/Hide flintshot*, *Capture*,
   *Settings*, *Quit*; the *Open app window* item is gone and its id
   pinned dead by test.
2. No window running → the toggle spawns and shows the window,
   indistinguishable from today's *Open app window* click.
3. Window visible → the toggle closes it: the window process exits,
   the daemon and tray are unaffected, and a mid-editor session
   discards exactly as X-close does.
4. Window capture-hidden → the toggle reopens the existing process
   (no exit, no duplicate spawn).
5. No hide-then-reshow race: dispatch never spawns an app worker when
   the presence oracle says a window is alive (machine-checked).
6. *Settings* behaves unchanged: opens-or-focuses on its page even
   immediately after a hide.
7. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
   -- -D warnings`, both test commands: green; `CHANGELOG.md` and the
   new ADR entry exist.

## Open Questions

- Label wording: *Show/Hide flintshot* proposed; *Show/hide window*
  is the alternative. Cosmetic, plan-phase.
- Exact names `ToggleWindow` / `AppToggle` proposed; plan may refine
  (e.g. `WindowToggle`) — names are pre-1.0 and internal to the
  wire enum.
- COSMIC applet: **decided — no applet change** (it has no window
  item today; Capture / Open last / Settings / Quit stays).
- Liveness resolution: **decided — the existing presence oracle**
  (`AnnounceAppWindow` connections + session registry via
  `CaptureDispatch::app_window_present`), riding the
  `forwards_to_app_window` → `dispatch_open_blocking` seam; no new
  probe, no connection-layer changes.
