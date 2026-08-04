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
    /// Capture a region (interactive selection) and open the editor.
    /// `monitor` selects the source monitor when set; `delay_ms` is the
    /// pre-capture delay in milliseconds.
    CaptureRegion {
        /// Index of the monitor to capture, or the primary when `None`.
        monitor: Option<u32>,
        /// Pre-capture delay in milliseconds.
        delay_ms: u32,
    },
    /// Capture an entire monitor and open the editor. `monitor` selects
    /// the source monitor when set; primary otherwise.
    CaptureFullscreen {
        /// Index of the monitor to capture, or the primary when `None`.
        monitor: Option<u32>,
    },
    /// Tell the daemon to quit (used by `flintshot daemon --replace` and
    /// the tray *Quit* item).
    Quit,
    /// Open a long-lived subscription stream; the client receives
    /// [`Event`]s as they happen.
    Subscribe,
    /// Worker → daemon: report that the editor saved to `path`.
    ///
    /// Direction inversion: every other variant in this enum is
    /// client → daemon (a client asks, the daemon acts). `ReportSaved`
    /// flows the other way — a worker process (spawned by the daemon
    /// for `CaptureRegion`/`CaptureFullscreen`) reports its final
    /// editor outcome back to the daemon. The daemon is still the IPC
    /// server; this is a request the daemon *receives* rather than
    /// *acts on* in the spawn sense.
    ///
    /// Fire-and-forget: the daemon stores the path as its last-saved
    /// capture (so [`PinLast`](Self::PinLast) can re-pin it) and
    /// broadcasts [`Event::Saved`] to subscribers. No response is sent
    /// (matching [`Subscribe`](Self::Subscribe)'s no-response shape);
    /// a missing daemon (standalone capture) is silent and non-fatal.
    ReportSaved {
        /// Filesystem path the editor wrote to.
        path: PathBuf,
    },
    // Forward-compat variants — defined now, unsupported until Phase 3:
    /// Re-open the most recently saved capture (Phase 3).
    OpenLast,
    /// Pin the most recently saved capture to the desktop (Phase 3).
    PinLast,
    /// Upload the most recently saved capture via the named backend
    /// (Phase 4).
    UploadLast {
        /// Backend id, e.g. `"imgur"` (matches [`crate::upload::UploadBackend::id`]).
        backend: String,
    },
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
    /// The editor window opened over the capture.
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
    /// A capture was pinned to the desktop (Phase 3).
    Pinned {
        /// Opaque id the pin window is tracked under.
        id: u64,
    },
    /// A capture was uploaded (Phase 4).
    Uploaded {
        /// Resulting public URL.
        url: String,
        /// Backend-specific delete handle when the backend supports
        /// deletion, otherwise `None`.
        delete_handle: Option<String>,
    },
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
            Request::CaptureRegion {
                monitor: None,
                delay_ms: 0,
            },
            Request::CaptureRegion {
                monitor: Some(2),
                delay_ms: 500,
            },
            Request::CaptureFullscreen { monitor: None },
            Request::CaptureFullscreen { monitor: Some(1) },
            Request::Quit,
            Request::Subscribe,
            Request::ReportSaved {
                path: PathBuf::from("/tmp/x.png"),
            },
            Request::OpenLast,
            Request::PinLast,
            Request::UploadLast {
                backend: "imgur".into(),
            },
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
            Event::Error {
                code: "unsupported".into(),
                message: "nope".into(),
            },
            Event::Pinned { id: 42 },
            Event::Uploaded {
                url: "https://i/x".into(),
                delete_handle: None,
            },
            Event::Uploaded {
                url: "https://i/y".into(),
                delete_handle: Some("hk".into()),
            },
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
