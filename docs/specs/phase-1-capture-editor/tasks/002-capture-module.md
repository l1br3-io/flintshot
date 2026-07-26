# Task 002 — Capture module in `flintshot-core::capture`

> Phase 1.A · size: S · depends on: none

## Description

Add `capture` module wrapping `xcap`. Public API:
`capture_fullscreen(monitor: Option<u32>) -> Result<RgbaImage>` and
`capture_region(rect: Rect, monitor: Option<u32>) -> Result<RgbaImage>`.
Map `xcap` errors into `FlintshotError::Capture` (new variant). The
`monitor` arg indexes into `xcap::Monitor::enumerate()`; `None` picks the
primary. HiDPI: capture in physical px (xcap default), document that fact.

## Acceptance criteria

- [x] `capture_fullscreen(None)` returns the primary monitor's image
      with `.dimensions()` matching the monitor's physical size.
- [x] `capture_region` crops the supplied `Rect` (physical px) out of
      the full capture; rejects empty / negative rects with
      `FlintshotError::InvalidRect` (new variant).
- [x] `FlintshotError` gains `Capture { source: String }` and
      `InvalidRect` variants; both documented. _(Field renamed to
      `detail` in the implementation — `thiserror` treats a field
      literally named `source` as the error-chain source and requires
      it to be `dyn Error`. Functionally equivalent; see Task 2
      close-out note.)_
- [x] No `unwrap`/`expect`; all `xcap` fallible calls mapped to
      `FlintshotError`.
- [x] macOS permission failure surfaces as `FlintshotError::Capture`
      with a message containing "permission" (so Task 5 can detect and
      guide).

## Verification

- [x] Standing gate passes.
- [x] `#[ignore]`'d integration test `capture::smoke_can_grab_primary`
      in `crates/flintshot-core/tests/capture.rs` — requires a display;
      listed in `TEST_MATRIX.md`.
- [x] Unit test `capture::invalid_rect_rejected` runs headless and
      verifies the error path without calling xcap. _(Plus two more
      headless tests: `valid_rect_passes_validation` and
      `crop_to_rect_clamps_out_of_bounds`.)_

## Dependencies

None.

## Files likely touched

- `crates/flintshot-core/src/capture.rs` (new)
- `crates/flintshot-core/src/lib.rs` (re-export)
- `crates/flintshot-core/src/error.rs` (new variants)
- `crates/flintshot-core/Cargo.toml` (dep: `xcap`)
- `crates/flintshot-core/tests/capture.rs` (new, `#[ignore]`'d)

## Size

S
