# Task 002: Wire pump into `run_event_loop` + correct module doc

**Description:** Call `pump_glib_context()` at the top of
`run_event_loop`'s loop body (before draining tray menu events —
muda delivers through the same context), under the same
`#[cfg(all(feature = "tray", target_os = "linux"))]` gate. Rewrite
the daemon.rs module-header paragraph that claims "raw polling
suffices on Linux (both libraries pump the OS internally)" — say
what is true: `global-hotkey` hands events to a static crossbeam
receiver, but the GTK-backed tray half needs the GLib main context
iterated, which the pump now does.

**Acceptance criteria:**

- [x] Pump called once per loop iteration before
      `drain_tray_menu`, with a short why-comment at the call site
- [x] daemon.rs module doc no longer claims polling suffices for
      the tray; it states the per-source truth (crossbeam receiver
      vs GLib context)
- [x] No behavior change for non-tray builds (cfg-gated call)
- [x] Tray-feature unit tests from Task 1 still green

**Verification:**

- [x] `cargo build -p flintshot-app --features tray --quiet` clean
- [x] `cargo test -p flintshot-app --features tray --quiet` green
- [x] `cargo check -p flintshot-app --no-default-features --quiet`
      clean
- [x] Standing gates: `cargo fmt --all --check`;
      `cargo clippy --workspace --all-targets -- -D warnings`;
      `cargo clippy -p flintshot-app --features tray --all-targets
      -- -D warnings` (tray path is not covered by the default
      clippy run); `cargo test --workspace --all-targets --quiet`;
      doc tests per AGENTS.md

**Dependencies:** Task 1 (the helper)

**Files likely touched:**

- `crates/flintshot-app/src/daemon.rs`

**Docs:**

- Module-header correction *is* the doc deliverable here; ADR-0031
  (Task 4) records the decision formally.

**Estimated scope:** Small: 1 file
