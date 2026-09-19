//! IPC protocol types for `flintshot`.
//!
//! Only the *schema* lives here so external clients (the Phase 3 COSMIC
//! applet, scripts, future shells) can depend on [`flintshot-core`]
//! alone. The transport ([`interprocess`](https://docs.rs/interprocess)
//! local socket) and length-prefixed framing live in `flintshot-app`;
//! this module must not import any transport or async-runtime crate. See
//! `docs/ARCHITECTURE.md` §IPC protocol.
//!
//! On the wire, [`Request`] and [`Event`] are encoded with `postcard`
//! (compact, serde-compatible). Every enum is `#[non_exhaustive]` so the
//! daemon can grow new request/event kinds without breaking older
//! clients.

use std::path::{Path, PathBuf};

/// Filename of the Unix-domain socket under the resolved runtime dir.
#[cfg(unix)]
const SOCKET_FILENAME: &str = "flintshot.sock";

/// Name of the Windows named pipe (without the `\\.\pipe\` prefix).
#[cfg(windows)]
const PIPE_NAME: &str = "flintshot";

/// A client → daemon request.
///
/// Encoded length-prefixed postcard on the wire. Variants marked
/// "forward-compat" are defined now so the schema is stable; the daemon
/// returns [`FlintshotError::Ipc`] ("unsupported") for them until their
/// phases land.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Request {
    /// Tell the daemon to quit (used by `flintshot daemon --replace` and
    /// the tray *Quit* item).
    Quit,
    /// Open a long-lived subscription stream; the client receives
    /// [`Event`]s as they happen.
    Subscribe,
    /// Worker → daemon save report (Phase 3). A worker that closed its
    /// editor with a save sends the resulting path back so the daemon can
    /// track it as "last saved" (for [`Request::OpenLast`]
    /// and Phase 4 upload-from-last). Sent
    /// best-effort: silently dropped when no daemon answers (a standalone
    /// worker has no one to report to).
    ReportSaved {
        /// Filesystem path the worker just wrote the image to.
        path: PathBuf,
    },
    // Forward-compat variants — defined now, unsupported until Phase 3:
    /// Re-open the most recently saved capture (Phase 3).
    OpenLast,
    /// Upload the most recently saved capture via the named backend
    /// (Phase 4).
    UploadLast {
        /// Backend id, e.g. `"imgur"` (matches [`crate::upload::UploadBackend::id`]).
        backend: String,
    },
    /// Worker → daemon upload-result report (Phase 4). A worker that
    /// completed an upload sends the resulting URL + delete-handle back
    /// so the daemon can broadcast [`Event::Uploaded`] to subscribers.
    /// Sent best-effort: silently dropped when no daemon answers (a
    /// standalone worker has no one to report to). Parallels
    /// [`Request::ReportSaved`].
    ReportUploaded {
        /// Resulting public URL of the uploaded image.
        url: String,
        /// Backend-specific delete handle when the backend supports
        /// deletion, otherwise `None`.
        delete_handle: Option<String>,
    },
    // Phase 6 — appended rather than inserted: postcard encodes the
    // variant index, so appending keeps the wire bytes of every
    // earlier variant stable across daemon/CLI version skew.
    /// Open the xdg-desktop-portal `GlobalShortcuts` bind dialog in the
    /// daemon's portal session (Phase 6), with the daemon's `[hotkeys]`
    /// chords as suggested bindings. Intercepted by the IPC connection
    /// layer: binding belongs to the daemon's portal session, so the
    /// worker dispatch table never acts on it.
    BindShortcuts,
    /// Worker → daemon, pre-grab: the sender is about to capture the
    /// screen; the daemon hides its own tray icon so it doesn't land
    /// in the frozen frame. Request/reply like `BindShortcuts`: the
    /// daemon answers with [`Event::TrayHidden`] only once the icon is
    /// hidden *and* a propagation grace has elapsed (the panel removes
    /// the item asynchronously — an early ack would race the grab).
    /// A daemon without a tray replies promptly (the hide is vacuous).
    HideTrayForCapture,
    /// Worker → daemon, post-grab: the screen has been captured
    /// (successfully or not), so the tray icon may return. Sent
    /// best-effort and fire-and-forget: silently dropped when no
    /// daemon answers. Pairs with [`Request::HideTrayForCapture`];
    /// the daemon tracks concurrent captures with a depth counter and
    /// restores the icon when the last one reports.
    ReportCaptured,
    // app-window effort — appended (postcard variant-index rule):
    /// Client → daemon: open the app window. The daemon broadcasts
    /// [`Event::AppFocus`] (an already-running window shows, focuses
    /// itself, and routes to its home page) and spawns the `app`
    /// worker (which exits promptly when the app instance lock is
    /// held — the broadcast did the real work). Net effect: one
    /// window, focused, on the home page.
    OpenApp,
    // smart-window-hiding effort — appended (postcard variant-index rule):
    /// Worker → daemon, pre-grab: the sender is about to capture the
    /// screen; the daemon clears every flintshot surface (app window,
    /// tray icon) and replies [`Event::CaptureReady`]
    /// only once the screen is clear or the bounds expire (a capture
    /// must never fail because a window was slow). Supersedes and
    /// internally includes [`Request::HideTrayForCapture`]: the tray
    /// gate is driven as part of the same handshake.
    PrepareCapture,
    /// App window → daemon, best-effort ack: "my window is destroyed".
    /// Pairs with [`Event::HideForCapture`] — the daemon waits for
    /// this (bounded) before replying [`Event::CaptureReady`] to the
    /// worker that is about to grab.
    ReportAppHidden,
    /// Worker → daemon, fire-and-forget (mirrors
    /// [`Request::ReportSaved`]): an editor/overlay window is about
    /// to run. The daemon broadcasts [`Event::EditorOpened`] on
    /// receipt — one event per actual window, never at spawn time.
    /// Silently dropped when no daemon answers (standalone worker).
    ReportEditorWindowOpened,
    // native-capture-ux effort — appended (postcard variant-index rule):
    /// The single Capture action (native-capture-ux): one request for
    /// every surface (hotkey, tray, applet, CLI). The worker resolves
    /// the backend (CLI > config > auto), runs the OS's own selection
    /// UI when the backend supports it, else today's monitor-grab →
    /// editor/overlay flow. `delay_ms` is the pre-capture delay.
    /// The 0.x schema ships all consumers together, so the old
    /// `CaptureRegion`/`CaptureFullscreen` variants were removed
    /// outright rather than deprecated.
    Capture {
        /// Pre-capture delay in milliseconds.
        delay_ms: u32,
    },
    // fix-app-window-capture-races effort — appended (postcard
    // variant-index rule):
    /// App window → daemon: "I exist, on this connection." The daemon
    /// tracks the sender's liveness by the connection itself — the
    /// frame after this one blocks until the app window's process
    /// exits (or the socket dies), at which point the presence is
    /// withdrawn. Feeds the pre-grab handshake
    /// ([`Request::PrepareCapture`]): a live app window owes a
    /// [`Request::ReportAppHidden`] ack before a grab is blessed,
    /// which only works if the daemon knows the window exists even
    /// when it spawned the app itself (the registry only tracks
    /// daemon-spawned workers). A daemon that predates this variant
    /// answers `unsupported` — the app window logs it once and
    /// continues (the handshake degrades to today's grace-only
    /// behavior).
    AnnounceAppWindow,
    // single-window effort — appended (postcard variant-index rule):
    /// CLI `open` → daemon: open `path` in an editor. When an app
    /// window is present the daemon broadcasts
    /// [`Event::OpenRequested`] instead of spawning the open worker
    /// (the window routes itself to its editor page); without one,
    /// today's worker path runs unchanged.
    OpenImage {
        /// Filesystem path of the image to open.
        path: PathBuf,
    },
    // settings-improvement effort — appended (postcard variant-index
    // rule):
    /// Client → daemon: open the app window on its settings page.
    /// Mirrors [`Request::OpenApp`] (broadcast + app-worker spawn)
    /// but carries the destination route: the daemon broadcasts
    /// [`Event::SettingsFocus`] (an already-running window shows,
    /// focuses, and routes itself to Settings) and spawns the `app`
    /// worker with `--route settings` (which exits promptly when the
    /// app instance lock is held — the broadcast did the real work).
    OpenSettings,
}

/// A daemon → client event. Subscribers (see [`Request::Subscribe`])
/// receive a stream of these.
///
/// Uses postcard's default *externally-tagged* representation (a variant
/// index followed by its fields) for binary wire-compatibility — the
/// `#[serde(tag = "kind")]` internally-tagged form shown in
/// `ARCHITECTURE.md` is a JSON concern and is `WontImplement` under
/// postcard. `rename_all` is kept for forward-compat with any future
/// JSON view; it has no effect on the postcard wire bytes. See ADR-0011.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Event {
    /// A capture was initiated (worker spawned).
    CaptureStarted,
    /// An annotation window (editor or overlay) actually opened —
    /// reported by the worker that runs it (one
    /// [`Request::ReportEditorWindowOpened`] per window: fresh
    /// capture, recapture reopen, or `open`; never for `--out`
    /// headless captures, never at spawn time).
    EditorOpened,
    /// The user saved an annotated image to `path`.
    Saved {
        /// Filesystem path the image was written to.
        path: PathBuf,
    },
    /// Something went wrong servicing a request.
    Error {
        /// Stable machine-readable code (e.g. `"unsupported"`,
        /// `"capture-failed"`).
        code: String,
        /// Human-readable message.
        message: String,
    },
    // Phase 3/4 variants — defined now, emitted later:
    /// An upload was initiated (worker spawned). Emitted optimistically
    /// on `UploadLast` dispatch (Phase 4). Mirrors `CaptureStarted`.
    UploadStarted {
        /// Backend id, e.g. `"imgur"`.
        backend: String,
    },
    /// A capture was uploaded (Phase 4).
    Uploaded {
        /// Resulting public URL.
        url: String,
        /// Backend-specific delete handle when the backend supports
        /// deletion, otherwise `None`.
        delete_handle: Option<String>,
    },
    // Phase 6 — appended rather than inserted (postcard encodes the
    // variant index; appending keeps earlier wire bytes stable).
    /// The user completed a portal `GlobalShortcuts` bind (Phase 6).
    /// Written back on the connection that sent
    /// [`Request::BindShortcuts`] so the CLI can print the outcome.
    ShortcutsBound {
        /// Every shortcut now bound in the daemon's portal session:
        /// what the user actually chose, not the config suggestions.
        bindings: Vec<ShortcutBinding>,
    },
    /// Reply to [`Request::HideTrayForCapture`]: the daemon's tray
    /// icon is out of the frame (hidden for long enough that the
    /// panel has re-rendered), so the worker may grab now. Reply-only
    /// — never broadcast to subscribers (see ADR-0036).
    TrayHidden,
    // app-window effort — appended (postcard variant-index rule):
    /// A capture worker process exited — any exit status, emitted by
    /// the daemon's worker reaper. Crash-safe by construction: a
    /// killed worker still "reports" (unlike a worker self-report,
    /// which a crash would swallow). Subscribers that hide for grabs
    /// (the app window) restore on this. Only `capture` workers emit
    /// it.
    SessionEnded,
    /// Someone asked the app window to come forward (tray *Open app*,
    /// a second `flintshot app`, launcher entry). Broadcast on
    /// [`Request::OpenApp`]; a subscribed app window shows, focuses
    /// itself, and routes to its home page (an open window on a
    /// non-home page swaps home in place).
    AppFocus,
    // smart-window-hiding effort — appended (postcard variant-index rule):
    /// A new capture replaced the live capture session (Replace
    /// policy: newest intent wins). Broadcast before the new worker
    /// spawns; subscribed capture workers close their editor/overlay
    /// gracefully (`Cancelled`, no partial saves) so the hard-kill
    /// fallback stays rare.
    CaptureSuperseded,
    /// Reply to [`Request::PrepareCapture`]: every flintshot surface
    /// is out of the frame (or the bounds expired) — the worker may
    /// grab now. Reply-only, never broadcast (see ADR-0036 for the
    /// handshake shape it generalizes).
    CaptureReady,
    /// Broadcast to the app window: a capture is imminent; close and
    /// ack with [`Request::ReportAppHidden`] once the window is
    /// destroyed. Pairs with [`Request::PrepareCapture`].
    HideForCapture,
    // single-window effort — appended (postcard variant-index rule):
    /// Broadcast to the app window: run a capture in-process. The
    /// daemon emits this instead of spawning a capture worker when an
    /// app window is present; the window closes, grabs via the shared
    /// capture flow, and reopens on its editor page. `delay_ms`
    /// carries the pre-capture delay of the forwarded
    /// [`Request::Capture`].
    CaptureTriggered {
        /// Pre-capture delay in milliseconds.
        delay_ms: u32,
    },
    /// Broadcast to the app window: open `path` on the editor page.
    /// Forwarded from [`Request::OpenImage`] (and tray *Open last*
    /// with the last-saved path); without an app window the daemon
    /// spawns today's open worker instead.
    OpenRequested {
        /// Filesystem path of the image to open.
        path: PathBuf,
    },
    // settings-improvement effort — appended (postcard variant-index
    // rule):
    /// Someone asked for the settings page (tray / applet
    /// *Settings*). Broadcast on [`Request::OpenSettings`]; a
    /// subscribed app window shows, focuses, and routes itself to
    /// its settings page — the Settings analogue of
    /// [`Event::AppFocus`].
    SettingsFocus,
}

/// One entry of an [`Event::ShortcutsBound`] reply: a bound shortcut's
/// stable id and the effective trigger the user chose.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ShortcutBinding {
    /// Stable shortcut id (`"capture"`).
    pub id: String,
    /// The effective trigger, in the portal's human-readable form
    /// (e.g. `"Print"`, `"<Ctrl><Shift>P"` — whatever the portal
    /// reports as the trigger description).
    pub trigger: String,
}

/// Resolve the IPC socket/pipe path.
///
/// A non-empty `override_path` always wins. Otherwise the platform
/// default is used:
///
/// | Platform | Default                                            |
/// | -------- | -------------------------------------------------- |
/// | Linux    | `$XDG_RUNTIME_DIR/flintshot.sock`                 |
/// | macOS    | `$XDG_RUNTIME_DIR/flintshot.sock` (falls back to a per-user temp dir, since macOS rarely sets `XDG_RUNTIME_DIR`) |
/// | Windows  | `\\.\pipe\flintshot`                               |
///
/// `override_path` being `None` or an empty path (matching the config
/// default `socket_path = ""`) yields the platform default.
#[must_use]
pub fn socket_path(override_path: Option<&Path>) -> PathBuf {
    if let Some(p) = override_path.filter(|p| !p.as_os_str().is_empty()) {
        return p.to_path_buf();
    }
    default_socket_path()
}

fn default_socket_path() -> PathBuf {
    #[cfg(windows)]
    {
        default_windows_pipe()
    }
    #[cfg(unix)]
    {
        default_unix_socket_from_env()
    }
}

#[cfg(windows)]
fn default_windows_pipe() -> PathBuf {
    PathBuf::from(format!(r"\\.\pipe\{}", PIPE_NAME))
}

#[cfg(unix)]
fn default_unix_socket_from_env() -> PathBuf {
    default_unix_socket_in(
        std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
        &std::env::temp_dir(),
    )
}

/// Pure, testable seam for the Unix default: given an explicit runtime
/// dir (or `None`) and a temp-dir fallback, return the socket path.
#[cfg(unix)]
fn default_unix_socket_in(runtime_dir: Option<PathBuf>, temp_fallback: &Path) -> PathBuf {
    match runtime_dir {
        Some(dir) => dir.join(SOCKET_FILENAME),
        None => temp_fallback.join(SOCKET_FILENAME),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_postcard_roundtrip() {
        let cases = [
            Request::Quit,
            Request::Subscribe,
            Request::ReportSaved {
                path: PathBuf::from("/tmp/flintshot/shot.png"),
            },
            Request::OpenLast,
            Request::UploadLast {
                backend: "imgur".into(),
            },
            Request::ReportUploaded {
                url: "https://imgur.com/abc".into(),
                delete_handle: Some("delhash".into()),
            },
            Request::ReportUploaded {
                url: "https://imgur.com/xyz".into(),
                delete_handle: None,
            },
            Request::BindShortcuts,
            Request::HideTrayForCapture,
            Request::ReportCaptured,
            Request::PrepareCapture,
            Request::ReportAppHidden,
            Request::ReportEditorWindowOpened,
            // native-capture-ux — appended (postcard variant-index rule)
            Request::Capture { delay_ms: 0 },
            Request::Capture { delay_ms: 2_500 },
            // fix-app-window-capture-races — appended
            Request::AnnounceAppWindow,
            // single-window — appended (postcard variant-index rule)
            Request::OpenImage {
                path: PathBuf::from("/tmp/my shots/screen 1.png"),
            },
            // settings-improvement — appended (postcard variant-index rule)
            Request::OpenSettings,
        ];
        for original in cases {
            let bytes = postcard::to_allocvec(&original).expect("encode request");
            let back: Request = postcard::from_bytes(&bytes).expect("decode request");
            assert_eq!(original, back, "Request roundtrip failed for {original:?}");
        }
    }

    #[test]
    fn event_postcard_roundtrip() {
        let cases = [
            Event::CaptureStarted,
            Event::EditorOpened,
            Event::Saved {
                path: PathBuf::from("/tmp/x.png"),
            },
            Event::ShortcutsBound {
                bindings: vec![ShortcutBinding {
                    id: "capture".into(),
                    trigger: "<Control><Shift>Print".into(),
                }],
            },
            Event::Error {
                code: "unsupported".into(),
                message: "nope".into(),
            },
            Event::UploadStarted {
                backend: "imgur".into(),
            },
            Event::Uploaded {
                url: "https://i/x".into(),
                delete_handle: None,
            },
            Event::Uploaded {
                url: "https://i/y".into(),
                delete_handle: Some("hk".into()),
            },
            Event::TrayHidden,
            Event::SessionEnded,
            Event::AppFocus,
            Event::CaptureSuperseded,
            Event::CaptureReady,
            Event::HideForCapture,
            // single-window — appended (postcard variant-index rule)
            Event::CaptureTriggered { delay_ms: 2_500 },
            Event::OpenRequested {
                path: PathBuf::from("/tmp/my shots/screen 1.png"),
            },
            // settings-improvement — appended (postcard variant-index rule)
            Event::SettingsFocus,
        ];
        for original in cases {
            let bytes = postcard::to_allocvec(&original).expect("encode event");
            let back: Event = postcard::from_bytes(&bytes).expect("decode event");
            assert_eq!(original, back, "Event roundtrip failed for {original:?}");
        }
    }

    #[test]
    fn socket_path_override_wins_when_nonempty() {
        let custom = Path::new("/custom/run/flintshot.sock");
        let resolved = socket_path(Some(custom));
        assert_eq!(resolved, PathBuf::from("/custom/run/flintshot.sock"));
    }

    #[test]
    fn socket_path_none_uses_platform_default() {
        let resolved = socket_path(None);
        assert_ne!(
            resolved.as_os_str(),
            "",
            "platform default must be a concrete path, got empty"
        );
    }

    #[test]
    fn socket_path_empty_override_falls_back_to_default() {
        // Config default `socket_path = ""` must yield the platform default.
        let from_empty = socket_path(Some(Path::new("")));
        let from_none = socket_path(None);
        assert_eq!(from_empty, from_none);
    }

    #[cfg(unix)]
    #[test]
    fn unix_socket_uses_xdg_runtime_dir_when_set() {
        let path = default_unix_socket_in(Some(PathBuf::from("/run/user/1000")), Path::new("/tmp"));
        assert_eq!(path, PathBuf::from("/run/user/1000/flintshot.sock"));
    }

    #[cfg(unix)]
    #[test]
    fn unix_socket_falls_back_to_temp_dir_when_xdg_unset() {
        let path = default_unix_socket_in(None, Path::new("/tmp"));
        assert_eq!(path, PathBuf::from("/tmp/flintshot.sock"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_default_is_named_pipe() {
        assert_eq!(default_windows_pipe(), PathBuf::from(r"\\.\pipe\flintshot"));
    }
}
