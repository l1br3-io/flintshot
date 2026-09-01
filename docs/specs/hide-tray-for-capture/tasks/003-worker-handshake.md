# Task 003 — Worker side: handshake client + RAII guard around the grab

> Status: complete
> Effort: [`hide-tray-for-capture`](../spec.md) · Plan decision 4

## Description

- `ipc_client::hide_tray_for_capture_to` — request/reply mirror of
  `bind_shortcuts_to` with a 2 s reply bound (deliberately shorter
  than the daemon's 3 s confirm bound so the worker is never the one
  left hanging). Returns `HideTrayOutcome`:
  `NoDaemon` (nothing registered, nothing owed) vs
  `Registered { acked }` (the daemon registered the hide — an
  unconfirmed hide is the pre-fix behaviour, not a failure).
- `cli::TrayHiddenGuard` — RAII: request on construction, send
  `ReportCaptured` on Drop when owed. Errors map to "capture with the
  tray as-is" (warn), never a failed capture.
- `capture_with_tray_hidden` — the handshake, the `xcap` grab, and
  the restore report all sit inside one function scope, so the icon
  returns the instant the pixels are taken (success or error); the
  editor / overlay session beyond it never holds the tray hostage.
  `capture_and_dispatch` now calls it — one choke point covers
  `region`, `fullscreen`, `--out`, editor mode, overlay mode, the
  Wayland demotion, and daemon-spawned `--standalone` workers.

## Acceptance criteria

1. Standalone (no daemon): one socket probe, no hide, no report —
   pinned by guard + report tests (silent no-ops).
2. The reply wait is bounded; timeout logs a warning naming the risk
   (the icon may appear in the shot) and proceeds.
3. The guard's report fires on both grab outcomes by construction
   (Drop at scope exit, `?` early-returns included).
4. Standing gates green in both feature configs.

## Verification

```sh
cargo test -p flintshot-app ipc_client
cargo test -p flintshot-app tray_hidden_guard
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p flintshot-app --features tray --all-targets -- -D warnings
cargo test --workspace --all-targets
```

Manual checkpoint (needs a display + tray daemon): click *Capture
region* in the tray → icon vanishes at the click, the frozen frame
has no icon, the icon is back with the editor up; `flintshot region`
via CLI delegate ditto; kill -9 the worker mid-grab → icon returns
within the safety bound.
