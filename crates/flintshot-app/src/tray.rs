//! System tray menu (`tray-icon` + `muda`), behind the `tray` feature.
//!
//! The menu offers one **Capture** item (the single Capture action,
//! native-capture-ux), *Open app window*, *Open config*, and *Quit*
//! — plus *Bind shortcuts…* on Wayland sessions (the portal hotkey
//! path, Phase 6). Since ADR-0042 this icon is
//! built by the **tray helper** process (`flintshot tray`,
//! [`crate::tray_helper`]), spawned and supervised by the daemon:
//! the helper's process-exit is the only causal SNI unregistration,
//! which is what closes the panel's open popup before a grab. Clicks
//! arrive on [`tray_icon::menu::MenuEvent`]'s
//! global receiver (muda pumps the OS internally and hands events to a
//! static crossbeam channel); the helper's loop drains it via
//! [`action_for`] and forwards each click to the daemon over IPC, so
//! a tray-initiated capture reaches IPC subscribers and vice versa.

use color_eyre::eyre;
use flintshot_core::ipc::Request;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

// Stable string ids — `MenuEvent::id` is matched against these. Kept
// short and namespaced so they don't collide with any muda defaults.
const ID_OPEN_APP: &str = "flintshot.app";
const ID_CAPTURE: &str = "flintshot.capture";
const ID_BIND_SHORTCUTS: &str = "flintshot.bind-shortcuts";
const ID_OPEN_CONFIG: &str = "flintshot.open-config";
const ID_QUIT: &str = "flintshot.quit";
/// Stable `TrayIcon` id. Without it, `TrayIconBuilder::new()` mints a
/// fresh `{pid}-{counter}` id per build, which becomes the SNI object
/// path — so any second registration (a stray re-`build` in the same
/// process) would surface as a *second* panel item instead of
/// replacing this one (ADR-0039).
const TRAY_ID: &str = "flintshot";

/// What a tray menu click resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TrayAction {
    /// A request the daemon dispatches via the worker table
    /// (capture spawns a worker; quit shuts down).
    Request(Request),
    /// Open the portal bind dialog (the Wayland portal path; the menu
    /// item only exists on a Wayland session — see [`build`]).
    BindShortcuts,
    /// Open `config.toml` in the user's `$EDITOR`.
    OpenConfig,
}

/// Map a menu event to the action it should trigger, or `None` for an
/// unknown id (e.g. events not from our menu).
#[must_use]
pub(crate) fn action_for(event: &MenuEvent) -> Option<TrayAction> {
    match event.id.as_ref() {
        ID_OPEN_APP => Some(TrayAction::Request(Request::OpenApp)),
        ID_CAPTURE => Some(TrayAction::Request(Request::Capture { delay_ms: 0 })),
        ID_BIND_SHORTCUTS => Some(TrayAction::BindShortcuts),
        ID_OPEN_CONFIG => Some(TrayAction::OpenConfig),
        ID_QUIT => Some(TrayAction::Request(Request::Quit)),
        _ => None,
    }
}

/// Build the tray icon with its context menu. `wayland` gates the
/// *Bind shortcuts…* item: it drives the xdg-desktop-portal bind
/// dialog, which only exists on Wayland portal sessions — an X11 tray
/// must not grow a dead item (there hotkeys re-register from
/// `[hotkeys]` directly). The returned [`TrayIcon`] must be held
/// alive for the daemon's **whole** lifetime — exactly one
/// registration per process (a dropped handle never unregisters the
/// SNI item, and a second build would duplicate it, ADR-0039);
/// visibility is driven by `set_visible` flips. muda retains the
/// menu items itself (they're reference-counted), so they can drop
/// here.
pub(crate) fn build(wayland: bool) -> eyre::Result<TrayIcon> {
    // libappindicator (tray-icon's Linux backend) touches GTK widgets
    // for the menu, so GTK must be initialized first — on the main
    // thread, which is where the daemon calls us. macOS/Windows use
    // native backends and skip this.
    #[cfg(target_os = "linux")]
    {
        gtk::init().map_err(|e| eyre::eyre!("gtk::init failed: {e}"))?;
    }

    let menu = Menu::new();
    let open_app = MenuItem::with_id(ID_OPEN_APP, "Open app window", true, None);
    let capture = MenuItem::with_id(ID_CAPTURE, "Capture", true, None);
    let bind_shortcuts = MenuItem::with_id(ID_BIND_SHORTCUTS, "Bind shortcuts…", true, None);
    let open_config = MenuItem::with_id(ID_OPEN_CONFIG, "Open config", true, None);
    let quit = MenuItem::with_id(ID_QUIT, "Quit", true, None);

    let mut appending = menu
        .append(&open_app)
        .and_then(|()| menu.append(&capture))
        .and_then(|()| menu.append(&PredefinedMenuItem::separator()));
    if wayland {
        appending = appending
            .and_then(|()| menu.append(&bind_shortcuts))
            .and_then(|()| menu.append(&PredefinedMenuItem::separator()));
    }
    if let Err(e) = appending
        .and_then(|()| menu.append(&open_config))
        .and_then(|()| menu.append(&PredefinedMenuItem::separator()))
        .and_then(|()| menu.append(&quit))
    {
        return Err(eyre::eyre!("building tray menu: {e}"));
    }

    let icon = icon()?;
    TrayIconBuilder::new()
        .with_id(TRAY_ID)
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
    fn open_app_maps_to_request() {
        assert!(matches!(
            action_for(&event(ID_OPEN_APP)),
            Some(TrayAction::Request(Request::OpenApp))
        ));
    }

    #[test]
    fn capture_maps_to_capture_request() {
        let Some(TrayAction::Request(Request::Capture { delay_ms })) =
            action_for(&event(ID_CAPTURE))
        else {
            panic!("expected Capture");
        };
        assert_eq!(delay_ms, 0, "the tray's single Capture item");
    }

    #[test]
    fn open_config_maps_directly() {
        assert_eq!(
            action_for(&event(ID_OPEN_CONFIG)),
            Some(TrayAction::OpenConfig)
        );
    }

    #[test]
    fn bind_shortcuts_maps_directly() {
        assert_eq!(
            action_for(&event(ID_BIND_SHORTCUTS)),
            Some(TrayAction::BindShortcuts)
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

    /// Pinned (native-capture-ux Task 007): the old capture ids —
    /// *Capture region*, *Capture fullscreen*, and the per-monitor
    /// submenu prefix — must not resolve to anything; one **Capture**
    /// item (`flintshot.capture`) is the whole capture set.
    #[test]
    fn legacy_capture_ids_are_gone() {
        for legacy in [
            "flintshot.region",
            "flintshot.fullscreen",
            "flintshot.fullscreen-monitor.0",
            "flintshot.fullscreen-monitor.3",
            "flintshot.fullscreen-monitor.abc",
        ] {
            assert!(
                action_for(&event(legacy)).is_none(),
                "legacy id `{legacy}` must not resolve anymore"
            );
        }
    }
}
