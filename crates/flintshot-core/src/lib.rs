//! Platform-agnostic core for `flintshot`.
//!
//! Owns geometry types, the [`editor::EditorState`] model, the
//! [`render::Canvas`] and [`shell::EditorShell`] traits that UI toolkits
//! implement, and the [`upload::UploadBackend`] trait for pluggable upload
//! providers. No GUI toolkit types appear in this crate's public API.
//!
//! See `docs/ARCHITECTURE.md` for the full design.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]

pub mod editor;
pub mod error;
pub mod geom;
pub mod render;
pub mod shell;
pub mod upload;

pub use editor::{Annotation, BlurKind, EditorState, Tool};
pub use error::{FlintshotError, Result};
pub use geom::{Color, Key, Point, Rect, Stroke};
pub use render::{render, Canvas};
pub use shell::{EditorOutcome, EditorShell};
pub use upload::{ImageMime, UploadBackend, UploadContext, UploadError, UploadOutcome};
