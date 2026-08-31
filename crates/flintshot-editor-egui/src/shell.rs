//! egui implementation of the [`EditorShell`] trait.
//!
//! [`EditorShell`]: flintshot_core::shell::EditorShell

use std::cell::RefCell;
use std::rc::Rc;

use eframe::{AppCreator, NativeOptions};
use flintshot_core::{EditorOutcome, EditorShell, EditorState, PinConfig, Rect};

use crate::app::{eframe_err_to_flintshot, EditorApp, OutcomeSlot};
use crate::overlay::{OverlayApp, OVERLAY_TITLE};
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
/// shell as a side channel. The selection-first request rides the
/// same way via [`with_selection_first`](Self::with_selection_first).
#[derive(Default)]
pub struct EguiShell {
    /// Resolved pin config applied to the next `run_pin` call.
    /// Defaults to `[pin]` config defaults; the CLI applies its
    /// `--opacity` / `--no-clipboard` overrides before calling.
    pin: PinConfig,
    /// Whether the next `run_editor` call opens selection-first (the
    /// `region` capture landing experience: Select pre-armed, dim
    /// mask, hint line — see `EditorApp::selection_first`). Set by
    /// the app crate's region path only.
    selection_first: bool,
}

impl EguiShell {
    /// Construct the shell with default pin config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pin: PinConfig::default(),
            selection_first: false,
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

    /// Open the next [`EditorShell::run_editor`] session
    /// **selection-first**: `Tool::Select` pre-armed, the overlay's
    /// dim mask outside the live selection, and the hint line — the
    /// `region` capture landing experience. The phase ends
    /// permanently on the session's first crop.
    ///
    /// A concrete builder method mirroring
    /// [`with_pin_config`](Self::with_pin_config): no
    /// [`EditorShell`] trait change — the request rides the shell
    /// into the editor state construction (region-selection-first
    /// plan decision 2).
    #[must_use]
    pub fn with_selection_first(mut self) -> Self {
        self.selection_first = true;
        self
    }
}

impl EditorShell for EguiShell {
    fn run_editor(&self, state: EditorState) -> flintshot_core::Result<EditorOutcome> {
        run_editor_blocking(state, self.selection_first)
    }

    fn run_pin(&self, image: image::RgbaImage, rect: Rect) -> flintshot_core::Result<()> {
        run_pin_blocking(image, rect, self.pin.clone())
    }
}

impl EguiShell {
    /// Open the Phase 5 fullscreen frozen-capture overlay and block
    /// until the user closes it (`Esc` cancel / `Enter` accept),
    /// returning the outcome the session produced.
    ///
    /// A **concrete method**, not an [`EditorShell`] trait addition —
    /// the spec's §Boundaries explicitly allows the shell to grow
    /// overlay entries this way (mirroring how `run_pin`'s config
    /// rides the shell). The overlay is a flintshot-editor-egui
    /// feature, not a cross-toolkit contract.
    ///
    /// # Errors
    /// Surfaces [`crate::FlintshotError`] for window-system or
    /// render failures.
    pub fn run_overlay(&self, state: EditorState) -> flintshot_core::Result<EditorOutcome> {
        run_overlay_blocking(state)
    }
}

/// Block on eframe's native event loop, returning the outcome the
/// editor produced before the window closed.
///
/// Single-threaded by construction: eframe's `run_native` owns the
/// calling thread until the window closes, so the [`OutcomeSlot`]
/// (`Rc<RefCell<…>>`) is shared only between this function and the
/// [`EditorApp`] running on the same thread.
///
/// `selection_first` seeds the region-capture landing phase in
/// [`EditorApp::new`] (Select pre-arm + mask + hint; see the app
/// module for the phase lifecycle).
fn run_editor_blocking(
    state: EditorState,
    selection_first: bool,
) -> flintshot_core::Result<EditorOutcome> {
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
        let app = EditorApp::new(state, &cc.egui_ctx, outcome_for_app, selection_first);
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

/// Open the fullscreen frozen-capture overlay and block until the
/// user closes it.
///
/// Mirrors [`run_editor_blocking`]'s single-threaded lifecycle: the
/// [`OutcomeSlot`] is shared between this function and the
/// [`OverlayApp`] running on the same thread; a close without a
/// published outcome (WM kill, Alt+F4) defaults to
/// `Ok(EditorOutcome::Cancelled)`.
///
/// A copy-close hands the `arboard::Clipboard` handle to
/// [`linger_for_clipboard`] before returning — the X11
/// owner-clipboard rule adapted to the overlay's close-on-accept
/// semantics (plan decision 6).
fn run_overlay_blocking(state: EditorState) -> flintshot_core::Result<EditorOutcome> {
    let options = NativeOptions {
        viewport: crate::overlay::overlay_viewport(),
        ..Default::default()
    };

    let outcome: OutcomeSlot = Rc::new(RefCell::new(None));
    let outcome_for_app = Rc::clone(&outcome);
    let clipboard: crate::app::ClipboardSlot = Rc::new(RefCell::new(None));
    let clipboard_for_app = Rc::clone(&clipboard);

    let app_creator: AppCreator<'_> = Box::new(move |cc| {
        let app = OverlayApp::new(state, &cc.egui_ctx, outcome_for_app, clipboard_for_app);
        Ok(Box::new(app))
    });

    eframe::run_native(OVERLAY_TITLE, options, app_creator)
        .map_err(|e| eframe_err_to_flintshot(&e))?;

    let resolved = outcome
        .borrow_mut()
        .take()
        .unwrap_or(Ok(EditorOutcome::Cancelled));
    linger_for_clipboard(&clipboard, &resolved);
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

/// Whether the overlay session's outcome requires holding the
/// clipboard handle after the window closes. Only a *successful*
/// copy does: Save / Pin / Upload never touch the clipboard, and
/// `Cancelled` / `Err` must have no side effects at all.
///
/// Pure so the X11 linger policy is unit-testable without a display
/// (the sleep itself lives in [`linger_for_clipboard`]).
#[must_use]
pub(crate) fn clipboard_linger_needed(outcome: &flintshot_core::Result<EditorOutcome>) -> bool {
    matches!(outcome, Ok(EditorOutcome::CopiedToClipboard))
}

/// How long `run_overlay_blocking` holds the `arboard::Clipboard`
/// handle after the window closes, in seconds.
///
/// X11's clipboard is owner-based: the process that set the
/// selection must keep the handle (and itself) alive, or the data
/// is lost the moment it exits. The editor window solves this by
/// staying open after a copy; the overlay must close (Flameshot
/// Enter semantics), so the process instead lingers invisibly for
/// a bounded grace period — long enough to switch apps and paste,
/// short enough not to camp on the image's memory forever.
///
/// macOS / Windows clipboards are OS-owned (the data survives the
/// process), so no linger is needed there.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const CLIPBOARD_LINGER_SECS: u64 = 60;

/// macOS / Windows: the OS owns the clipboard contents, so there is
/// nothing to hold (see the X11 constant's doc for the full story).
#[cfg(any(target_os = "macos", target_os = "windows"))]
const CLIPBOARD_LINGER_SECS: u64 = 0;

/// Hold the overlay session's clipboard handle (if any) for the
/// linger grace period when the outcome was a copy — the X11
/// owner-clipboard rule adapted to the overlay's close-on-accept
/// semantics (plan decision 6). No-op for every other outcome and
/// when the platform's linger is 0.
///
/// Runs *after* the eframe loop returns, so the user never sees the
/// window linger — only the process does.
fn linger_for_clipboard(
    slot: &crate::app::ClipboardSlot,
    outcome: &flintshot_core::Result<EditorOutcome>,
) {
    if !clipboard_linger_needed(outcome) || CLIPBOARD_LINGER_SECS == 0 {
        return;
    }
    // The binding keeps the arboard handle (and the X11 selection
    // ownership it represents) alive through the sleep.
    if let Some(_handle) = slot.borrow_mut().take() {
        tracing::debug!(
            secs = CLIPBOARD_LINGER_SECS,
            "holding clipboard ownership after overlay close (X11 owner rule)"
        );
        std::thread::sleep(std::time::Duration::from_secs(CLIPBOARD_LINGER_SECS));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linger_required_only_for_successful_copy() {
        assert!(
            clipboard_linger_needed(&Ok(EditorOutcome::CopiedToClipboard)),
            "a copy needs the handle held"
        );
    }

    #[test]
    fn linger_not_required_for_other_outcomes() {
        for outcome in [
            Ok(EditorOutcome::Saved(std::path::PathBuf::from("/tmp/x.png"))),
            Ok(EditorOutcome::Pinned),
            Ok(EditorOutcome::Uploaded(String::new())),
            Ok(EditorOutcome::Cancelled),
            Err(flintshot_core::FlintshotError::Shell {
                detail: "boom".into(),
            }),
        ] {
            assert!(
                !clipboard_linger_needed(&outcome),
                "{outcome:?} must not linger"
            );
        }
    }

    // --- Selection-first builder (region-selection-first Task 001) ---

    #[test]
    fn default_shell_is_not_selection_first() {
        assert!(!EguiShell::new().selection_first);
        assert!(!EguiShell::default().selection_first);
    }

    #[test]
    fn with_selection_first_flags_the_next_run_editor() {
        let shell = EguiShell::new().with_selection_first();
        assert!(shell.selection_first);
    }

    #[test]
    fn with_selection_first_composes_with_pin_config() {
        // Both builder flags ride the shell independently — the
        // region path sets selection-first while the CLI merges pin
        // config (the `with_pin_config` precedent).
        let shell = EguiShell::new()
            .with_pin_config(PinConfig::default())
            .with_selection_first();
        assert!(shell.selection_first);
    }
}
