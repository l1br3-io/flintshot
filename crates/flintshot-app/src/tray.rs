//! System tray menu (`tray-icon` + `muda`), behind the `tray` feature.
//!
//! The menu offers *Capture region*, *Capture fullscreen*, *Open
//! config*, and *Quit*. Clicks arrive on [`tray_icon::menu::MenuEvent`]'s
//! global receiver (muda pumps the OS internally and hands events to a
//! static crossbeam channel); the daemon's main loop drains it via
//! [`action_for`] and routes each click through the same dispatch path
//! the IPC server uses (Task 006), so a tray-initiated capture reaches
//! IPC subscribers and vice versa.

use color_eyre::eyre;
use flintshot_core::ipc::Request;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

// Stable string ids — `MenuEvent::id` is matched against these. Kept
// short and namespaced so they don't collide with any muda defaults.
const ID_CAPTURE_REGION: &str = "flintshot.region";
const ID_CAPTURE_FULLSCREEN: &str = "flintshot.fullscreen";
const ID_OPEN_CONFIG: &str = "flintshot.open-config";
const ID_QUIT: &str = "flintshot.quit";

/// What a tray menu click resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TrayAction {
    /// A request the daemon dispatches via the worker table
    /// (region/fullscreen spawn a worker; quit shuts down).
    Request(Request),
    /// Open `config.toml` in the user's `$EDITOR`.
    OpenConfig,
}

/// Map a menu event to the action it should trigger, or `None` for an
/// unknown id (e.g. events not from our menu).
#[must_use]
pub(crate) fn action_for(event: &MenuEvent) -> Option<TrayAction> {
    match event.id.as_ref() {
        ID_CAPTURE_REGION => Some(TrayAction::Request(Request::CaptureRegion {
            monitor: None,
            delay_ms: 0,
        })),
        ID_CAPTURE_FULLSCREEN => Some(TrayAction::Request(Request::CaptureFullscreen {
            monitor: None,
        })),
        ID_OPEN_CONFIG => Some(TrayAction::OpenConfig),
        ID_QUIT => Some(TrayAction::Request(Request::Quit)),
        _ => None,
    }
}

/// Build the tray icon with its context menu. The returned [`TrayIcon`]
/// must be held alive for the daemon's lifetime; muda retains the menu
/// items itself (they're reference-counted), so they can drop here.
pub(crate) fn build() -> eyre::Result<TrayIcon> {
    // libappindicator (tray-icon's Linux backend) touches GTK widgets
    // for the menu, so GTK must be initialized first — on the main
    // thread, which is where the daemon calls us. macOS/Windows use
    // native backends and skip this.
    #[cfg(target_os = "linux")]
    {
        gtk::init().map_err(|e| eyre::eyre!("gtk::init failed: {e}"))?;
    }

    let menu = Menu::new();
    let region = MenuItem::with_id(ID_CAPTURE_REGION, "Capture region", true, None);
    let fullscreen = MenuItem::with_id(ID_CAPTURE_FULLSCREEN, "Capture fullscreen", true, None);
    let open_config = MenuItem::with_id(ID_OPEN_CONFIG, "Open config", true, None);
    let quit = MenuItem::with_id(ID_QUIT, "Quit", true, None);

    if let Err(e) = menu
        .append(&region)
        .and_then(|()| menu.append(&fullscreen))
        .and_then(|()| menu.append(&PredefinedMenuItem::separator()))
        .and_then(|()| menu.append(&open_config))
        .and_then(|()| menu.append(&PredefinedMenuItem::separator()))
        .and_then(|()| menu.append(&quit))
    {
        return Err(eyre::eyre!("building tray menu: {e}"));
    }

    let icon = icon()?;
    TrayIconBuilder::new()
        .with_tooltip("flintshot")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .map_err(|e| eyre::eyre!("building tray icon: {e}"))
}

/// A simple two-tone 32×32 mark — a bright lens on a dark rounded
/// plate — so the tray isn't blank on any platform. Replaced with a
/// real logo in the polish sweep. Returns `Result` (rather than
/// infallible) to keep the no-`expect` rule; `build` propagates an icon
/// failure as a non-fatal "tray unavailable" warning.
fn icon() -> eyre::Result<Icon> {
    const W: u32 = 32;
    const H: u32 = 32;
    // Dark plate + bright disc, with a darker ring for definition.
    let plate = [38_u8, 38, 48, 255];
    let disc = [255_u8, 90, 70, 255];
    let ring = [22_u8, 22, 30, 255];

    let mut rgba = Vec::with_capacity((W * H * 4) as usize);
    let cx = W / 2;
    let cy = H / 2;
    // Integer distance-squared thresholds (avoid float casts): disc
    // radius 7, ring outer radius 9.
    let disc_r2: u32 = 7 * 7;
    let ring_r2: u32 = 9 * 9;
    for y in 0..H {
        for x in 0..W {
            let dx = x.abs_diff(cx);
            let dy = y.abs_diff(cy);
            let dist_sq = dx * dx + dy * dy;
            let pixel = if dist_sq < disc_r2 {
                disc
            } else if dist_sq < ring_r2 {
                ring
            } else {
                plate
            };
            rgba.extend_from_slice(&pixel);
        }
    }

    Icon::from_rgba(rgba, W, H).map_err(|e| eyre::eyre!("encoding tray icon: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tray_icon::menu::MenuId;

    fn event(id: &str) -> MenuEvent {
        MenuEvent {
            id: MenuId::new(id),
        }
    }

    #[test]
    fn capture_region_maps_to_request() {
        let Some(TrayAction::Request(Request::CaptureRegion { monitor, delay_ms })) =
            action_for(&event(ID_CAPTURE_REGION))
        else {
            panic!("expected CaptureRegion");
        };
        assert!(monitor.is_none(), "tray region uses the default monitor");
        assert_eq!(delay_ms, 0);
    }

    #[test]
    fn capture_fullscreen_maps_to_request() {
        assert!(matches!(
            action_for(&event(ID_CAPTURE_FULLSCREEN)),
            Some(TrayAction::Request(Request::CaptureFullscreen {
                monitor: None
            }))
        ));
    }

    #[test]
    fn open_config_maps_directly() {
        assert_eq!(
            action_for(&event(ID_OPEN_CONFIG)),
            Some(TrayAction::OpenConfig)
        );
    }

    #[test]
    fn quit_maps_to_request() {
        assert!(matches!(
            action_for(&event(ID_QUIT)),
            Some(TrayAction::Request(Request::Quit))
        ));
    }

    #[test]
    fn unknown_id_is_ignored() {
        assert!(action_for(&event("something-else")).is_none());
    }
}
