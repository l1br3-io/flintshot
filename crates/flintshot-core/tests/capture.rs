//! Integration tests for [`flintshot_core::capture`].
//!
//! The smoke test below needs a real display server, so it's marked
//! `#[ignore]` and listed in `docs/TEST_MATRIX.md`. Run it manually:
//!
//! ```sh
//! cargo test -p flintshot-core --test capture -- --ignored \
//!     smoke_can_grab_primary
//! ```
//!
//! On Wayland, the first capture per session may pop an
//! `xdg-desktop-portal` permission dialog — grant it once and the test
//! should proceed. On macOS, the first run triggers a system Screen
//! Recording permission prompt; the test will surface the denial as a
//! `FlintshotError::Capture` whose `detail` contains the substring
//! `"permission"`.

#![forbid(unsafe_code)]

use flintshot_core::{capture_fullscreen, FlintshotError};

#[cfg(target_os = "linux")]
use flintshot_core::capture::{CaptureOptions, Capturer, PortalScreenshotCapturer};

/// Grab the primary monitor and sanity-check that we got a non-empty
/// image back. Marked `#[ignore]` so CI doesn't fail on a headless
/// runner; humans invoke it explicitly per `docs/TEST_MATRIX.md`.
#[test]
#[ignore]
fn smoke_can_grab_primary() {
    let img = match capture_fullscreen(None) {
        Ok(img) => img,
        Err(FlintshotError::Capture { detail }) if detail.contains("permission") => {
            // macOS Screen Recording permission was denied — surface a
            // readable message instead of failing the test. Treating
            // this as a soft-skip keeps the test useful on machines
            // where the user hasn't yet granted permission.
            eprintln!("SKIP: capture permission denied ({detail})");
            return;
        }
        Err(e) => panic!("capture_fullscreen(None) failed: {e}"),
    };

    assert!(
        img.width() > 0 && img.height() > 0,
        "captured image had zero dimensions: {}x{}",
        img.width(),
        img.height(),
    );
}

/// Live portal-backend grab of the primary monitor. Needs a real
/// session with `xdg-desktop-portal` running (spike B: prompt-free on
/// COSMIC; other impls may show a dialog — that's data, not failure).
/// Run it manually per `docs/TEST_MATRIX.md`:
///
/// ```sh
/// cargo test -p flintshot-core --test capture -- --ignored \
///     portal_smoke_grabs_primary
/// ```
#[test]
#[ignore]
#[cfg(target_os = "linux")]
fn portal_smoke_grabs_primary() {
    let capturer = PortalScreenshotCapturer;
    if let Err(FlintshotError::Capture { detail }) = capturer.available() {
        eprintln!("SKIP: portal unavailable ({detail})");
        return;
    }

    let img = match capturer.capture_monitor(None, &CaptureOptions::default()) {
        Ok(img) => img,
        Err(FlintshotError::Capture { detail }) if detail.contains("cancelled") => {
            eprintln!("SKIP: portal request was dismissed ({detail})");
            return;
        }
        Err(e) => panic!("portal capture_monitor(None) failed: {e}"),
    };

    assert!(
        img.width() > 0 && img.height() > 0,
        "portal capture had zero dimensions: {}x{}",
        img.width(),
        img.height(),
    );
}
