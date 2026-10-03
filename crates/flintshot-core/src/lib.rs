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

pub mod ai;
pub mod capture;
pub mod config;
pub mod editor;
pub mod error;
pub mod freehand;
pub mod geom;
pub mod history;
pub mod image_ops;
pub mod input;
pub mod inspector;
pub mod ipc;
pub mod output;
pub mod recents;
pub mod render;
pub mod serde_util;
pub mod session;
pub mod shell;
pub mod text;
pub mod upload;

pub use ai::{
    image_part, png_data_url, text_part, transform_store_path, vision_probe_messages, AiChoice,
    AiClient, AiError, AiRequest, AiResponse, AiResponseMessage, AiStreamChunk, AiTransform,
    ChatMessage, ContentPart, ImageUrl, ProviderPreset, SseDecoder, SseEvent, TransformStore,
};
#[cfg(target_os = "linux")]
pub use capture::PortalScreenshotCapturer;
pub use capture::{
    backend_report, backend_report_with, capture_fullscreen, capture_region, monitors,
    registered_backends, select_backend, BackendReport, BackendReportEntry, CaptureOptions,
    Capturer, MonitorInfo, SessionKind, XcapCapturer,
};
pub use config::{
    config_path, default_config_toml, ensure_user_config, ensure_user_config_at,
    estimate_size_bytes, heuristic_bytes, parse_default_mode, parse_output_format, parse_size_mode,
    render_filename_template, resolve_save_dir_with, sanitize_filename, AiConfig, CaptureConfig,
    Config, DefaultMode, HotkeyConfig, OutputConfig, OutputFormat, SizeMode, UiConfig,
    UploadConfig, PANEL_MAX_WIDTH, PANEL_MIN_WIDTH,
};
pub use editor::{
    Annotation, BlurKind, EditorState, Snapshot, Tool, NUMBER_MAX_DIAMETER, NUMBER_MIN_DIAMETER,
};
pub use error::{FlintshotError, Result};
pub use freehand::{smooth_polyline, MARKER_ALPHA};
pub use geom::{Color, Key, Point, Rect, Stroke};
pub use history::{format_elapsed, EditKind};
pub use image_ops::{blur, crop_region};
pub use input::{DispatchOutcome, InputEvent, Modifiers};
pub use inspector::{PropertyEditKind, SelectionCapabilities};
pub use ipc::{socket_path, Event as IpcEvent, Request as IpcRequest};
pub use output::{
    chrono_date, chrono_time, clipboard_copy_stem, next_free_path, output_ext,
    persist_clipboard_copy, save_capture_image,
};
pub use render::{
    arrow_barbs, compose_final, compose_final_region, render, render_ann, BlurCache, Canvas,
    DrawCall, MockCanvas,
};
pub use session::{session_dir, SessionCapture, SessionId, SessionStore, SessionSummary};
pub use shell::{EditorOutcome, EditorShell, NewCaptureMode};
pub use text::{
    FontFamily, FontStyle, FontWeight, TextAlign, TextMetrics, TextOutline, TextStyle,
    DEFAULT_LINE_SPACING,
};
pub use upload::{ImageMime, UploadBackend, UploadContext, UploadError, UploadOutcome};
