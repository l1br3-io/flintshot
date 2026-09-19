//! egui implementation of the [`EditorShell`] trait.
//!
//! [`EditorShell`]: flintshot_core::shell::EditorShell

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use eframe::{AppCreator, NativeOptions};
use flintshot_core::{EditorOutcome, EditorShell, EditorState};

use crate::app::{eframe_err_to_flintshot, EditorApp, OutcomeSlot};
use crate::home::{HomeBridge, HomeDirective};
use crate::overlay::{OverlayApp, OVERLAY_TITLE};
use crate::window::{Route, RouteHost, WindowExit, WindowStart};

/// Window title shown in the titlebar and used as the WM class hint.
/// Matches the Phase 1 editor-window spec (`"flintshot — annotate"` with
/// an em-dash). Shared with the window router's geometry table.
pub(crate) const WINDOW_TITLE: &str = "flintshot — annotate";

/// Window title + WM class for the home (app) window — the
/// capture-controls surface, distinct from the editor so WMs and
/// `.desktop` `StartupWMClass` can route it separately. Shared with
/// the window router's geometry table.
pub(crate) const HOME_WINDOW_TITLE: &str = "flintshot";

/// The editor page's default logical size (OQ-2's decision):
/// roomy enough for the toolbar plus annotation chrome, small
/// enough for any laptop panel. The image fits inside (Task 003's
/// transform) — the window no longer tracks image dimensions.
pub(crate) const EDITOR_PAGE_DEFAULT_SIZE: egui::Vec2 = egui::vec2(1024.0, 640.0);

/// Margin reserved against the monitor edge when clamping the
/// default editor-page size — approximates WM panels and
/// decorations (egui exposes the full monitor size, not the work
/// area).
const MONITOR_MARGIN: f32 = 48.0;

/// Floor for a clamped size — a degenerate monitor must not
/// collapse the editor window.
const SIZE_FLOOR: egui::Vec2 = egui::vec2(320.0, 240.0);

/// Clamp a desired window size to a monitor (pure seam behind the
/// editor page's open-time clamp). `None` monitor (headless / not
/// yet reported by eframe) keeps the desired size; a small monitor
/// pulls each axis in by [`MONITOR_MARGIN`], never below
/// [`SIZE_FLOOR`].
#[must_use]
pub(crate) fn clamp_default_size(desired: egui::Vec2, monitor: Option<egui::Vec2>) -> egui::Vec2 {
    let Some(monitor) = monitor else {
        return desired;
    };
    egui::vec2(
        desired
            .x
            .min((monitor.x - MONITOR_MARGIN).max(SIZE_FLOOR.x))
            .max(SIZE_FLOOR.x),
        desired
            .y
            .min((monitor.y - MONITOR_MARGIN).max(SIZE_FLOOR.y))
            .max(SIZE_FLOOR.y),
    )
}

/// Thread-safe handle for closing an egui window from **another
/// thread** — the graceful-close seam for superseded capture workers
/// (smart-window-hiding). The worker's IPC listener thread calls
/// [`CloseHandle::close`]; the window's app creator installs the real
/// context via [`CloseHandle::install`].
///
/// Thread-safety rationale: `egui::Context` is internally `Arc`'d and
/// documented as the supported cross-thread channel —
/// `send_viewport_cmd` + `request_repaint` from any thread are the
/// blessed way to drive a viewport without the UI thread's
/// cooperation. The `Mutex<Option<…>>` slot exists because the handle
/// is created (and may be closed) before the window exists: `close`
/// on an uninstalled handle is a no-op, and each recapture session's
/// `install` replaces the previous window's context.
#[derive(Clone, Default)]
pub struct CloseHandle {
    ctx: Arc<Mutex<Option<egui::Context>>>,
}

impl CloseHandle {
    /// A handle with no window behind it yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Install (or replace) the context of the window this handle
    /// closes. Called from the `app_creator` closure, the only place
    /// the real context exists.
    ///
    /// # Panics
    /// Only if the handle's mutex is poisoned (a previous close
    /// panicked mid-fire) — not an expected runtime condition.
    pub fn install(&self, ctx: &egui::Context) {
        *self.ctx.lock().expect("close-handle mutex poisoned") = Some(ctx.clone());
    }

    /// Ask the installed window to close. No-op before the first
    /// install. The window then exits through its normal path: the
    /// outcome slot stays `None` and resolves to
    /// [`EditorOutcome::Cancelled`] — no save, clean exit.
    ///
    /// # Panics
    /// Only if the handle's mutex is poisoned (a previous close
    /// panicked mid-fire) — not an expected runtime condition.
    pub fn close(&self) {
        let guard = self.ctx.lock().expect("close-handle mutex poisoned");
        if let Some(ctx) = guard.as_ref() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            ctx.request_repaint();
        }
    }
}

/// The editor page's only seam to the outside world — the single
/// window's analogue of [`HomeBridge`], implemented by the app crate;
/// every method runs on the UI thread. Side-effect outcomes (Save /
/// Copy / Upload) are reported here while the page stays open; the
/// terminal exit travels back as [`EguiShell::run_editor_page`]'s
/// return value instead.
pub trait EditorPageBridge {
    /// An outcome whose side effect just landed (file saved,
    /// clipboard copy, upload initiated): the implementation applies
    /// the daemon report (the worker path's equivalent) and any
    /// status feedback. The page stays open — no close follows.
    fn report_outcome(&mut self, outcome: &EditorOutcome);

    /// Replace the status line text (surfaced by the home page
    /// when the window routes there).
    fn set_status(&mut self, status: &str);

    /// Take one pending directive, oldest first (see
    /// [`HomeDirective`] — `Hide` means "external capture, close
    /// now"; the page drains the rest).
    fn poll_directive(&mut self) -> Option<HomeDirective>;

    /// The editor chrome's **Open** action (single-window): decode
    /// `path` and replace the page's image. Returns whether it
    /// routed (`true`: the window closes for the fresh editor epoch;
    /// `false`: a status line explains, the page keeps its image).
    /// The Capture action deliberately does **not** ride the bridge
    /// — it is the `+` chip's [`EditorOutcome::NewCapture`] exit,
    /// which carries the selection-first mode label.
    fn open_image(&mut self, path: std::path::PathBuf) -> bool;

    /// The editor chrome's **Paste** action: read the clipboard
    /// image and replace the page's image. Same contract as
    /// [`Self::open_image`].
    fn request_paste(&mut self) -> bool;
}

/// Shared handle to the boxed [`EditorPageBridge`]: the shell keeps
/// one side (post-close status on error), the running [`EditorApp`]
/// holds the other. Single-threaded like [`OutcomeSlot`] — eframe's
/// native loop owns the whole lifetime.
pub(crate) type PageBridgeSlot = Rc<RefCell<Box<dyn EditorPageBridge>>>;

/// egui implementation of the [`EditorShell`] trait.
///
/// Phase 1 Task 5 implements `run_editor` as a single-window blocking
/// eframe app: it opens a window sized to the screenshot, runs the
/// shared `render()` path each frame, and returns [`EditorOutcome`]
/// when the window closes (`Esc` → `Cancelled`, X button → `Cancelled`
/// via the slot defaulting).
///
/// The selection-first request rides the shell via
/// [`with_selection_first`](Self::with_selection_first).
#[derive(Default)]
pub struct EguiShell {
    /// Whether the next `run_editor` call opens selection-first (the
    /// `region` capture landing experience: Select pre-armed, dim
    /// mask, hint line — see `EditorApp::selection_first`). Set by
    /// the app crate's region path only.
    selection_first: bool,
    /// Cross-thread close handle installed into the next editor /
    /// overlay window (graceful supersede close). `None` keeps the
    /// unclosable-from-outside behavior (open path).
    close: Option<CloseHandle>,
}

impl EguiShell {
    /// Construct the shell.
    #[must_use]
    pub fn new() -> Self {
        Self {
            selection_first: false,
            close: None,
        }
    }

    /// Open the next [`EditorShell::run_editor`] session
    /// **selection-first**: `Tool::Select` pre-armed, the overlay's
    /// dim mask outside the live selection, and the hint line — the
    /// `region` capture landing experience. The phase ends
    /// permanently on the session's first crop.
    ///
    /// A concrete builder method: no
    /// [`EditorShell`] trait change — the request rides the shell
    /// into the editor state construction (region-selection-first
    /// plan decision 2).
    #[must_use]
    pub fn with_selection_first(mut self) -> Self {
        self.selection_first = true;
        self
    }

    /// Attach the cross-thread close handle installed into the next
    /// editor / overlay window: the worker's supersede listener closes
    /// the window through it (see [`CloseHandle`]). The
    /// [`EditorShell`] trait stays GUI-free.
    #[must_use]
    pub fn with_close_handle(mut self, handle: CloseHandle) -> Self {
        self.close = Some(handle);
        self
    }
}

impl EditorShell for EguiShell {
    fn run_editor(&self, state: EditorState) -> flintshot_core::Result<EditorOutcome> {
        run_editor_blocking(state, self.selection_first, self.close.as_ref())
    }
}

impl EguiShell {
    /// Open the Phase 5 fullscreen frozen-capture overlay and block
    /// until the user closes it (`Esc` cancel / `Enter` accept),
    /// returning the outcome the session produced.
    ///
    /// A **concrete method**, not an [`EditorShell`] trait addition —
    /// the spec's §Boundaries explicitly allows the shell to grow
    /// overlay entries this way. The overlay is a
    /// flintshot-editor-egui feature, not a cross-toolkit contract.
    ///
    /// # Errors
    /// Surfaces [`crate::FlintshotError`] for window-system or
    /// render failures.
    pub fn run_overlay(&self, state: EditorState) -> flintshot_core::Result<EditorOutcome> {
        run_overlay_blocking(state, self.close.as_ref())
    }

    /// Open the single window **router** and block until the window
    /// closes (one eframe lifetime per window *generation*; page
    /// transitions swap routes in place — see [`crate::window`]).
    /// Returns the [`WindowExit`] — the route at close (restore
    /// input) plus any editor outcome left unpublished (a capture
    /// intent whose close is landing). The window's outside world is
    /// entirely the bridges + route host — this crate stays
    /// IPC-free.
    ///
    /// # Errors
    /// Surfaces [`crate::FlintshotError`] for window-system or
    /// render failures.
    pub fn run_window(
        &self,
        start: WindowStart,
        home_bridge: Box<dyn HomeBridge>,
        page_bridge: Box<dyn EditorPageBridge>,
        host: Box<dyn RouteHost>,
        initial_status: String,
    ) -> flintshot_core::Result<WindowExit> {
        let geometry = crate::window::route_geometry(&start_route(&start));
        let viewport = egui::ViewportBuilder::default()
            .with_title(geometry.title)
            .with_inner_size(geometry.size)
            .with_resizable(geometry.resizable);

        let options = NativeOptions {
            viewport,
            centered: true,
            ..Default::default()
        };

        let route_slot: crate::window::RouteSlot = Rc::new(RefCell::new(start_route(&start)));
        let slot_for_app = Rc::clone(&route_slot);
        let editor_outcome: OutcomeSlot = Rc::new(RefCell::new(None));
        let outcome_for_app = Rc::clone(&editor_outcome);
        let page_bridge_slot: PageBridgeSlot = Rc::new(RefCell::new(page_bridge));
        let bridge_for_app = Rc::clone(&page_bridge_slot);
        let app_creator: AppCreator<'_> = Box::new(move |cc| {
            Ok(Box::new(crate::window::WindowApp::new(
                &cc.egui_ctx,
                start,
                home_bridge,
                bridge_for_app,
                host,
                initial_status,
                outcome_for_app,
                slot_for_app,
            )))
        });

        eframe::run_native(geometry.title, options, app_creator)
            .map_err(|e| eframe_err_to_flintshot(&e))?;
        let route = route_slot.borrow().clone();
        let editor_outcome = editor_outcome.borrow_mut().take();
        Ok(WindowExit {
            route,
            editor_outcome,
        })
    }
}

/// The opening [`Route`] for a [`WindowStart`] (the geometry and
/// slot seeding).
fn start_route(start: &WindowStart) -> Route {
    match start {
        WindowStart::Home => Route::Home,
        WindowStart::Editor(routed) => Route::Editor(Box::new((**routed).clone())),
        WindowStart::Loading => Route::Loading,
        WindowStart::Settings => Route::Settings,
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
/// module for the phase lifecycle). `close`, when present, is
/// installed into the window so the worker's supersede listener can
/// close it from another thread (see [`CloseHandle`]).
fn run_editor_blocking(
    state: EditorState,
    selection_first: bool,
    close: Option<&CloseHandle>,
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
        if let Some(handle) = close {
            handle.install(&cc.egui_ctx);
        }
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
/// semantics (plan decision 6). `close`, when present, is installed
/// into the window for the worker's supersede listener (see
/// [`CloseHandle`]).
fn run_overlay_blocking(
    state: EditorState,
    close: Option<&CloseHandle>,
) -> flintshot_core::Result<EditorOutcome> {
    let options = NativeOptions {
        viewport: crate::overlay::overlay_viewport(),
        ..Default::default()
    };

    let outcome: OutcomeSlot = Rc::new(RefCell::new(None));
    let outcome_for_app = Rc::clone(&outcome);
    let clipboard: crate::app::ClipboardSlot = Rc::new(RefCell::new(None));
    let clipboard_for_app = Rc::clone(&clipboard);

    let app_creator: AppCreator<'_> = Box::new(move |cc| {
        if let Some(handle) = close {
            handle.install(&cc.egui_ctx);
        }
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

/// Whether the overlay session's outcome requires holding the
/// clipboard handle after the window closes. Only a *successful*
/// copy does: Save / Upload never touch the clipboard, and
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

    // --- CloseHandle (smart-window-hiding Task 004) ---

    #[test]
    fn close_before_install_is_a_noop() {
        // The daemon's supersede broadcast can beat the window's
        // first frame; close() on an uninstalled handle must be a
        // silent no-op, not a panic.
        CloseHandle::new().close();
    }

    #[test]
    fn install_then_close_does_not_panic_on_a_default_context() {
        // A `Context::default()` stands in for the real eframe
        // context: the command queues, no display needed.
        let handle = CloseHandle::new();
        let ctx = egui::Context::default();
        handle.install(&ctx);
        handle.close();
    }

    #[test]
    fn close_handle_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CloseHandle>();
    }

    #[test]
    fn with_close_handle_stores_the_handle() {
        let shell = EguiShell::new().with_close_handle(CloseHandle::new());
        assert!(shell.close.is_some());
    }

    #[test]
    fn reinstall_replaces_the_context() {
        // Recapture chains reuse one handle across windows: each
        // session's install overwrites the previous context.
        let handle = CloseHandle::new();
        let first = egui::Context::default();
        handle.install(&first);
        let second = egui::Context::default();
        handle.install(&second);
        handle.close();
        let guard = handle.ctx.lock().expect("close-handle mutex poisoned");
        let installed = guard.as_ref().expect("an install is present");
        let installed_ptr = std::ptr::from_ref(installed);
        assert_ne!(
            installed_ptr,
            std::ptr::from_ref(&first),
            "the slot holds the most recent install"
        );
    }

    // --- Stable editor viewport (editor-capture-ux Task 004) ---

    #[test]
    fn editor_page_default_size_is_pinned() {
        // OQ-2's decision: ~1024×640 logical — big enough for the
        // toolbar + annotation chrome, small enough for any laptop.
        assert_eq!(EDITOR_PAGE_DEFAULT_SIZE, egui::vec2(1024.0, 640.0));
    }

    #[test]
    fn clamp_keeps_default_when_monitor_unknown() {
        assert_eq!(
            clamp_default_size(EDITOR_PAGE_DEFAULT_SIZE, None),
            EDITOR_PAGE_DEFAULT_SIZE
        );
    }

    #[test]
    fn clamp_keeps_default_on_roomy_monitors() {
        assert_eq!(
            clamp_default_size(EDITOR_PAGE_DEFAULT_SIZE, Some(egui::vec2(1920.0, 1080.0))),
            EDITOR_PAGE_DEFAULT_SIZE
        );
        // Exactly at the margin boundary: still fits.
        assert_eq!(
            clamp_default_size(EDITOR_PAGE_DEFAULT_SIZE, Some(egui::vec2(1072.0, 688.0))),
            EDITOR_PAGE_DEFAULT_SIZE
        );
    }

    #[test]
    fn clamp_shrinks_on_small_monitors_per_axis() {
        // A tiny panel (VM / embedded): both axes pull in, keeping
        // the margin for WM chrome.
        assert_eq!(
            clamp_default_size(EDITOR_PAGE_DEFAULT_SIZE, Some(egui::vec2(800.0, 600.0))),
            egui::vec2(752.0, 552.0)
        );
        // Only one axis too small: the other keeps the default.
        assert_eq!(
            clamp_default_size(EDITOR_PAGE_DEFAULT_SIZE, Some(egui::vec2(4000.0, 600.0))),
            egui::vec2(1024.0, 552.0)
        );
    }

    #[test]
    fn clamp_never_collapses_below_the_floor() {
        // A degenerate monitor still leaves a usable editor window.
        assert_eq!(
            clamp_default_size(EDITOR_PAGE_DEFAULT_SIZE, Some(egui::vec2(300.0, 200.0))),
            egui::vec2(320.0, 240.0)
        );
    }
}
