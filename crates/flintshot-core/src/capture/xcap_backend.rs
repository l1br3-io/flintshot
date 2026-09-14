//! `xcap` capture backend — default provider and universal fallback.
//!
//! Wraps the pre-registry `capture.rs` behavior unchanged: monitor
//! enumeration plus per-monitor grabs via [`xcap`], error messages
//! preserved verbatim. Every other backend and the compatibility free
//! functions in [`super`] bottom out here.

use image::RgbaImage;
use xcap::Monitor;

use super::{CaptureOptions, Capturer, MonitorInfo};
use crate::error::{FlintshotError, Result};

/// Cross-platform capture backend backed by [`xcap`].
///
/// Never gated by availability probes in `auto` mode — [`super`]'s
/// selection treats it as the terminal fallback so a grab always has
/// somewhere to land.
///
/// No native selection capability: [`super::Capturer::supports_selection`]
/// keeps its `false` default and [`super::Capturer::capture_selection`]
/// its `NotImplemented` default — `xcap` has no OS-picker mechanism,
/// so callers run their own selection flow (editor overlay) over a
/// [`Capturer::capture_monitor`] grab.
pub struct XcapCapturer;

impl Capturer for XcapCapturer {
    fn id(&self) -> &'static str {
        "xcap"
    }

    fn name(&self) -> &'static str {
        "xcap"
    }

    fn available(&self) -> Result<()> {
        enumerate_monitors().map(|_| ())
    }

    fn capture_monitor(&self, monitor: Option<u32>, _opts: &CaptureOptions) -> Result<RgbaImage> {
        // xcap 0.9 has no cursor toggle; `include_cursor` follows the
        // platform default (cursor composited), which matches the
        // pre-registry behavior byte-for-byte.
        let monitors = Monitor::all().map_err(|e| xcap_err(&e))?;
        let chosen = pick_monitor(&monitors, monitor)?;
        chosen.capture_image().map_err(|e| xcap_err(&e))
    }
}

/// Enumerate monitors in virtual-desktop order. Shared by the
/// [`Capturer::monitors`] default impl and the compatibility free
/// function so enumeration always rides `xcap`, regardless of which
/// backend performs the grab.
///
/// # Errors
/// - [`FlintshotError::Capture`] if `xcap`'s enumeration fails or the
///   monitor count overflows `u32`.
pub(crate) fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
    let all = Monitor::all().map_err(|e| xcap_err(&e))?;
    let mut out = Vec::with_capacity(all.len());
    for (i, m) in all.into_iter().enumerate() {
        out.push(MonitorInfo {
            index: u32::try_from(i).map_err(|_| FlintshotError::Capture {
                detail: format!("too many monitors to index ({i})"),
            })?,
            name: m.name().unwrap_or_default(),
            is_primary: m.is_primary().unwrap_or(false),
            x: m.x().unwrap_or(0),
            y: m.y().unwrap_or(0),
            width: m.width().unwrap_or(0),
            height: m.height().unwrap_or(0),
        });
    }
    Ok(out)
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

/// Map an `xcap` error into the core error type with the message
/// preserved verbatim (macOS permission denials rely on the
/// `"permission"` substring surviving).
fn xcap_err(e: &xcap::XCapError) -> FlintshotError {
    FlintshotError::Capture {
        detail: e.to_string(),
    }
}
