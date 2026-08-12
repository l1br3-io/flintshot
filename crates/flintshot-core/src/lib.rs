//! Platform-agnostic core for `flintshot`.
//!
//! Owns geometry types, the [`editor::EditorState`] model and its
//! [`input::dispatch_input`] state machine, the [`render::Canvas`] and
//! [`shell::EditorShell`] traits that UI toolkits implement, the
//! [`upload::UploadBackend`] trait for pluggable upload providers
//! (with `keyring::Entry` as the credential handle — see ADR-0024),
//! screen capture via [`capture`], and the user-facing
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
pub mod render;
pub mod serde_util;
pub mod shell;
pub mod text;
pub mod upload;

pub use capture::{capture_fullscreen, capture_region, monitors, MonitorInfo};
pub use config::{config_path, CaptureConfig, Config, HotkeyConfig, PinConfig, UiConfig};
pub use editor::{Annotation, BlurKind, EditorState, Snapshot, Tool};
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
pub use shell::{EditorOutcome, EditorShell};
pub use text::{
    FontFamily, FontStyle, FontWeight, TextAlign, TextMetrics, TextOutline, TextStyle,
    DEFAULT_LINE_SPACING,
};
pub use upload::{ImageMime, UploadBackend, UploadContext, UploadError, UploadOutcome};
