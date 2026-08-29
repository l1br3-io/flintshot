---
status: shipped
started: 2026-08-28
shipped: 2026-08-29
---

# Spec — Fix delegated capture: fork loop + at-spi window crash

> Bug-fix effort. For how delegation was supposed to work, see
> [Phase 2](../phase-2-daemon-tray-hotkeys/spec.md) (detect-and-delegate,
> ADR-0019 daemon-spawns-workers) and
> [ADR-0023](../../adrs/ADR-0023-worker-report-saved.md) (worker
> reporting). The bugs were surfaced by the launcher effort's L5 pass
> (2026-08-28, commit 54383b1) and are recorded in TEST_MATRIX "L5
> pass findings" 1–2. This effort makes daemon-mediated captures
> work end-to-end; it does not touch tray SNI registration (finding
> 3, separate bug, no capture path).

## Objective

Every daemon-mediated capture path — tray menu, desktop actions,
global hotkeys, CLI-with-daemon — has been broken since Phase 2's
detect-and-delegate, and standalone captures crash on at-spi
desktops since Phase 6 unified zbus to 5.18. Two bugs, one symptom:
no editor ever opens.

1. **Fork loop.** `worker.rs::region_argv` / `fullscreen_argv`
   spawn `flintshot region …` without `--standalone`, and
   `cli.rs::run_region` re-delegates whenever the daemon is up —
   so each daemon-spawned worker delegates right back and the
   daemon spawns the next one. Observed ~470 worker processes in
   8 s; the daemon log fills with `IPC connection ended … early
   eof`; no editor appears.
2. **zbus-5/at-spi reactor panic.** Window-opening workers
   (`region`/`fullscreen` → `run_capture_shell`, `open` →
   `run_editor_for_capture`, `pin` → `run_pin_window`, overlay)
   have no ambient tokio reactor when `eframe::run_native` drives
   accesskit → atspi → zbus 5, which requires one. Panic before
   any window opens: *"there is no reactor running, must be called
   from the context of a Tokio 1.x runtime"*. Affects every
   desktop with `org.a11y.Bus` running (GNOME/KDE/COSMIC), not
   just COSMIC.

**Target users:** anyone using Flintshot through the daemon —
which is the intended steady-state (launcher, autostart, hotkeys).

**Success looks like:** from a desktop action or CLI-with-daemon,
a capture opens exactly one editor window on an at-spi session,
saving reports back to the daemon (PinLast/OpenLast keep working),
and the process table stays at daemon + one worker.

## Assumptions

1. The `--standalone` argv guard is sufficient to break the loop:
   `delegate_to_daemon` already short-circuits on it, and argv is
   the seam the existing `RecordingSpawner` unit tests assert.
2. `pin` / `open` / `upload` workers need no guard — only
   `run_region` / `run_fullscreen` call `delegate_to_daemon`
   (`run_open`, `run_pin`, `run_upload` operate on a path and never
   delegate).
3. An ambient runtime per window-worker process is affordable:
   workers are short-lived (one editor/pin session), and the daemon
   already runs a multi-thread tokio runtime of its own.
4. The zbus-5 tokio choice is intentional (workspace `Cargo.toml`
   pins ashpd to `features = ["tokio"]` to avoid the async-io
   backend clash) and must not be reverted or cargo-patched.
5. The `ReportSaved` fire-and-forget path from workers is
   independent of delegation and unaffected by the `--standalone`
   guard.

## Scope (in / out)

**In:**

- `worker.rs::region_argv` / `fullscreen_argv` append
  `--standalone`; dispatch tests updated to assert it (this *is*
  the no-re-delegation regression test — argv content is the
  contract).
- Ambient tokio runtime guard at the window-opening entry points
  in `flintshot-app` (one helper, e.g. `with_ambient_runtime`,
  entered around the eframe-driven helpers; exact placement is a
  plan-phase decision). Covers editor, overlay, pin, and open
  paths. No changes to `flintshot-core` or
  `flintshot-editor-egui` expected.
- Regression test that the guard makes
  `tokio::runtime::Handle::try_current()` succeed on the calling
  thread (the exact condition zbus 5 checks).
- TEST_MATRIX: mark findings 1–2 fixed with commit refs; fill the
  Phase 2 delegated-capture rows that were never run.
- CHANGELOG under `## [Unreleased]`.
- ADR for the ambient-runtime decision (zbus 5 via accesskit,
  runtime-per-worker trade-off, why not zbus 4/async-io).

**Out:**

- Tray SNI registration (finding 3) — separate bug, no fix shape,
  tray capture cannot be re-verified on COSMIC until it lands.
- Any change to delegation *semantics* (when to delegate, fallback
  policy, `--out`/interaction-flag skips stay as-is).
- Daemon architecture changes (in-process capture, worker pooling).
- Windows/macOS verification (the at-spi path is Linux-only; no
  code is expected to touch other platforms).

## Deliverables

1. Guarded worker argv + updated dispatch unit tests.
2. Ambient-runtime helper + window-entry-point wiring + guard unit
   test.
3. TEST_MATRIX + CHANGELOG updates.
4. ADR-00NN: ambient tokio runtime for window workers.

## Exit criteria

1. On L5 (COSMIC, at-spi running) with the daemon up: a desktop
   action *Capture region* and `flintshot region` (delegated) each
   open exactly one editor; `pgrep -c flintshot` stays at daemon +
   one worker sustained over several seconds; no `early eof` spam
   in the daemon log.
2. `flintshot region --standalone` on the same session opens the
   editor instead of panicking (previously crashed before any
   window).
3. Save in a delegated editor still reports back: PinLast /
   OpenLast work immediately after.
4. New regression tests green; standing gates green (`fmt`,
   `clippy --workspace --all-targets -D warnings`, workspace tests,
   doc tests).
5. TEST_MATRIX findings 1–2 marked resolved; launcher-desktop-entry
   task 007 is unblocked (its re-run stays in that effort).

## Technologies

No new dependencies. tokio 1 is already a workspace dep with the
`full` feature set; the guard is `Runtime::new()` + an `enter()`
guard held across the window loop (or an equivalent dropped-in
ambient handle). No GUI-toolkit types cross into `flintshot-core`
(boundary test continues to pass — core is untouched).

## Commands

```sh
cargo test -p flintshot-app            # dispatch argv + runtime-guard tests
cargo test --workspace --all-targets   # full regression
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
# manual, on an at-spi session (L5):
cargo build && ./target/debug/flintshot daemon &
./target/debug/flintshot region        # delegated: one editor, no loop
./target/debug/flintshot region --standalone   # no panic
watch -n1 'pgrep -c flintshot'         # stays at 2 during a capture
```

## Project structure

| Change                     | Location                                 |
| -------------------------- | ---------------------------------------- |
| Worker argv guard          | `crates/flintshot-app/src/worker.rs`     |
| Ambient runtime helper     | `crates/flintshot-app/src/` (new small module or `cli.rs`) |
| Window entry-point wiring  | `crates/flintshot-app/src/cli.rs`        |
| Tests                      | same files (`#[cfg(test)]`)              |
| Docs                       | `CHANGELOG.md`, `docs/TEST_MATRIX.md`, `docs/adrs/` |

## Code style

Standing repo conventions (AGENTS.md): `#![forbid(unsafe_code)]`,
no `unwrap()`/`expect()` outside tests, comments only for
non-obvious "why", `thiserror`/`color_eyre` as established. The
runtime guard is small and documented — it exists *only* because
zbus 5 needs an ambient reactor and window workers have none.

## Testing strategy

- **Unit (fork loop):** `RecordingSpawner` dispatch tests assert
  `region`/`fullscreen` argv end with `--standalone`. If a future
  edit drops the flag, these fail before anything ships.
- **Unit (runtime guard):** `Handle::try_current()` fails outside
  the guard and succeeds inside `with_ambient_runtime` on the same
  thread — the precise condition zbus 5's executor checks.
- **Manual (L5):** exit criteria 1–3 above; results recorded in
  TEST_MATRIX with commit hash. Tray-menu capture stays
  un-verified on COSMIC (blocked by SNI, finding 3) — desktop
  actions + CLI-with-daemon cover the dispatch path; tray uses the
  same `dispatch` table.
- **Not attempted:** a CI end-to-end daemon+worker spawn test —
  the worker would need a display to open an editor; the argv
  contract plus the manual pass is the coverage story, stated
  honestly.

## Boundaries

- **Always:** run the full gate suite before declaring done;
  update TEST_MATRIX with real (not predicted) results; keep the
  `--standalone` flag's user-facing semantics unchanged (it stays
  the documented escape hatch; the daemon merely uses it).
- **Ask first:** dependency changes (goal: zero); touching
  `flintshot-core` or `flintshot-editor-egui`; changes to
  `ipc_server.rs` beyond what argv wiring demands; CI changes.
- **Never:** cargo-patch zbus to 4.x or switch ashpd to async-io
  (deliberate workspace decision); `#[allow(...)]` without a
  justifying comment; unsafe code.

## Success criteria

- Delegated capture from every reachable entry point opens one
  editor, once, on an at-spi desktop — and keeps doing so.
- Standalone captures stop crashing on at-spi desktops.
- No behaviour change when no daemon is running (the default
  CLI path) beyond the crash fix.
- The two failure modes are each pinned by an automated test that
  fails if the fix regresses.

## Decisions

- **argv `--standalone` guard, not an env var** — reuses existing
  semantics ("run in this process"), stays visible in `ps` for
  debugging, and the daemon-dispatch argv tests are the regression
  seam that already exists. An env-var guard would add a second
  mechanism for no added safety.
- **Ambient runtime, not a zbus downgrade** — Phase 6 deliberately
  put zbus 5 (tokio) in the tree for ashpd; fighting it with
  patches reintroduces the backend clash the workspace `Cargo.toml`
  comment warns about. A runtime per short-lived worker is cheap
  and local to `flintshot-app`.
- **Guard lives in `flintshot-app`, not `flintshot-editor-egui`** —
  needing a reactor is a property of the *process* (zbus via
  accesskit), not of the editor crate; the app owns process
  lifecycle. Also keeps the editor crate free of tokio.

## Open questions

1. Exact guard placement: one `with_ambient_runtime` wrapping the
   three eframe-calling helpers (`run_capture_shell`,
   `run_editor_for_capture`, `run_pin_window`) vs. wrapping at the
   public `run_*` entry points. Resolved in plan.md — preference
   for the narrowest wrapping that covers every window path, so
   future non-window paths don't pay for a runtime.
2. Does `flintshot upload` (no window, but also a daemon-spawned
   worker) need the guard? Expected no — it never reaches eframe —
   but verify during implementation; if its HTTP client is async
   it already manages its own runtime.
