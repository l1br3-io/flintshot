---
status: active
started: 2026-09-20
---

# Spec — Tray + window lifecycle consistency (self-healing)

> Fix effort. Live report (2026-09-20): *sometimes the tray menu
> "crashes" along with the window app, and duplicate windows still
> appear.* Root causes found by investigation (two parallel code
> audits + first-hand verification), not guessed — see §Diagnosis.

## Diagnosis

Nothing panics. Every symptom is a **designed exit, a leak, or a
missing recovery path** in the daemon → tray-helper / app-window
choreography:

### A. Tray "crash" — ranked mechanisms

1. **Daemon death is unrecoverable.** If the daemon exits (Ctrl-C on
   its terminal, OOM, `--replace` race), the tray helper follows it
   *by design* within ~100 ms — its one-shot daemon watch marks any
   connect failure or stream EOF as "gone" and the process exits
   (`tray_helper.rs:158-194`), unregistering the SNI item. The app
   window simultaneously degrades **permanently**: the pump never
   reconnects and force-shows the banner *"daemon connection lost —
   restart `flintshot app`"* (`app_window.rs:486`). Nobody respawns
   the daemon. User sees: tray gone + window broken, at the same
   time. (Documented-open: `fix-app-window-capture-races` spec:98.)
2. **Tray-gate depth leak → 30 s tray ghost.** A capture worker whose
   `PrepareCapture` exceeds the 2 s client bound
   (`ipc_client.rs:216`) falls back to a **second** handshake
   (`ipc_client.rs:353-364`) — but the daemon already `begin_capture`
   d for the prepare (`ipc_server.rs:540`) and begins *again* for the
   fallback (`ipc_server.rs:497`). The worker's guard owes exactly one
   `ReportCaptured`, so `hide_depth` ends at 1 and the tray stays
   hidden until the 30 s `SAFETY_RESTORE` (`tray_gate.rs:67`). Slow
   prepares (>2 s) are realistic whenever an editor session is live.
   Perceived as "tray crashed for half a minute after a capture".
3. **Helper respawn churn / silent loss.** A spawn failure leaves the
   tray gone until the next capture's Hide→Show cycle — no retry, and
   `is_present()` unconditionally returns `true` so the gate cannot
   detect the loss (`daemon.rs:399-405`, `daemon.rs:417`). An
   IPC-less daemon (`daemon.rs:657-663`, bind failure → "continuing
   without it") produces an **infinite no-backoff respawn loop**: every
   helper generation fails `Stream::connect`, exits, respawns
   (~0.5-2 s cadence, each costing `ensure_watcher` + `gtk::init` +
   SNI registration).

### B. Duplicate windows — ranked mechanisms

1. **Worker editor's Settings entry opens a second window and never
   closes.** `TopbarAction::Settings` calls `open()` with no close
   (`app.rs:1736-1740`) while its exact twin `NavigateHome`
   publishes `Cancelled` + closes behind the handoff (`app.rs:1750-
   1756`, the ADR-0065 fix applied to Home **only**). Flow: capture
   with no app window → worker editor → ⋮ Settings → daemon spawns
   `app --route settings --standalone` → new window while the editor
   stays open. Violates single-window AC1 (`single-window/spec.md:66`).
2. **Degraded app window + daemon restarted → presence
   false-negative.** After mechanism A1, a restarted daemon has a
   fresh registry and no announce connection from the degraded
   window → `app_window_present() == false` → the next capture spawns
   a worker editor **beside** the still-visible window (which also
   lands in the shot). One daemon death thus produces *both* reported
   symptoms plus a later duplicate.
3. **Minor consistency gaps.** The no-daemon fallback spawns
   `spawn_settings_app` / `spawn_home_app` (`cli.rs:1216`, `cli.rs:1252`)
   omit `--standalone` — against a lock-holder the child dies on the
   "stale `flintshot app` process?" error instead of exiting quietly;
   the startup presence announce is one-shot and silently no-ops on
   failure (`ipc_client.rs:119-144`).

The lock choreography itself is sound: no path shows two windows from
app-window processes, and no window ever maps without the app lock.
The fixes below harden recovery, not the lock.

## Objective

Make the tray/window lifecycle **consistent and self-healing**: a
daemon death recovers without user action, the tray never stays
hidden past its designed graces, and every worker handoff closes
behind itself.

- **Daemon death self-heals.** The app window becomes the liveness
  anchor: on subscription loss it reconnects with backoff, respawns
  the daemon (rate-limited) if none answers, and re-announces
  presence. A fresh daemon's supervisor spawns a fresh tray helper —
  tray, hotkeys and IPC all return. *(Confirmed by user, 2026-09-20:
  yes, auto-restart.)*
- **Tray returns promptly.** The prepare-timeout fallback no longer
  double-begins the tray gate; helper spawns retry with capped
  backoff instead of failing silently or churning.
- **Handoffs close behind themselves.** Worker-editor Settings
  mirrors the ADR-0065 Home choreography exactly. *(Confirmed by
  user, 2026-09-20: yes, mirror Home.)*

### Acceptance criteria

1. Worker editor + daemon + no app window: ⋮ Settings → app window
   opens on Settings; **worker editor closes** (never two windows).
2. Worker editor + daemon + app window running: ⋮ Settings →
   existing window focuses and routes to Settings; editor closes.
3. Worker editor + no daemon: ⋮ Settings → worker spawns the settings
   app itself (with `--standalone`); editor closes.
4. App window open, daemon killed (`pkill`): window shows a
   *reconnecting* status (not a terminal banner), the daemon is
   respawned, the tray icon returns, and the **next capture routes to
   the app window** — no worker-editor duplicate beside it.
5. Slow-prepare path (worker's `PrepareCapture` exceeds the 2 s
   client bound): the tray returns within the handshake graces after
   the grab — **not** after 30 s.
6. Helper spawn failure or repeated fast self-exits: respawns with
   capped exponential backoff; never a 0.5 s infinite churn loop;
   never silently gone.
7. Existing semantics unchanged: Esc / X / capture-intent closes, the
   app lock as the single window-presence truth, `--replace`, Ctrl-C.
8. Daemon respawn is rate-limited: a crash-looping daemon cannot
   storm process spawns.

## Technologies

Pure Rust workspace, **no new dependencies**. Touches:

- `flintshot-app` — `daemon.rs` (TraySupervisor), `tray_helper.rs`
  (watch retry), `ipc_client.rs` (prepare-fallback split),
  `app_window.rs` (pump reconnect), `cli.rs` (fallback argv).
- `flintshot-editor-egui` — `app.rs` `Settings` arm only.

## Commands

```sh
cargo build                                  # default members
cargo test --workspace --all-targets         # full suite (CI parity)
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

Manual matrix (needs a session with tray/daemon):

```sh
# A/C 1-3 — Settings handoff closes the editor
cargo run -- daemon &            # no `flintshot app` running
# tray → Capture → editor opens → topbar ⋮ → Settings
#   expect: app window opens on Settings; editor closes
# repeat with an app window already open (expect: focus + route)
pkill flintshot; cargo run -- capture   # no-daemon arm

# A/C 4 — daemon-death self-healing
cargo run -- daemon & cargo run -- app
pkill -f 'flintshot daemon'       # expect: "reconnecting…", daemon
                                  # respawns, tray icon returns; a
                                  # capture then routes in-window

# A/C 5 — prompt tray return after a slow prepare
# (keep an editor session live, trigger a second capture; the tray
#  must reappear within ~1-2 s of the grab, not 30 s)
```

Diagnostic log kit (post-hoc mechanism confirmation — no logs
captured yet):

```sh
RUST_LOG=flintshot=debug cargo run -- daemon 2>daemon.log
# "tray helper exited on its own" + "daemon connection lost"
#   → mechanism A1 (daemon death)
# "falling back to the tray-only hide" + "tray stayed hidden past
#   the safety bound" → mechanism A2 (30 s ghost)
# rapid repeated "tray helper spawned" / "IPC server unavailable"
#   → mechanism A3 (churn)
```

## Project Structure

| Change                                                        | File                                            |
| ------------------------------------------------------------- | ----------------------------------------------- |
| `Settings` arm closes behind the handoff (ADR-0065 mirror)     | `crates/flintshot-editor-egui/src/app.rs`       |
| Fallback spawns route through `worker::app_argv` (`--standalone`) | `crates/flintshot-app/src/cli.rs`            |
| `Prepared::Fallback` split: timeout ≠ old-daemon "unsupported" | `crates/flintshot-app/src/ipc_client.rs`        |
| Pump reconnect loop + daemon respawn rate-limit + re-announce  | `crates/flintshot-app/src/app_window.rs`        |
| TraySupervisor: spawn retry, churn backoff, truthful `is_present` | `crates/flintshot-app/src/daemon.rs`         |
| Daemon-watch bounded connect retry (~2 s)                     | `crates/flintshot-app/src/tray_helper.rs`       |
| Tests (per file, `#[cfg(test)]` modules)                      | same files                                       |
| Changelog + ADR-0066 (window as liveness anchor)              | `CHANGELOG.md`, `docs/adrs/`                    |

## Code Style

Follows `.opencode/tech/rust/coding-style.md` and the surrounding
code. The two smallest fixes, as they will read:

```rust
// crates/flintshot-editor-egui/src/app.rs — the Settings arm,
// closing behind the handoff like its NavigateHome twin
TopbarAction::Settings => {
    if let Some(open) = &self.open_settings {
        open();
    }
    self.publish_outcome_if_unset(Ok(EditorOutcome::Cancelled));
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
}
```

```rust
// crates/flintshot-app/src/ipc_client.rs — the fallback split:
// only a known-old daemon still owes the tray-only hide handshake;
// a timeout means the daemon already armed the gate for this
// capture, and a second begin would leak hide_depth (the 30 s ghost)
Ok(Ok(payload)) => match decode_event(&payload)? {
    IpcEvent::CaptureReady => Ok(Prepared::Ready),
    IpcEvent::Error { code, .. } if code == "unsupported" => {
        Ok(Prepared::LegacyFallback)
    }
    other => Err(eyre::eyre!("unexpected reply: {other:?}")),
},
// …
Err(_) => Ok(Prepared::ArmedSlow), // daemon began; skip the 2nd hide
```

Conventions that apply: `#![forbid(unsafe_code)]`, no
`unwrap()`/`expect()` outside tests, comments only for the
non-obvious "why", every new wait bounded, backoffs exponential and
capped.

## Testing Strategy

- **Unit (editor-egui):** pin the Settings arm — hook fired +
  `Cancelled` published + `ViewportCommand::Close` issued (twin of
  the existing NavigateHome test; hook-absent fallback preserved).
- **Unit (ipc_client, fake-server pattern):** silent server past the
  bound → **no** `HideTrayForCapture` sent; "unsupported" reply →
  legacy fallback hide sent.
- **Unit (daemon.rs supervisor tests, real-binary `tray`-arg
  pattern):** spawn-failure retry via an injectable spawn seam;
  consecutive fast self-exits back off; stable uptime resets.
- **Unit (app_window):** reconnect policy as a pure fn
  (delay-per-attempt, cap, reset-on-connect); daemon-spawn
  rate-limit as a pure fn.
- **Unit (tray_helper):** connect-retry seam — late-appearing
  listener is accepted within the window, absence marks gone.
- **CI gate:** the four commands under §Commands, in order.
- **Manual matrix:** the three scenarios under §Commands.

## Boundaries

- **Always:** run the finish checklist before declaring done; mirror
  existing seams (ADR-0065 shape, fake-server pattern, supervisor
  test pattern); keep every new wait bounded and every retry capped.
- **Ask first:** changing tray-gate state-machine semantics or the
  30 s `SAFETY_RESTORE` value; changing `OpenApp`/`OpenSettings`
  dispatch (broadcast-first choreography stays); touching the app
  window's router/`RouteHost`; new dependencies (none expected).
- **Never:** `unsafe`; `unwrap()`/`expect()` outside tests; silencing
  clippy without a justifying comment; unbounded daemon spawns (no
  rate limit); changing Esc/X/capture-intent close semantics;
  removing the "app lock as single window-presence truth" invariant.

## Success Criteria

- All eight acceptance criteria hold, demonstrated by the manual
  matrix and pinned by the unit tests above.
- `cargo test --workspace --all-targets`,
  `cargo test --doc -p flintshot-core -p flintshot-editor-egui`,
  `cargo fmt --all --check`, and
  `cargo clippy --workspace --all-targets -- -D warnings` all pass.
- CHANGELOG `[Unreleased]` updated; ADR-0066 records the
  window-as-liveness-anchor decision.

## Open Questions

None — daemon auto-restart, the Settings mirror, and the no-logs-yet
diagnostic kit were all confirmed by the user (2026-09-20).
