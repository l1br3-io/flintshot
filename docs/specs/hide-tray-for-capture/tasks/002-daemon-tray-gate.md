# Task 002 — Daemon side: tray gate + loop wiring + IPC interception

> Status: complete
> Effort: [`hide-tray-for-capture`](../spec.md) · Plan decisions 2–3

## Description

- New `crates/flintshot-app/src/tray_gate.rs`: pure `TrayVisibility`
  state machine (depth, grace-anchored confirmation epoch, pre-hide
  hold window, safety bound — injected `Instant`, fully unit-tested)
  inside a thread-safe `TrayGate` shell shared between the IPC
  (tokio) side and the main loop. TDD surfaced two real design bugs
  before they could ship: a stale pre-hide hold blocking restores
  after a handshake takes over, and second waiters never getting an
  epoch bump (re-arm now happens on every begin-while-confirmed).
- `daemon.rs`: `DaemonTray` (icon handle present in both feature
  configs, `set_visible` + `GLib` pump side effects, non-fatal on
  error); the loop steps the gate every iteration; tray-menu and
  hotkey drains pre-hide at dispatch behind the new
  `worker::spawns_capture_worker` predicate (side effect only — the
  worker's handshake owns the depth).
- `ipc_server.rs`: `HideTrayForCapture` intercepted in
  `handle_connection` (begin → bounded `wait_confirmed` → reply
  `Event::TrayHidden`), `ReportCaptured` (end, fire-and-forget);
  `Arc<TrayGate>` threaded through `bind_with_bus` / `serve`.
- `worker.rs`: defensive no-op dispatch arms + variant labels +
  `spawns_capture_worker` with tests.

## Acceptance criteria

1. State-machine transitions unit-tested: hide-on-begin, grace before
   confirm, restore on last report, depth across concurrent
   captures, per-waiter confirmation, safety force-restore with
   reset, pre-hide hold + expiry, vacuous confirm without a tray.
2. IPC tests: the ack arrives once confirmed; the ack still arrives
   (bounded) when never confirmed; `ReportCaptured` drains the hold.
3. Dispatch arms: handshake requests are silent no-ops (never
   "unsupported"); predicate true only for region/fullscreen.
4. Gates green in **both** feature configs (default +
   `--features tray`, the CI `tray-build` job).

## Verification

```sh
cargo test -p flintshot-app tray_gate
cargo test -p flintshot-app worker
cargo test -p flintshot-app --features tray
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p flintshot-app --features tray --all-targets -- -D warnings
cargo test --workspace --all-targets
```
