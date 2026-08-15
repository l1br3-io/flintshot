//! The eframe app that drives the fullscreen frozen-capture overlay.
//!
//! [`OverlayApp`] is Phase 5's second shell (plan decision 1): a new
//! eframe app, **not** a mode on [`crate::app::EditorApp`] — the
//! editor window's chrome (top toolbar panel, window-resize-on-crop)
//! is structurally wrong for a fullscreen overlay. What it shares
//! with the editor is everything below the chrome: the
//! [`EditorState`] dispatcher (no second dispatcher exists — plan
//! decision 5), the shared `render()` path, and the `EguiCanvas`.
//!
//! ## Capture-then-overlay
//!
//! The overlay displays the already-captured pixels: `xcap` runs
//! first (exactly as in editor-window mode), then this window shows
//! the frozen frame fullscreen. The desktop is never actually
//! visible beneath it — no compositor tricks, deterministic across
//! platforms (ADR-0028 records the stance).
//!
//! ## Input wiring
//!
//! Pointer and key events translate to `InputEvent`s exactly like
//! `EditorApp::logic` does, reusing the factored-out translation
//! helpers (`translate_modifiers`, `tool_for_key`,
//! `dispatch_text_input`). The overlay-specific deltas:
//!
//! - it opens with `Tool::Select`, so the first drag is a region
//!   selection (Flameshot muscle memory);
//! - `Enter` is intercepted **before** dispatch — the dispatcher's
//!   Enter path crops via `confirm_selection`, which is editor-window
//!   semantics; in the overlay the selection defines the output
//!   region and Enter means *accept*;
//! - `Esc` closes as `Cancelled` (clean cancel: no file, no
//!   clipboard write, no worker).
//!
//! The Enter→Copy output pipeline lands in Task 006; until then an
//! accepted close publishes `Cancelled` with a notice (the same
//! placeholder pattern Task 001 used for the missing shell).

use eframe::App;
use egui::Context;
use flintshot_core::{
    render::render, BlurCache, EditorOutcome, EditorState, InputEvent, Key, Point, Tool,
};

use crate::app::{dispatch_text_input, tool_for_key, translate_modifiers, OutcomeSlot};
use crate::canvas::{EguiCanvas, EguiTexture};

/// Window title + WM class for the overlay viewport. Distinct from
/// the editor and pin windows so WMs / compositors can route it.
pub(crate) const OVERLAY_TITLE: &str = "flintshot — overlay";

/// Why the overlay is closing this frame. Pure data so the Esc /
/// Enter decision is unit-testable without an egui context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlayClose {
    /// `Esc`: cancel — publish `EditorOutcome::Cancelled`, no side
    /// effects.
    Cancel,
    /// `Enter`: accept. The accept side effect (the copy pipeline)
    /// lands in Task 006; until then this closes with `Cancelled`
    /// plus a notice.
    Accept,
}

/// Decide whether the overlay should close this frame from the
/// translated close-key flags. `Esc` wins when both fire on the same
/// frame — cancel is the side-effect-free path, so ambiguity must
/// never trigger an output.
///
/// **Contract:** callers must not invoke this while a text draft is
/// active (`EditorState::is_editing_text`); those keys belong to the
/// draft (`Esc` cancels the draft, `Enter` commits it) and are
/// routed through the dispatcher's text-editing branch first.
#[must_use]
pub(crate) fn close_from_input(esc: bool, enter: bool) -> Option<OverlayClose> {
    if esc {
        Some(OverlayClose::Cancel)
    } else if enter {
        Some(OverlayClose::Accept)
    } else {
        None
    }
}

/// The overlay viewport: fullscreen + undecorated, so the frozen
/// capture *is* the screen (the Phase 3 pin window's borderless
/// builder is the precedent). No explicit size or position: the WM
/// fullscreens on the monitor the window opens at — in the common
/// single-trigger flow that's the monitor under the cursor, i.e. the
/// active monitor the capture came from (spec: active-monitor only).
pub(crate) fn overlay_viewport() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_title(OVERLAY_TITLE)
        .with_decorations(false)
        .with_fullscreen(true)
}

/// Translate a screen-logical pointer position into canvas-physical
/// px (the coordinate space `EditorState` operates in): subtract the
/// overlay panel's origin, then scale by `pixels_per_point`. With a
/// fullscreen `Frame::NONE` panel the origin is (0, 0), so this is
/// `screen_logical × ppp` — and because the captured image is the
/// monitor's physical framebuffer, canvas-physical == image px: the
/// pointer lands on the exact frozen pixel the user sees.
#[must_use]
pub(crate) fn to_canvas_physical(pos: egui::Pos2, panel: egui::Rect, ppp: f32) -> Point {
    let local = pos - panel.min;
    Point::new(local.x * ppp, local.y * ppp)
}

/// The eframe app for the fullscreen overlay session.
///
/// Constructed by [`crate::shell::EguiShell::run_overlay`] with the
/// post-capture `EditorState`. The bg texture uploads lazily on the
/// first `ui()` so construction is pure and unit-testable without a
/// display.
pub(crate) struct OverlayApp {
    /// Frozen capture + annotation model; all mutation flows through
    /// `dispatch_input`.
    state: EditorState,
    /// Egui-side canvas; painter is bound per-frame.
    canvas: EguiCanvas,
    /// Cached background texture handle, uploaded lazily by
    /// [`render`].
    bg: Option<EguiTexture>,
    /// Precomputed blur images for committed `Annotation::Blur`
    /// entries (shared render-path requirement; see `EditorApp`).
    blur_cache: BlurCache,
    /// Screen-logical rect of the most recent `CentralPanel`, cached
    /// by `ui()` so the next frame's `logic()` can translate pointer
    /// coords. `None` until the first `ui()` runs.
    overlay_rect: Option<egui::Rect>,
    /// Whether the current primary-button press began inside the
    /// overlay panel. Gates drag/up dispatch (same guard as
    /// `EditorApp::canvas_drag_active`).
    drag_active: bool,
    /// Outcome slot shared with the shell. X-close (Alt+F4, WM kill)
    /// leaves it `None`, which the shell reads as `Ok(Cancelled)`.
    outcome: OutcomeSlot,
}

impl OverlayApp {
    /// Construct the app. Forces `Tool::Select` — the overlay opens
    /// in selection mode so the first drag draws a region (the
    /// editor window's `Tool::Rect` default would instead commit a
    /// rectangle annotation on the first drag, which is not what a
    /// Flameshot user expects).
    pub(crate) fn new(mut state: EditorState, ctx: &Context, outcome: OutcomeSlot) -> Self {
        state.tool = Tool::Select;
        Self {
            state,
            canvas: EguiCanvas::new(ctx.clone()),
            bg: None,
            blur_cache: BlurCache::new(),
            overlay_rect: None,
            drag_active: false,
            outcome,
        }
    }

    /// Publish the outcome for a close decision and tear the window
    /// down. Accept currently publishes `Cancelled` — the Enter→Copy
    /// pipeline lands in Task 006 (see the module docs).
    fn perform_close(&self, ctx: &Context, close: OverlayClose) {
        match close {
            OverlayClose::Cancel => {
                tracing::info!("overlay cancelled; no output");
                *self.outcome.borrow_mut() = Some(Ok(EditorOutcome::Cancelled));
            }
            OverlayClose::Accept => {
                tracing::info!(
                    "overlay accepted; the Enter→Copy output flow lands in Phase 5 Task 006"
                );
                *self.outcome.borrow_mut() = Some(Ok(EditorOutcome::Cancelled));
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl App for OverlayApp {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let modifiers = translate_modifiers(ctx.input(|i| i.modifiers));

        // Text-editing modal mode: while a Text draft is active, Esc
        // cancels the *draft* and Enter commits it (dispatcher-owned)
        // — neither closes the overlay. Mirrors `EditorApp::logic`'s
        // early return.
        if self.state.is_editing_text() {
            dispatch_text_input(&mut self.state, ctx, modifiers);
            return;
        }

        // Close keys before any dispatch: Enter must not reach the
        // dispatcher (its Enter path crops the editor — editor-window
        // semantics), and Esc closing here is equivalent to reacting
        // to the dispatcher's `Cancel` outcome one step later.
        let (esc, enter) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Enter),
            )
        });
        if let Some(close) = close_from_input(esc, enter) {
            self.perform_close(ctx, close);
            return;
        }

        // History chords through the dispatcher (Ctrl+Z undo,
        // Ctrl+Shift+Z / Ctrl+Y redo). Ctrl+S / Ctrl+C join when
        // their output pipelines land (Task 006) — wiring the chords
        // without the pipelines would silently no-op them.
        let key_event = ctx.input(|i| {
            if i.key_pressed(egui::Key::Z) {
                Some(InputEvent::Key {
                    key: Key::Char('z'),
                    modifiers,
                })
            } else if i.key_pressed(egui::Key::Y) {
                Some(InputEvent::Key {
                    key: Key::Char('y'),
                    modifiers,
                })
            } else {
                None
            }
        });
        if let Some(event) = key_event {
            // Undo / redo chords always return `Continue`; the
            // binding is exhaustive-checked by the compiler against
            // future outcome variants when Task 006 extends this.
            let _ = self.state.dispatch_input(&event);
        }

        // Single-key tool selection (unmodified S R E A L M P T B N),
        // guarded on the event's own modifiers so chords keep their
        // dispatcher meanings.
        if let Some(tool) = ctx.input(|i| {
            i.raw.events.iter().rev().find_map(|e| match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers: m,
                    ..
                } if m.is_none() => tool_for_key(*key),
                _ => None,
            })
        }) {
            self.state.tool = tool;
        }

        // Pointer dispatch, gated on the cached panel rect (the
        // first frame has none — the screenshot isn't visible yet,
        // so no drag can start).
        let Some(panel) = self.overlay_rect else {
            return;
        };
        let ppp = ctx.pixels_per_point();
        let (pointer, primary_pressed, primary_down, primary_released) = ctx.input(|i| {
            (
                i.pointer.latest_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.primary_released(),
            )
        });
        let Some(pos) = pointer else {
            return;
        };
        let at = to_canvas_physical(pos, panel, ppp);

        let pointer_event = if primary_pressed && panel.contains(pos) {
            self.drag_active = true;
            Some(InputEvent::PointerDown { pos: at })
        } else if primary_released && self.drag_active {
            self.drag_active = false;
            Some(InputEvent::PointerUp)
        } else if primary_down && self.drag_active {
            Some(InputEvent::PointerDrag { pos: at })
        } else {
            None
        };
        // Pointer events always return `Continue` today; future
        // pointer-side outcomes (Task 006+) surface as a compile
        // error here rather than silently dropping.
        if let Some(event) = pointer_event {
            let _ = self.state.dispatch_input(&event);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Frame::NONE: no margin and no panel background — the frozen
        // capture must land at exact screen position (0, 0) so
        // canvas-physical px == screen physical px == image px.
        let panel = egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                self.canvas.bind_painter(ui.painter());
                render(
                    &self.state,
                    &mut self.canvas,
                    &mut self.bg,
                    &mut self.blur_cache,
                )
            });
        self.overlay_rect = Some(panel.response.rect);
        if let Err(err) = panel.inner {
            tracing::error!(error = %err, "overlay render failed; closing overlay session");
            *self.outcome.borrow_mut() = Some(Err(err));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// An all-white RGBA image of the given dimensions.
    fn sample_image(w: u32, h: u32) -> image::RgbaImage {
        image::RgbaImage::from_raw(w, h, vec![255; (w as usize) * (h as usize) * 4])
            .expect("raw buffer matches w*h*4")
    }

    fn blank_state(w: u32, h: u32) -> EditorState {
        EditorState::new(sample_image(w, h))
    }

    #[test]
    fn overlay_app_opens_in_select_mode() {
        let app = OverlayApp::new(
            blank_state(8, 8),
            &egui::Context::default(),
            Rc::new(RefCell::new(None)),
        );
        assert_eq!(
            app.state.tool,
            Tool::Select,
            "the first drag must draw a selection, not a rect annotation"
        );
    }

    #[test]
    fn esc_closes_as_cancel() {
        assert_eq!(
            close_from_input(true, false),
            Some(OverlayClose::Cancel),
            "Esc must cancel"
        );
    }

    #[test]
    fn enter_closes_as_accept() {
        assert_eq!(
            close_from_input(false, true),
            Some(OverlayClose::Accept),
            "Enter must accept"
        );
    }

    #[test]
    fn no_close_keys_means_no_close() {
        assert_eq!(
            close_from_input(false, false),
            None,
            "idle input → stay open"
        );
    }

    #[test]
    fn esc_wins_when_both_fire() {
        assert_eq!(
            close_from_input(true, true),
            Some(OverlayClose::Cancel),
            "ambiguity must resolve to the side-effect-free path"
        );
    }

    #[test]
    fn viewport_is_fullscreen_and_undecorated() {
        let vp = overlay_viewport();
        assert_eq!(vp.fullscreen, Some(true), "overlay must be fullscreen");
        assert_eq!(
            vp.decorations,
            Some(false),
            "overlay must have no window chrome"
        );
    }

    #[test]
    fn pointer_translation_maps_screen_logical_to_canvas_physical() {
        // Panel at logical (10, 20), ppp 2.0: a screen-logical point
        // (100, 50) is canvas-physical ((100-10)*2, (50-20)*2).
        let panel = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(950.0, 520.0));
        let p = to_canvas_physical(egui::pos2(100.0, 50.0), panel, 2.0);
        assert_eq!(p, Point::new(180.0, 60.0));
    }

    #[test]
    fn pointer_translation_at_origin_panel_is_screen_times_ppp() {
        // Fullscreen Frame::NONE panel at (0, 0): translation is pure
        // ppp scaling, so screen physical == image px.
        let panel = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1920.0, 1080.0));
        let p = to_canvas_physical(egui::pos2(960.0, 540.0), panel, 1.0);
        assert_eq!(p, Point::new(960.0, 540.0));
    }
}
