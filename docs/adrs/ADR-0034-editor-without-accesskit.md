# ADR-0034: Build the editor without eframe's accesskit feature

**Context.** Live on L5 (COSMIC, 2026-08-30): the installed
(release) binary aborted with a core dump on every
`flintshot region` — before any editor window appeared — with
`there is no reactor running, must be called from the context of
a Tokio 1.x runtime` at `zbus-5.18.0/src/abstractions/executor.rs`.
Debug builds survived the same event with a scary panic printed:
the window still opened, which is why the earlier
`fix-delegated-capture` finding (ambient runtime, ADR-0030)
looked fixed — the release profile (`panic = "abort"`, pinned in
the workspace `Cargo.toml`) turns the same panic into process
death.

Chain: eframe → egui-winit → accesskit_winit → accesskit_unix.
`accesskit_unix` spawns a **plain worker thread** for its a11y
event pump and builds a zbus 5 session bus connection there.
zbus 5 is feature-unified onto its **tokio** executor in this
workspace — unavoidable, since Phase 6's `ashpd` (portal hotkeys)
and the SNI watchdog both need zbus-with-tokio — and zbus's
`Task::spawn_blocking` then requires an ambient tokio reactor,
which accesskit's thread does not have. ADR-0030's ambient
runtime covers the *window-creating* thread only; accesskit's
internal thread is unreachable from our code.

Alternatives considered:

- **accesskit's own `tokio` executor** — its `block_on` builds a
  current-thread runtime on that thread (reactor context enough
  for zbus). Rejected as *unreachable*: `accesskit_winit` marks
  its `async-io` (default) and `tokio` features mutually
  exclusive (`compile_error!`), and eframe → egui-winit enables
  `accesskit_winit` with **default features** — Cargo's additive
  unification lets us add `tokio` but never remove their
  `async-io`, so both end up on and the build fails.
- **Drop zbus/tokio from the workspace** (ashpd on async-io) —
  re-architects the daemon's hotkey path against a deliberate
  earlier decision (the workspace pin documents async-io as
  clashing with the daemon's tokio runtime). High risk, low
  relation to the editor.
- **Wrap every worker in an ambient runtime** — already done
  (ADR-0030) and provably insufficient: the panic is on
  accesskit's *own* thread.

**Decision.** `flintshot-editor-egui` builds eframe with
`default-features = false`, re-enabling eframe's defaults
verbatim **minus `accesskit`** (`default_fonts`, `wayland`,
`wgpu`, `x11`). accesskit_unix leaves the dependency graph
entirely (verified via `cargo tree -i`). The workspace `eframe`
entry keeps `default-features = false` so the feature choice
lives at the consumer. Cost: the editor / overlay / pin windows
have **no screen-reader accessibility** on desktop builds until
egui exposes an executor switch (or accesskit's unix backend
self-hosts a runtime unconditionally). That trade beats a
release binary that core-dumps on every capture — an editor that
cannot open serves no user, with or without a11y.

**Consequences.** L5-verified in the release build: standalone
and daemon-delegated `region` both open the editor with no panic
output; `TEST_MATRIX` finding 2 amended accordingly. Revisit when
egui/eframe (≥ whatever version) exposes the accesskit executor
choice — flipping back is a one-feature change in
`crates/flintshot-editor-egui/Cargo.toml`.
