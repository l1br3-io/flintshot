//! Platform-agnostic core for `flintshot`.
//!
//! Owns geometry types, the [`editor::EditorState`] model and its
//! [`input::dispatch_input`] state machine, the [`render::Canvas`] and
//! [`shell::EditorShell`] traits that UI toolkits implement, the
//! [`upload::UploadBackend`] trait for pluggable upload providers,
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
pub mod geom;
pub mod input;
pub mod render;
pub mod serde_util;
pub mod shell;
pub mod upload;

pub use capture::{capture_fullscreen, capture_region};
pub use config::{CaptureConfig, Config, UiConfig};
pub use editor::{Annotation, BlurKind, EditorState, Tool};
pub use error::{FlintshotError, Result};
pub use geom::{Color, Key, Point, Rect, Stroke};
pub use input::{DispatchOutcome, InputEvent, Modifiers};
pub use render::{compose_final, render, render_ann, Canvas, DrawCall, MockCanvas};
pub use shell::{EditorOutcome, EditorShell};
pub use upload::{ImageMime, UploadBackend, UploadContext, UploadError, UploadOutcome};
