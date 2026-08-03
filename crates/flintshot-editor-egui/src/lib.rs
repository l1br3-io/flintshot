//! egui implementation of `flintshot-core`'s [`Canvas`] and
//! [`EditorShell`] traits.
//!
//! ## Layout
//!
//! - [`canvas`] — the `EguiCanvas` / `EguiTexture` types and the
//!   `impl Canvas` (background primitives live here).
//! - [`app`] — the internal `EditorApp` (`impl eframe::App`) that
//!   drives the per-frame render loop.
//! - [`magnifier`] — the toolkit-owned pixel-zoom inset widget (no
//!   core abstraction; reads `EditorState::image` directly).
//! - [`shell`] — the public `EguiShell` (`impl EditorShell`) that
//!   owns the eframe lifecycle and the outcome slot.
//!
//! Phase 1 Task 4 filled in the background-blit path (canvas module);
//! Task 5 added the eframe shell (app + shell modules) with the
//! "open window, show screenshot, Esc cancels" minimum viable editor.
//! Annotation primitives and tool input land in Tasks 7+.
//!
//! [`Canvas`]: flintshot_core::render::Canvas
//! [`EditorShell`]: flintshot_core::shell::EditorShell

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]

mod app;
mod canvas;
mod magnifier;
mod shell;

pub use canvas::{EguiCanvas, EguiTexture};
pub use magnifier::{sample_zoomed_region, Magnifier};
pub use shell::EguiShell;
