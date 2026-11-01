//! Screen-recording engine for `flintshot`.
//!
//! This crate owns everything between "the user asked to record" and
//! "a finalized MP4 exists on disk": the XDG `ScreenCast` portal dance
//! on `Wayland` (`ashpd`), the X11 region grab, and the `GStreamer`
//! pipeline (source → H.264 encode → `mp4mux` → file) with its
//! EOS/finalize lifecycle. It is the only place `GStreamer` appears in
//! the workspace — `flintshot-core` stays `GStreamer`-free and lends
//! this crate its geometry types ([`Rect`]) instead.
//!
//! v1 is Linux-first (`Wayland` portal, X11 fallback); the contract is
//! shaped so other platforms can slot in behind
//! [`Recorder`] later.
//!
//! Layering (filled in by the engine tasks):
//!
//! * [`RecordConfig`] / [`RecordError`] — the caller-facing contract
//! * [`encoder`] / [`probe`] — capability probing over the encoder
//!   fallback chain
//! * `pipeline` — construction, bus watch, EOS/finalize lifecycle
//! * `portal` — the `Wayland` `ScreenCast` session dance

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]

pub mod encoder;
pub mod mp4;
pub mod pipeline;
pub mod probe;

use std::path::PathBuf;
use std::time::Duration;

use flintshot_core::geom::Rect;
use thiserror::Error;

pub use encoder::{ElementRegistry, EncoderKind, ENCODER_CHAIN};
pub use pipeline::{RecordHandle, Recorder};
pub use probe::{probe_capabilities, GstElementRegistry, RecordCapability, RecordSourceKind};

/// Default capture rate in frames per second.
pub const FPS_DEFAULT: u32 = 30;
/// Lower bound for a sanitized capture rate.
pub const FPS_MIN: u32 = 1;
/// Upper bound for a sanitized capture rate.
pub const FPS_MAX: u32 = 60;

/// Which screen the pipeline records from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordSource {
    /// The user's real graphical session — the `Wayland` portal on
    /// Wayland, `ximagesrc` on X11 (resolved by the X11/portal
    /// tasks).
    #[default]
    Session,
    /// The hidden testability spine: `GStreamer` `videotestsrc`,
    /// headless-safe, so engine/CLI/UI integration tests run in CI
    /// with no display or portal. Selected by the CLI's hidden
    /// `--source testsrc`.
    TestSrc,
}

/// Configuration for one recording.
///
/// Fields are public for direct construction; call [`RecordConfig::sanitized`]
/// before handing a config to the engine so out-of-range values (a
/// hand-edited CLI flag, a stale config file) are clamped rather than
/// reaching the pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordConfig {
    /// Destination `.mp4` path. The parent directory must exist; the
    /// file is created by the pipeline.
    pub out: PathBuf,
    /// Capture rate in frames per second. Defaults to
    /// [`FPS_DEFAULT`]; [`RecordConfig::sanitized`] clamps into
    /// `FPS_MIN..=FPS_MAX`.
    pub fps: u32,
    /// Auto-stop cap. `None` records until [`RecordHandle::stop`] is
    /// called.
    pub max_duration: Option<Duration>,
    /// Sub-rectangle of the source to keep, cropped in-pipeline.
    /// `None` records the whole source.
    pub region: Option<Rect>,
    /// Monitor index to record when no [`Self::region`] is set.
    /// `None` lets the source pick its default.
    pub monitor: Option<u32>,
    /// Where the frames come from. Defaults to the real session;
    /// [`RecordSource::TestSrc`] is the CI spine.
    pub source: RecordSource,
}

impl RecordConfig {
    /// A config for `out` with defaults: 30 fps, no duration cap,
    /// whole source, default monitor, session source.
    #[must_use]
    pub fn new(out: impl Into<PathBuf>) -> Self {
        Self {
            out: out.into(),
            fps: FPS_DEFAULT,
            max_duration: None,
            region: None,
            monitor: None,
            source: RecordSource::default(),
        }
    }

    /// Clamp out-of-range values into the pipeline's supported range
    /// (fps into `FPS_MIN..=FPS_MAX`); other fields pass through
    /// untouched.
    #[must_use]
    pub fn sanitized(self) -> Self {
        Self {
            fps: self.fps.clamp(FPS_MIN, FPS_MAX),
            ..self
        }
    }
}

/// Every failure mode the engine reports to its caller. The engine
/// never panics: each variant carries an actionable message (for
/// [`RecordError::ElementMissing`], the exact plugin package to
/// install).
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordError {
    /// The `ScreenCast` portal request was refused — the user dismissed
    /// the consent dialog or the desktop environment denied it.
    #[error("screen-capture permission denied (portal request refused)")]
    PortalDenied,
    /// A required `GStreamer` element is not installed. `package` names
    /// the distro plugin package that provides it.
    #[error("missing GStreamer element `{plugin}` — install the plugin package `{package}`")]
    ElementMissing {
        /// The element's `GStreamer` factory name.
        plugin: String,
        /// The plugin package that installs it.
        package: String,
    },
    /// No graphical session or display is available to record from.
    #[error("no display available to record")]
    NoDisplay,
    /// The pipeline failed while setting up or running `element`.
    #[error("pipeline failed at `{element}`: {message}")]
    PipelineFailed {
        /// The element the pipeline was building or running.
        element: String,
        /// The upstream failure description.
        message: String,
    },
    /// End-of-stream was sent but the file did not finalize within
    /// the wait budget — the recording may be incomplete.
    #[error("timed out waiting for the recording to finalize")]
    FinalizeTimeout,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_fps(fps: u32) -> RecordConfig {
        let mut cfg = RecordConfig::new("/tmp/flintshot-e1.mp4");
        cfg.fps = fps;
        cfg
    }

    #[test]
    fn new_defaults_to_30fps_whole_source_no_cap() {
        let cfg = RecordConfig::new("/tmp/out.mp4");
        assert_eq!(cfg.out, PathBuf::from("/tmp/out.mp4"));
        assert_eq!(cfg.fps, FPS_DEFAULT);
        assert_eq!(cfg.max_duration, None);
        assert_eq!(cfg.region, None);
        assert_eq!(cfg.monitor, None);
        assert_eq!(cfg.source, RecordSource::Session);
    }

    #[test]
    fn sanitized_clamps_fps_zero_to_one() {
        assert_eq!(config_with_fps(0).sanitized().fps, FPS_MIN);
    }

    #[test]
    fn sanitized_clamps_fps_above_sixty_to_sixty() {
        assert_eq!(config_with_fps(FPS_MAX + 1).sanitized().fps, FPS_MAX);
        assert_eq!(config_with_fps(1000).sanitized().fps, FPS_MAX);
    }

    #[test]
    fn sanitized_keeps_in_range_fps_unchanged() {
        for fps in [FPS_MIN, 15, FPS_DEFAULT, 59, FPS_MAX] {
            assert_eq!(config_with_fps(fps).sanitized().fps, fps);
        }
    }

    #[test]
    fn sanitized_preserves_the_other_fields() {
        let region = Rect::new(
            flintshot_core::geom::Point::new(10.0, 20.0),
            flintshot_core::geom::Point::new(110.0, 220.0),
        );
        let mut cfg = RecordConfig::new("/tmp/keep.mp4");
        cfg.fps = 0; // gets clamped; everything below must not
        cfg.max_duration = Some(Duration::from_secs(12));
        cfg.region = Some(region);
        cfg.monitor = Some(1);

        let out = cfg.sanitized();

        assert_eq!(out.out, PathBuf::from("/tmp/keep.mp4"));
        assert_eq!(out.max_duration, Some(Duration::from_secs(12)));
        assert_eq!(out.region, Some(region));
        assert_eq!(out.monitor, Some(1));
    }

    #[test]
    fn element_missing_display_names_plugin_and_package() {
        let err = RecordError::ElementMissing {
            plugin: "x264enc".into(),
            package: "gstreamer1.0-plugins-ugly".into(),
        };
        assert_eq!(
            err.to_string(),
            "missing GStreamer element `x264enc` — install the plugin package `gstreamer1.0-plugins-ugly`"
        );
    }

    #[test]
    fn pipeline_failed_display_carries_element_and_message() {
        let err = RecordError::PipelineFailed {
            element: "mp4mux".into(),
            message: "streaming stopped, reason not-negotiated".into(),
        };
        assert_eq!(
            err.to_string(),
            "pipeline failed at `mp4mux`: streaming stopped, reason not-negotiated"
        );
    }

    #[test]
    fn fieldless_error_display_texts_are_pinned() {
        assert_eq!(
            RecordError::PortalDenied.to_string(),
            "screen-capture permission denied (portal request refused)"
        );
        assert_eq!(
            RecordError::NoDisplay.to_string(),
            "no display available to record"
        );
        assert_eq!(
            RecordError::FinalizeTimeout.to_string(),
            "timed out waiting for the recording to finalize"
        );
    }
}
