//! Global hotkey registration from `[hotkeys]` config.
//!
//! X11/macOS/Windows only — `global-hotkey` has no Wayland backend, so
//! on a Wayland session the daemon never builds this registry at all:
//! the xdg-desktop-portal session in [`crate::hotkeys_portal`] owns
//! hotkeys there, and this module's compositor-keybind notice below
//! only fires when the manager fails on a non-Wayland session
//! (e.g. a headless X11 box). Chord strings use `global-hotkey`'s
//! accelerator format (`"Ctrl+Shift+Print"`), parsed via
//! [`global_hotkey::hotkey::HotKey`]'s `FromStr`.
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
/// everything). Remembers the chords it registered so config hot-reload
/// (Task 010) can skip a costly re-registration when `[hotkeys]` is
/// unchanged.
pub(crate) struct HotkeyRegistry {
    // Held for its `Drop` side effect (unregistering the hotkeys on
    // drop); never read by name. `refresh` swaps it to re-register.
    #[allow(dead_code)]
    manager: Option<GlobalHotKeyManager>,
    by_id: HashMap<u32, Request>,
    region: String,
    fullscreen: String,
}

/// Pure check: has the `[hotkeys]` section changed relative to the
/// currently-registered chords? (Separate from [`HotkeyRegistry`] so it
/// is unit-testable without a display / `global-hotkey`.)
fn hotkeys_changed(config: &HotkeyConfig, region: &str, fullscreen: &str) -> bool {
    config.region != region || config.fullscreen != fullscreen
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
            manager: Some(manager),
            by_id,
            region: config.region.clone(),
            fullscreen: config.fullscreen.clone(),
        }
    }

    /// An empty registry: no manager, no lookups. The Wayland daemon
    /// path starts here — the portal session (`hotkeys_portal`) owns
    /// hotkeys instead, and `global-hotkey` X11 grabs are skipped
    /// entirely (under `XWayland` they can register-but-never-fire).
    pub(crate) fn empty() -> Self {
        Self {
            manager: None,
            by_id: HashMap::new(),
            region: String::new(),
            fullscreen: String::new(),
        }
    }

    /// Hot-reload: if `[hotkeys]` changed, drop the old manager (which
    /// unregisters its hotkeys) and register the new set. Returns `true`
    /// if a re-registration happened. A manager-creation failure during
    /// reload clears the chords so the next change retries; the daemon
    /// keeps running.
    pub(crate) fn refresh(&mut self, config: &HotkeyConfig) -> bool {
        if !hotkeys_changed(config, &self.region, &self.fullscreen) {
            return false;
        }
        // Drop the old manager — global-hotkey unregisters its hotkeys
        // on drop.
        self.manager = None;
        self.by_id.clear();

        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "global-hotkey manager unavailable after config change; \
                     hotkeys disabled until the next config change"
                );
                // Force a rebuild attempt next time.
                self.region.clear();
                self.fullscreen.clear();
                return true;
            }
        };
        Self::register(
            &manager,
            &mut self.by_id,
            &config.region,
            Request::CaptureRegion {
                monitor: None,
                delay_ms: 0,
            },
        );
        Self::register(
            &manager,
            &mut self.by_id,
            &config.fullscreen,
            Request::CaptureFullscreen { monitor: None },
        );
        self.manager = Some(manager);
        self.region.clone_from(&config.region);
        self.fullscreen.clone_from(&config.fullscreen);
        tracing::info!("re-registered global hotkeys after a config change");
        true
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
            manager: None,
            by_id,
            region: String::new(),
            fullscreen: String::new(),
        };
        assert!(matches!(
            reg.lookup(42),
            Some(Request::CaptureFullscreen { monitor: None })
        ));
        assert!(reg.lookup(7).is_none());
    }

    #[test]
    fn hotkeys_changed_detects_a_real_change() {
        let mut cfg = flintshot_core::HotkeyConfig::default();
        cfg.region = "Ctrl+Shift+A".into();
        cfg.fullscreen = "Ctrl+Alt+B".into();
        assert!(hotkeys_changed(&cfg, "Ctrl+Shift+Old", "Ctrl+Alt+B"));
        assert!(hotkeys_changed(&cfg, "Ctrl+Shift+A", "Ctrl+Alt+Old"));
        assert!(hotkeys_changed(&cfg, "", ""));
    }

    #[test]
    fn hotkeys_unchanged_is_detected() {
        let mut cfg = flintshot_core::HotkeyConfig::default();
        cfg.region = "Ctrl+Shift+PrintScreen".into();
        cfg.fullscreen = "Ctrl+Alt+PrintScreen".into();
        assert!(!hotkeys_changed(
            &cfg,
            "Ctrl+Shift+PrintScreen",
            "Ctrl+Alt+PrintScreen"
        ));
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
