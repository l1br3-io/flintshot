# Task 003: Capturer trait + registry + compat wrappers

**Description:** Split `capture.rs` into `capture/mod.rs` + `capture/xcap_backend.rs`.
Define the `Capturer` trait, `CaptureOptions`, and a testable
selection registry: session-kind detection (`XDG_SESSION_TYPE`) and
availability probes are injectable seams; the real `select_backend`
wraps them. Existing free functions (`monitors`,
`capture_fullscreen`, `capture_region`) become thin delegates over
`XcapCapturer` — public API stays source-compatible. All headless, no
new dependencies; native backend slots appear as linux-cfg'd
unavailable stubs so the registry's precedence is exercisable now.

**Acceptance criteria:**
- [ ] `Capturer` trait (`id`, `name`, `available`, `monitors`,
      `capture_monitor`) + `CaptureOptions { include_cursor }` public,
      documented, `#[non_exhaustive]` where plausible
- [ ] `select_backend(preference)` resolves: explicit known id →
      pinned; explicit known id with failed probe → `Err(Capture)`
      carrying probe detail; unknown id → `tracing::warn!` + auto;
      `"auto"` → precedence walk ending at xcap (never fails)
- [ ] Pure selection core (`select_backend_with`) unit-tested with
      fake backends covering: X11→xcap, Wayland+GNOME→gnome→portal→xcap,
      Wayland/other→portal→xcap, all-probes-fail→xcap, pin-to-failed→Err
- [ ] Existing capture unit tests (rect validation, crop) still pass;
      `tests/capture.rs` smoke test compiles unchanged
- [ ] `flintshot-cosmic` builds unchanged (`monitors()` wrapper intact)

**Verification:**
- [ ] `cargo test -p flintshot-core --quiet` green
- [ ] `cargo test --workspace --all-targets --quiet` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green

**Dependencies:** None (parallelizable with Tasks 1-2)

**Files likely touched:**
- `crates/flintshot-core/src/capture.rs` → `crates/flintshot-core/src/capture/mod.rs`
- `crates/flintshot-core/src/capture/xcap_backend.rs` (new)
- `crates/flintshot-core/src/lib.rs` (re-exports)

**Docs:**
- GOTCHA: index-vs-geometry monitor mapping trap → comment on the
  trait's `capture_monitor` doc.

**Estimated scope:** Medium (3-4 files)
