//! XDG Screenshot Portal backend — `org.freedesktop.portal.Screenshot`
//! over D-Bus (Linux only).
//!
//! Placeholder registration (native-capture-backends task 003): the id
//! `"portal"` occupies its Wayland auto-detection slot and `--backend
//! portal` pin paths before the real implementation lands in task 006,
//! which replaces this file wholesale. Spike B's findings (timing,
//! coverage crop, cursor gap, malformed `file:////` URIs) are recorded
//! in `docs/specs/native-capture-backends/tasks/002-spike-portal.md`.

use image::RgbaImage;

use super::{CaptureOptions, Capturer};
use crate::error::{FlintshotError, Result};

/// Cross-desktop portal screenshot backend (stub until task 006).
pub struct PortalScreenshotCapturer;

impl Capturer for PortalScreenshotCapturer {
    fn id(&self) -> &'static str {
        "portal"
    }

    fn name(&self) -> &'static str {
        "XDG Screenshot Portal"
    }

    fn available(&self) -> Result<()> {
        Err(FlintshotError::Capture {
            detail: "portal screenshot backend not implemented yet (task 006)".into(),
        })
    }

    fn capture_monitor(&self, _monitor: Option<u32>, _opts: &CaptureOptions) -> Result<RgbaImage> {
        Err(FlintshotError::NotImplemented(
            "portal screenshot backend (task 006)",
        ))
    }
}
