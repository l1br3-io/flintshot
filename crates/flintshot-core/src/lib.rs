//! Platform-agnostic core for `flintshot`.
//!
//! Owns geometry types, the [`editor::EditorState`] model and its
//! [`input::dispatch_input`] state machine, the [`render::Canvas`] and
//! [`shell::EditorShell`] traits that UI toolkits implement, the
//! [`upload::UploadBackend`] trait for pluggable upload providers
//! (with `keyring::Entry` as the credential handle — see ADR-0024),
//! screen capture via [`capture`] (the [`capture::Capturer`] trait and
//! its backend registry), and the user-facing
//! [`config::Config`] schema. No GUI toolkit types appear in this
//! crate's public API.
//!
//! See `docs/ARCHITECTURE.md` for the full design.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]

pub mod capture;
pub mod config;
pub mod editor;
pub mod error;
pub mod freehand;
pub mod geom;
pub mod image_ops;
pub mod input;
pub mod ipc;
pub mod recents;
pub mod render;
pub mod serde_util;
pub mod shell;
pub mod text;
pub mod upload;

#[cfg(target_os = "linux")]
pub use capture::PortalScreenshotCapturer;
pub use capture::{
    backend_report, backend_report_with, capture_fullscreen, capture_region, monitors,
    registered_backends, select_backend, BackendReport, BackendReportEntry, CaptureOptions,
    Capturer, MonitorInfo, SessionKind, XcapCapturer,
};
pub use config::{
    config_path, estimate_size_bytes, heuristic_bytes, parse_default_mode, parse_output_format,
    parse_size_mode, render_filename_template, sanitize_filename, CaptureConfig, Config,
    DefaultMode, HotkeyConfig, OutputConfig, OutputFormat, SizeMode, UiConfig, UploadConfig,
};
pub use editor::{
    Annotation, BlurKind, EditorState, Snapshot, Tool, NUMBER_MAX_DIAMETER, NUMBER_MIN_DIAMETER,
};
pub use error::{FlintshotError, Result};
pub use freehand::{smooth_polyline, MARKER_ALPHA};
pub use geom::{Color, Key, Point, Rect, Stroke};
pub use image_ops::{blur, crop_region};
pub use input::{DispatchOutcome, InputEvent, Modifiers};
pub use ipc::{socket_path, Event as IpcEvent, Request as IpcRequest};
pub use render::{
    arrow_barbs, compose_final, compose_final_region, render, render_ann, BlurCache, Canvas,
    DrawCall, MockCanvas,
};
pub use shell::{EditorOutcome, EditorShell, NewCaptureMode};
pub use text::{
    FontFamily, FontStyle, FontWeight, TextAlign, TextMetrics, TextOutline, TextStyle,
    DEFAULT_LINE_SPACING,
};
pub use upload::{ImageMime, UploadBackend, UploadContext, UploadError, UploadOutcome};
