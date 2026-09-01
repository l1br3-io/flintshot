# Task 005 — Menu grace: tray-launched captures wait out the popup close

> Status: complete
> Effort: [`hide-tray-for-capture`](../spec.md) · ADR-0037

## Description

Follow-up to the live L5 finding (2026-09-01): the handshake removed
the tray icon from frozen captures, but the **dismissed menu popup**
still appeared — the panel closes it via a DBusMenu round trip plus
a spring animation that outlasts the 250 ms icon grace, and nothing
observable says when it is gone.

- `tray_gate`: the per-hide grace becomes per-origin. `Hidden` now
  carries its `grace`; `pre_hide` takes `PreHideOrigin` —
  `TrayMenu` sets the click-anchored `MENU_GRACE` (600 ms), `Other`
  keeps the icon grace; step-created hides (IPC/applet path, origin
  unknowable) keep the icon grace. New const + tests.
- `daemon.rs`: the tray drain pre-hides with `TrayMenu`, the hotkey
  drain with `Other` — hotkeys pay nothing for a menu they never
  opened.
- No schema change: the worker's handshake is unchanged; only the
  ack leaves later on tray launches (600 ms ≪ the 2 s reply bound).

## Acceptance criteria

1. Tray-menu pre-hide is not confirmable at the icon grace, confirms
   at the menu grace, anchored at the click (unit test).
2. Hotkey pre-hide confirms at the icon grace alone (unit test).
3. Step-created hides keep the icon grace (unit test).
4. Standing gates green in both feature configs.

## Verification

```sh
cargo test -p flintshot-app tray_gate
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p flintshot-app --features tray --all-targets -- -D warnings
cargo test --workspace --all-targets
```

Manual checkpoint (L5): tray *Capture region* — menu fully gone from
the frozen frame (no mid-fade ghost), click-to-grab ≈ 600–700 ms;
hotkey capture unchanged (~300 ms).
