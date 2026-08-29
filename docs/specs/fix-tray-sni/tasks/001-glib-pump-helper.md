# Task 001: Bounded GLib pump helper + headless unit tests

**Description:** Add a `pump_glib_context()` helper to
`crates/flintshot-app/src/daemon.rs`, gated
`#[cfg(all(feature = "tray", target_os = "linux"))]`: drain the
default GLib main context with non-blocking
`gtk::glib::MainContext::default().iteration(false)` calls, capped
at a small per-call maximum (e.g. 64 iterations) so a pathological
always-ready source cannot wedge the daemon loop — leftover work
spreads over later ticks. The `glib` path comes via the `gtk`
crate's re-export; no dependency changes. Tests prove the seam
headless: glib idle sources need no display and no GTK init.

**Acceptance criteria:**

- [x] Helper exists, cfg-gated as above, documented with the *why*
      (libappindicator/muda drive from GLib sources; daemon never
      iterated the context — TEST_MATRIX finding 3), with the
      iteration cap as a named `const`
- [x] Unit test: an `idle_add` source setting an `AtomicBool` never
      fires after a plain bounded spin (no pump) — the RED condition
- [x] Unit test: the same source fires after `pump_glib_context()`
      runs
- [x] Unit test: a self-re-arming source (re-adds itself) cannot
      make `pump_glib_context()` run forever — it returns after the
      cap
- [x] Tests run under `cargo test -p flintshot-app --features tray`
      without a display; no `unwrap()`/`expect()` outside test code
- [x] `cargo check -p flintshot-app --no-default-features` stays
      clean (helper invisible to non-tray builds)

**Verification:**

- [x] `cargo test -p flintshot-app --features tray --quiet` green
- [x] `cargo check -p flintshot-app --no-default-features --quiet`
      clean
- [x] Manual: temporarily run the new tests with the helper stubbed
      to a no-op first (RED confirmed), then implement (GREEN)

**Dependencies:** None

**Files likely touched:**

- `crates/flintshot-app/src/daemon.rs` (helper + `#[cfg(test)]`
  tests; tests additionally gated on the same cfg)

**Docs:**

- Helper doc comment carries the why (finding 3 link); full
  rationale lands as ADR-0031 in Task 4.

**Estimated scope:** Small: 1 file
