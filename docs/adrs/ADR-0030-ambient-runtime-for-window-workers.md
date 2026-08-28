# ADR-0030: Ambient tokio runtime for window workers

**Context.** Phase 6 (ADR-0029) brought `ashpd` in pinned to
`features = ["tokio"]`, and `zbus` 5.18 arrived transitively —
unifying accesskit onto zbus 5 in the process. That made a
previously invisible dependency load-bearing for every window: on
at-spi desktops, `eframe` → `egui-winit` → `accesskit_winit` →
`accesskit_unix` → `atspi` → zbus 5, and zbus 5's tokio executor
requires an ambient reactor on the thread that creates the window.

Window-opening workers have none. The daemon spawns them as bare
processes (ADR-0019); the CLI's one-shot delegate runtime is already
dropped by the time a window opens. So `region`/`fullscreen` →
editor or overlay, `open` → editor, and `pin` → pin window all
panicked — `there is no reactor running, must be called from the
context of a Tokio 1.x runtime` at `zbus-5.18.0/src/abstractions/
executor.rs` — before any window appeared (TEST_MATRIX L5, finding
2). This hits **every desktop with `org.a11y.Bus` running**
(GNOME/KDE included), not just COSMIC; sessions without the a11y
bus skip the path entirely.

Alternatives considered:

- **Pin accesskit's zbus back to 4.x** — cargo-patching or
  duplicating zbus fights Phase 6's unification and leaves two zbus
  majors in one tree, forever. Rejected.
- **Switch ashpd to the async-io backend** — ashpd's defaults pull
  async-io, which is exactly the clash the workspace `Cargo.toml`
  comment warns about: it would fight the daemon's single tokio
  runtime (ADR-0018, ADR-0029). Rejected.
- **Enter a runtime for the whole worker process (e.g. at `main`)** —
  leaks reactor context to everything later on the thread;
  `ipc_client` builds one-shot `block_on` runtimes, and nesting a
  runtime inside an entered context panics. Rejected.
- **Put the guard in `flintshot-editor-egui`** — needing a reactor
  is a property of the *process* (zbus via accesskit), not of the
  editor; this would add a tokio dependency to a crate that has
  none. Rejected.

**Decision.** Window workers carry their own short-lived ambient
runtime.

- New `flintshot-app::ambient` module:
  `with_ambient_runtime(f)` builds a multi-thread `Runtime`, holds
  its `enter()` guard for `f`'s duration, and drops the guard before
  returning. Runtime-construction failure maps to an `eyre` error —
  never a panic.
- Wired at the **narrowest seam that covers every window path**:
  around the `EguiShell` call inside `run_editor_with_config`,
  `run_overlay_with_config`, and `run_pin_window`. Future non-window
  paths pay nothing, and each of the three call sites comments why
  the guard is there.
- Guard scope is load-bearing: it is dropped before
  `report_editor_outcome` fires `ipc_client`'s one-shot `block_on`
  runtimes, so no runtime is ever nested inside an entered context.
- `run_upload` is untouched — it already owns a runtime and never
  reaches eframe. Headless (`--no-default-features`) fallbacks are
  untouched — they open no window.
- The cost — one multi-thread runtime built and torn down per
  window worker — is accepted: these are exit-after-window
  processes, and the runtime dies with them.

**Consequences.**

- Standalone captures on at-spi sessions open the editor instead of
  panicking; no behaviour change anywhere else (fix landed `54d0bf1`
  2026-08-28, TEST_MATRIX L5 finding 2).
- CI cannot reproduce the real panic (no at-spi session on a
  runner), so the guard tests in `crates/flintshot-app/src/ambient.rs`
  proxy the exact condition zbus 5's executor checks via
  `tokio::runtime::Handle::try_current()`: no handle on a plain
  thread, visible inside `with_ambient_runtime`, gone after. The
  real-path proof stays a manual L5 pass — honest, not simulated.
- Every *future* eframe-calling path must remember the guard; the
  module doc on `ambient.rs` states the why so the omission reads as
  a bug, not a style choice.
- Desktops without at-spi pay for the runtime too (they never touch
  zbus) — accepted as noise relative to window creation itself.
- The decision is reversible in one direction only if upstream
  moves: should accesskit grow its own reactor management, the guard
  can shrink to nothing; the zbus-4/async-io downgrades would remain
  wrong either way.
