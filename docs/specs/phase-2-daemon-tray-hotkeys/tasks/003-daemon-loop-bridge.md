# Task 003 — Daemon event-loop bridge spike + single-instance lock

> Phase 2.B · size: M · depends on: 002

## Description

Make `run_daemon` real and de-risk the hardest integration in Phase 2:
running a tokio runtime (for the future IPC server + config watcher)
alongside the platform event sources `tray-icon` and `global-hotkey`
need. The spike acquires the single-instance lock, starts a tokio
runtime in a side thread, runs a main-thread loop that drains
`tray-icon` + `global-hotkey` event receivers plus an mpsc channel from
the tokio side, and proves all three sources reach a single handler.
No real capture yet — the handler just logs the source. This mirrors
Phase 1's Task 005 "de-risk the toolkit integration early" pattern.
ADR-0018 is drafted here and must be reviewed at Checkpoint B before
any feature builds on the bridge.

## Acceptance criteria

- [ ] `flintshot daemon` runs (does not immediately return the stub
      `Ok(())`); it blocks on its event loop until killed.
- [ ] Single-instance lock acquired on start; a second `flintshot
      daemon` exits with a clear "already running" error (basic case;
      `--replace` wired in Task 006).
- [ ] A tokio runtime runs in a background thread; a periodic tick sent
      from it reaches the main-thread handler.
- [ ] A minimal tray icon appears; clicking it reaches the handler.
- [ ] One dummy global hotkey (hardcoded chord) is registered; pressing
      it reaches the handler.
- [ ] Handler logs `action received from <source: tray|hotkey|tick>`.
- [ ] ADR-0018 drafted at `docs/adrs/ADR-0018-*.md` (Context · Decision
      · Consequences), recording the bridge approach.

## Verification

- [ ] Standing gate passes.
- [ ] Manual (TEST_MATRIX): `cargo run -- daemon` shows the tray icon;
      click → log line; press the dummy hotkey → log line; Ctrl-C (or
      tray Quit if wired) → clean exit.
- [ ] `cargo run -- daemon` in a second terminal → "already running"
      error, non-zero exit.
- [ ] On a headless box the daemon subcommand at least compiles and the
      single-instance lock logic is unit-testable (acquire-twice fails).

## Dependencies

Task 2 (deps available).

## Files likely touched

- `crates/flintshot-app/src/cli.rs` (`run_daemon` body)
- `crates/flintshot-app/src/daemon.rs` (new — event loop + bridge)
- `crates/flintshot-app/src/single.rs` (new — lock)
- `crates/flintshot-app/src/main.rs` (wire module)
- `crates/flintshot-app/Cargo.toml` (tray-icon/global-hotkey/single/tokio)
- `docs/adrs/ADR-0018-*.md` (new)

## Size

M

## Notes for the implementer

Start with raw polling of `tray-icon` / `global-hotkey` event receivers
+ an `mpsc::Receiver` select on the main thread. Only escalate to a
`winit` event loop if a platform (likely macOS) demands main-thread
run-loop pumping that raw polling can't satisfy — record that finding
in ADR-0018 either way. Keep the handler behind a trait so Task 005 can
swap in the real worker-spawning implementation without touching the
loop.
