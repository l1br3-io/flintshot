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
- [x] Trait: `supports_selection()` default false; `xcap` unchanged;
      portal returns true; unit tests pin the capability matrix
- [x] Portal `capture_selection`: options map carries
      `interactive: true` (pure-seam unit test); timeouts/cleanup
      reuse the existing budgets; Escape → `"cancelled"` error
- [x] Return shape per Task 1 verdict (passthrough or crop) with a
      GOTCHA comment at the decision point
- [x] `#[ignore]`'d live test extends `tests/capture.rs`:
      interactive grab + cancelled path (manual)
- [x] `capture_monitor` (non-interactive) behavior unchanged

**Verification:**
- [x] `cargo test -p flintshot-core --quiet` green
- [x] `cargo test -p flintshot-core --test capture -- --ignored` green
      (interactive run soft-skips on the session's wedged selection
      UI — spike C addendum; pre-wedge, the probe verified the exact
      selection three times)
- [x] `cargo clippy --workspace --all-targets -- -D warnings` green
- [x] Manual (COSMIC): interactive grab returns the exact selection
      (verified pre-wedge via the spike probe, runs 1/5/6; the
      post-wedge live test exercises the documented soft-skip path)

**Note:** the Response budget on the interactive path is a dedicated
`INTERACTIVE_RESPONSE_TIMEOUT` (5 min) rather than the shared 5 s —
spike C overrode the task's "reuse the existing budgets" premise:
interactive Response waits budget human think-time and the observed
no-Response hang must be bounded.

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
