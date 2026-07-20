//! Screen capture wrapper around [`xcap`].
//!
//! Two public entry points:
//! - [`capture_fullscreen`] grabs a whole monitor's framebuffer.
//! - [`capture_region`] crops a sub-rect out of a full capture.
//!
//! # Coordinate space (`HiDPI`)
//!
//! `xcap` returns images in **physical pixels** — the actual framebuffer
//! resolution, not the logical resolution the compositor reports. This
//! is what we want: annotations are stored and rendered against the
//! captured image, so a 1:1 pixel mapping keeps everything crisp. The
//! editor shell converts physical → logical pixels at the egui event
//! boundary (see `docs/ARCHITECTURE.md` §Cross-platform concerns).
//!
//! # Errors
//!
//! All `xcap` failures map to [`FlintshotError::Capture`] with the
//! underlying message preserved verbatim. On macOS, permission-denied
//! errors contain the substring `"permission"`; downstream code (e.g.
//! Task 5's editor shell) can match on that to guide the user to
//! System Settings → Privacy & Security → Screen Recording.
//!
//! # Wayland
//!
//! First capture per session may trigger an `xdg-desktop-portal`
//! permission dialog. This is `xcap`'s standard behaviour on Wayland
//! and not something this module controls; see `docs/ARCHITECTURE.md`
//! §Linux for the mitigation strategy.

use image::RgbaImage;
use xcap::Monitor;

use crate::error::{FlintshotError, Result};
use crate::geom::Rect;

/// Capture a full monitor's framebuffer in physical pixels.
///
/// `monitor = None` selects the primary monitor (the one `xcap` flags
/// via [`Monitor::is_primary`]). `monitor = Some(n)` picks the n-th
/// entry from [`Monitor::all()`], in the order `xcap` returns them
/// (typically: primary first, then by display index).
///
/// # Errors
///
/// - [`FlintshotError::Capture`] if monitor enumeration or the capture
///   call itself fails. The `detail` string preserves `xcap`'s message
///   verbatim, including the `"permission"` substring on macOS
///   permission denials.
/// - [`FlintshotError::Capture`] with a synthetic message if the
///   requested monitor index is out of range, or if no primary monitor
///   could be identified.
pub fn capture_fullscreen(monitor: Option<u32>) -> Result<RgbaImage> {
    let monitors = Monitor::all().map_err(|e| FlintshotError::Capture {
        detail: e.to_string(),
    })?;
    let chosen = pick_monitor(&monitors, monitor)?;
    chosen.capture_image().map_err(|e| FlintshotError::Capture {
        detail: e.to_string(),
    })
}

/// Capture a sub-rect of a monitor's framebuffer, in physical pixels.
///
/// The rect is validated **before** `xcap` is invoked, so degenerate
/// inputs fail fast without touching the display server. Rects that
/// extend beyond the captured image are silently clamped to the image
/// bounds (matches the convention in `image::imageops::crop_imm`).
///
/// # Errors
///
/// - [`FlintshotError::InvalidRect`] if `rect` has zero area or
///   inverted corners (`max.x < min.x` or `max.y < min.y`).
/// - Propagates any error from [`capture_fullscreen`].
pub fn capture_region(rect: Rect, monitor: Option<u32>) -> Result<RgbaImage> {
    validate_rect(rect)?;
    let full = capture_fullscreen(monitor)?;
    Ok(crop_to_rect(&full, rect))
}

/// Pick the monitor to capture, honouring the `None` → primary rule.
fn pick_monitor(monitors: &[Monitor], idx: Option<u32>) -> Result<&Monitor> {
    match idx {
        None => find_primary(monitors),
        Some(n) => monitors
            .get(n as usize)
            .ok_or_else(|| FlintshotError::Capture {
                detail: format!(
                    "monitor index {n} out of range; {} monitor(s) available",
                    monitors.len()
                ),
            }),
    }
}

/// Find the primary monitor. Falls back to the first one if `xcap`
/// reports no monitor as primary (some platforms / virtual heads-ups).
fn find_primary(monitors: &[Monitor]) -> Result<&Monitor> {
    for m in monitors {
        if m.is_primary().unwrap_or(false) {
            return Ok(m);
        }
    }
    monitors.first().ok_or_else(|| FlintshotError::Capture {
        detail: "no monitors available".into(),
    })
}

/// Reject degenerate rects without invoking `xcap`. Headless-testable.
///
/// `==` is correct here rather than an epsilon comparison: the test is
/// structural (does the rect have zero extent?), not the result of
/// floating-point arithmetic. A rect built with `min.x == max.x` was
/// constructed that way intentionally.
#[allow(
    clippy::float_cmp,
    reason = "structural zero-extent check, not arithmetic equality"
)]
fn validate_rect(rect: Rect) -> Result<()> {
    if rect.max.x < rect.min.x || rect.max.y < rect.min.y {
        return Err(FlintshotError::InvalidRect {
            rect,
            reason: "negative",
        });
    }
    if rect.max.x == rect.min.x || rect.max.y == rect.min.y {
        return Err(FlintshotError::InvalidRect {
            rect,
            reason: "empty",
        });
    }
    Ok(())
}

/// Crop `src` to `rect`, clamping to image bounds. Callers must have
/// validated that `rect` is non-empty and non-negative first.
///
/// The `f32 → u32` casts are sign/truncation-safe by construction:
/// each value is clamped to `[0.0, image_dim]` before the cast, so
/// nothing truncates or loses sign at runtime.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "values clamped to image bounds before cast"
)]
fn crop_to_rect(src: &RgbaImage, rect: Rect) -> RgbaImage {
    let x = (rect.min.x.max(0.0) as u32).min(src.width());
    let y = (rect.min.y.max(0.0) as u32).min(src.height());
    let w = ((rect.max.x - rect.min.x).max(0.0) as u32).min(src.width().saturating_sub(x));
    let h = ((rect.max.y - rect.min.y).max(0.0) as u32).min(src.height().saturating_sub(y));
    image::imageops::crop_imm(src, x, y, w, h).to_image()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Point;

    /// Headless unit test: invalid rects never reach `xcap`. Confirms
    /// both the empty and negative rejection paths. The happy-path
    /// capture is covered by the `#[ignore]`'d
    /// `tests/capture.rs::smoke_can_grab_primary` integration test.
    #[test]
    fn invalid_rect_rejected() {
        let negative = Rect {
            min: Point::new(100.0, 100.0),
            max: Point::new(50.0, 50.0),
        };
        match capture_region(negative, None) {
            Err(FlintshotError::InvalidRect { reason, .. }) => {
                assert_eq!(reason, "negative", "expected 'negative' reason");
            }
            other => panic!("expected InvalidRect(negative), got {other:?}"),
        }

        let empty = Rect {
            min: Point::new(10.0, 10.0),
            max: Point::new(10.0, 100.0),
        };
        match capture_region(empty, None) {
            Err(FlintshotError::InvalidRect { reason, .. }) => {
                assert_eq!(reason, "empty", "expected 'empty' reason");
            }
            other => panic!("expected InvalidRect(empty), got {other:?}"),
        }
    }

    /// Pure validation check: a normalised, non-zero rect passes the
    /// validation step. (Doesn't call `xcap` — we stop right before.)
    #[test]
    fn valid_rect_passes_validation() {
        let good = Rect::from_xywh(10.0, 10.0, 100.0, 100.0);
        validate_rect(good).expect("non-empty, non-negative rect should validate");
    }

    /// Pure validation check: clamping math handles a rect that extends
    /// beyond the source image. Doesn't touch `xcap`; just verifies
    /// `crop_to_rect` doesn't panic on out-of-bounds input.
    #[test]
    fn crop_to_rect_clamps_out_of_bounds() {
        let src = RgbaImage::new(100, 100);
        // Rect from (90, 90) extending past the right/bottom edge by 50px.
        let oversized = Rect::from_xywh(90.0, 90.0, 50.0, 50.0);
        let cropped = crop_to_rect(&src, oversized);
        // Clamped to (90, 90, 10, 10) — the remainder of the image.
        assert_eq!(cropped.dimensions(), (10, 10));
    }
}
