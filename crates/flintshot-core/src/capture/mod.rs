//! Screen capture: the [`Capturer`] trait, backend registry, and
//! compatibility free functions.
//!
//! Two layers live here:
//!
//! - **Trait layer** — [`Capturer`] abstracts a capture provider
//!   (`xcap` today, the XDG Screenshot Portal on Linux next).
//!   [`select_backend`] resolves which provider a grab should use:
//!   config/CLI preference → session-kind auto-detection → `xcap` as
//!   the universal fallback.
//! - **Compatibility layer** — [`monitors`], [`capture_fullscreen`],
//!   and [`capture_region`] remain the stable free-function API
//!   (`flintshot-core` 0.x surface, consumed by `flintshot-app` and
//!   `flintshot-cosmic`). They delegate to [`XcapCapturer`]; new code
//!   goes through the trait.
//!
//! # Coordinate space (`HiDPI`)
//!
//! Captures are returned in **physical pixels** — the actual
//! framebuffer resolution, not the logical resolution the compositor
//! reports. This is what we want: annotations are stored and rendered
//! against the captured image, so a 1:1 pixel mapping keeps everything
//! crisp. The editor shell converts physical → logical pixels at the
//! egui event boundary (see `docs/ARCHITECTURE.md` §Cross-platform
//! concerns).
//!
//! # Errors
//!
//! Capture failures map to [`FlintshotError::Capture`] with the
//! underlying message preserved verbatim. On macOS, permission-denied
//! errors contain the substring `"permission"`; downstream code can
//! match on that to guide the user to System Settings → Privacy &
//! Security → Screen Recording.
//!
//! # Wayland
//!
//! The `xcap` path may trigger an `xdg-desktop-portal` `ScreenCast`
//! permission dialog on first capture per session. The portal backend
//! (`--backend portal`, auto-selected on Wayland where available)
//! avoids the `ScreenCast` session dance entirely — see
//! `docs/specs/native-capture-backends/` for the spike findings.

use image::RgbaImage;

use crate::error::{FlintshotError, Result};
use crate::geom::Rect;

#[cfg(target_os = "linux")]
mod portal;
mod xcap_backend;

#[cfg(target_os = "linux")]
pub use portal::PortalScreenshotCapturer;
pub use xcap_backend::XcapCapturer;

/// A discoverable monitor, for UI surfaces (e.g. the tray's per-monitor
/// submenu). `index` is the value to pass to [`capture_fullscreen`]'s
/// `Some(index)` argument; `name`/`is_primary` are for labelling.
///
/// `x`/`y`/`width`/`height` give the monitor's geometry in the virtual
/// desktop (physical pixels).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MonitorInfo {
    /// Index into the registered backend's enumeration order — the
    /// `Some(n)` argument for [`capture_fullscreen`].
    pub index: u32,
    /// Human-readable name (backend-provided), empty if none.
    pub name: String,
    /// Whether the backend flags this as the primary monitor.
    pub is_primary: bool,
    /// Monitor origin X in the virtual desktop (physical px).
    pub x: i32,
    /// Monitor origin Y in the virtual desktop (physical px).
    pub y: i32,
    /// Monitor width in physical px.
    pub width: u32,
    /// Monitor height in physical px.
    pub height: u32,
}

/// Options for a capture grab.
///
/// `include_cursor` is honored where the underlying provider allows
/// it. Known gaps (documented per backend): the portal protocol has no
/// cursor option at all, and `xcap` 0.9 exposes no toggle (platform
/// default: cursor included).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CaptureOptions {
    /// Whether the pointer cursor should be composited into the grab.
    pub include_cursor: bool,
}

impl Default for CaptureOptions {
    fn default() -> Self {
        Self {
            include_cursor: true,
        }
    }
}

/// A screen-capture provider behind the registry.
///
/// Implementations are synchronous: every current capture call site is
/// synchronous worker code, and D-Bus-backed implementations use
/// blocking facades internally.
///
/// # Monitor selection: index vs geometry
///
/// `monitor` is an index into the [`Capturer::monitors`] enumeration
/// (`None` = primary), **not** into any native service's own monitor
/// numbering. Compositor screenshot services disagree with each other
/// and with `xcap` about ordering and indexing — backends that talk to
/// such a service must resolve the requested monitor to its *geometry*
/// (position + size in the virtual desktop) and crop/select by rect,
/// never by passing an index through. The spike record for this effort
/// measured exactly this divergence.
pub trait Capturer: Send + Sync {
    /// Stable machine id (`"xcap"`, `"portal"`) — the key used by the
    /// `[capture] backend` config and `--backend` CLI flag.
    fn id(&self) -> &'static str;

    /// Human-readable name for the `flintshot backends` report.
    fn name(&self) -> &'static str;

    /// Probe availability without capturing (bus-name check,
    /// display-server check). `Ok(())` means the backend is usable
    /// right now; the error detail feeds the `backends` report and
    /// pin-failure messages.
    ///
    /// # Errors
    /// - [`FlintshotError::Capture`] with the probe's failure detail.
    fn available(&self) -> Result<()>;

    /// Enumerate monitors in virtual-desktop order (physical px).
    ///
    /// Deliberately always rides the `xcap` enumeration regardless of
    /// backend: monitor listing feeds tray/applet menus and must stay
    /// cheap (no bus round-trip per menu open), and spike B confirmed
    /// `xcap` enumeration is prompt-free on Wayland.
    ///
    /// # Errors
    /// - [`FlintshotError::Capture`] if enumeration fails.
    fn monitors(&self) -> Result<Vec<MonitorInfo>> {
        xcap_backend::enumerate_monitors()
    }

    /// Grab a whole monitor's framebuffer in physical pixels.
    ///
    /// `monitor = None` selects the primary monitor; `Some(n)` picks
    /// the n-th entry of [`Capturer::monitors`]. Backends normalize
    /// any native pixel format to RGBA.
    ///
    /// # Errors
    /// - [`FlintshotError::Capture`] if the grab fails, the monitor
    ///   index is out of range, or the provider refuses the call.
    fn capture_monitor(&self, monitor: Option<u32>, opts: &CaptureOptions) -> Result<RgbaImage>;

    /// Whether this backend can hand region selection to the OS's own
    /// UI (compositor selection overlay, system picker). Backends
    /// without such a mechanism keep the `false` default and callers
    /// fall back to their own selection flow (e.g. the editor
    /// overlay).
    fn supports_selection(&self) -> bool {
        false
    }

    /// Grab a user-selected region via the OS's own selection UI.
    ///
    /// Blocks for as long as the user interacts with the native UI —
    /// implementations must budget for human think-time, not machine
    /// latency. A dismissal (Escape / cancel) surfaces as
    /// [`FlintshotError::Capture`] whose `detail` contains the
    /// substring `"cancelled"` so callers can exit quietly.
    ///
    /// # Errors
    /// - [`FlintshotError::NotImplemented`] on backends that don't
    ///   support native selection (the trait default).
    /// - [`FlintshotError::Capture`] if the user dismissed the
    ///   selection, the provider failed, or no answer arrived within
    ///   the backend's interactive budget.
    fn capture_selection(&self, opts: &CaptureOptions) -> Result<RgbaImage> {
        let _ = opts;
        Err(FlintshotError::NotImplemented(
            "backend does not support native region selection",
        ))
    }
}

/// The kind of display session we're running under, from
/// `XDG_SESSION_TYPE`. Drives auto-detection precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SessionKind {
    /// `XDG_SESSION_TYPE=x11` — direct grabs; no native services.
    X11,
    /// `XDG_SESSION_TYPE=wayland` — portal-native capture preferred.
    Wayland,
    /// Everything else (unset, `tty`, headless, macOS/Windows).
    Other,
}

impl SessionKind {
    /// Read the session kind from the environment.
    #[must_use]
    pub fn detect() -> Self {
        Self::from_xdg(std::env::var("XDG_SESSION_TYPE").ok().as_deref())
    }

    /// Pure mapping from an `XDG_SESSION_TYPE` value; unit-testable
    /// without touching the process environment.
    #[must_use]
    pub fn from_xdg(value: Option<&str>) -> Self {
        match value {
            Some("wayland") => Self::Wayland,
            Some("x11") => Self::X11,
            _ => Self::Other,
        }
    }
}

/// Auto-detection precedence per session kind, most preferred first.
/// `xcap` is always terminal: if every earlier candidate is
/// unavailable, selection falls back to it rather than failing.
#[must_use]
pub fn candidate_ids(session: SessionKind) -> &'static [&'static str] {
    match session {
        SessionKind::X11 | SessionKind::Other => &["xcap"],
        SessionKind::Wayland => &["portal", "xcap"],
    }
}

/// Resolve which backend a grab should use.
///
/// Resolution order mirrors the upload backends' policy:
/// - explicit known id with a passing availability probe → pinned to
///   it;
/// - explicit known id whose probe fails → [`FlintshotError::Capture`]
///   carrying the probe detail (the caller asked for it explicitly,
///   so silently falling back would hide a misconfiguration);
/// - unknown id → `tracing::warn!` + auto (string-tolerant, same
///   policy as `interaction-mode`; the CLI layer hard-errors instead);
/// - `None` / `"auto"` → walk [`candidate_ids`] and pick the first
///   registered, available backend. **Auto never fails**: if every
///   probe fails, `xcap` is returned and any real failure surfaces at
///   grab time with `xcap`'s own error.
///
/// # Errors
/// - [`FlintshotError::Capture`] only in the pin-with-failed-probe
///   case described above.
pub fn select_backend(preference: Option<&str>) -> Result<Box<dyn Capturer>> {
    let session = SessionKind::detect();
    let registry = registered_backends();
    let refs: Vec<&dyn Capturer> = registry.iter().map(AsRef::as_ref).collect();
    let id = select_backend_with(session, &refs, preference)?;
    registry
        .into_iter()
        .find(|b| b.id() == id)
        .ok_or_else(|| FlintshotError::Capture {
            detail: format!("backend '{id}' selected but not registered"),
        })
}

/// Pure selection core behind [`select_backend`]: resolves over an
/// injected backend list so precedence is unit-testable headless.
/// Returns the winning backend id. See [`select_backend`] for the
/// resolution rules.
///
/// # Errors
/// - [`FlintshotError::Capture`] when `preference` names a registered
///   backend whose availability probe fails.
pub fn select_backend_with(
    session: SessionKind,
    backends: &[&dyn Capturer],
    preference: Option<&str>,
) -> Result<&'static str> {
    match preference {
        None | Some("auto") => Ok(auto_select(session, backends)),
        Some(pin) => {
            if let Some(backend) = find_by_id(backends, pin) {
                match backend.available() {
                    Ok(()) => Ok(backend.id()),
                    Err(e) => Err(FlintshotError::Capture {
                        detail: format!("backend '{pin}' unavailable: {}", probe_detail(&e)),
                    }),
                }
            } else {
                tracing::warn!(
                    backend = pin,
                    "unknown capture backend id; falling back to auto"
                );
                Ok(auto_select(session, backends))
            }
        }
    }
}

/// Walk the precedence list; `xcap` is the never-failing terminal.
fn auto_select(session: SessionKind, backends: &[&dyn Capturer]) -> &'static str {
    for id in candidate_ids(session) {
        if let Some(backend) = find_by_id(backends, id) {
            if backend.available().is_ok() {
                return backend.id();
            }
        }
    }
    "xcap"
}

/// First backend whose id matches, or `None`.
fn find_by_id<'a>(backends: &[&'a dyn Capturer], id: &str) -> Option<&'a dyn Capturer> {
    backends.iter().copied().find(|b| b.id() == id)
}

/// Flatten a probe error into its human-facing detail string.
fn probe_detail(e: &FlintshotError) -> String {
    match e {
        FlintshotError::Capture { detail } => detail.clone(),
        other => other.to_string(),
    }
}

/// Every backend compiled into this binary, in registration order
/// (which mirrors Wayland precedence; the actual walk follows
/// [`candidate_ids`]). Used by [`select_backend`] and, later, the
/// `flintshot backends` report.
#[must_use]
pub fn registered_backends() -> Vec<Box<dyn Capturer>> {
    #[cfg(target_os = "linux")]
    {
        vec![
            Box::new(portal::PortalScreenshotCapturer) as Box<dyn Capturer>,
            Box::new(XcapCapturer),
        ]
    }
    #[cfg(not(target_os = "linux"))]
    {
        vec![Box::new(XcapCapturer) as Box<dyn Capturer>]
    }
}

/// One backend's row in the [`BackendReport`] diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BackendReportEntry {
    /// Machine id — the config/CLI key.
    pub id: &'static str,
    /// Human-readable name.
    pub name: &'static str,
    /// Whether the availability probe passed.
    pub available: bool,
    /// Probe failure detail; `None` when available.
    pub detail: Option<String>,
}

impl BackendReportEntry {
    /// Build a row directly (printer-side tests; the live path goes
    /// through [`backend_report`]).
    #[must_use]
    pub fn new(
        id: &'static str,
        name: &'static str,
        available: bool,
        detail: Option<String>,
    ) -> Self {
        Self {
            id,
            name,
            available,
            detail,
        }
    }
}

/// The `flintshot backends` report: every registered backend's probe
/// result plus what `auto` would select right now. Diagnostics only —
/// building one never grabs pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BackendReport {
    /// Session kind the auto resolution ran under.
    pub session: SessionKind,
    /// One entry per registered backend, in registration order.
    pub entries: Vec<BackendReportEntry>,
    /// Backend id `auto` resolves to right now (never fails).
    pub auto_selects: &'static str,
}

impl BackendReport {
    /// Assemble a report directly (printer-side tests).
    #[must_use]
    pub fn new(
        session: SessionKind,
        entries: Vec<BackendReportEntry>,
        auto_selects: &'static str,
    ) -> Self {
        Self {
            session,
            entries,
            auto_selects,
        }
    }
}

/// Probe every registered backend and compute the auto selection —
/// the data behind `flintshot backends`.
#[must_use]
pub fn backend_report() -> BackendReport {
    let registry = registered_backends();
    let refs: Vec<&dyn Capturer> = registry.iter().map(AsRef::as_ref).collect();
    backend_report_with(SessionKind::detect(), &refs)
}

/// Pure report core over an injected backend list (headless-testable
/// with fakes); [`backend_report`] is the live wrapper.
#[must_use]
pub fn backend_report_with(session: SessionKind, backends: &[&dyn Capturer]) -> BackendReport {
    let entries = backends
        .iter()
        .map(|b| match b.available() {
            Ok(()) => BackendReportEntry::new(b.id(), b.name(), true, None),
            Err(e) => BackendReportEntry::new(b.id(), b.name(), false, Some(probe_detail(&e))),
        })
        .collect();
    BackendReport::new(session, entries, auto_select(session, backends))
}

/// Enumerate monitors in the order [`capture_fullscreen`] expects
/// (primary first, then by display index — whatever `xcap` returns).
///
/// Compatibility wrapper over [`XcapCapturer::monitors`]; kept so
/// `flintshot-app` / `flintshot-cosmic` call sites stay
/// source-compatible.
///
/// # Errors
/// - [`FlintshotError::Capture`] if `xcap`'s enumeration fails, or if
///   there are somehow more monitors than `u32` can index (impossible
///   in practice — the index round-trips through [`capture_fullscreen`]'s
///   `Some(n)` argument).
pub fn monitors() -> Result<Vec<MonitorInfo>> {
    xcap_backend::enumerate_monitors()
}

/// Capture a full monitor's framebuffer in physical pixels.
///
/// Compatibility wrapper over [`XcapCapturer::capture_monitor`] — the
/// pre-registry behavior, byte-identical. `monitor = None` selects the
/// primary monitor; `Some(n)` picks the n-th entry from
/// [`monitors`].
///
/// # Errors
///
/// - [`FlintshotError::Capture`] if monitor enumeration or the capture
///   call itself fails. The `detail` string preserves `xcap`'s message
///   verbatim, including the `"permission"` substring on macOS
///   permission denials.
/// - [`FlintshotError::Capture`] with a synthetic message if the
///   requested monitor index is out of range, or if no primary monitor
///   could be identified.
pub fn capture_fullscreen(monitor: Option<u32>) -> Result<RgbaImage> {
    XcapCapturer.capture_monitor(monitor, &CaptureOptions::default())
}

/// Capture a sub-rect of a monitor's framebuffer, in physical pixels.
///
/// The rect is validated **before** any capture provider is invoked,
/// so degenerate inputs fail fast without touching the display server.
/// Rects that extend beyond the captured image are silently clamped to
/// the image bounds (matches the convention in
/// `image::imageops::crop_imm`).
///
/// # Errors
///
/// - [`FlintshotError::InvalidRect`] if `rect` has zero area or
///   inverted corners (`max.x < min.x` or `max.y < min.y`).
/// - Propagates any error from [`capture_fullscreen`].
pub fn capture_region(rect: Rect, monitor: Option<u32>) -> Result<RgbaImage> {
    validate_rect(rect)?;
    let full = capture_fullscreen(monitor)?;
    Ok(crop_to_rect(&full, rect))
}

/// Reject degenerate rects without invoking any capture provider.
/// Headless-testable.
///
/// `==` is correct here rather than an epsilon comparison: the test is
/// structural (does the rect have zero extent?), not the result of
/// floating-point arithmetic. A rect built with `min.x == max.x` was
/// constructed that way intentionally.
#[allow(
    clippy::float_cmp,
    reason = "structural zero-extent check, not arithmetic equality"
)]
fn validate_rect(rect: Rect) -> Result<()> {
    if rect.max.x < rect.min.x || rect.max.y < rect.min.y {
        return Err(FlintshotError::InvalidRect {
            rect,
            reason: "negative",
        });
    }
    if rect.max.x == rect.min.x || rect.max.y == rect.min.y {
        return Err(FlintshotError::InvalidRect {
            rect,
            reason: "empty",
        });
    }
    Ok(())
}

/// Crop `src` to `rect`, clamping to image bounds. Callers must have
/// validated that `rect` is non-empty and non-negative first.
///
/// The `f32 → u32` casts are sign/truncation-safe by construction:
/// each value is clamped to `[0.0, image_dim]` before the cast, so
/// nothing truncates or loses sign at runtime.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "values clamped to image bounds before cast"
)]
fn crop_to_rect(src: &RgbaImage, rect: Rect) -> RgbaImage {
    let x = (rect.min.x.max(0.0) as u32).min(src.width());
    let y = (rect.min.y.max(0.0) as u32).min(src.height());
    let w = ((rect.max.x - rect.min.x).max(0.0) as u32).min(src.width().saturating_sub(x));
    let h = ((rect.max.y - rect.min.y).max(0.0) as u32).min(src.height().saturating_sub(y));
    image::imageops::crop_imm(src, x, y, w, h).to_image()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Programmable stand-in for a backend; selection logic never
    /// grabs pixels, so `monitors`/`capture_monitor` are unreachable
    /// stubs.
    struct FakeCapturer {
        id: &'static str,
        available_detail: Option<String>,
    }

    impl FakeCapturer {
        fn available(id: &'static str) -> Self {
            Self {
                id,
                available_detail: None,
            }
        }

        fn unavailable(id: &'static str, detail: &str) -> Self {
            Self {
                id,
                available_detail: Some(detail.to_string()),
            }
        }
    }

    impl Capturer for FakeCapturer {
        fn id(&self) -> &'static str {
            self.id
        }

        fn name(&self) -> &'static str {
            self.id
        }

        fn available(&self) -> Result<()> {
            match &self.available_detail {
                Some(detail) => Err(FlintshotError::Capture {
                    detail: detail.clone(),
                }),
                None => Ok(()),
            }
        }

        fn capture_monitor(
            &self,
            _monitor: Option<u32>,
            _opts: &CaptureOptions,
        ) -> Result<RgbaImage> {
            Err(FlintshotError::NotImplemented("fake never captures"))
        }
    }

    fn registry(fakes: &[FakeCapturer]) -> Vec<&dyn Capturer> {
        fakes.iter().map(|f| f as &dyn Capturer).collect()
    }

    #[test]
    fn session_kind_mapping() {
        assert_eq!(SessionKind::from_xdg(Some("wayland")), SessionKind::Wayland);
        assert_eq!(SessionKind::from_xdg(Some("x11")), SessionKind::X11);
        assert_eq!(SessionKind::from_xdg(None), SessionKind::Other);
        assert_eq!(SessionKind::from_xdg(Some("tty")), SessionKind::Other);
        assert_eq!(
            SessionKind::from_xdg(Some("WAYLAND")),
            SessionKind::Other,
            "mapping is case-sensitive by spec"
        );
    }

    #[test]
    fn precedence_x11_and_other_go_straight_to_xcap() {
        for session in [SessionKind::X11, SessionKind::Other] {
            let fakes = [
                FakeCapturer::available("portal"),
                FakeCapturer::available("xcap"),
            ];
            let picked =
                select_backend_with(session, &registry(&fakes), None).expect("auto never fails");
            assert_eq!(picked, "xcap", "X11/Other must skip native services");
        }
    }

    #[test]
    fn precedence_wayland_prefers_portal_then_falls_through() {
        let both = [
            FakeCapturer::available("portal"),
            FakeCapturer::available("xcap"),
        ];
        assert_eq!(
            select_backend_with(SessionKind::Wayland, &registry(&both), None).unwrap(),
            "portal"
        );

        let portal_down = [
            FakeCapturer::unavailable("portal", "no bus"),
            FakeCapturer::available("xcap"),
        ];
        assert_eq!(
            select_backend_with(SessionKind::Wayland, &registry(&portal_down), None).unwrap(),
            "xcap"
        );
    }

    #[test]
    fn auto_never_fails_when_all_probes_fail() {
        let all_down = [
            FakeCapturer::unavailable("portal", "portal missing"),
            FakeCapturer::unavailable("xcap", "no display"),
        ];
        assert_eq!(
            select_backend_with(SessionKind::Wayland, &registry(&all_down), None).unwrap(),
            "xcap",
            "terminal fallback even with a failing xcap probe"
        );
    }

    #[test]
    fn auto_skips_ids_missing_from_the_registry() {
        let xcap_only = [FakeCapturer::available("xcap")];
        assert_eq!(
            select_backend_with(SessionKind::Wayland, &registry(&xcap_only), None).unwrap(),
            "xcap",
            "unregistered portal id is skipped, not an error"
        );
    }

    #[test]
    fn pin_to_available_backend_wins_over_session_precedence() {
        let fakes = [
            FakeCapturer::available("portal"),
            FakeCapturer::available("xcap"),
        ];
        assert_eq!(
            select_backend_with(SessionKind::X11, &registry(&fakes), Some("portal")).unwrap(),
            "portal",
            "an explicit pin ignores the X11 precedence list"
        );
    }

    #[test]
    fn pin_to_failed_probe_is_a_hard_error_with_detail() {
        let fakes = [
            FakeCapturer::unavailable("portal", "org.freedesktop.portal.Desktop not owned"),
            FakeCapturer::available("xcap"),
        ];
        let err = select_backend_with(SessionKind::Wayland, &registry(&fakes), Some("portal"))
            .unwrap_err();
        match err {
            FlintshotError::Capture { detail } => {
                assert!(
                    detail.contains("org.freedesktop.portal.Desktop not owned"),
                    "probe detail must survive into the error, got: {detail}"
                );
                assert!(
                    detail.contains("'portal'"),
                    "error must name the pinned backend, got: {detail}"
                );
            }
            other => panic!("expected Capture, got {other:?}"),
        }
    }

    #[test]
    fn unknown_pin_warns_and_falls_back_to_auto() {
        let fakes = [
            FakeCapturer::available("portal"),
            FakeCapturer::available("xcap"),
        ];
        assert_eq!(
            select_backend_with(SessionKind::Wayland, &registry(&fakes), Some("bogus")).unwrap(),
            "portal",
            "unknown id resolves like auto (portal is up on Wayland)"
        );
        assert_eq!(
            select_backend_with(SessionKind::X11, &registry(&fakes), Some("bogus")).unwrap(),
            "xcap",
            "unknown id resolves like auto (X11 precedence)"
        );
    }

    #[test]
    fn explicit_auto_matches_none() {
        let fakes = [
            FakeCapturer::available("portal"),
            FakeCapturer::available("xcap"),
        ];
        let via_none = select_backend_with(SessionKind::Wayland, &registry(&fakes), None);
        let via_auto = select_backend_with(SessionKind::Wayland, &registry(&fakes), Some("auto"));
        assert_eq!(via_none.unwrap(), via_auto.unwrap());
    }

    #[test]
    fn registered_backends_always_contain_xcap() {
        let ids: Vec<&str> = registered_backends().iter().map(|b| b.id()).collect();
        assert!(
            ids.contains(&"xcap"),
            "xcap is the universal fallback: {ids:?}"
        );
    }

    #[test]
    fn report_lists_entries_with_probe_details() {
        let fakes = [
            FakeCapturer::unavailable("portal", "org.freedesktop.portal.Desktop not owned"),
            FakeCapturer::available("xcap"),
        ];
        let report = backend_report_with(SessionKind::Wayland, &registry(&fakes));
        assert_eq!(report.session, SessionKind::Wayland);
        assert_eq!(report.entries.len(), 2);

        let portal = &report.entries[0];
        assert_eq!(portal.id, "portal");
        assert!(!portal.available);
        assert_eq!(
            portal.detail.as_deref(),
            Some("org.freedesktop.portal.Desktop not owned")
        );

        let xcap = &report.entries[1];
        assert_eq!(xcap.id, "xcap");
        assert!(xcap.available);
        assert!(xcap.detail.is_none());
    }

    #[test]
    fn report_auto_matches_selection_precedence() {
        let portal_up = [
            FakeCapturer::available("portal"),
            FakeCapturer::available("xcap"),
        ];
        assert_eq!(
            backend_report_with(SessionKind::Wayland, &registry(&portal_up)).auto_selects,
            "portal"
        );

        let portal_down = [
            FakeCapturer::unavailable("portal", "no bus"),
            FakeCapturer::available("xcap"),
        ];
        assert_eq!(
            backend_report_with(SessionKind::Wayland, &registry(&portal_down)).auto_selects,
            "xcap"
        );

        assert_eq!(
            backend_report_with(SessionKind::X11, &registry(&portal_up)).auto_selects,
            "xcap",
            "X11 precedence skips portal even when it is up"
        );
    }

    /// Headless unit test: invalid rects never reach a capture
    /// provider. Confirms both the empty and negative rejection paths.
    /// The happy-path capture is covered by the `#[ignore]`'d
    /// `tests/capture.rs::smoke_can_grab_primary` integration test.
    #[test]
    fn selection_defaults_are_off() {
        let fake = FakeCapturer::available("xcap");
        assert!(
            !fake.supports_selection(),
            "trait default must advertise no selection capability"
        );
        let err = fake
            .capture_selection(&CaptureOptions::default())
            .expect_err("trait default must refuse capture_selection");
        assert!(
            matches!(err, FlintshotError::NotImplemented(_)),
            "expected NotImplemented, got {err:?}"
        );
    }

    #[test]
    fn invalid_rect_rejected() {
        let negative = Rect {
            min: crate::geom::Point::new(100.0, 100.0),
            max: crate::geom::Point::new(50.0, 50.0),
        };
        match capture_region(negative, None) {
            Err(FlintshotError::InvalidRect { reason, .. }) => {
                assert_eq!(reason, "negative", "expected 'negative' reason");
            }
            other => panic!("expected InvalidRect(negative), got {other:?}"),
        }

        let empty = Rect {
            min: crate::geom::Point::new(10.0, 10.0),
            max: crate::geom::Point::new(10.0, 100.0),
        };
        match capture_region(empty, None) {
            Err(FlintshotError::InvalidRect { reason, .. }) => {
                assert_eq!(reason, "empty", "expected 'empty' reason");
            }
            other => panic!("expected InvalidRect(empty), got {other:?}"),
        }
    }

    /// Pure validation check: a normalised, non-zero rect passes the
    /// validation step. (Doesn't call any provider — we stop right
    /// before.)
    #[test]
    fn valid_rect_passes_validation() {
        let good = Rect::from_xywh(10.0, 10.0, 100.0, 100.0);
        validate_rect(good).expect("non-empty, non-negative rect should validate");
    }

    /// Pure validation check: clamping math handles a rect that extends
    /// beyond the source image. Just verifies `crop_to_rect` doesn't
    /// panic on out-of-bounds input.
    #[test]
    fn crop_to_rect_clamps_out_of_bounds() {
        let src = RgbaImage::new(100, 100);
        // Rect from (90, 90) extending past the right/bottom edge by 50px.
        let oversized = Rect::from_xywh(90.0, 90.0, 50.0, 50.0);
        let cropped = crop_to_rect(&src, oversized);
        // Clamped to (90, 90, 10, 10) — the remainder of the image.
        assert_eq!(cropped.dimensions(), (10, 10));
    }
}
