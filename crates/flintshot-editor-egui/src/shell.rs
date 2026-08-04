//! egui implementation of the [`EditorShell`] trait.
//!
//! [`EditorShell`]: flintshot_core::shell::EditorShell

use std::cell::RefCell;
use std::rc::Rc;

use eframe::{AppCreator, NativeOptions};
use flintshot_core::{EditorOutcome, EditorShell, EditorState, Rect};

use crate::app::{eframe_err_to_flintshot, EditorApp, OutcomeSlot};
use crate::pin_window::{pin_viewport_builder, PinApp};

/// Window title shown in the titlebar and used as the WM class hint.
/// Matches the Phase 1 editor-window spec (`"flintshot — annotate"` with
/// an em-dash).
const WINDOW_TITLE: &str = "flintshot — annotate";

/// egui implementation of the [`EditorShell`] trait.
///
/// Phase 1 Task 5 implements `run_editor` as a single-window blocking
/// eframe app: it opens a window sized to the screenshot, runs the
/// shared `render()` path each frame, and returns [`EditorOutcome`]
/// when the window closes (`Esc` → `Cancelled`, X button → `Cancelled`
/// via the slot defaulting). `run_pin` opens a borderless
/// always-on-top window that blits the image and supports drag-to-move.
#[derive(Default)]
pub struct EguiShell;

impl EguiShell {
    /// Construct the shell.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl EditorShell for EguiShell {
    fn run_editor(&self, state: EditorState) -> flintshot_core::Result<EditorOutcome> {
        run_editor_blocking(state)
    }

    fn run_pin(&self, image: image::RgbaImage, rect: Rect) -> flintshot_core::Result<()> {
        run_pin_blocking(image, rect)
    }
}

/// Block on eframe's native event loop, returning the outcome the
/// editor produced before the window closed.
///
/// Single-threaded by construction: eframe's `run_native` owns the
/// calling thread until the window closes, so the [`OutcomeSlot`]
/// (`Rc<RefCell<…>>`) is shared only between this function and the
/// [`EditorApp`] running on the same thread.
fn run_editor_blocking(state: EditorState) -> flintshot_core::Result<EditorOutcome> {
    let (img_w, img_h) = state.image.dimensions();
    // egui's `ViewportBuilder::with_inner_size` takes logical px.
    // We pass the screenshot's physical dimensions directly; on a
    // HiDPI display the window manager scales the window to fit
    // the monitor and egui's `pixels_per_point` keeps the blit
    // 1:1 with the source texture (sharp screenshot pixels). Task
    // 14 may add explicit clamping when region selection makes
    // window-size precision matter more.
    #[allow(
        clippy::cast_precision_loss,
        reason = "image dims ≤ 2^24 are exact in f32"
    )]
    let inner_size = egui::vec2(img_w as f32, img_h as f32);

    let viewport = egui::ViewportBuilder::default()
        .with_title(WINDOW_TITLE)
        .with_inner_size(inner_size)
        .with_resizable(true);

    let options = NativeOptions {
        viewport,
        centered: true,
        ..Default::default()
    };

    let outcome: OutcomeSlot = Rc::new(RefCell::new(None));
    let outcome_for_app = Rc::clone(&outcome);

    let app_creator: AppCreator<'_> = Box::new(move |cc| {
        let app = EditorApp::new(state, &cc.egui_ctx, outcome_for_app);
        Ok(Box::new(app))
    });

    eframe::run_native(WINDOW_TITLE, options, app_creator)
        .map_err(|e| eframe_err_to_flintshot(&e))?;

    // The window closed. If Esc / Save / render-error set the
    // outcome, honour it (propagating any `Err(_)` via `?`);
    // otherwise (X button, Alt+F4, kill) default to Cancelled.
    let resolved = outcome
        .borrow_mut()
        .take()
        .unwrap_or(Ok(EditorOutcome::Cancelled));
    resolved
}

/// Block on eframe's native event loop for a pin window. The pin is
/// borderless, always-on-top, and sized to the image; it closes on
/// `Esc` or the WM close button and has no outcome to report (unlike
/// the editor, `run_pin` returns `Ok(())` once the window closes).
///
/// RATIONALE: ADR-0023 (drafted in Task 005) — a pin is its own
/// process, not a deferred viewport inside the editor's eframe
/// instance. egui viewports die with their root window; an
/// editor-spawned pin viewport would close when the editor closes,
/// which is the opposite of "pin to desktop." The editor and daemon
/// both spawn `flintshot pin <path>` as a detached child to keep the
/// pin alive after they exit.
fn run_pin_blocking(image: image::RgbaImage, rect: Rect) -> flintshot_core::Result<()> {
    let img_dims = image.dimensions();
    let viewport = pin_viewport_builder(img_dims, rect);

    let options = NativeOptions {
        viewport,
        ..Default::default()
    };

    let app_creator: AppCreator<'_> = Box::new(move |cc| {
        let app = PinApp::new(image, &cc.egui_ctx);
        Ok(Box::new(app))
    });

    eframe::run_native("flintshot-pin", options, app_creator)
        .map_err(|e| eframe_err_to_flintshot(&e))?;
    Ok(())
}
