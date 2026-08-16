//! Display-server detection for interaction-mode resolution.
//!
//! The Phase 5 overlay's fullscreen borderless window cannot
//! reliably be raised above other windows or grab input on all
//! Wayland compositors (spec §stance 2), so a Wayland session
//! demotes a config-resolved `overlay` to the editor window
//! (`cli::resolve_interaction_mode`). Detection is environment-based
//! — the same signals winit keys off — split pure (testable truth
//! table) from the real-environment read.

/// Pure decision: do these environment values indicate a Wayland
/// session?
///
/// `WAYLAND_DISPLAY` non-empty (a compositor socket exists) or
/// `XDG_SESSION_TYPE = "wayland"` (case-insensitive) counts as
/// Wayland. When both are present `WAYLAND_DISPLAY` wins: with a
/// socket available, winit prefers Wayland even inside an
/// `XDG_SESSION_TYPE = "x11"` `XWayland` setup.
pub(crate) fn wayland_from_env(
    wayland_display: Option<&str>,
    xdg_session_type: Option<&str>,
) -> bool {
    wayland_display.is_some_and(|v| !v.is_empty())
        || xdg_session_type.is_some_and(|v| v.eq_ignore_ascii_case("wayland"))
}

/// [`wayland_from_env`] over an environment lookup (by variable
/// name), so tests can exercise the real read path — including the
/// variable names — without touching the process environment.
///
/// Constant `false` on macOS / Windows: there is no Wayland there,
/// so the overlay is never demoted (the manual `TEST_MATRIX` pass
/// covers those platforms; CI is Linux-only).
pub(crate) fn session_is_wayland_with(lookup: impl Fn(&str) -> Option<String>) -> bool {
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        return false;
    }
    wayland_from_env(
        lookup("WAYLAND_DISPLAY").as_deref(),
        lookup("XDG_SESSION_TYPE").as_deref(),
    )
}

/// Detect a Wayland session in the current process environment.
/// Daemon-spawned workers inherit the session env, so the same
/// resolution code path detects the same session.
pub(crate) fn session_is_wayland() -> bool {
    session_is_wayland_with(|name| std::env::var(name).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── wayland_from_env truth table ─────────────────────────────

    #[test]
    fn wayland_display_socket_means_wayland() {
        assert!(wayland_from_env(Some("wayland-0"), None));
    }

    #[test]
    fn xdg_session_type_wayland_means_wayland() {
        assert!(wayland_from_env(None, Some("wayland")));
    }

    #[test]
    fn xdg_session_type_match_is_case_insensitive() {
        assert!(wayland_from_env(None, Some("Wayland")));
    }

    #[test]
    fn no_signals_means_not_wayland() {
        assert!(!wayland_from_env(None, None));
    }

    #[test]
    fn x11_session_type_means_not_wayland() {
        assert!(!wayland_from_env(None, Some("x11")));
    }

    #[test]
    fn empty_wayland_display_is_not_a_socket() {
        assert!(!wayland_from_env(Some(""), None));
    }

    #[test]
    fn wayland_display_beats_x11_session_type() {
        assert!(wayland_from_env(Some("wayland-1"), Some("x11")));
    }

    // ── session_is_wayland_with: the real read path ──────────────

    fn lookup(vars: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> + 'static {
        let vars = vars.to_vec();
        move |name| {
            vars.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn session_detection_reads_wayland_display() {
        assert!(session_is_wayland_with(lookup(&[(
            "WAYLAND_DISPLAY",
            "wayland-0"
        )])));
    }

    #[test]
    fn session_detection_reads_xdg_session_type() {
        assert!(session_is_wayland_with(lookup(&[(
            "XDG_SESSION_TYPE",
            "wayland"
        )])));
    }

    #[test]
    fn session_detection_without_wayland_vars_is_false() {
        assert!(!session_is_wayland_with(lookup(&[("DISPLAY", ":0")])));
        assert!(!session_is_wayland_with(lookup(&[])));
    }
}
