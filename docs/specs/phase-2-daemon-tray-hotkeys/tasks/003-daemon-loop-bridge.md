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

- [x] `flintshot daemon` runs (does not immediately return the stub
      `Ok(())`); it blocks on its event loop until killed.
      _Proven at runtime: loop runs until Ctrl-C (see smoke log)._
- [x] Single-instance lock acquired on start; a second `flintshot
      daemon` exits with a clear "already running" error (basic case;
      `--replace` wired in Task 006).
      _Proven at runtime (second instance errored "already running") +
      unit-tested (`single::tests::second_acquire_with_same_name_fails`)._
- [x] A tokio runtime runs in a background thread; a periodic tick sent
      from it reaches the main-thread handler.
      _Proven at runtime: `action received source="tick"` every 500 ms._
- [x] A minimal tray icon appears; clicking it reaches the handler.
      _Code-level (compiles + lints under `--features tray`); tray code
      behind the `tray` feature. The icon-build + click→handler path
      shares the identical drain mechanism as the proven tick path.
      **Tray visibly appearing + a real click are manual TEST_MATRIX**
      (needs an SNI host + interactive input the harness can't inject
      headlessly)._
- [x] One dummy global hotkey (hardcoded chord) is registered; pressing
      it reaches the handler.
      _Registration proven at runtime (`registered spike hotkey:
      Ctrl+Shift+P` under X11/XWayland). The press→handler path shares
      the proven tick drain. **The actual key press is manual
      TEST_MATRIX** (no `xdotool` on the dev box to inject it)._
- [x] Handler logs `action received from <source: tray|hotkey|tick>`.
      _Proven: observed `source="tick"`; `tray`/`hotkey` strings pinned
      by `source_strings_are_stable` unit test._
- [x] ADR-0018 drafted at `docs/adrs/ADR-0018-*.md` (Context · Decision
      · Consequences), recording the bridge approach.
      _`ADR-0018-tokio-side-thread-main-event-loop-bridge.md`; ADR-0018
      added to the ADR index. Awaits Checkpoint B review._

## Verification

- [x] Standing gate passes. _(fmt, default clippy, tray clippy,
      `--workspace` tests, doc tests all green.)_
- [ ] Manual (TEST_MATRIX): `cargo run -- daemon` shows the tray icon;
      click → log line; press the dummy hotkey → log line; Ctrl-C (or
      tray Quit if wired) → clean exit.
      _Ctrl-C clean exit proven at runtime; tray-icon-visible +
      click + hotkey-press deferred to manual (no input injection on the
      dev box). See ADR-0018 §Decision (proven)._
- [x] `cargo run -- daemon` in a second terminal → "already running"
      error, non-zero exit. _Proven at runtime._
- [x] On a headless box the daemon subcommand at least compiles and the
      single-instance lock logic is unit-testable (acquire-twice fails).
      _4 unit tests in `single::tests` + 5 in `daemon::tests`._

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
