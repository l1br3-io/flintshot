---
status: active
started: 2026-09-20
---

# Spec — Worker editor: Home link navigates instead of closing

> Fix effort. Live report (2026-09-20): *capture from the tray → editor
> opens → clicking the breadcrumb's Home link immediately closes the
> window.* Root cause found in investigation, not guessed — see
> §Diagnosis.

## Diagnosis

Tray Capture with no app window running follows the **worker
fallback** (single-window spec: "worker-editor stays the fallback
when no window answers"): the daemon spawns a capture worker, the
worker grabs and opens its own editor window.

That worker window shares the unified topbar, whose breadcrumb
renders `Home › Editor` with **Home as a live link** on the Editor
surface (`topbar.rs`). In the **app window** the router intercepts
`TopbarAction::NavigateHome` and swaps to the Home route in-place
(`window.rs`, test-pinned by `navigate_home_from_the_editor_routes_home`).
In the **worker window** the action reaches
`EditorApp::handle_topbar_action`
(`crates/flintshot-editor-egui/src/app.rs`, `NavigateHome` arm),
which — having no home route behind it — publishes `Cancelled` and
sends `ViewportCommand::Close`. The window dies the instant the
link is clicked. This is the "leftover from a previous decision":
the Esc-contract stance predates the topbar giving every editor a
Home navigation affordance.

## Objective

The Home breadcrumb link must do what it promises on **every**
surface it renders on:

- **App window** (unchanged): route Home in-place, same window.
- **Worker editor** (the fix): clicking Home delegates
  `Request::OpenApp` to the daemon — a running app window is
  focused (broadcast-first choreography), otherwise the daemon
  spawns `flintshot app` on Home — and then the worker editor
  closes with the Esc contract (`Cancelled` + close). The handoff
  mirrors the worker's existing Settings delegate
  (`open_settings_from_worker`, `cli.rs`), byte-for-byte in shape.
- **No daemon at all** (standalone `flintshot capture`): the worker
  performs the spawn step itself (detached `flintshot app`), then
  closes — the `NoDaemon` arm of the Settings delegate, mirrored.

Success looks like: after a tray capture, clicking Home leaves the
user looking at flintshot's home page (in the app window), not at a
vanishing window.

### Acceptance criteria

1. Worker editor + daemon + no app window: Home click → app window
   opens on Home; worker editor closes.
2. Worker editor + daemon + app window already running: Home click
   → existing app window is focused; worker editor closes.
3. Worker editor + no daemon: Home click → `flintshot app` spawns
   (Home route); worker editor closes.
4. Worker editor + Home delegate unavailable (headless tests, hook
   not installed): current behavior preserved (publish `Cancelled`
   + close) — never a panic, never a dead click.
5. App window routes: zero behavior change (regression-pinned by
   existing tests).
6. Esc, X / Alt+F4, `+` / Ctrl+N in the worker editor: unchanged.

## Technologies

Pure Rust workspace, no new dependencies. Touches:

- `flintshot-editor-egui` — the `NavigateHome` handler + a new
  `with_open_home` shell hook (mirror of `with_open_settings`).
- `flintshot-app` — the worker-side delegate `open_home_from_worker`
  + its no-daemon spawn fallback (mirror of the Settings pair).

## Commands

```sh
cargo build                                  # default members
cargo test -p flintshot-editor-egui          # the handler + hook tests
cargo test --workspace --all-targets         # full suite (CI parity)
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

Manual verification (needs a session with tray/daemon):

```sh
# daemon + tray up, no `flintshot app` running
# tray → Capture → editor opens → click the Home breadcrumb
#   expect: app window appears on Home; editor closes
cargo run -- daemon & cargo run -- app   # then quit the app window
# tray → Capture → click Home again      # expect: same, via focus
pkill flintshot                          # no daemon: standalone check
cargo run -- capture                     # editor → Home → app spawns
```

## Project Structure

| Change | File |
| --- | --- |
| `NavigateHome` handler (worker arm calls the hook, then Esc contract) | `crates/flintshot-editor-egui/src/app.rs` |
| Shell builder `with_open_home` + `EditorApp::set_open_home` | `crates/flintshot-editor-egui/src/shell.rs`, `app.rs` |
| Worker-side delegate + spawn fallback | `crates/flintshot-app/src/cli.rs` (next to the Settings twins) |
| Tests (hook-called, no-hook fallback, handler order) | same files' `#[cfg(test)]` modules |
| Changelog entry | `CHANGELOG.md` → `## [Unreleased]` |
| ADR (revises the worker-navigation stance) | `docs/adr/` (numbered, Context · Decision · Consequences) |

## Code Style

Follows `.opencode/tech/rust/coding-style.md` and the surrounding
code. The new hook is a byte-for-byte shape-mate of `open_settings`:

```rust
// crates/flintshot-editor-egui/src/app.rs — the revised arm
TopbarAction::NavigateHome => {
    // Worker windows have no home route behind them: the link
    // delegates the daemon's OpenApp choreography (focus-or-spawn
    // the app window on Home) and then leaves via the Esc contract.
    if let Some(open) = &self.open_home {
        open();
    }
    self.publish_outcome_if_unset(Ok(EditorOutcome::Cancelled));
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
}
```

```rust
// crates/flintshot-app/src/cli.rs — the Settings twins' third member
#[cfg(feature = "egui")]
fn open_home_from_worker() {
    match crate::ipc_client::try_delegate(&Request::OpenApp) {
        Ok(crate::ipc_client::DelegateOutcome::Delegated) => {}
        Ok(crate::ipc_client::DelegateOutcome::NoDaemon) => {
            if let Err(e) = spawn_home_app() {
                tracing::warn!(error = %e, "editor Home: no daemon and the direct spawn failed");
            }
        }
        Err(e) => tracing::warn!(error = %e, "editor Home: delegating to the daemon"),
    }
}
```

Conventions that apply: `#![forbid(unsafe_code)]`, no
`unwrap()`/`expect()` outside tests, comments only for the
non-obvious "why", `#[non_exhaustive]` untouched (no public-type
shape changes beyond the hook mirroring existing API).

## Testing Strategy

- **Unit (editor-egui, `app.rs` tests):**
  - existing `navigate_home_publishes_cancelled_on_worker_windows`
    stays green (hook absent → publish + close);
  - new: with `set_open_home(Arc<AtomicBool …>)` installed, the
    handler fires the hook **and** publishes `Cancelled` **and**
    issues `ViewportCommand::Close` (assert via `close_command_issued`
    on a `ctx.run` full output);
  - new: page-mode editor with the hook installed still publishes +
    closes (the router normally intercepts; the handler arm is
    defensive).
- **Unit (shell):** `with_open_home` round-trips into the app's
  hook field (mirror `with_selection_first_flags_the_next_run_editor`).
- **App crate:** the delegate is IPC-bound like its Settings twin
  and stays untested-by-unit (precedent); behavior covered by the
  manual matrix above. If a pure seam falls out (argv builder), it
  gets a unit test.
- **CI gate:** the four commands under §Commands, in order.

## Boundaries

- **Always:** run fmt + clippy + the workspace tests before
  declaring done; mirror existing seams rather than inventing new
  plumbing; keep the app-window routing path untouched.
- **Ask first:** any change to the app window's router or
  `RouteHost`; touching the daemon's `OpenApp` dispatch; new
  dependencies (none expected).
- **Never:** silence clippy with `#[allow]` without a justifying
  comment; change Esc / X / capture-intent close semantics; commit
  secrets.

## Success Criteria

- The reported repro (tray capture → editor → Home link) ends on
  flintshot's home page in an app window, never on a bare close.
- `cargo test --workspace --all-targets`,
  `cargo test --doc -p flintshot-core -p flintshot-editor-egui`,
  `cargo fmt --all --check`, and
  `cargo clippy --workspace --all-targets -- -D warnings` all pass.
- CHANGELOG `[Unreleased]` updated; ADR records the revised
  worker-navigation stance.

## Open Questions

None — behavior and no-daemon fallback confirmed by the user
(2026-09-20).
