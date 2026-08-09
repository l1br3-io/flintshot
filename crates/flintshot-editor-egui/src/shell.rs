//! egui implementation of the [`EditorShell`] trait.
//!
//! [`EditorShell`]: flintshot_core::shell::EditorShell

use std::cell::RefCell;
use std::rc::Rc;

use eframe::{AppCreator, NativeOptions};
use flintshot_core::{EditorOutcome, EditorShell, EditorState, PinConfig, Rect};

use crate::app::{eframe_err_to_flintshot, EditorApp, OutcomeSlot};
use crate::pin::PinApp;

/// Window title shown in the titlebar and used as the WM class hint.
/// Matches the Phase 1 editor-window spec (`"flintshot — annotate"` with
/// an em-dash).
const WINDOW_TITLE: &str = "flintshot — annotate";

/// Window title + WM class for a pin-to-desktop window. Distinct from
/// the editor so WMs / compositors can route / style pins separately.
const PIN_WINDOW_TITLE: &str = "flintshot — pin";

/// egui implementation of the [`EditorShell`] trait.
///
/// Phase 1 Task 5 implements `run_editor` as a single-window blocking
/// eframe app: it opens a window sized to the screenshot, runs the
/// shared `render()` path each frame, and returns [`EditorOutcome`]
/// when the window closes (`Esc` → `Cancelled`, X button → `Cancelled`
/// via the slot defaulting). Phase 3 Task 2 implements `run_pin` as a
/// borderless, always-on-top, transparent viewport driven by
/// [`PinApp`].
///
/// The resolved [`PinConfig`] for a pin session is passed via the
/// [`with_pin_config`](Self::with_pin_config) builder — the
/// [`EditorShell::run_pin`] trait signature stays `(image, rect)` (a
/// core boundary, GUI-toolkit-free), and the config rides along on the
/// shell as a side channel.
#[derive(Default)]
pub struct EguiShell {
    /// Resolved pin config applied to the next `run_pin` call.
    /// Defaults to `[pin]` config defaults; the CLI applies its
    /// `--opacity` / `--no-clipboard` overrides before calling.
    pin: PinConfig,
}

impl EguiShell {
    /// Construct the shell with default pin config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pin: PinConfig::default(),
        }
    }

    /// Override the pin-window config (opacity, copy-on-close) applied
    /// to the next [`EditorShell::run_pin`] call. The CLI merges its
    /// `--opacity` / `--no-clipboard` overrides into a [`PinConfig`]
    /// and passes it here; without this call, `[pin]` config defaults
    /// apply.
    #[must_use]
    pub fn with_pin_config(mut self, config: PinConfig) -> Self {
        self.pin = config;
        self
    }
}

impl EditorShell for EguiShell {
    fn run_editor(&self, state: EditorState) -> flintshot_core::Result<EditorOutcome> {
        run_editor_blocking(state)
    }

    fn run_pin(&self, image: image::RgbaImage, rect: Rect) -> flintshot_core::Result<()> {
        run_pin_blocking(image, rect, self.pin.clone())
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

/// Open a borderless, always-on-top, transparent pin window and block
/// until the user closes it (Esc / double-click / window close).
///
/// Mirrors [`run_editor_blocking`]'s single-threaded lifecycle but
/// needs no outcome slot — a pin returns `()` (the window simply
/// closes). Window geometry comes from `rect`: `rect.min` is the
/// outer position (physical px, passed straight to winit) and
/// `rect.max - rect.min` is the inner size (treated as logical px,
/// matching the editor's convention so egui's `pixels_per_point`
/// keeps the blit sharp on `HiDPI`).
fn run_pin_blocking(
    image: image::RgbaImage,
    rect: Rect,
    config: PinConfig,
) -> flintshot_core::Result<()> {
    let inner_size = egui::vec2(rect.width(), rect.height());
    let position = egui::pos2(rect.min.x, rect.min.y);

    let viewport = egui::ViewportBuilder::default()
        .with_title(PIN_WINDOW_TITLE)
        .with_decorations(false)
        .with_always_on_top()
        .with_transparent(true)
        .with_resizable(true)
        .with_inner_size(inner_size)
        .with_position(position);

    let options = NativeOptions {
        viewport,
        centered: false,
        ..Default::default()
    };

    let app_creator: AppCreator<'_> =
        Box::new(move |_cc| Ok(Box::new(PinApp::new(image, rect, config))));

    eframe::run_native(PIN_WINDOW_TITLE, options, app_creator)
        .map_err(|e| eframe_err_to_flintshot(&e))?;
    Ok(())
}
