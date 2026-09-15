//! egui implementation of the [`EditorShell`] trait.
//!
//! [`EditorShell`]: flintshot_core::shell::EditorShell

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use eframe::{AppCreator, NativeOptions};
use flintshot_core::{EditorOutcome, EditorShell, EditorState, NewCaptureMode};

use crate::app::{eframe_err_to_flintshot, EditorApp, OutcomeSlot};
use crate::home::{HomeApp, HomeBridge, HomeDirective};
use crate::overlay::{OverlayApp, OVERLAY_TITLE};

/// Window title shown in the titlebar and used as the WM class hint.
/// Matches the Phase 1 editor-window spec (`"flintshot — annotate"` with
/// an em-dash).
const WINDOW_TITLE: &str = "flintshot — annotate";

/// Window title + WM class for the home (app) window — the
/// capture-controls surface, distinct from the editor so WMs and
/// `.desktop` `StartupWMClass` can route it separately.
const HOME_WINDOW_TITLE: &str = "flintshot";

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

/// How the editor page epoch ended — the single window's router (the
/// app crate's outer loop) reads this to pick the next page.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditorPageExit {
    /// Back to the three-button home (Esc, window close, or a
    /// terminal error — the empty state).
    Home,
    /// The user asked for a new capture from inside the editor
    /// (`+` chip / Ctrl+N); `mode` carries the selection-first label
    /// the recapture flow keys on.
    Capture(NewCaptureMode),
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

    /// Replace the app's status line text (surfaced by the home page
    /// when the window routes there).
    fn set_status(&mut self, status: &str);

    /// Take one pending directive, oldest first (see
    /// [`HomeDirective`] — `Hide` means "external capture, close
    /// now"; the page drains the rest).
    fn poll_directive(&mut self) -> Option<HomeDirective>;
}

/// Shared handle to the boxed [`EditorPageBridge`]: the shell keeps
/// one side (post-close status on error), the running [`EditorApp`]
/// holds the other. Single-threaded like [`OutcomeSlot`] — eframe's
/// native loop owns the whole lifetime.
pub(crate) type PageBridgeSlot = Rc<RefCell<Box<dyn EditorPageBridge>>>;

/// Resolve the page exit from the editor epoch's terminal outcome
/// (the slot the closed window left). Pure, headless-testable seam
/// of [`EguiShell::run_editor_page`].
#[must_use]
pub(crate) fn page_exit_for(resolved: &flintshot_core::Result<EditorOutcome>) -> EditorPageExit {
    match resolved {
        Ok(EditorOutcome::NewCapture { mode }) => EditorPageExit::Capture(*mode),
        // Esc / X (Cancelled), side-effect closes, and terminal
        // errors all route home — the empty state; the error case
        // adds a bridge status first (see `settle_page_exit`).
        Ok(_) | Err(_) => EditorPageExit::Home,
    }
}

/// Resolve the exit and surface a terminal error through the bridge's
/// status line (spec: a render error closes with `Home` plus a
/// status — the returning home page shows why the editor died).
pub(crate) fn settle_page_exit(
    resolved: &flintshot_core::Result<EditorOutcome>,
    bridge: &PageBridgeSlot,
) -> EditorPageExit {
    if let Err(e) = resolved {
        bridge
            .borrow_mut()
            .set_status(&format!("editor closed on error: {e}"));
    }
    page_exit_for(resolved)
}

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

    /// Open the home (app) window and block until the user closes
    /// it. The window's outside world is entirely `bridge` — this
    /// crate stays IPC-free (see [`HomeBridge`]).
    ///
    /// Concrete method, mirroring [`Self::run_overlay`]'s precedent.
    ///
    /// # Errors
    /// Surfaces [`crate::FlintshotError`] for window-system or
    /// render failures.
    pub fn run_home(
        &self,
        bridge: Box<dyn HomeBridge>,
        initial_status: String,
    ) -> flintshot_core::Result<()> {
        let viewport = egui::ViewportBuilder::default()
            .with_title(HOME_WINDOW_TITLE)
            .with_inner_size(egui::vec2(360.0, 240.0))
            .with_resizable(false);

        let options = NativeOptions {
            viewport,
            centered: true,
            ..Default::default()
        };

        let app_creator: AppCreator<'_> =
            Box::new(move |_cc| Ok(Box::new(HomeApp::new(bridge, initial_status))));

        eframe::run_native(HOME_WINDOW_TITLE, options, app_creator)
            .map_err(|e| eframe_err_to_flintshot(&e))?;
        Ok(())
    }

    /// Open the editor as a **page** of the single window: the same
    /// image-sized viewport and title as [`EditorShell::run_editor`],
    /// but the single-window outcome lifecycle — Save / Copy /
    /// Upload apply their side effect and the page stays open
    /// (reported through the [`EditorPageBridge`]), `Esc` exits to
    /// [`EditorPageExit::Home`], and `NewCapture` exits with
    /// [`EditorPageExit::Capture`] so the router can run the grab
    /// flow. Blocks until the page epoch ends.
    ///
    /// # Errors
    /// Surfaces [`crate::FlintshotError`] for window-system or
    /// render failures (render failures exit `Home` with a bridge
    /// status; the `Err` return is for the loop itself failing).
    pub fn run_editor_page(
        &self,
        state: EditorState,
        bridge: Box<dyn EditorPageBridge>,
    ) -> flintshot_core::Result<EditorPageExit> {
        let (img_w, img_h) = state.image.dimensions();
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
        let page_bridge: PageBridgeSlot = Rc::new(RefCell::new(bridge));
        let bridge_for_app = Rc::clone(&page_bridge);

        let app_creator: AppCreator<'_> = Box::new(move |cc| {
            let app = EditorApp::new_page(
                state,
                &cc.egui_ctx,
                outcome_for_app,
                self.selection_first,
                bridge_for_app,
            );
            Ok(Box::new(app))
        });

        eframe::run_native(WINDOW_TITLE, options, app_creator)
            .map_err(|e| eframe_err_to_flintshot(&e))?;

        // Same defaulting as the worker path: a close with nothing
        // published (X button, Alt+F4) is a cancel → Home.
        let resolved = outcome
            .borrow_mut()
            .take()
            .unwrap_or(Ok(EditorOutcome::Cancelled));
        Ok(settle_page_exit(&resolved, &page_bridge))
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

    // --- Editor page exit (single-window Task 003) ---

    use crate::home::HomeDirective;
    use flintshot_core::NewCaptureMode;

    #[derive(Default)]
    struct PageRecorder {
        reported: Vec<EditorOutcome>,
        statuses: Vec<String>,
    }

    #[derive(Default, Clone)]
    struct RecordingPageBridge {
        rec: std::sync::Arc<std::sync::Mutex<PageRecorder>>,
    }

    impl EditorPageBridge for RecordingPageBridge {
        fn report_outcome(&mut self, outcome: &EditorOutcome) {
            self.rec
                .lock()
                .expect("recorder mutex")
                .reported
                .push(outcome.clone());
        }

        fn set_status(&mut self, status: &str) {
            self.rec
                .lock()
                .expect("recorder mutex")
                .statuses
                .push(status.into());
        }

        fn poll_directive(&mut self) -> Option<HomeDirective> {
            None
        }
    }

    #[test]
    fn page_exit_maps_every_terminal_outcome() {
        // NewCapture carries its mode label (the recapture loop's
        // selection-first decision keys on it); every other terminal
        // outcome — Cancelled (Esc / X), Saved, CopiedToClipboard,
        // Uploaded — exits to Home.
        assert_eq!(
            page_exit_for(&Ok(EditorOutcome::NewCapture {
                mode: NewCaptureMode::Region
            })),
            EditorPageExit::Capture(NewCaptureMode::Region)
        );
        assert_eq!(
            page_exit_for(&Ok(EditorOutcome::NewCapture {
                mode: NewCaptureMode::Fullscreen
            })),
            EditorPageExit::Capture(NewCaptureMode::Fullscreen)
        );
        assert_eq!(
            page_exit_for(&Ok(EditorOutcome::Cancelled)),
            EditorPageExit::Home
        );
        assert_eq!(
            page_exit_for(&Ok(EditorOutcome::CopiedToClipboard)),
            EditorPageExit::Home
        );
        assert_eq!(
            page_exit_for(&Ok(EditorOutcome::Saved(std::path::PathBuf::from(
                "/tmp/x.png"
            )))),
            EditorPageExit::Home
        );
        assert_eq!(
            page_exit_for(&Ok(EditorOutcome::Uploaded(String::new()))),
            EditorPageExit::Home
        );
    }

    #[test]
    fn page_render_error_exits_home_with_a_bridge_status() {
        // Spec: a render error closes with Home plus a status through
        // the bridge, so the returning home page can show why.
        let bridge = RecordingPageBridge::default();
        let rec = bridge.rec.clone();
        let slot: PageBridgeSlot = Rc::new(RefCell::new(Box::new(bridge)));
        let exit = settle_page_exit(
            &Err(flintshot_core::FlintshotError::Shell {
                detail: "render blew up".into(),
            }),
            &slot,
        );
        assert_eq!(exit, EditorPageExit::Home);
        let guard = rec.lock().expect("recorder mutex");
        assert_eq!(
            guard.statuses.len(),
            1,
            "the error surfaced through the bridge's status line"
        );
        assert!(
            guard.statuses[0].contains("render blew up"),
            "the status carries the error detail: {}",
            guard.statuses[0]
        );
    }

    #[test]
    fn page_clean_exit_sets_no_status() {
        // Only errors speak through the status line — a plain Esc
        // home must not overwrite the app's standing status.
        let bridge = RecordingPageBridge::default();
        let rec = bridge.rec.clone();
        let slot: PageBridgeSlot = Rc::new(RefCell::new(Box::new(bridge)));
        let exit = settle_page_exit(&Ok(EditorOutcome::Cancelled), &slot);
        assert_eq!(exit, EditorPageExit::Home);
        assert!(
            rec.lock().expect("recorder mutex").statuses.is_empty(),
            "no status on a clean exit"
        );
    }
}
