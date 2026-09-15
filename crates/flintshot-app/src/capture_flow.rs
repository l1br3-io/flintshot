//! Process-agnostic grab machinery shared by every capture entry
//! point: the CLI worker flows (`cli.rs`) and — single-window effort —
//! the app window's in-process captures. Backend resolution, both grab
//! shapes (OS selection UI and monitor fallback) under the prepare
//! handshake, the portal's clipboard read-back dance, and the error
//! classifiers that keep quiet exits quiet live here. Session
//! plumbing (`CaptureGuard`, `SupersedeWatch`, post-grab dispatch)
//! stays in `cli.rs` until the app-window pump picks its own.

use color_eyre::{eyre::Context, Result};
use flintshot_core::{select_backend, CaptureOptions, Config};

use crate::cli::{CaptureGuard, SupersedeWatch};

/// Resolve the capture backend for the flow decision (CLI > config >
/// auto). Shared by [`crate::cli::run_capture`]'s two flows so both
/// branches log the same resolution.
pub(crate) fn resolve_capture_backend(
    backend_pin: Option<&str>,
) -> Result<Box<dyn flintshot_core::capture::Capturer>> {
    let cfg = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "could not load config; using built-in defaults");
            Config::default()
        }
    };
    let preference = backend_pin.or(Some(cfg.capture.backend.as_str()));
    let backend = select_backend(preference).map_err(|e| color_eyre::eyre::eyre!(e.to_string()))?;
    tracing::info!(backend = backend.id(), "capture backend resolved");
    Ok(backend)
}

/// The native grab under the prepare handshake — the twin of
/// [`capture_prepared`] for backends that select in the OS's own UI.
/// The guard's scope is the grab alone (handshake → grab → release):
/// the tray is restored the moment the pixels are taken and never
/// outlives the grab into the editor session *or* the clipboard
/// wait. `None` is a quiet exit (superseded before the grab, the OS
/// selection UI dismissed, or its Response timed out — the COSMIC
/// wedge); each carries one info log here.
pub(crate) fn native_grab_prepared(
    watch: &SupersedeWatch,
    backend: &dyn flintshot_core::capture::Capturer,
    override_path: Option<&std::path::Path>,
) -> Result<Option<image::RgbaImage>> {
    // Snapshotted before the OS UI opens: after a `clipboard:///`
    // answer, the read-back wait must distinguish the portal's write
    // from an image already sitting on the clipboard (the user's
    // previous Copy — their workflow keeps one there).
    let pre_grab = clipboard_image_digest();
    let grab = {
        let _prepared = CaptureGuard::request_at(override_path);
        if watch.superseded() {
            tracing::info!("native capture superseded before the grab; exiting");
            return Ok(None);
        }
        let cfg = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "could not load config; using built-in defaults");
                Config::default()
            }
        };
        // `CaptureOptions` is `#[non_exhaustive]`: mutate-from-default is
        // the cross-crate construction path.
        let mut opts = CaptureOptions::default();
        opts.include_cursor = cfg.capture.include_cursor;

        tracing::info!(
            backend = backend.id(),
            "opening the OS selection UI (native capture)"
        );
        backend.capture_selection(&opts)
    };
    match grab {
        Ok(img) => Ok(Some(img)),
        // The portal already put the grab on the clipboard and
        // answered `clipboard:///` (no file) — but it sends that
        // Response *before* its queued clipboard-write task runs (and
        // that task sits behind the overlay's layer-surface teardown),
        // so the write lands some time after this arm: observed >1 s
        // after the Response in the wild. The wait rides that out.
        Err(e) if portal_grab_on_clipboard(&e) => read_portal_clipboard_grab(pre_grab).map(Some),
        Err(e) if quiet_capture_exit(&e) => {
            // Dismissal / Response timeout: not an error UX (spec
            // Flow). One log line, exit 0, no notification.
            tracing::info!(error = %e, "native capture dismissed; exiting quietly");
            Ok(None)
        }
        Err(e) => Err(color_eyre::eyre::eyre!(e.to_string()))
            .with_context(|| "native capture via the OS selection UI"),
    }
}

/// Wait out the portal's Response-before-write race and read the
/// grab back from the system clipboard ([`portal_grab_on_clipboard`]:
/// the portal wrote it there itself and answered `clipboard:///`).
/// `pre_grab` is the digest snapshot taken before the OS UI opened;
/// only an image that differs from it counts as the delivery, so a
/// stale image already on the clipboard is never mistaken for the
/// grab. A delivery that never lands is a real capture failure — the
/// grab exists nowhere else.
fn read_portal_clipboard_grab(pre_grab: Option<u64>) -> Result<image::RgbaImage> {
    tracing::info!("portal delivered the grab to the clipboard; waiting for its write to land");
    let probe = || {
        read_image_from_clipboard()
            .map_err(|e| {
                tracing::debug!(error = %e, "clipboard not readable yet; waiting");
                e
            })
            .ok()
    };
    let Some((img, wait)) =
        await_changed_clipboard(&PORTAL_CLIPBOARD_WAIT_SLOTS, pre_grab, probe, |img| {
            rgba_digest(img.width(), img.height(), img.as_raw())
        })
    else {
        return Err(color_eyre::eyre::eyre!(
            "the portal's clipboard write never became readable (waited ~{} s \
             after the `clipboard:///` response)",
            PORTAL_CLIPBOARD_WAIT_SLOTS.len() as u64 * PORTAL_CLIPBOARD_SLOT_MS / 1000
        ));
    };
    if wait.stale_accepted {
        tracing::info!(
            "clipboard image is byte-identical to the pre-grab snapshot; \
             accepting it as the grab"
        );
    } else {
        tracing::debug!(slots = wait.slots_waited, "portal clipboard write landed");
    }
    Ok(img)
}

/// Content digest of raw RGBA pixels. Only in-process determinism is
/// needed (the snapshot and the wait probes share one process);
/// `DefaultHasher` ships fixed keys, so digests are stable across
/// calls.
fn rgba_digest(width: u32, height: u32, bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// Digest of the clipboard's current image, taken before the OS
/// selection UI opens. `None` = no image readable right now (or no
/// clipboard) — any later image is then fresh by definition.
fn clipboard_image_digest() -> Option<u64> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let arboard::ImageData {
        width,
        height,
        bytes,
    } = clipboard.get_image().ok()?;
    let w = u32::try_from(width).ok()?;
    let h = u32::try_from(height).ok()?;
    Some(rgba_digest(w, h, &bytes))
}

/// Copy the grab to the system clipboard as an RGBA image. Unlike the
/// best-effort URL copies, a failure here is a real capture failure —
/// the user asked for the clipboard and nothing else happened.
pub(crate) fn copy_image_to_clipboard(img: &image::RgbaImage) -> Result<()> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| color_eyre::eyre::eyre!("clipboard: {e}"))?;
    clipboard
        .set_image(arboard::ImageData {
            width: img.width() as usize,
            height: img.height() as usize,
            bytes: std::borrow::Cow::Borrowed(img.as_raw()),
        })
        .map_err(|e| color_eyre::eyre::eyre!("copying image to clipboard: {e}"))
}

/// Per-slot wait for the portal's clipboard write
/// ([`read_portal_clipboard_grab`]): xdg-desktop-portal-cosmic
/// spawns the `clipboard:///` D-Bus Response *before* its update
/// fn returns the queued clipboard-write task, and that task sits
/// behind the screenshot overlay's layer-surface teardown —
/// observed >1 s from Response to readable selection in the wild
/// (daemon.log, 2026-09-14), where the old ~1 s budget failed.
/// 50 × 100 ms ≈ 5 s bounds a wedged portal without guessing
/// tighter.
const PORTAL_CLIPBOARD_SLOT_MS: u64 = 100;
const PORTAL_CLIPBOARD_WAIT_SLOTS: [std::time::Duration; 50] =
    [std::time::Duration::from_millis(PORTAL_CLIPBOARD_SLOT_MS); 50];

/// How [`await_changed_clipboard`] resolved.
struct GrabWait {
    /// Sleep slots consumed before the winning probe.
    slots_waited: usize,
    /// Every readable image through the deadline was byte-identical
    /// to the pre-grab snapshot, and the deadline image was accepted
    /// as the grab anyway.
    stale_accepted: bool,
}

/// Poll `probe` for a clipboard image that differs from the
/// pre-grab digest `pre`: one probe immediately, then one per delay
/// slot's sleep (`delays.len() + 1` attempts). A probe miss (no
/// image / read error, folded by the caller) just waits out its
/// slot. Returns the first changed image, or the deadline image
/// when it is byte-identical to `pre` (`stale_accepted` — two
/// identical screenshots back to back must not be read as "the
/// write never landed", and byte-identical content is correct
/// content either way), or `None` when nothing ever became
/// readable.
fn await_changed_clipboard<T>(
    delays: &[std::time::Duration],
    pre: Option<u64>,
    mut probe: impl FnMut() -> Option<T>,
    digest: impl Fn(&T) -> u64,
) -> Option<(T, GrabWait)> {
    let mut stale: Option<T> = None;
    for attempt in 0..=delays.len() {
        if attempt > 0 {
            std::thread::sleep(delays[attempt - 1]);
        }
        let Some(grab) = probe() else {
            continue;
        };
        if Some(digest(&grab)) != pre {
            return Some((
                grab,
                GrabWait {
                    slots_waited: attempt.saturating_sub(1),
                    stale_accepted: false,
                },
            ));
        }
        stale = Some(grab);
    }
    stale.map(|grab| {
        (
            grab,
            GrabWait {
                slots_waited: delays.len(),
                stale_accepted: true,
            },
        )
    })
}

/// Read the portal's Copy-confirmed grab back from the system
/// clipboard ([`portal_grab_on_clipboard`]: the portal wrote it
/// there itself and answered `clipboard:///`). A failure here is a
/// real capture failure — the grab exists nowhere else.
fn read_image_from_clipboard() -> Result<image::RgbaImage> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| color_eyre::eyre::eyre!("clipboard: {e}"))?;
    let arboard::ImageData {
        width,
        height,
        bytes,
    } = clipboard
        .get_image()
        .map_err(|e| color_eyre::eyre::eyre!("reading the grab back from the clipboard: {e}"))?;
    let len = bytes.len();
    let w = u32::try_from(width)
        .map_err(|_| color_eyre::eyre::eyre!("clipboard image width {width} overflows u32"))?;
    let h = u32::try_from(height)
        .map_err(|_| color_eyre::eyre::eyre!("clipboard image height {height} overflows u32"))?;
    image::RgbaImage::from_raw(w, h, bytes.into_owned()).ok_or_else(|| {
        color_eyre::eyre::eyre!(
            "clipboard image is {width}x{height} but carries {len} bytes (expected width*height*4)"
        )
    })
}

/// One in-process grab's outcome for the app window's page router
/// (single-window): what the pump should route when its grab thread
/// finishes.
#[derive(Debug)]
pub(crate) enum AppGrab {
    /// An image landed. `selection_first` picks the editor page's
    /// landing phase: the fallback monitor grab pre-arms Select (the
    /// worker's `"region"` labels); the native path (the OS UI
    /// already selected) opens plain (`"capture"`).
    Image {
        /// The grabbed pixels.
        img: image::RgbaImage,
        /// Whether the editor page lands selection-first.
        selection_first: bool,
    },
    /// Quiet exit — superseded before the grab, the OS selection UI
    /// dismissed, or its Response timed out. Reopen the prior page;
    /// no error surface.
    Quiet,
    /// Real failure: reopen with the message on the status line.
    Failed(String),
}

/// Grab for the app window's pump (single-window): resolve the
/// backend (config > auto — the app window carries no CLI pin), then
/// the OS's own selection UI where the backend supports it, else the
/// primary-monitor grab — both under the prepare handshake, both
/// quiet-exit aware. `delay_ms` sleeps before anything runs (the
/// forwarded trigger's pre-capture delay; the button path passes 0).
/// Blocking (the OS UI blocks for human think-time); run on a
/// dedicated thread so the pump keeps pumping.
pub(crate) fn grab_for_app_window(watch: &SupersedeWatch, delay_ms: u32) -> AppGrab {
    if delay_ms > 0 {
        tracing::debug!(delay_ms, "sleeping before the app-window capture");
        std::thread::sleep(std::time::Duration::from_millis(u64::from(delay_ms)));
    }
    match resolve_capture_backend(None) {
        Ok(backend) => grab_with_backend(watch, backend.as_ref(), None),
        Err(e) => AppGrab::Failed(format!("{e:#}")),
    }
}

/// The testable seam under [`grab_for_app_window`]: the grab shape
/// per already-resolved backend. A superseded watch short-circuits
/// before any backend call — the newest intent owns the screen.
pub(crate) fn grab_with_backend(
    watch: &SupersedeWatch,
    backend: &dyn flintshot_core::capture::Capturer,
    override_path: Option<&std::path::Path>,
) -> AppGrab {
    if watch.superseded() {
        tracing::info!("app-window capture superseded before the grab; exiting quietly");
        return AppGrab::Quiet;
    }
    if backend.supports_selection() {
        match native_grab_prepared(watch, backend, override_path) {
            Ok(Some(img)) => AppGrab::Image {
                img,
                selection_first: false,
            },
            Ok(None) => AppGrab::Quiet,
            Err(e) => AppGrab::Failed(format!("{e:#}")),
        }
    } else {
        let cfg = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "could not load config; using built-in defaults");
                Config::default()
            }
        };
        match monitor_grab_prepared("region", watch, override_path, backend, &cfg) {
            Ok(Some(img)) => AppGrab::Image {
                img,
                selection_first: true,
            },
            Ok(None) => AppGrab::Quiet,
            Err(e) => AppGrab::Failed(format!("{e:#}")),
        }
    }
}

/// Classify a capture error as a **quiet exit**: the user dismissed
/// the OS selection UI, or the interactive Response never arrived
/// within its budget (spike C: the wedged-session hang). Both are
/// "changed my mind" outcomes — exit 0 with one info log, never an
/// error toast.
///
/// GOTCHA: substring matching over the backend's error detail is the
/// established contract — `capture_selection` documents
/// `"cancelled"` (portal code 1, macOS `"permission"` precedent) and
/// the portal adds `"timed out"` for the interactive budget. Core's
/// error enum is stringly for backend detail by design (0.x; see
/// `FlintshotError::Capture`).
fn quiet_capture_exit(err: &flintshot_core::FlintshotError) -> bool {
    match err {
        flintshot_core::FlintshotError::Capture { detail } => {
            detail.contains("cancelled") || detail.contains("timed out")
        }
        _ => false,
    }
}

/// Whether the portal delivered the grab to the system clipboard
/// itself (COSMIC's interactive UI confirmed with its Copy button
/// answers `clipboard:///`): the caller reads the pixels back
/// instead of failing. Matches only the canonical core marker — a
/// bare "non-file uri" wording stays a real error.
fn portal_grab_on_clipboard(err: &flintshot_core::FlintshotError) -> bool {
    match err {
        flintshot_core::FlintshotError::Capture { detail } => {
            detail.contains(flintshot_core::capture::PORTAL_GRAB_ON_CLIPBOARD)
        }
        _ => false,
    }
}

/// Grab the primary monitor with every flintshot surface cleared from
/// the screen (smart-window-hiding; the ADR-0036 tray-hide
/// generalized): the prepare handshake, the capture, and the restore
/// report all sit inside the guard's scope, so the tray returns the
/// instant the pixels are taken — success or failure — while the
/// editor / overlay session beyond this function never holds anything
/// hostage. No daemon (standalone capture) costs one socket probe.
///
/// `None` means a supersede arrived mid-handshake (Task 004's
/// subscription flag): the capture is abandoned without grabbing —
/// the newer capture owns the screen.
///
/// `backend_pin` is the CLI `--backend` value; the config's
/// `[capture] backend` joins here (CLI > config > auto — upload's
/// `--backend` precedence). The grab goes through the backend
/// registry; the chosen id is logged so a mis-pinned backend is
/// diagnosable from the worker log.
pub(crate) fn capture_prepared(
    mode: &str,
    watch: &SupersedeWatch,
    override_path: Option<&std::path::Path>,
    backend_pin: Option<&str>,
) -> Result<Option<image::RgbaImage>> {
    let cfg = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "could not load config; using built-in defaults");
            Config::default()
        }
    };
    let preference = backend_pin.or(Some(cfg.capture.backend.as_str()));
    let backend = select_backend(preference).map_err(|e| color_eyre::eyre::eyre!(e.to_string()))?;
    tracing::info!(backend = backend.id(), "{mode} capture backend resolved");
    monitor_grab_prepared(mode, watch, override_path, backend.as_ref(), &cfg)
}

/// The backend-injected seam under [`capture_prepared`]: the prepare
/// handshake + the primary-monitor grab over an already-resolved
/// backend. The app window's pump grabs through this with its own
/// backend instance (single-window); tests through this with fakes.
pub(crate) fn monitor_grab_prepared(
    mode: &str,
    watch: &SupersedeWatch,
    override_path: Option<&std::path::Path>,
    backend: &dyn flintshot_core::capture::Capturer,
    cfg: &Config,
) -> Result<Option<image::RgbaImage>> {
    let _prepared = CaptureGuard::request_at(override_path);
    if watch.superseded() {
        return Ok(None);
    }
    // `CaptureOptions` is `#[non_exhaustive]`: mutate-from-default is
    // the cross-crate construction path.
    let mut opts = CaptureOptions::default();
    opts.include_cursor = cfg.capture.include_cursor;
    backend
        .capture_monitor(None, &opts)
        .map(Some)
        .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))
        .with_context(|| format!("capturing {mode} on the primary monitor"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::SupersedeWatch;

    #[test]
    fn capture_prepared_abandons_when_superseded_before_the_grab() {
        // The flag may flip during the handshake wait (Task 004's
        // subscription): the guard returns without grabbing — no
        // display is ever touched.
        let watch = SupersedeWatch::inert();
        watch
            .flag()
            .store(true, std::sync::atomic::Ordering::Release);
        let outcome = capture_prepared(
            "region",
            &watch,
            Some(std::path::Path::new(
                "/this/does/not/exist/flintshot-test-superseded.sock",
            )),
            None,
        )
        .expect("an abandoned grab is Ok(None), not an error");
        assert!(outcome.is_none(), "superseded → no grab at all");
    }

    #[test]
    fn capture_prepared_grabs_when_not_superseded() {
        // Inert watch, no daemon: the vacuous prepare proceeds to the
        // grab — which needs a display, so this runs only under a
        // session. Headless CI asserts the abandonment path above.
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return; // headless: covered by capture_prepared_abandons
        }
        let watch = SupersedeWatch::inert();
        let outcome = capture_prepared(
            "region",
            &watch,
            Some(std::path::Path::new(
                "/this/does/not/exist/flintshot-test-grab.sock",
            )),
            None,
        );
        assert!(
            matches!(&outcome, Ok(Some(_)) | Err(_)),
            "not superseded: the grab runs ({outcome:?})"
        );
    }

    /// Quiet-cancel classification: dismissal in the OS selection UI
    /// and the interactive Response timeout both exit quietly (exit 0,
    /// one info log, no notification). Everything else is a real
    /// failure the user must see.
    #[test]
    fn quiet_capture_exit_classifies_dismissal_and_timeout_only() {
        use flintshot_core::FlintshotError;
        let quiet = [
            "portal screenshot cancelled (response code 1)",
            "portal screenshot timed out after 300 s (no Response signal)",
        ];
        for detail in quiet {
            assert!(
                quiet_capture_exit(&FlintshotError::Capture {
                    detail: detail.into()
                }),
                "must classify quietly: {detail}"
            );
        }
        let loud = [
            "portal screenshot decode failed: bad bytes",
            "shm screencopy failed",
            "permission denied",
        ];
        for detail in loud {
            assert!(
                !quiet_capture_exit(&FlintshotError::Capture {
                    detail: detail.into()
                }),
                "must NOT be quiet: {detail}"
            );
        }
        assert!(
            !quiet_capture_exit(&FlintshotError::NotImplemented("no selection")),
            "non-capture errors are never quiet cancels"
        );
    }

    /// On-clipboard classification: only the portal's canonical
    /// marker (COSMIC's Copy-confirmed screenshot answers
    /// `clipboard:///`) routes to the clipboard read-back. The old
    /// bare "non-file uri" wording must NOT match — it would mask
    /// genuinely unsupported URI forms behind a clipboard read.
    #[test]
    fn portal_grab_on_clipboard_classifies_marker_only() {
        use flintshot_core::FlintshotError;
        let marked = FlintshotError::Capture {
            detail: flintshot_core::capture::PORTAL_GRAB_ON_CLIPBOARD.into(),
        };
        assert!(portal_grab_on_clipboard(&marked));
        let near_miss = [
            "portal returned a non-file uri: clipboard:///",
            "portal screenshot cancelled (response code 1)",
            "portal screenshot decode failed: bad bytes",
        ];
        for detail in near_miss {
            assert!(
                !portal_grab_on_clipboard(&FlintshotError::Capture {
                    detail: detail.into()
                }),
                "must NOT classify as on-clipboard: {detail}"
            );
        }
        assert!(
            !portal_grab_on_clipboard(&FlintshotError::NotImplemented("no selection")),
            "non-capture errors are never clipboard deliveries"
        );
    }

    /// The read-back waits out the portal's own clipboard write: it
    /// spawns the `clipboard:///` Response *before* executing its
    /// queued clipboard task (and that task queues behind the
    /// overlay teardown), so the write lands some time after the
    /// Response — observed over 1 s in the wild. An image that
    /// differs from the pre-grab snapshot wins on the first probe
    /// it appears in.
    #[test]
    fn await_changed_clipboard_returns_a_fresh_image_immediately() {
        let attempts = std::cell::Cell::new(0u32);
        let probe = || {
            attempts.set(attempts.get() + 1);
            Some(2_u64)
        };
        let (grab, wait) =
            await_changed_clipboard(&[], Some(1), probe, |digest: &u64| *digest).expect("fresh");
        assert_eq!(grab, 2);
        assert!(!wait.stale_accepted);
        assert_eq!(wait.slots_waited, 0);
        assert_eq!(attempts.get(), 1, "single probe with no delays");
    }

    #[test]
    fn await_changed_clipboard_ignores_a_stale_image_until_it_changes() {
        let attempts = std::cell::Cell::new(0u32);
        let probe = || {
            attempts.set(attempts.get() + 1);
            if attempts.get() < 3 {
                Some(1_u64) // the pre-grab image, still on the clipboard
            } else {
                Some(9_u64) // the portal's write has landed
            }
        };
        // Zero delays keep the test sleep-free; the policy (one probe
        // per slot + one final) is what is under test.
        let (grab, wait) = await_changed_clipboard(
            &[std::time::Duration::ZERO, std::time::Duration::ZERO],
            Some(1),
            probe,
            |digest: &u64| *digest,
        )
        .expect("changed");
        assert_eq!(grab, 9, "the stale image must never win");
        assert!(!wait.stale_accepted);
        assert_eq!(attempts.get(), 3);
    }

    /// Two identical screenshots back to back: the write lands but
    /// never differs from the snapshot — accepted at the deadline
    /// (byte-identical is correct content either way), never a
    /// timeout.
    #[test]
    fn await_changed_clipboard_accepts_an_identical_image_at_the_deadline() {
        let attempts = std::cell::Cell::new(0u32);
        let probe = || {
            attempts.set(attempts.get() + 1);
            Some(1_u64)
        };
        let (grab, wait) = await_changed_clipboard(
            &[std::time::Duration::ZERO],
            Some(1),
            probe,
            |digest: &u64| *digest,
        )
        .expect("stale-accept");
        assert_eq!(grab, 1);
        assert!(wait.stale_accepted, "deadline image identical to snapshot");
        assert_eq!(attempts.get(), 2, "one probe per slot plus a final");
    }

    /// With no snapshot (`pre == None`) any readable image is fresh
    /// by definition — the common clipboard-with-no-image start.
    #[test]
    fn await_changed_clipboard_treats_any_image_as_fresh_without_a_snapshot() {
        let (grab, wait) =
            await_changed_clipboard(&[], None, || Some(7_u64), |digest: &u64| *digest)
                .expect("fresh without snapshot");
        assert_eq!(grab, 7);
        assert!(!wait.stale_accepted);
    }

    #[test]
    fn await_changed_clipboard_times_out_when_nothing_lands() {
        let attempts = std::cell::Cell::new(0u32);
        let probe = || {
            attempts.set(attempts.get() + 1);
            None
        };
        let outcome = await_changed_clipboard(
            &[std::time::Duration::ZERO, std::time::Duration::ZERO],
            None,
            probe,
            |digest: &u64| *digest,
        );
        assert!(outcome.is_none(), "no delivery within the window");
        assert_eq!(attempts.get(), 3, "one probe per slot plus a final");
    }

    /// The snapshot and the probes must agree on digests for the
    /// same pixels — and disagree for any change.
    #[test]
    fn rgba_digest_matches_identical_pixels_and_distinguishes_changes() {
        let px = vec![7_u8; 16];
        assert_eq!(rgba_digest(2, 2, &px), rgba_digest(2, 2, &px));
        let other = vec![7_u8; 15];
        assert_ne!(rgba_digest(2, 2, &px), rgba_digest(2, 2, &other));
        assert_ne!(rgba_digest(2, 2, &px), rgba_digest(4, 1, &px));
    }

    // --- App-window grab classification (single-window Task 006) ---

    use flintshot_core::capture::{CaptureOptions, Capturer};
    use flintshot_core::{FlintshotError, Result};

    /// What the fake backend hands back from either grab method.
    enum FakeGrab {
        Ok,
        Cancelled,
        Fail,
    }

    /// Inert backend double: no bus, no display — the grab result is
    /// whatever the test arms.
    struct FakeCapturer {
        selection: bool,
        outcome: FakeGrab,
    }

    fn tiny_img() -> image::RgbaImage {
        image::RgbaImage::from_raw(2, 2, vec![255; 16]).expect("2×2×4 bytes")
    }

    fn fake_result(outcome: &FakeGrab) -> Result<image::RgbaImage> {
        match outcome {
            FakeGrab::Ok => Ok(tiny_img()),
            FakeGrab::Cancelled => Err(FlintshotError::Capture {
                detail: "portal screenshot cancelled (response code 1)".into(),
            }),
            FakeGrab::Fail => Err(FlintshotError::Capture {
                detail: "shm screencopy failed".into(),
            }),
        }
    }

    impl Capturer for FakeCapturer {
        fn id(&self) -> &'static str {
            "fake"
        }

        fn name(&self) -> &'static str {
            "fake"
        }

        fn available(&self) -> Result<()> {
            Ok(())
        }

        fn capture_monitor(
            &self,
            _monitor: Option<u32>,
            _opts: &CaptureOptions,
        ) -> Result<image::RgbaImage> {
            fake_result(&self.outcome)
        }

        fn capture_selection(&self, _opts: &CaptureOptions) -> Result<image::RgbaImage> {
            fake_result(&self.outcome)
        }

        fn supports_selection(&self) -> bool {
            self.selection
        }
    }

    /// A socket path that cannot exist — keeps the fake grabs'
    /// `CaptureGuard` probes vacuous even when a real daemon runs on
    /// the dev machine.
    fn bogus_socket() -> &'static std::path::Path {
        std::path::Path::new("/this/does/not/exist/flintshot-test-grab-flow.sock")
    }

    #[test]
    fn app_grab_native_path_routes_a_plain_image() {
        // The OS UI already selected: the editor page opens plain
        // ("capture" landing, no selection-first phase).
        let backend = FakeCapturer {
            selection: true,
            outcome: FakeGrab::Ok,
        };
        match grab_with_backend(&SupersedeWatch::inert(), &backend, Some(bogus_socket())) {
            AppGrab::Image {
                img,
                selection_first,
            } => {
                assert_eq!(img.dimensions(), (2, 2));
                assert!(!selection_first, "native path opens plain");
            }
            other => panic!("expected an image, got {other:?}"),
        }
    }

    #[test]
    fn app_grab_fallback_routes_selection_first() {
        // The fallback monitor grab lands selection-first — the
        // worker's "region" labels, byte-identical.
        let backend = FakeCapturer {
            selection: false,
            outcome: FakeGrab::Ok,
        };
        match grab_with_backend(&SupersedeWatch::inert(), &backend, Some(bogus_socket())) {
            AppGrab::Image {
                img,
                selection_first,
            } => {
                assert_eq!(img.dimensions(), (2, 2));
                assert!(selection_first, "fallback path pre-arms Select");
            }
            other => panic!("expected an image, got {other:?}"),
        }
    }

    #[test]
    fn app_grab_dismissal_is_quiet() {
        // A dismissed OS selection UI reopens the prior page; no
        // error surface (spec Flow).
        let backend = FakeCapturer {
            selection: true,
            outcome: FakeGrab::Cancelled,
        };
        assert!(matches!(
            grab_with_backend(&SupersedeWatch::inert(), &backend, Some(bogus_socket())),
            AppGrab::Quiet
        ));
    }

    #[test]
    fn app_grab_real_failure_carries_the_detail() {
        let backend = FakeCapturer {
            selection: false,
            outcome: FakeGrab::Fail,
        };
        match grab_with_backend(&SupersedeWatch::inert(), &backend, Some(bogus_socket())) {
            AppGrab::Failed(msg) => assert!(msg.contains("screencopy")),
            other => panic!("expected a failure, got {other:?}"),
        }
    }

    #[test]
    fn app_grab_superseded_short_circuits_before_the_backend() {
        // The newest intent owns the screen: a superseded watch never
        // touches the backend (this fake would fail the test by
        // panicking on grab).
        struct TouchyBackend;
        impl Capturer for TouchyBackend {
            fn id(&self) -> &'static str {
                "touchy"
            }
            fn name(&self) -> &'static str {
                "touchy"
            }
            fn available(&self) -> Result<()> {
                Ok(())
            }
            fn capture_monitor(
                &self,
                _monitor: Option<u32>,
                _opts: &CaptureOptions,
            ) -> Result<image::RgbaImage> {
                panic!("a superseded grab must not reach the backend")
            }
            fn capture_selection(&self, _opts: &CaptureOptions) -> Result<image::RgbaImage> {
                panic!("a superseded grab must not reach the backend")
            }
        }
        let watch = SupersedeWatch::inert();
        watch
            .flag()
            .store(true, std::sync::atomic::Ordering::Release);
        assert!(matches!(
            grab_with_backend(&watch, &TouchyBackend, Some(bogus_socket())),
            AppGrab::Quiet
        ));
    }
}
