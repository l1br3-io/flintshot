//! Global hotkey registration from `[hotkeys]` config.
//!
//! X11/macOS/Windows only — `global-hotkey` has no Wayland backend, so
//! on Wayland the daemon logs the compositor-keybind workaround and the
//! `[hotkeys]` section is simply unused. Chord strings use
//! `global-hotkey`'s accelerator format (`"Ctrl+Shift+Print"`), parsed
//! via [`global_hotkey::hotkey::HotKey`]'s `FromStr`.
//!
//! Fires are dispatched by the daemon's main loop (same worker path as
//! tray clicks and IPC requests) — this module only owns registration +
//! the id → request lookup.

use std::collections::HashMap;

use flintshot_core::ipc::Request;
use flintshot_core::HotkeyConfig;
use global_hotkey::hotkey::HotKey;
use global_hotkey::GlobalHotKeyManager;

/// Registered global hotkeys + the id → request map. Holds the manager
/// alive for the daemon's lifetime (dropping the manager unregisters
/// everything).
pub(crate) struct HotkeyRegistry {
    // Underscore-prefixed: the field is unused by name, but keeping the
    // manager alive is the whole point — it must not be dropped early.
    _manager: Option<GlobalHotKeyManager>,
    by_id: HashMap<u32, Request>,
}

impl HotkeyRegistry {
    /// Build from config: create the manager, parse + register each
    /// chord, recording id → request. Manager-creation failure (e.g.
    /// Wayland, or a headless box) yields an empty registry plus the
    /// compositor-keybind notice; the daemon keeps running (tray + IPC
    /// still work).
    pub(crate) fn from_config(config: &HotkeyConfig) -> Self {
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "global hotkeys unavailable on this platform (likely Wayland); \
                     bind a compositor keybinding to `flintshot region` / \
                     `flintshot fullscreen` instead"
                );
                return Self::empty();
            }
        };
        let mut by_id = HashMap::new();
        Self::register(
            &manager,
            &mut by_id,
            &config.region,
            Request::CaptureRegion {
                monitor: None,
                delay_ms: 0,
            },
        );
        Self::register(
            &manager,
            &mut by_id,
            &config.fullscreen,
            Request::CaptureFullscreen { monitor: None },
        );
        Self {
            _manager: Some(manager),
            by_id,
        }
    }

    fn empty() -> Self {
        Self {
            _manager: None,
            by_id: HashMap::new(),
        }
    }

    /// Parse + register one chord, recording its id → `request`.
    /// A bad chord or an already-bound/unsupported combo is logged and
    /// skipped (non-fatal).
    fn register(
        manager: &GlobalHotKeyManager,
        by_id: &mut HashMap<u32, Request>,
        chord: &str,
        request: Request,
    ) {
        match chord.parse::<HotKey>() {
            Ok(hotkey) => {
                let id = hotkey.id();
                match manager.register(hotkey) {
                    Ok(()) => {
                        tracing::info!(chord = %chord, "registered global hotkey");
                        by_id.insert(id, request);
                    }
                    Err(e) => {
                        tracing::warn!(
                            chord = %chord,
                            error = %e,
                            "could not register hotkey; skipping"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(chord = %chord, error = %e, "invalid hotkey chord; skipping");
            }
        }
    }

    /// Look up the request a fired hotkey id maps to (`None` for an
    /// unregistered id, or when hotkeys are unavailable on this
    /// platform).
    pub(crate) fn lookup(&self, id: u32) -> Option<&Request> {
        self.by_id.get(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_registry_looks_up_nothing() {
        let reg = HotkeyRegistry::empty();
        assert!(reg.lookup(0).is_none());
        assert!(reg.lookup(u32::MAX).is_none());
    }

    #[test]
    fn lookup_returns_the_registered_request() {
        let mut by_id = HashMap::new();
        by_id.insert(42, Request::CaptureFullscreen { monitor: None });
        let reg = HotkeyRegistry {
            _manager: None,
            by_id,
        };
        assert!(matches!(
            reg.lookup(42),
            Some(Request::CaptureFullscreen { monitor: None })
        ));
        assert!(reg.lookup(7).is_none());
    }

    #[test]
    fn default_chords_parse_validly() {
        // The default config chords must be accepted by global-hotkey's
        // parser (pure — no display/registration needed).
        use global_hotkey::hotkey::HotKey;
        use std::str::FromStr;
        assert!(HotKey::from_str("Ctrl+Shift+PrintScreen").is_ok());
        assert!(HotKey::from_str("Ctrl+Alt+PrintScreen").is_ok());
    }

    #[test]
    fn invalid_chord_fails_to_parse() {
        use global_hotkey::hotkey::HotKey;
        assert!("Ctrl+NotARealKey".parse::<HotKey>().is_err());
        assert!("".parse::<HotKey>().is_err());
    }
}
