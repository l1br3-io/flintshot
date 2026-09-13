# Task 002: `Capturer::capture_selection` + portal interactive impl

**Description:** Add the selection capability to the capture trait
and implement it in the portal backend. `Capturer` grows
`supports_selection(&self) -> bool` (default `false`) and
`capture_selection(&self, opts: &CaptureOptions) -> Result<RgbaImage>`
(default `Err(NotImplemented)`); `PortalScreenshotCapturer` overrides
both — the existing `grab_whole_desktop` dance with
`interactive: true` in the options map, then (per Task 1's verdict)
passthrough or composite crop. Code 1 keeps flowing as the
`"cancelled"` capture error — callers decide quietness. The options
map construction gets a pure seam so the `interactive` key is
unit-testable headless.

**Acceptance criteria:**
- [ ] Trait: `supports_selection()` default false; `xcap` unchanged;
      portal returns true; unit tests pin the capability matrix
- [ ] Portal `capture_selection`: options map carries
      `interactive: true` (pure-seam unit test); timeouts/cleanup
      reuse the existing budgets; Escape → `"cancelled"` error
- [ ] Return shape per Task 1 verdict (passthrough or crop) with a
      GOTCHA comment at the decision point
- [ ] `#[ignore]`'d live test extends `tests/capture.rs`:
      interactive grab + cancelled path (manual)
- [ ] `capture_monitor` (non-interactive) behavior unchanged

**Verification:**
- [ ] `cargo test -p flintshot-core --quiet` green
- [ ] `cargo test -p flintshot-core --test capture -- --ignored` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] Manual (COSMIC): interactive grab returns the exact selection

**Dependencies:** Task 1 (return-shape verdict)

**Files likely touched:**
- `crates/flintshot-core/src/capture/mod.rs` (trait)
- `crates/flintshot-core/src/capture/portal.rs` (impl)
- `crates/flintshot-core/src/capture/xcap_backend.rs` (doc note only)
- `crates/flintshot-core/tests/capture.rs` (live test)

**Docs:**
- GOTCHA: interactive-mode quirks from Task 1 → comment at the
  options-map site in `portal.rs`.

**Estimated scope:** Medium (4 files)
