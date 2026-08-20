//! Wayland global hotkeys via the xdg-desktop-portal `GlobalShortcuts`
//! interface (Phase 6).
//!
//! `global-hotkey` has no Wayland backend, and under `XWayland` its X11
//! grabs can register-but-never-fire — so on a Wayland session the
//! daemon is *portal-first*: it skips [`crate::hotkeys::HotkeyRegistry`]
//! entirely and runs the portal session owned here on the tokio side
//! (see ADR-0018's bridge; activations dispatch through the same
//! `worker::dispatch` table the IPC server uses on that thread).
//!
//! Binding is user-initiated (`flintshot hotkeys bind`, Task 003).
//! Startup is a *silent attach*: `list_shortcuts` reports whatever the
//! portal already has bound under our ids — no dialog at daemon start.

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, ListShortcutsOptions, Shortcut};
use ashpd::desktop::CreateSessionOptions;
use futures_util::StreamExt;

use crate::ipc_server::SubscriberBus;
use crate::worker::DispatchState;
use flintshot_core::ipc::Request;

/// Stable portal shortcut id for region capture. The same string is
/// used as the `BindShortcuts` id (Task 003), so re-binds keep
/// activating the same mapping.
pub(crate) const REGION_ID: &str = "region";

/// Stable portal shortcut id for fullscreen capture.
pub(crate) const FULLSCREEN_ID: &str = "fullscreen";

/// Map a portal `Activated::shortcut_id` to the [`Request`] it should
/// dispatch. Mirrors the chord → request mapping `HotkeyRegistry` uses
/// on X11. `None` for unknown ids (logged + dropped by the caller).
pub(crate) fn request_for_shortcut_id(id: &str) -> Option<Request> {
    match id {
        REGION_ID => Some(Request::CaptureRegion {
            monitor: None,
            delay_ms: 0,
        }),
        FULLSCREEN_ID => Some(Request::CaptureFullscreen { monitor: None }),
        _ => None,
    }
}

/// Which portal startup step failed. Each step fails for distinct
/// causes (see [`classify_failure`]), so classification needs to know
/// where the error came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PortalStage {
    /// Connecting to the session bus / building the portal proxy.
    Connect,
    /// `CreateSession` — a session bus exists, the portal answered.
    Session,
    /// `ListShortcuts` (silent attach) or listening for `Activated`.
    Attach,
}

/// Why the portal path is unavailable, in order of how far startup got.
/// Every variant is non-fatal: the daemon keeps serving tray + IPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FallbackKind {
    /// No D-Bus session bus reachable at all (headless box, or the
    /// compositor session is gone).
    NoSessionBus,
    /// The portal frontend is missing, or it does not implement the
    /// `GlobalShortcuts` interface (GNOME, most wlroots compositors).
    PortalMissing,
    /// The interface exists but is older than what we use.
    InterfaceUnsupported,
    /// Anything else — reported with the raw error.
    Other,
}

impl FallbackKind {
    /// The user-facing notice for this fallback. Every variant names
    /// the compositor-keybind workaround so the log line is actionable
    /// on its own.
    pub(crate) fn notice(self) -> &'static str {
        match self {
            Self::NoSessionBus => {
                "no D-Bus session bus reachable; Wayland global hotkeys unavailable. \
                 Workaround: bind a compositor keybinding to `flintshot region` / \
                 `flintshot fullscreen` (tray + IPC still work)"
            }
            Self::PortalMissing => {
                "no xdg-desktop-portal frontend with the GlobalShortcuts interface \
                 (GNOME and most wlroots compositors lack it; COSMIC and KDE ship it). \
                 Workaround: bind a compositor keybinding to `flintshot region` / \
                 `flintshot fullscreen` (tray + IPC still work)"
            }
            Self::InterfaceUnsupported => {
                "xdg-desktop-portal GlobalShortcuts interface is too old for flintshot; \
                 update xdg-desktop-portal (>= 1.16). Workaround meanwhile: bind a \
                 compositor keybinding to `flintshot region` / `flintshot fullscreen`"
            }
            Self::Other => {
                "portal GlobalShortcuts startup failed for an unknown reason. \
                 Workaround: bind a compositor keybinding to `flintshot region` / \
                 `flintshot fullscreen` (tray + IPC still work)"
            }
        }
    }
}

/// Classify a portal startup failure from the failing [`PortalStage`]
/// and the error's `Display` string. String-based on purpose: the
/// structured causes live in zbus/ashpd error types we do not depend
/// on, and the matched fragments are stable spec'd D-Bus / ashpd
/// messages, not incidental formatting.
pub(crate) fn classify_failure(stage: PortalStage, error: &str) -> FallbackKind {
    // ashpd::Error::PortalNotFound: frontend missing or interface not
    // implemented by the installed frontend.
    if error.contains("A portal frontend implementing") {
        return FallbackKind::PortalMissing;
    }
    // Spec'd D-Bus ServiceUnknown message: the portal service is not
    // installed / not owning org.freedesktop.portal.Desktop.
    if error.contains("was not provided by any .service files") {
        return FallbackKind::PortalMissing;
    }
    // ashpd::Error::RequiresVersion.
    if error.contains("This interface requires version") {
        return FallbackKind::InterfaceUnsupported;
    }
    match stage {
        // Connection/address/handshake errors surface at proxy
        // construction — no session bus.
        PortalStage::Connect => FallbackKind::NoSessionBus,
        PortalStage::Session | PortalStage::Attach => FallbackKind::Other,
    }
}

/// Run the portal `GlobalShortcuts` session for the daemon's lifetime:
/// connect, create a session, silently attach to whatever shortcuts
/// are already bound under our ids, then dispatch every `Activated`
/// through the shared worker table (same path as IPC requests on this
/// thread). `Deactivated` is deliberately not consumed — dispatching
/// on `Activated` only mirrors the Pressed-only rule in
/// `daemon::drain_hotkeys` and avoids double-firing.
///
/// Any startup failure is classified + logged and the task returns:
/// non-fatal by design, the daemon keeps serving tray + IPC. When the
/// runtime drops (Ctrl-C / IPC `Quit`), this future is dropped, the
/// zbus connection goes away, and the portal closes the session with
/// it — no explicit close needed.
pub(crate) async fn run(bus: SubscriberBus, state: DispatchState) {
    // GOTCHA (app-id; flagged for ADR-0029 in Task 005): the portal
    // resolves the caller's app-id from the bus connection's sender
    // credentials and matches it against installed .desktop files. Our
    // daemon is windowless and runs as the `flintshot` binary — if the
    // bind dialog (Task 003) shows no/wrong app identity, install a
    // desktop file whose basename matches the binary
    // (`flintshot.desktop`). Worst case is cosmetic.
    let shortcuts = match GlobalShortcuts::new().await {
        Ok(s) => s,
        Err(e) => return log_fallback(PortalStage::Connect, &e),
    };
    let session = match shortcuts
        .create_session(CreateSessionOptions::default())
        .await
    {
        Ok(s) => s,
        Err(e) => return log_fallback(PortalStage::Session, &e),
    };

    // Silent attach: ListShortcuts never shows a dialog (only
    // BindShortcuts does), so this reports the current bindings without
    // touching the user. (The portal Response signal is consumed inside
    // the call; `response()` just takes the cached result.)
    let listed = match shortcuts
        .list_shortcuts(&session, ListShortcutsOptions::default())
        .await
    {
        Ok(request) => match request.response() {
            Ok(listed) => listed,
            Err(e) => return log_fallback(PortalStage::Attach, &e),
        },
        Err(e) => return log_fallback(PortalStage::Attach, &e),
    };
    log_bound_shortcuts(listed.shortcuts());

    let mut activated = match shortcuts.receive_activated().await {
        Ok(stream) => stream,
        Err(e) => return log_fallback(PortalStage::Attach, &e),
    };
    while let Some(event) = activated.next().await {
        let id = event.shortcut_id();
        match request_for_shortcut_id(id) {
            Some(request) => {
                let outcome = crate::worker::dispatch(
                    &request,
                    &crate::worker::CommandSpawner,
                    &state,
                    &mut |ev| bus.broadcast(ev),
                );
                if let Err(e) = outcome {
                    tracing::warn!(error = %e, shortcut_id = %id, "portal hotkey dispatch failed");
                }
            }
            None => {
                tracing::debug!(shortcut_id = %id, "activation for unknown shortcut id; dropping");
            }
        }
    }
}

/// Log the bound-shortcut set from the silent attach: only our ids
/// matter; anything else bound in the session is left alone.
fn log_bound_shortcuts(shortcuts: &[Shortcut]) {
    let bound: Vec<String> = shortcuts
        .iter()
        .filter(|s| request_for_shortcut_id(s.id()).is_some())
        .map(|s| format!("{}={}", s.id(), s.trigger_description()))
        .collect();
    if bound.is_empty() {
        tracing::info!("portal session attached; nothing bound yet — run `flintshot hotkeys bind`");
    } else {
        tracing::info!(
            bound = %bound.join(", "),
            "portal session attached; bound shortcuts"
        );
    }
}

/// Log a classified startup failure. `NoSessionBus` is `info` (it is the
/// expected state on a headless box); everything else is `warn`.
fn log_fallback(stage: PortalStage, error: &ashpd::Error) {
    match classify_failure(stage, &error.to_string()) {
        FallbackKind::NoSessionBus => {
            tracing::info!(error = %error, "{}", FallbackKind::NoSessionBus.notice());
        }
        kind @ (FallbackKind::PortalMissing
        | FallbackKind::InterfaceUnsupported
        | FallbackKind::Other) => {
            tracing::warn!(error = %error, "{}", kind.notice());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── request_for_shortcut_id ──────────────────────────────────

    #[test]
    fn region_id_maps_to_region_capture() {
        assert!(matches!(
            request_for_shortcut_id(REGION_ID),
            Some(Request::CaptureRegion {
                monitor: None,
                delay_ms: 0
            })
        ));
    }

    #[test]
    fn fullscreen_id_maps_to_fullscreen_capture() {
        assert!(matches!(
            request_for_shortcut_id(FULLSCREEN_ID),
            Some(Request::CaptureFullscreen { monitor: None })
        ));
    }

    #[test]
    fn unknown_ids_map_to_none() {
        assert!(request_for_shortcut_id("nope").is_none());
        assert!(request_for_shortcut_id("").is_none());
        assert!(request_for_shortcut_id("Region").is_none());
    }

    // ── classify_failure ─────────────────────────────────────────

    #[test]
    fn portal_not_found_string_means_portal_missing() {
        // ashpd::Error::PortalNotFound Display.
        let err = "A portal frontend implementing \
                   `org.freedesktop.portal.GlobalShortcuts` was not found";
        assert_eq!(
            classify_failure(PortalStage::Connect, err),
            FallbackKind::PortalMissing
        );
    }

    #[test]
    fn service_unknown_string_means_portal_missing() {
        // zbus fdo ServiceUnknown Display (spec'd message) — portal
        // frontend not installed / not owning its bus name.
        let err = "Portal request failed: ZBus Error: The name \
                   org.freedesktop.portal.Desktop was not provided by any .service files";
        assert_eq!(
            classify_failure(PortalStage::Session, err),
            FallbackKind::PortalMissing
        );
    }

    #[test]
    fn version_gated_string_means_interface_unsupported() {
        // ashpd::Error::RequiresVersion Display.
        let err = "This interface requires version 3, but 1 is available";
        assert_eq!(
            classify_failure(PortalStage::Attach, err),
            FallbackKind::InterfaceUnsupported
        );
    }

    #[test]
    fn connect_stage_failure_means_no_session_bus() {
        // Any zbus connection failure surfacing at proxy construction
        // is a session-bus problem (address, handshake, io).
        for err in [
            "ZBus Error: Could not determine the address of the session bus",
            "ZBus Error: Handshake failed",
        ] {
            assert_eq!(
                classify_failure(PortalStage::Connect, err),
                FallbackKind::NoSessionBus,
                "expected NoSessionBus for: {err}"
            );
        }
    }

    #[test]
    fn later_stage_unknown_failure_is_other() {
        assert_eq!(
            classify_failure(PortalStage::Session, "something odd"),
            FallbackKind::Other
        );
    }

    // ── notices ──────────────────────────────────────────────────

    #[test]
    fn every_notice_names_the_compositor_keybind_workaround() {
        for kind in [
            FallbackKind::NoSessionBus,
            FallbackKind::PortalMissing,
            FallbackKind::InterfaceUnsupported,
            FallbackKind::Other,
        ] {
            let notice = kind.notice();
            assert!(!notice.is_empty(), "{kind:?} has no notice");
            assert!(
                notice.contains("compositor"),
                "{kind:?} notice must name the compositor-keybind workaround: {notice}"
            );
            assert!(
                notice.contains("flintshot region"),
                "{kind:?} notice must point at `flintshot region`: {notice}"
            );
        }
    }
}
