//! XDG portal `PickColor` — the eyedropper's screen arm (eyedropper ED3).
//!
//! The portal picks a colour from **anywhere on screen**, outside the
//! editor window — the one thing canvas sampling honestly cannot do
//! (Wayland's model: no global screen access without the portal).
//!
//! Posture is advisory, never modal: the round-trip runs on a worker
//! thread (the editor's UI thread has no reactor and must not block),
//! and *every* failure — user denial (the dialog's Cancel), timeout,
//! transport — degrades to `Err(toast copy)`. The editor toasts it
//! and keeps the popover exactly as it was. Denial is not an error
//! worth a dialog; it is a user changing their mind.
//!
//! Availability is probed, not assumed by display server: X11
//! sessions get the screen chip too whenever xdg-desktop-portal runs
//! (`org.freedesktop.portal.Desktop` owns its bus name). Non-Linux
//! builds compile the surface with `false` — the chip never shows.

#[cfg(target_os = "linux")]
use std::time::Duration;

use flintshot_core::Color;

/// Toast copy for a user-cancelled dialog (denial).
pub(crate) const CANCELLED_COPY: &str = "Screen colour pick cancelled";

/// How long the round-trip may run before the client gives up on the
/// portal. The consent dialog is user-paced, so the guard is
/// deliberately generous (an idle dialog is not a hang — the user
/// simply has not picked yet); it exists so a wedged portal backend
/// cannot strand a pending flight forever.
#[cfg(target_os = "linux")]
const PICK_TIMEOUT_SECS: u64 = 600;

/// The timeout guard must stay generous: the consent dialog is
/// user-paced, and a refactor that shortens this into a race with a
/// slow user breaks the build, not just a test.
#[cfg(target_os = "linux")]
const _: () = assert!(PICK_TIMEOUT_SECS >= 300, "the dialog is user-paced");

/// Map one portal round-trip outcome onto the advisory contract:
/// `Ok` → the picked colour (f64 unit channels → u8 through
/// [`Color::from_unit_rgb`]'s documented rounding); denial (the
/// portal's structured `Response(Cancelled)`) → the cancelled copy;
/// every other failure → the failed copy. Pure.
#[cfg(target_os = "linux")]
pub(crate) fn map_pick_result(
    result: Result<ashpd::desktop::Color, ashpd::Error>,
) -> Result<Color, String> {
    match result {
        Ok(c) => Ok(Color::from_unit_rgb(c.red(), c.green(), c.blue())),
        Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => {
            Err(CANCELLED_COPY.into())
        }
        Err(err) => Err(format!("Screen colour pick failed: {err}")),
    }
}

/// Whether the xdg-desktop-portal service is reachable (its bus name
/// has an owner on the session bus). Probed once per page
/// construction on the UI thread — a fresh current-thread runtime +
/// connection, the SNI watchdog's `SessionBus` precedent; the cost is
/// milliseconds and never repeats.
#[must_use]
pub(crate) fn portal_reachable() -> bool {
    name_has_owner()
}

#[cfg(target_os = "linux")]
fn name_has_owner() -> bool {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            tracing::warn!(error = %err, "building the portal probe runtime");
            return false;
        }
    };
    runtime.block_on(async {
        let conn = match ashpd::zbus::Connection::session().await {
            Ok(conn) => conn,
            Err(err) => {
                tracing::debug!(error = %err, "no session bus for the portal probe");
                return false;
            }
        };
        let dbus = match ashpd::zbus::fdo::DBusProxy::new(&conn).await {
            Ok(dbus) => dbus,
            Err(err) => {
                tracing::debug!(error = %err, "the fdo proxy refused the portal probe");
                return false;
            }
        };
        let Ok(name) = ashpd::zbus::names::BusName::try_from("org.freedesktop.portal.Desktop")
        else {
            return false;
        };
        dbus.name_has_owner(name).await.unwrap_or(false)
    })
}

/// Non-Linux: the portal is a Linux facility — the chip never shows.
#[cfg(not(target_os = "linux"))]
fn name_has_owner() -> bool {
    false
}

/// Fire the portal `PickColor` round-trip on a worker thread and
/// return the arrival channel. The worker owns a short-lived
/// current-thread runtime (the AI worker's shape); the editor polls
/// the channel each frame and never blocks.
#[cfg(target_os = "linux")]
pub(crate) fn spawn_screen_color_pick() -> std::sync::mpsc::Receiver<Result<Color, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("flintshot-color-pick".into())
        .spawn(move || {
            let arrived = worker_round_trip();
            let _ = tx.send(arrived);
        })
        .expect("spawning the colour-pick worker thread");
    rx
}

/// The blocking round-trip body: connect, `PickColor`, map. Timeout
/// wraps the whole dance so a wedged backend cannot outlive
/// [`PICK_TIMEOUT_SECS`].
#[cfg(target_os = "linux")]
fn worker_round_trip() -> Result<Color, String> {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => return Err(format!("Screen colour pick failed: {err}")),
    };
    runtime.block_on(async {
        let pick = async {
            let screenshot = ashpd::desktop::screenshot::ScreenshotProxy::new().await?;
            screenshot
                .pick_color(None, ashpd::desktop::screenshot::ColorOptions::default())
                .await?
                .response()
        };
        match tokio::time::timeout(Duration::from_secs(PICK_TIMEOUT_SECS), pick).await {
            Ok(result) => map_pick_result(result),
            Err(_elapsed) => Err(format!(
                "Screen colour pick failed: no answer within {PICK_TIMEOUT_SECS}s"
            )),
        }
    })
}

/// Non-Linux: no portal to ask — requests are never spawned (the
/// chip is hidden by [`portal_reachable`]); the stub keeps the
/// bridge's call sites compiling.
#[cfg(not(target_os = "linux"))]
pub(crate) fn spawn_screen_color_pick() -> std::sync::mpsc::Receiver<Result<Color, String>> {
    drop(std::sync::mpsc::channel::<Result<Color, String>>().1);
    std::sync::mpsc::channel::<Result<Color, String>>().1
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    /// A picked portal colour converts through the documented
    /// rounding (0.5 → 128).
    #[test]
    fn picked_color_maps_through_unit_rgb() {
        let picked = ashpd::desktop::Color::new(0.5, 0.0, 1.0);
        assert_eq!(
            map_pick_result(Ok(picked)),
            Ok(Color::rgb(128, 0, 255)),
            "the portal's f64 channels land on u8 per the ED1 contract"
        );
    }

    /// Denial (the dialog's Cancel) is the structured
    /// `Response(Cancelled)` — it maps to the cancelled toast copy,
    /// never an error dialog's copy.
    #[test]
    fn denial_maps_to_the_cancelled_copy() {
        assert_eq!(
            map_pick_result(Err(ashpd::Error::Response(
                ashpd::desktop::ResponseError::Cancelled
            ))),
            Err(CANCELLED_COPY.to_owned())
        );
    }

    /// Every other failure (transport, portal method errors — any
    /// non-Cancelled response, any zbus error) maps to the failed
    /// copy with the error named — advisory posture: one line, the
    /// popover stays.
    #[test]
    fn other_failures_map_to_the_failed_copy() {
        let err = map_pick_result(Err(ashpd::Error::Response(
            ashpd::desktop::ResponseError::Other,
        )));
        assert!(
            matches!(err, Err(ref msg) if msg.starts_with("Screen colour pick failed:")),
            "non-denial failures carry the failed copy: {err:?}"
        );
    }
}
