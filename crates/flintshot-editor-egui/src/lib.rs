//! egui implementation of `flintshot-core`'s [`Canvas`] and
//! [`EditorShell`] traits.
//!
//! ## Layout
//!
//! - [`canvas`] — the `EguiCanvas` / `EguiTexture` types and the
//!   `impl Canvas` (background primitives live here).
//! - [`app`] — the internal `EditorApp` (`impl eframe::App`) that
//!   drives the per-frame render loop.
//! - `chrome` (private) — the Move-tool hover / grab outline shared
//!   by the editor window and the overlay (see `crate::chrome`).
//! - [`magnifier`] — the toolkit-owned pixel-zoom inset widget (no
//!   core abstraction; reads `EditorState::image` directly).
//! - [`overlay`] (private) — the internal `OverlayApp` (`impl
//!   eframe::App`) for the Phase 5 fullscreen frozen-capture shell;
//!   entered via [`EguiShell::run_overlay`].
//! - [`shell`] — the public `EguiShell` (`impl EditorShell`) that
//!   owns the eframe lifecycle and the outcome slot.
//! - `theme` (private) — the pop-up book design-token system (cream
//!   paper, white cutouts, forest accents) applied to every window.
//! - `panel` (private) — the docked right side panel (tools,
//!   contextual bay, bottom actions) drawn by `app`.
//! - `toolbar` (private) — the chip/pod/glyph primitives + bay
//!   rows shared by the panel and the overlay's floating rows.
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
mod chrome;
mod home;
mod magnifier;
mod overlay;
mod panel;
mod shell;
mod theme;
mod toolbar;
mod window;

pub use canvas::{CanvasView, EguiCanvas, EguiTexture};
pub use home::{HomeBridge, HomeDirective};
pub use magnifier::{sample_zoomed_region, Magnifier};
pub use shell::{CloseHandle, EditorPageBridge, EguiShell};
pub use window::{Route, RouteHost, RoutedEditor, WindowExit, WindowStart};
