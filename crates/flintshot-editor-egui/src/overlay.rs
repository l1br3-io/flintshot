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
//! ## Selection chrome (Task 003)
//!
//! The overlay layers Flameshot selection semantics over the
//! dispatcher's selection geometry: a dim mask outside the active
//! region, 8 grab handles, move-by-drag inside the region, a live
//! `W × H` badge, and the magnifier on hover. The dispatcher keeps
//! owning *fresh* selection drags; handle and interior presses with
//! `Tool::Select` active are intercepted by the shell and run as
//! [`Gesture`]s that mutate `EditorState::selection` (a `pub` field)
//! through the pure helpers in this module. This is the effort's one
//! documented, tested exception to "all mutation flows through
//! `dispatch_input`": the dispatcher's selection transitions model
//! fresh anchor-to-corner drags only — they have no concept of grab
//! points — and extending `InputEvent` for chrome-only gestures
//! would leak overlay semantics into every shell (spec §Success
//! criteria allows exactly this shape of exception).
//!
//! The Enter→Copy output pipeline lands in Task 006; until then an
//! accepted close publishes `Cancelled` with a notice (the same
//! placeholder pattern Task 001 used for the missing shell).

use eframe::App;
use egui::Context;
use flintshot_core::{
    render::render, BlurCache, EditorOutcome, EditorState, InputEvent, Key, Point, Rect, Tool,
};

use crate::app::{dispatch_text_input, tool_for_key, translate_modifiers, OutcomeSlot};
use crate::canvas::{EguiCanvas, EguiTexture};
use crate::magnifier::Magnifier;

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

// --- Selection chrome (Task 003) ---
//
// Pure geometry + decision helpers backing the overlay's Flameshot
// selection semantics. All are headless-testable; egui value types
// (`Pos2`, `Align2`, `CursorIcon`) appear only as inert data, never
// via a live context. Canvas-space helpers operate in the same
// physical-px space `EditorState` uses; DPI-dependent sizes (handle
// edge, hit radius) are parameters the caller scales by
// `pixels_per_point` so affordances stay logical-px sized.

// physical-px space `EditorState` uses; DPI-dependent sizes (handle
// edge, hit radius) are parameters the caller scales by
// `pixels_per_point` so affordances stay logical-px sized.

/// Dim-mask alpha over the area outside the active region. ~50%
/// matches Flameshot's shade: dark enough to read the undimmed
/// region as "selected", light enough that unselected content stays
/// recognizable for a fresh drag.
const MASK_ALPHA: u8 = 128;

/// Handle square edge, logical px (scaled by `ppp` into physical).
/// 10 px matches Flameshot's grab squares: big enough to hit, small
/// enough to stay out of the annotated content.
const HANDLE_EDGE_LOGICAL: f32 = 10.0;

/// Handle hit radius, logical px — slightly larger than half the
/// square so near-misses still grab (Fitts-ish forgiveness).
const HANDLE_HIT_LOGICAL: f32 = 12.0;

/// Gap between the selection edge and the W×H badge, logical px.
const BADGE_GAP: f32 = 6.0;

/// Badge font size, logical px. Monospace so live width changes
/// don't jitter the label horizontally.
const BADGE_FONT_SIZE: f32 = 12.0;

/// One of the 8 selection grab points: 4 corners + 4 edge midpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Handle {
    /// Top-left corner.
    Nw,
    /// Top edge midpoint.
    N,
    /// Top-right corner.
    Ne,
    /// Right edge midpoint.
    E,
    /// Bottom-right corner.
    Se,
    /// Bottom edge midpoint.
    S,
    /// Bottom-left corner.
    Sw,
    /// Left edge midpoint.
    W,
}

/// The 8 handles in [`Handle`] declaration order — the order
/// [`handle_centers`] returns centers in, so `zip` pairs them.
pub(crate) const ALL_HANDLES: [Handle; 8] = [
    Handle::Nw,
    Handle::N,
    Handle::Ne,
    Handle::E,
    Handle::Se,
    Handle::S,
    Handle::Sw,
    Handle::W,
];

/// Filter a raw `state.selection` down to the form the chrome cares
/// about: `Some(normalized)` only when the rect is non-degenerate
/// (both normalized dimensions ≥ 1 px, matching the render path's
/// selection-outline guard and `EditorState::crop`'s no-op bound).
#[must_use]
pub(crate) fn usable_selection(sel: Option<Rect>) -> Option<Rect> {
    sel.map(Rect::normalized)
        .filter(|n| n.width() >= 1.0 && n.height() >= 1.0)
}

/// Centers of the 8 grab handles for a selection, in [`ALL_HANDLES`]
/// order. `sel` is normalized first so an in-flight un-normalized
/// drag still lands handles at the visual corners.
#[must_use]
pub(crate) fn handle_centers(sel: Rect) -> [Point; 8] {
    let r = sel.normalized();
    let mid_x = r.min.x.midpoint(r.max.x);
    let mid_y = r.min.y.midpoint(r.max.y);
    [
        Point::new(r.min.x, r.min.y), // Nw
        Point::new(mid_x, r.min.y),   // N
        Point::new(r.max.x, r.min.y), // Ne
        Point::new(r.max.x, mid_y),   // E
        Point::new(r.max.x, r.max.y), // Se
        Point::new(mid_x, r.max.y),   // S
        Point::new(r.min.x, r.max.y), // Sw
        Point::new(r.min.x, mid_y),   // W
    ]
}

/// The handle whose center is nearest `at`, if within `radius`
/// (physical px — callers pass `HANDLE_HIT_LOGICAL * ppp`).
/// Nearest-wins resolves overlaps on tiny selections.
#[must_use]
pub(crate) fn handle_at(sel: Rect, at: Point, radius: f32) -> Option<Handle> {
    let centers = handle_centers(sel);
    let mut best: Option<(f32, Handle)> = None;
    for (handle, center) in std::iter::zip(ALL_HANDLES, centers) {
        let dx = at.x - center.x;
        let dy = at.y - center.y;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq <= radius * radius && best.is_none_or(|(best_sq, _)| dist_sq < best_sq) {
            best = Some((dist_sq, handle));
        }
    }
    best.map(|(_, handle)| handle)
}

/// Resize `sel` so the grabbed corner/edge follows `at` while the
/// opposite corner(s) stay anchored. The result is intentionally
/// un-normalized during the drag (mirroring
/// `EditorState::update_selection`); normalize on release.
#[must_use]
pub(crate) fn resize_from(handle: Handle, sel: Rect, at: Point) -> Rect {
    let mut r = sel;
    match handle {
        Handle::Nw => r.min = at,
        Handle::Se => r.max = at,
        Handle::Ne => {
            r.min.y = at.y;
            r.max.x = at.x;
        }
        Handle::Sw => {
            r.min.x = at.x;
            r.max.y = at.y;
        }
        Handle::N => r.min.y = at.y,
        Handle::S => r.max.y = at.y,
        Handle::E => r.max.x = at.x,
        Handle::W => r.min.x = at.x,
    }
    r
}

/// Translate `sel` so the grab point follows `at` (`offset` = grab
/// position − `sel.min`, captured at press), clamped so the region
/// stays inside `bounds` on both axes.
#[must_use]
pub(crate) fn moved_rect(sel: Rect, at: Point, offset: Point, bounds: Rect) -> Rect {
    let (w, h) = (sel.width(), sel.height());
    let max_min_x = (bounds.max.x - w).max(bounds.min.x);
    let max_min_y = (bounds.max.y - h).max(bounds.min.y);
    let min_x = (at.x - offset.x).clamp(bounds.min.x, max_min_x);
    let min_y = (at.y - offset.y).clamp(bounds.min.y, max_min_y);
    Rect::from_xywh(min_x, min_y, w, h)
}

/// An overlay-owned selection manipulation in progress. Presses with
/// `Tool::Select` active resolve to one of the gesture variants
/// *before* dispatcher dispatch (see `gesture_for_press`); while a
/// gesture is active, pointer drags mutate `state.selection` via the
/// pure helpers above instead of dispatching `PointerDrag`s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Gesture {
    /// No manipulation in progress — pointer events flow to the
    /// dispatcher normally.
    None,
    /// Drag inside the region moves it; `offset` = press position −
    /// selection `min`, preserved so the grab point tracks the cursor.
    Move {
        /// Press position minus selection top-left, physical px.
        offset: Point,
    },
    /// Drag on a grab handle resizes the region around the opposite
    /// corner(s).
    Resize {
        /// Which handle was grabbed.
        handle: Handle,
    },
}

/// Decide what a primary press inside the overlay does when a usable
/// selection exists: grabs a handle (`Resize`), grabs the interior
/// (`Move`), or returns `None` to fall through to the dispatcher
/// (fresh selection / annotation). Interception is Select-tool-only:
/// with an annotation tool active, presses must draw, not manipulate
/// the region.
#[must_use]
pub(crate) fn gesture_for_press(
    sel: Option<Rect>,
    at: Point,
    tool: Tool,
    hit_radius: f32,
) -> Option<Gesture> {
    if tool != Tool::Select {
        return None;
    }
    let sel = usable_selection(sel)?;
    if let Some(handle) = handle_at(sel, at, hit_radius) {
        return Some(Gesture::Resize { handle });
    }
    if sel.contains(at) {
        return Some(Gesture::Move {
            offset: Point::new(at.x - sel.min.x, at.y - sel.min.y),
        });
    }
    None
}

/// Badge label for a selection: `"{w} × {h}"` in whole physical px.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    reason = "selection dims are screen-sized; the rounded value fits i32 with room to spare"
)]
pub(crate) fn badge_text(w: f32, h: f32) -> String {
    format!("{} × {}", w.round() as i32, h.round() as i32)
}

/// Where the W×H badge anchors against the (logical-space)
/// selection: above its top-left when there's panel room, flipped
/// below the bottom edge otherwise. Returns the anchor point plus
/// the `Align2` that places the badge relative to it.
#[must_use]
pub(crate) fn badge_anchor(sel: egui::Rect, panel: egui::Rect) -> (egui::Pos2, egui::Align2) {
    if sel.min.y - BADGE_GAP >= panel.min.y {
        (
            egui::pos2(sel.min.x, sel.min.y - BADGE_GAP),
            egui::Align2::LEFT_BOTTOM,
        )
    } else {
        (
            egui::pos2(sel.min.x, sel.max.y + BADGE_GAP),
            egui::Align2::LEFT_TOP,
        )
    }
}

/// Cursor icon for hovering / dragging a handle.
#[must_use]
pub(crate) fn cursor_for_handle(handle: Handle) -> egui::CursorIcon {
    match handle {
        Handle::Nw | Handle::Se => egui::CursorIcon::ResizeNwSe,
        Handle::Ne | Handle::Sw => egui::CursorIcon::ResizeNeSw,
        Handle::N | Handle::S => egui::CursorIcon::ResizeVertical,
        Handle::E | Handle::W => egui::CursorIcon::ResizeHorizontal,
    }
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
    /// Overlay-owned selection manipulation in progress (Task 003's
    /// documented exception to all-mutation-through-dispatch: the
    /// dispatcher's selection transitions model fresh
    /// anchor-to-corner drags only and have no grab points). While a
    /// gesture is active, drags mutate `state.selection` through the
    /// pure chrome helpers instead of dispatching pointer events.
    gesture: Gesture,
    /// Pixel-zoom magnifier (hover-triggered in the overlay, per the
    /// spec — no modifier hold). Reused verbatim from the editor
    /// window; reads `state.image` directly.
    magnifier: Magnifier,
    /// Outcome slot shared with the shell. X-close (Alt+F4, WM kill)
    /// leaves it `None`, which the shell reads as `Ok(Cancelled)`.
    outcome: OutcomeSlot,
}

impl Gesture {
    /// Whether no gesture is active (`Gesture` has no `None` variant
    /// to keep the field non-optional and the match arms total).
    fn is_none(self) -> bool {
        matches!(self, Gesture::None)
    }
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
            gesture: Gesture::None,
            magnifier: Magnifier::new(),
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

    /// Pointer dispatch plus the Task 003 chrome interaction: gesture
    /// interception, gesture-driven selection mutation, and the hover
    /// cursor shape. Split out of `logic()` so each concern stays
    /// readable; runs only when a cached panel rect exists.
    fn pointer_and_chrome(&mut self, ctx: &Context, panel: egui::Rect) {
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

        // Chrome interception (Task 003): with Select active and a
        // usable region present, a press on a handle or inside the
        // region starts an overlay-owned gesture instead of a
        // dispatcher `PointerDown` (which would restart the selection
        // from a point — `begin_selection`). While the gesture runs,
        // drags mutate `state.selection` directly (the documented
        // exception; see `OverlayApp::gesture`) and release
        // normalizes, mirroring the dispatcher's `finish_selection`.
        let image_bounds = {
            let (w, h) = self.state.image.dimensions();
            #[allow(
                clippy::cast_precision_loss,
                reason = "image dims ≤ 2^24 are exact in f32"
            )]
            Rect::from_xywh(0.0, 0.0, w as f32, h as f32)
        };
        let clamped = Point::new(
            at.x.clamp(image_bounds.min.x, image_bounds.max.x),
            at.y.clamp(image_bounds.min.y, image_bounds.max.y),
        );
        let hit_radius = HANDLE_HIT_LOGICAL * ppp;

        let pointer_event = if primary_pressed && panel.contains(pos) {
            self.drag_active = true;
            match gesture_for_press(self.state.selection, at, self.state.tool, hit_radius) {
                Some(gesture) => {
                    self.gesture = gesture;
                    None
                }
                None => Some(InputEvent::PointerDown { pos: at }),
            }
        } else if primary_released && self.drag_active {
            self.drag_active = false;
            if self.gesture.is_none() {
                Some(InputEvent::PointerUp)
            } else {
                // Normalize the dragged rect so `min` is top-left —
                // the same contract the dispatcher's release path
                // (`finish_selection`) upholds for fresh drags.
                self.state.selection = self.state.selection.map(Rect::normalized);
                self.gesture = Gesture::None;
                None
            }
        } else if primary_down && self.drag_active {
            match self.gesture {
                Gesture::None => Some(InputEvent::PointerDrag { pos: at }),
                Gesture::Move { offset } => {
                    if let Some(sel) = usable_selection(self.state.selection) {
                        self.state.selection = Some(moved_rect(sel, clamped, offset, image_bounds));
                    }
                    None
                }
                Gesture::Resize { handle } => {
                    if let Some(sel) = usable_selection(self.state.selection) {
                        self.state.selection = Some(resize_from(handle, sel, clamped));
                    }
                    None
                }
            }
        } else {
            None
        };
        // Pointer events always return `Continue` today; future
        // pointer-side outcomes (Task 006+) surface as a compile
        // error here rather than silently dropping.
        if let Some(event) = pointer_event {
            let _ = self.state.dispatch_input(&event);
        }

        // Cursor shape: reflects the active gesture while dragging,
        // otherwise the hover target. Select-only outside gestures —
        // annotation tools keep the default cursor, matching the
        // editor window.
        if panel.contains(pos) {
            let icon = match self.gesture {
                Gesture::Resize { handle } => Some(cursor_for_handle(handle)),
                Gesture::Move { .. } => Some(egui::CursorIcon::Move),
                Gesture::None => self.hover_cursor(at, hit_radius),
            };
            if let Some(icon) = icon {
                ctx.set_cursor_icon(icon);
            }
        }
    }

    /// Cursor icon for the current hover position when no gesture is
    /// active: a per-handle resize cursor over a handle, `Move`
    /// inside the region, `Crosshair` elsewhere while the Select tool
    /// is active. `None` leaves the default cursor (annotation
    /// tools).
    fn hover_cursor(&self, at: Point, hit_radius: f32) -> Option<egui::CursorIcon> {
        if self.state.tool != Tool::Select {
            return None;
        }
        let sel = usable_selection(self.state.selection);
        if let Some(sel) = sel {
            if let Some(handle) = handle_at(sel, at, hit_radius) {
                return Some(cursor_for_handle(handle));
            }
            if sel.contains(at) {
                return Some(egui::CursorIcon::Move);
            }
        }
        Some(egui::CursorIcon::Crosshair)
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

        // Pointer dispatch + selection-chrome interaction, gated on
        // the cached panel rect (the first frame has none — the
        // screenshot isn't visible yet, so no drag can start).
        if let Some(panel) = self.overlay_rect {
            self.pointer_and_chrome(ctx, panel);
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
            return;
        }

        // Selection chrome (Task 003), painted over the shared render
        // output in screen-logical space. Order: mask → handles →
        // badge → magnifier, so later elements stay legible over the
        // dimmed area.
        let panel_rect = panel.response.rect;
        let painter = ui.painter().clone();
        let ctx = ui.ctx().clone();
        let ppp = ctx.pixels_per_point();
        let sel_phys = usable_selection(self.state.selection);
        let to_logical =
            |p: Point| egui::pos2(panel_rect.min.x + p.x / ppp, panel_rect.min.y + p.y / ppp);

        // Dim mask: four rects framing the region (the full panel
        // when nothing is selected — everything is "unselected").
        let mask = egui::Color32::from_black_alpha(MASK_ALPHA);
        if let Some(s) = sel_phys {
            let min = to_logical(s.min);
            let max = to_logical(s.max);
            let panel_max = panel_rect.max;
            let panel_min = panel_rect.min;
            let top = egui::Rect::from_two_pos(panel_min, egui::pos2(panel_max.x, min.y));
            let bottom = egui::Rect::from_two_pos(egui::pos2(panel_min.x, max.y), panel_max);
            let left =
                egui::Rect::from_two_pos(egui::pos2(panel_min.x, min.y), egui::pos2(min.x, max.y));
            let right =
                egui::Rect::from_two_pos(egui::pos2(max.x, min.y), egui::pos2(panel_max.x, max.y));
            for band in [top, bottom, left, right] {
                painter.rect_filled(band, egui::CornerRadius::ZERO, mask);
            }
        } else {
            painter.rect_filled(panel_rect, egui::CornerRadius::ZERO, mask);
        }

        // Grab handles: visible only while they're actually
        // interactive — Select tool active and no fresh
        // dispatcher-driven selection drag in progress (during a
        // fresh drag the midpoint handles would jitter as the rect
        // grows; Flameshot hides them until release too).
        if self.state.tool == Tool::Select && !(self.drag_active && self.gesture.is_none()) {
            if let Some(s) = sel_phys {
                let edge = HANDLE_EDGE_LOGICAL;
                for center in handle_centers(s) {
                    let rect =
                        egui::Rect::from_center_size(to_logical(center), egui::vec2(edge, edge));
                    painter.rect_filled(rect, egui::CornerRadius::same(2), egui::Color32::WHITE);
                    painter.rect_stroke(
                        rect,
                        egui::CornerRadius::same(2),
                        egui::Stroke::new(1.0, egui::Color32::from_black_alpha(200)),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }

        // Live W×H badge (whole physical px), clamped into the panel.
        if let Some(s) = sel_phys {
            let sel_logical = egui::Rect::from_two_pos(to_logical(s.min), to_logical(s.max));
            let text = badge_text(s.width(), s.height());
            let galley = painter.layout_no_wrap(
                text,
                egui::FontId::monospace(BADGE_FONT_SIZE),
                egui::Color32::WHITE,
            );
            let (anchor, align) = badge_anchor(sel_logical, panel_rect);
            let mut rect = align.anchor_size(anchor, galley.size());
            rect.min.x = rect.min.x.clamp(
                panel_rect.min.x,
                (panel_rect.max.x - rect.width()).max(panel_rect.min.x),
            );
            rect.min.y = rect.min.y.clamp(
                panel_rect.min.y,
                (panel_rect.max.y - rect.height()).max(panel_rect.min.y),
            );
            painter.rect_filled(
                rect.expand(4.0),
                egui::CornerRadius::same(3),
                egui::Color32::from_black_alpha(200),
            );
            painter.galley(rect.min, galley, egui::Color32::WHITE);
        }

        // Magnifier on hover (overlay UX: no modifier hold, per the
        // spec). Suppressed during text editing, same as the editor
        // window's loupe.
        if !self.state.is_editing_text() {
            if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
                if panel_rect.contains(pos) {
                    let cursor_phys = to_canvas_physical(pos, panel_rect, ppp);
                    self.magnifier.show(
                        &painter,
                        &ctx,
                        &self.state.image,
                        cursor_phys,
                        panel_rect,
                        ppp,
                    );
                }
            }
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

    // --- Selection chrome (Task 003) ---

    /// A 200×100 selection at (100, 100) — the shared fixture for the
    /// chrome geometry tests.
    fn sel() -> Rect {
        Rect::from_xywh(100.0, 100.0, 200.0, 100.0)
    }

    #[test]
    fn usable_selection_drops_none_and_degenerate() {
        assert_eq!(usable_selection(None), None, "no selection");
        assert_eq!(
            usable_selection(Some(Rect::from_xywh(10.0, 10.0, 0.5, 50.0))),
            None,
            "sub-1px width is degenerate"
        );
        assert_eq!(
            usable_selection(Some(Rect::from_xywh(10.0, 10.0, 50.0, 0.0))),
            None,
            "zero height is degenerate"
        );
    }

    #[test]
    fn usable_selection_normalizes() {
        // Flipped corners (drag up-left): min/max swap.
        assert_eq!(
            usable_selection(Some(Rect::new(
                Point::new(300.0, 200.0),
                Point::new(100.0, 100.0)
            ))),
            Some(Rect::from_xywh(100.0, 100.0, 200.0, 100.0))
        );
    }

    #[test]
    fn handle_centers_hit_corners_and_midpoints() {
        let centers = handle_centers(sel());
        let expected = [
            Point::new(100.0, 100.0), // Nw
            Point::new(200.0, 100.0), // N
            Point::new(300.0, 100.0), // Ne
            Point::new(300.0, 150.0), // E
            Point::new(300.0, 200.0), // Se
            Point::new(200.0, 200.0), // S
            Point::new(100.0, 200.0), // Sw
            Point::new(100.0, 150.0), // W
        ];
        for (i, (got, want)) in centers.iter().zip(&expected).enumerate() {
            assert_eq!(got, want, "handle {i} center");
        }
    }

    #[test]
    fn handle_at_finds_nearest_within_radius() {
        let s = sel();
        assert_eq!(
            handle_at(s, Point::new(100.0, 100.0), 10.0),
            Some(Handle::Nw)
        );
        // A few px off the corner still grabs it.
        assert_eq!(
            handle_at(s, Point::new(106.0, 106.0), 10.0),
            Some(Handle::Nw)
        );
        assert_eq!(
            handle_at(s, Point::new(300.0, 200.0), 10.0),
            Some(Handle::Se)
        );
        // Interior point is near no handle.
        assert_eq!(handle_at(s, Point::new(200.0, 150.0), 10.0), None);
        // Well outside the radius.
        assert_eq!(handle_at(s, Point::new(120.0, 120.0), 8.0), None);
    }

    #[test]
    fn handle_at_picks_nearest_on_tiny_selection() {
        // 4×4 selection: all 8 handles overlap; the query point at
        // (12, 10) must pick the closest (Ne at (12, 10)... on a
        // 8..12 rect: Nw (8,8), N (10,8), Ne (12,8), E (12,10),
        // Se (12,12), S (10,12), Sw (8,12), W (8,10). (12,10) ties
        // E exactly — assert E.
        let s = Rect::from_xywh(8.0, 8.0, 4.0, 4.0);
        assert_eq!(handle_at(s, Point::new(12.0, 10.0), 20.0), Some(Handle::E));
    }

    #[test]
    fn handle_at_normalizes_flipped_rect() {
        let flipped = Rect::new(Point::new(300.0, 200.0), Point::new(100.0, 100.0));
        assert_eq!(
            handle_at(flipped, Point::new(100.0, 100.0), 10.0),
            Some(Handle::Nw),
            "flipped drag rect still hit-tests at visual corners"
        );
    }

    #[test]
    fn resize_from_corner_drags_anchor_opposite() {
        let s = sel();
        // Grabbing Nw and dragging to (50, 60): Se stays anchored.
        assert_eq!(
            resize_from(Handle::Nw, s, Point::new(50.0, 60.0)),
            Rect::new(Point::new(50.0, 60.0), Point::new(300.0, 200.0))
        );
        // Se: Nw stays anchored.
        assert_eq!(
            resize_from(Handle::Se, s, Point::new(340.0, 260.0)),
            Rect::new(Point::new(100.0, 100.0), Point::new(340.0, 260.0))
        );
        // Ne: only the top-right follows; bottom-left anchored.
        assert_eq!(
            resize_from(Handle::Ne, s, Point::new(350.0, 70.0)),
            Rect::new(Point::new(100.0, 70.0), Point::new(350.0, 200.0))
        );
        // Sw: bottom-left follows; top-right anchored.
        assert_eq!(
            resize_from(Handle::Sw, s, Point::new(80.0, 240.0)),
            Rect::new(Point::new(80.0, 100.0), Point::new(300.0, 240.0))
        );
    }

    #[test]
    fn resize_from_edges_move_one_axis() {
        let s = sel();
        // N: only min.y follows.
        assert_eq!(
            resize_from(Handle::N, s, Point::new(999.0, 80.0)),
            Rect::new(Point::new(100.0, 80.0), Point::new(300.0, 200.0)),
            "N drag must ignore the x coordinate"
        );
        assert_eq!(
            resize_from(Handle::S, s, Point::new(999.0, 260.0)),
            Rect::new(Point::new(100.0, 100.0), Point::new(300.0, 260.0)),
            "S drag must ignore the x coordinate"
        );
        assert_eq!(
            resize_from(Handle::W, s, Point::new(60.0, 999.0)),
            Rect::new(Point::new(60.0, 100.0), Point::new(300.0, 200.0)),
            "W drag must ignore the y coordinate"
        );
        assert_eq!(
            resize_from(Handle::E, s, Point::new(350.0, 999.0)),
            Rect::new(Point::new(100.0, 100.0), Point::new(350.0, 200.0)),
            "E drag must ignore the y coordinate"
        );
    }

    #[test]
    fn moved_rect_translates_preserving_size() {
        let s = sel();
        let moved = moved_rect(
            s,
            Point::new(200.0, 200.0),
            Point::new(10.0, 20.0),
            image_bounds(),
        );
        assert_eq!(moved, Rect::from_xywh(190.0, 180.0, 200.0, 100.0));
    }

    #[test]
    fn moved_rect_clamps_to_bounds() {
        let s = sel();
        let bounds = image_bounds();
        // Dragging past the top-left corner clamps at (0, 0).
        assert_eq!(
            moved_rect(s, Point::new(0.0, 0.0), Point::new(50.0, 50.0), bounds),
            Rect::from_xywh(0.0, 0.0, 200.0, 100.0),
            "region must not leave the screen at the top-left"
        );
        // Dragging past the bottom-right clamps so max == bounds.max.
        let moved = moved_rect(s, Point::new(1900.0, 1000.0), Point::new(0.0, 0.0), bounds);
        assert_eq!(moved.max, Point::new(1920.0, 1080.0));
        assert_eq!(
            (moved.width(), moved.height()),
            (200.0, 100.0),
            "clamping must preserve size"
        );
    }

    #[test]
    fn gesture_for_press_grabs_handle_first() {
        assert_eq!(
            gesture_for_press(Some(sel()), Point::new(100.0, 100.0), Tool::Select, 10.0),
            Some(Gesture::Resize { handle: Handle::Nw }),
            "a press on a handle resizes, even though it's inside the rect too"
        );
    }

    #[test]
    fn gesture_for_press_inside_is_move_with_offset() {
        assert_eq!(
            gesture_for_press(Some(sel()), Point::new(150.0, 140.0), Tool::Select, 10.0),
            Some(Gesture::Move {
                offset: Point::new(50.0, 40.0)
            }),
            "interior press grabs at cursor minus selection origin"
        );
    }

    #[test]
    fn gesture_for_press_outside_falls_through() {
        assert_eq!(
            gesture_for_press(Some(sel()), Point::new(50.0, 50.0), Tool::Select, 10.0),
            None,
            "exterior press starts a fresh selection via the dispatcher"
        );
    }

    #[test]
    fn gesture_for_press_ignores_non_select_tools() {
        assert_eq!(
            gesture_for_press(Some(sel()), Point::new(150.0, 140.0), Tool::Rect, 10.0),
            None,
            "annotation tools must draw through the dispatcher, not move the region"
        );
        assert_eq!(
            gesture_for_press(Some(sel()), Point::new(100.0, 100.0), Tool::Arrow, 10.0),
            None,
            "even a handle press must not grab when a drawing tool is active"
        );
    }

    #[test]
    fn gesture_for_press_without_selection_falls_through() {
        assert_eq!(
            gesture_for_press(None, Point::new(150.0, 140.0), Tool::Select, 10.0),
            None
        );
        // Degenerate selections are treated as absent.
        assert_eq!(
            gesture_for_press(
                Some(Rect::from_xywh(10.0, 10.0, 0.5, 0.5)),
                Point::new(10.0, 10.0),
                Tool::Select,
                10.0
            ),
            None
        );
    }

    #[test]
    fn badge_text_rounds_to_whole_pixels() {
        assert_eq!(badge_text(800.0, 600.0), "800 × 600");
        assert_eq!(badge_text(123.6, 45.4), "124 × 45");
    }

    #[test]
    fn badge_anchor_sits_above_when_there_is_room() {
        let panel = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1920.0, 1080.0));
        let sel_l = egui::Rect::from_min_size(egui::pos2(100.0, 200.0), egui::vec2(300.0, 200.0));
        let (anchor, align) = badge_anchor(sel_l, panel);
        assert_eq!(anchor, egui::pos2(100.0, 200.0 - BADGE_GAP));
        assert_eq!(
            align,
            egui::Align2::LEFT_BOTTOM,
            "badge bottom sits at the gap offset"
        );
    }

    #[test]
    fn badge_anchor_flips_below_at_panel_top() {
        let panel = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1920.0, 1080.0));
        // Selection hugging the top edge: no room above.
        let sel_l = egui::Rect::from_min_size(egui::pos2(100.0, 4.0), egui::vec2(300.0, 200.0));
        let (anchor, align) = badge_anchor(sel_l, panel);
        assert_eq!(anchor, egui::pos2(100.0, 204.0 + BADGE_GAP));
        assert_eq!(align, egui::Align2::LEFT_TOP);
    }

    #[test]
    fn cursor_for_handle_maps_directions() {
        assert_eq!(cursor_for_handle(Handle::Nw), egui::CursorIcon::ResizeNwSe);
        assert_eq!(cursor_for_handle(Handle::Se), egui::CursorIcon::ResizeNwSe);
        assert_eq!(cursor_for_handle(Handle::Ne), egui::CursorIcon::ResizeNeSw);
        assert_eq!(cursor_for_handle(Handle::Sw), egui::CursorIcon::ResizeNeSw);
        assert_eq!(
            cursor_for_handle(Handle::N),
            egui::CursorIcon::ResizeVertical
        );
        assert_eq!(
            cursor_for_handle(Handle::S),
            egui::CursorIcon::ResizeVertical
        );
        assert_eq!(
            cursor_for_handle(Handle::E),
            egui::CursorIcon::ResizeHorizontal
        );
        assert_eq!(
            cursor_for_handle(Handle::W),
            egui::CursorIcon::ResizeHorizontal
        );
    }

    /// 1920×1080 image-bounds fixture for the move-clamp tests.
    fn image_bounds() -> Rect {
        Rect::from_xywh(0.0, 0.0, 1920.0, 1080.0)
    }
}
