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
//! - it opens with `Tool::Crop`, so the first drag is a region
//!   selection (Flameshot muscle memory);
//! - `Enter` is intercepted **before** dispatch — the dispatcher's
//!   Enter path crops via `confirm_selection`, which is editor-window
//!   semantics; in the overlay the selection defines the output
//!   region and Enter means *accept*;
//! - `Esc` closes as `Cancelled` (clean cancel: no file, no
//!   clipboard write, no worker) — except when a Hand-tool selection
//!   exists, when the first `Esc` only deselects (annotation-delete
//!   spec; the layering lives at the `close_from_input` call site);
//! - `Del` / `Backspace` forward to the dispatcher, which deletes the
//!   persistent selection (no-modifier chord; text drafts own
//!   Backspace via the modal branch above).
//!
//! ## Output flows (Task 006, plan decision 6)
//!
//! **Enter = Copy** (also the check / Copy chips and `Ctrl+C`, which the
//! dispatcher recognizes and hands back as a `Copy` outcome): the
//! region is composed selection-aware onto the clipboard and the
//! overlay closes, publishing `CopiedToClipboard`. Save (`rfd`
//! dialog) and Upload (temp PNG + detached worker) reuse the
//! editor window's pipelines with its error policy verbatim — only
//! Save differs in *crop semantics*, composing selection-aware
//! because in the overlay the region is what ships (the editor
//! window's full-image Save contract is untouched).
//!
//! The X11 clipboard is **owner-based**: the process that set the
//! selection must stay alive or the data dies with it. The editor
//! window solves this by staying open after a copy; the overlay must
//! *close* on accept, so instead the `arboard::Clipboard` handle is
//! handed to the shell before close and the process lingers
//! invisibly past window close holding it (see
//! `run_overlay_blocking`).
//!
//! ## Selection chrome (Task 003)
//!
//! The overlay layers Flameshot selection semantics over the
//! dispatcher's selection geometry: a dim mask outside the active
//! region, 8 grab handles, move-by-drag inside the region, a live
//! `W × H` badge, and the magnifier on hover. The dispatcher keeps
//! owning *fresh* selection drags; handle and interior presses with
//! `Tool::Crop` active are intercepted by the shell and run as
//! [`Gesture`]s that mutate `EditorState::selection` (a `pub` field)
//! through the pure helpers in this module. This is the effort's one
//! documented, tested exception to "all mutation flows through
//! `dispatch_input`": the dispatcher's selection transitions model
//! fresh anchor-to-corner drags only — they have no concept of grab
//! points — and extending `InputEvent` for chrome-only gestures
//! would leak overlay semantics into every shell (spec §Success
//! criteria allows exactly this shape of exception).
//!
//! ## Floating toolbar (Task 004)
//!
//! A compact rail + accept row in a foreground `egui::Area` anchored
//! to the selection — below its bottom-left, flipped above when the
//! space below is short, clamped into the viewport (plan decision
//! 4). Row 1 walks the shared tool pods (selection · shapes ·
//! freehand · labels · redact, no capture `+`) plus a history pod
//! (undo / redo / delete), each its own island; row 2 reads as two
//! islands: the check / cross pair and the outputs (Save / Upload,
//! and the forest Copy primary — Copy is Enter-equivalent: the spec
//! pins Enter = Copy). The toolbar shows only while a usable
//! selection exists and no fresh region drag runs, and swallows
//! pointer presses before chrome interception so chip clicks never
//! reach the dispatcher; the hover cursor and magnifier stand down
//! over it. The output chips run the Task 006 pipelines described
//! above.
//!
//! ## Annotation confinement (Task 005)
//!
//! All tools draw *inside* the region through the unchanged
//! dispatcher — no second input path. Two shell-level rules produce
//! the Flameshot behaviour:
//!
//! - **Input gating** (`annotation_press_allowed`): with an
//!   annotation tool active, a primary press dispatches only when it
//!   lands inside the usable selection; elsewhere it is swallowed
//!   whole (no `drag_active`, so its drags / release never reach the
//!   dispatcher either). `Tool::Crop` and `Tool::Hand` always flow
//!   — the dispatcher owns fresh region drags and annotation grabs,
//!   and the chrome owns the gestures. Drags
//!   that start inside and wander outside keep dispatching raw
//!   positions, so tool semantics stay byte-identical to the editor
//!   window.
//! - **Render clipping** (`EguiCanvas::set_clip`): the shared
//!   `render()` pass runs with the canvas clipped to the region, so a
//!   stroke dragged past the edge simply stops painting there — the
//!   same cut the region-cropped output applies, keeping WYSIWYG. The
//!   background blit is exempt so the frozen screen outside the
//!   region stays visible under the dim mask.
//!
//! ## Repaint discipline (Task 008)
//!
//! The overlay repaints **on demand only**: at rest (pointer parked,
//! no keys) the process burns no CPU on rendering. This rides on
//! eframe 0.35's reactive event loop (`ControlFlow::Wait` at idle —
//! repaints happen on input events, explicit `request_repaint`s, and
//! egui's own machinery), plus two rules this module upholds:
//!
//! 1. **No unconditional repaint requests.** Every state change the
//!    overlay renders originates from an input event that already
//!    triggered the frame (pointer/key → dispatcher or chrome
//!    gesture); construction requests nothing (pinned by test). The
//!    floating toolbar's appearance transitions are covered by egui's
//!    `Area` machinery — a new or re-shown area runs an invisible
//!    sizing pass and egui itself requests the follow-up repaint.
//! 2. **A repaint is requested exactly when shell-side ui state needs
//!    one more frame to settle.** The one such site: the toolbar
//!    anchor runs a frame behind the measured size
//!    ([`toolbar_anchor`]), so a size change while visible requests a
//!    single convergence repaint ([`OverlayApp::show_toolbar`]) —
//!    transition-only, never per frame.

use eframe::App;
use egui::Context;
use flintshot_core::{
    render::render, BlurCache, DispatchOutcome, EditorOutcome, EditorState, InputEvent, Key,
    Modifiers, Point, Rect, Tool,
};

use crate::app::{
    compose_and_write_upload, copy_to_clipboard, dispatch_text_input, save_region_to_disk,
    spawn_upload_worker, tool_for_key, translate_modifiers, ClipboardSlot, OutcomeSlot,
};
use crate::canvas::{EguiCanvas, EguiTexture};
use crate::magnifier::Magnifier;
use crate::panel::{self, ToolbarAction};
use crate::rail;

/// Window title + WM class for the overlay viewport. Distinct from
/// the editor window so WMs / compositors can route it.
pub(crate) const OVERLAY_TITLE: &str = "flintshot — overlay";

/// Why the overlay is closing this frame. Pure data so the Esc /
/// Enter decision is unit-testable without an egui context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlayClose {
    /// `Esc`: cancel — publish `EditorOutcome::Cancelled`, no side
    /// effects.
    Cancel,
    /// `Enter` / check chip / Copy chip / `Ctrl+C`: accept — copy the region
    /// to the clipboard, publish `CopiedToClipboard`, close (the
    /// Task 006 Enter=Copy pipeline).
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
/// capture *is* the screen. No explicit size or position: the WM
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

/// Dim-mask alpha over the area outside the active region. ~50%
/// matches Flameshot's shade: dark enough to read the undimmed
/// region as "selected", light enough that unselected content stays
/// recognizable for a fresh drag.
const MASK_ALPHA: u8 = 128;

/// The dim-mask fill: black at [`MASK_ALPHA`]. Exposed as a
/// constructor (not a second constant) so the overlay and the editor
/// window's selection-first phase cannot drift apart in shade.
#[must_use]
pub(crate) fn mask_color() -> egui::Color32 {
    crate::theme::palette::readability_black(MASK_ALPHA)
}

/// The dim-mask coverage of `panel` minus `hole`: the four bands
/// framing the selection (`[top, bottom, left, right]`), or the whole
/// panel when `hole` is `None` (everything is "unselected"). Bands
/// collapse to zero width/height when the hole hugs a panel edge —
/// the painter skips degenerate rects, so no special-casing is
/// needed at the call sites. A `hole` larger than the panel is
/// clamped by `Rect::from_two_pos`'s ordering; callers pass a
/// selection inside the canvas anyway.
///
/// Shared by the overlay and the editor window's selection-first
/// phase (region-selection-first plan decision 3): one mask
/// implementation, one [`MASK_ALPHA`], two shells.
#[must_use]
pub(crate) fn mask_rects(panel: egui::Rect, hole: Option<egui::Rect>) -> Vec<egui::Rect> {
    let Some(hole) = hole else {
        return vec![panel];
    };
    let top = egui::Rect::from_two_pos(panel.min, egui::pos2(panel.max.x, hole.min.y));
    let bottom = egui::Rect::from_two_pos(egui::pos2(panel.min.x, hole.max.y), panel.max);
    let left = egui::Rect::from_two_pos(
        egui::pos2(panel.min.x, hole.min.y),
        egui::pos2(hole.min.x, hole.max.y),
    );
    let right = egui::Rect::from_two_pos(
        egui::pos2(hole.max.x, hole.min.y),
        egui::pos2(panel.max.x, hole.max.y),
    );
    vec![top, bottom, left, right]
}

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

/// Gap between the selection edge and the floating toolbar, logical
/// px. Matches the badge's offset so the two chrome elements share a
/// rhythm.
const TOOLBAR_GAP: f32 = 6.0;

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

/// Filter a raw `state.crop_region` down to the form the chrome cares
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
/// `EditorState::update_crop_region`); normalize on release.
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
/// `Tool::Crop` active resolve to one of the gesture variants
/// *before* dispatcher dispatch (see `gesture_for_press`); while a
/// gesture is active, pointer drags mutate `state.crop_region` via the
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
/// (fresh region / annotation). Interception is Crop-tool-only:
/// with an annotation tool active, presses must draw, not manipulate
/// the region; with Hand active, presses must reach the dispatcher so
/// it can grab a committed annotation.
#[must_use]
pub(crate) fn gesture_for_press(
    sel: Option<Rect>,
    at: Point,
    tool: Tool,
    hit_radius: f32,
) -> Option<Gesture> {
    if tool != Tool::Crop {
        return None;
    }
    let sel = usable_selection(sel);
    if let Some(handle) = sel.and_then(|s| handle_at(s, at, hit_radius)) {
        return Some(Gesture::Resize { handle });
    }
    if let Some(sel) = sel.filter(|s| s.contains(at)) {
        return Some(Gesture::Move {
            offset: Point::new(at.x - sel.min.x, at.y - sel.min.y),
        });
    }
    None
}

/// Whether a primary press may start drawing at `at`: annotations
/// are confined to the usable selection (Flameshot semantics — the
/// region defines both what is visible and what ships in the output,
/// so anything drawn outside it would be invisible-overflow).
/// `Tool::Crop` presses always flow: the dispatcher owns fresh
/// region drags and the chrome owns the region gestures, so
/// Crop must keep working outside (and without) a region.
/// `Tool::Hand` presses likewise always flow — the dispatcher owns
/// annotation grabs, and annotations only exist inside the region.
#[must_use]
pub(crate) fn annotation_press_allowed(tool: Tool, sel: Option<Rect>, at: Point) -> bool {
    if matches!(tool, Tool::Crop | Tool::Hand) {
        return true;
    }
    usable_selection(sel).is_some_and(|s| s.contains(at))
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

/// Where the floating toolbar's top-left anchors against the
/// (logical-space) selection: below its bottom-left when there's
/// panel room, flipped above the top edge otherwise, then clamped
/// horizontally into the panel. `size` is the toolbar's last
/// *measured* size — the flip/clamp decision runs one frame behind
/// the layout, which is invisible in practice (egui keeps an area
/// invisible for its first, sizing frame). A zero/stale seed simply
/// yields the unclamped below-left placement.
///
/// When neither side fits (a near-full-height selection), the
/// toolbar pins to the panel top — overlapping the region beats
/// sliding off-screen.
#[must_use]
pub(crate) fn toolbar_anchor(sel: egui::Rect, panel: egui::Rect, size: egui::Vec2) -> egui::Pos2 {
    let below_y = sel.max.y + TOOLBAR_GAP;
    let above_y = sel.min.y - TOOLBAR_GAP - size.y;
    let y = if below_y + size.y <= panel.max.y {
        below_y
    } else if above_y >= panel.min.y {
        above_y
    } else {
        panel.min.y
    };
    let x = sel
        .min
        .x
        .clamp(panel.min.x, (panel.max.x - size.x).max(panel.min.x));
    egui::pos2(x, y)
}

/// The floating toolbar's compact rail row: the shared
/// [`rail::TOOL_PODS`] walk plus a history pod, each rendered as
/// its own rounded island (plan decision 4 — the editor rail's
/// chips, glyphs, and pod grouping; the overlay has no capture
/// `+`). Tool clicks switch `state.tool` directly, the same
/// editor-rail semantics; history and delete clicks return as
/// actions for the caller to replay.
fn overlay_rail_row(ui: &mut egui::Ui, state: &mut EditorState) -> ToolbarAction {
    let mut action = ToolbarAction::None;
    for pod in rail::TOOL_PODS {
        panel::pod_island(ui, |ui| {
            for spec in pod.tools {
                let selected = state.tool == spec.tool;
                let tooltip = format!("{} · {}", spec.name, spec.key);
                // Hand renders labeled (rail emphasis), matching the editor
                // rail; the wider chip simply widens the measured toolbar
                // size that drives `toolbar_anchor`.
                let resp = if spec.tool == Tool::Hand {
                    panel::labeled_chip(
                        ui,
                        egui::Id::new(("overlay_tool", spec.key)),
                        tooltip.as_str(),
                        selected,
                        spec.name,
                        spec.glyph,
                        true,
                    )
                } else {
                    panel::chip(
                        ui,
                        egui::Id::new(("overlay_tool", spec.key)),
                        tooltip.as_str(),
                        selected,
                        spec.glyph,
                        true,
                    )
                };
                if resp.clicked() {
                    // Core `set_tool` owns the selection lifecycle: leaving
                    // Hand drops the persistent selection (same reroute as
                    // the editor rail and the hotkey block).
                    state.set_tool(spec.tool);
                }
            }
        });
        panel::pod_gap(ui);
    }
    let history = panel::pod_island(ui, |ui| {
        let mut action = ToolbarAction::None;
        if panel::chip(
            ui,
            egui::Id::new("overlay_undo"),
            "Undo · Ctrl+Z",
            false,
            panel::g_undo,
            true,
        )
        .clicked()
        {
            action = ToolbarAction::Undo;
        }
        if panel::chip(
            ui,
            egui::Id::new("overlay_redo"),
            "Redo · Ctrl+Y",
            false,
            panel::g_redo,
            true,
        )
        .clicked()
        {
            action = ToolbarAction::Redo;
        }
        // Enabled iff at least one annotation is selected
        // (persistent selection set, multi-select task 001);
        // disabled reads greyed / hover-only.
        if panel::chip(
            ui,
            egui::Id::new("overlay_delete"),
            "Delete · Del",
            false,
            panel::g_trash,
            !state.selected.is_empty(),
        )
        .clicked()
        {
            action = ToolbarAction::Delete;
        }
        action
    });
    if action == ToolbarAction::None {
        action = history;
    }
    action
}

/// The floating toolbar's accept row, read as two islands: the check
/// accept (Enter semantics) / ✕ cancel (Esc semantics) pair, then
/// the outputs (Save / Upload and the forest Copy primary).
/// Copy is Enter-equivalent (spec: Enter = Copy); the output
/// chips' pipelines are the Task 006 flows described in the module
/// docs.
fn overlay_accept_row(ui: &mut egui::Ui) -> (ToolbarAction, Option<OverlayClose>) {
    let mut action = ToolbarAction::None;
    let mut close = None;
    panel::pod_island(ui, |ui| {
        if panel::chip(
            ui,
            egui::Id::new("overlay_accept"),
            "Accept · Enter",
            false,
            panel::g_check,
            true,
        )
        .clicked()
        {
            close = Some(OverlayClose::Accept);
        }
        if panel::chip(
            ui,
            egui::Id::new("overlay_cancel"),
            "Cancel · Esc",
            false,
            panel::g_x,
            true,
        )
        .clicked()
        {
            close = Some(OverlayClose::Cancel);
        }
    });
    panel::pod_gap(ui);
    panel::pod_island(ui, |ui| {
        if panel::chip(
            ui,
            egui::Id::new("overlay_save"),
            "Save · Ctrl+S",
            false,
            panel::g_save,
            true,
        )
        .clicked()
        {
            action = ToolbarAction::Save;
        }
        if panel::chip(
            ui,
            egui::Id::new("overlay_upload"),
            "Upload",
            false,
            panel::g_upload,
            true,
        )
        .clicked()
        {
            action = ToolbarAction::Upload;
        }
        if panel::copy_chip(ui, false).clicked() {
            action = ToolbarAction::Copy;
        }
    });
    (action, close)
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
    /// gesture is active, drags mutate `state.crop_region` through the
    /// pure chrome helpers instead of dispatching pointer events.
    gesture: Gesture,
    /// Pixel-zoom magnifier (hover-triggered in the overlay, per the
    /// spec — no modifier hold). Reused verbatim from the editor
    /// window; reads `state.image` directly.
    magnifier: Magnifier,
    /// Screen-logical rect of the floating toolbar's last drawn
    /// frame, cached by `ui()` so the next frame's `logic()` can
    /// swallow presses that land on it (it floats *inside* the
    /// overlay panel, so the `panel.contains` guard alone would let
    /// toolbar clicks start drags / dispatch `PointerDown`s). `None`
    /// whenever the toolbar is hidden.
    toolbar_rect: Option<egui::Rect>,
    /// This frame's Hand-tool object chrome (see [`crate::chrome`]):
    /// This frame's Hand-tool object chrome (see [`crate::chrome`]):
    /// the hovered / grabbed bounding boxes plus the kind, in
    /// canvas-physical px — plural because a live multi-drag
    /// outlines every member of the selection set. Computed at the
    /// end of [`Self::pointer_and_chrome`] (where the gating inputs
    /// live), painted by `ui_impl` over the shared render output.
    /// `None` whenever no affordance applies.
    move_chrome: Option<(Vec<Rect>, crate::chrome::MoveChromeKind)>,
    /// This frame's Text-tool hover affordance (text-select-edit):
    /// the topmost committed Text under the pointer, dashed-outline
    /// bbox in canvas-physical px. Computed beside `move_chrome`,
    /// painted with its `Hover` kind.
    text_hover: Option<Rect>,
    /// Press-time double-click detector (text-select-edit): fed
    /// exactly when a `PointerDown` dispatches (the
    /// `annotation_press_allowed` fall-through arm), fires the
    /// synthesized `DoubleClick` right after it. See
    /// `crate::dblclick`.
    dblclick: crate::dblclick::DoubleClickTracker,
    /// Last measured toolbar size, fed to [`toolbar_anchor`] for the
    /// flip/clamp decision. Seeded zero (the anchor fn tolerates it);
    /// one frame behind the layout, which is invisible in practice
    /// because egui keeps a new area hidden for its sizing frame.
    toolbar_size: egui::Vec2,
    /// Clipboard handle kept alive after a copy. The overlay closes
    /// on accept (unlike the editor window, which stays open), so on
    /// success the handle is handed to [`Self::clipboard_out`] — the
    /// shell holds it through the post-close linger, satisfying the
    /// X11 owner-clipboard rule (plan decision 6).
    clipboard: Option<arboard::Clipboard>,
    /// Shell-side receiver for the clipboard handle (see
    /// [`Self::clipboard`]).
    clipboard_out: ClipboardSlot,
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
    /// Construct the app. Forces `Tool::Crop` — the overlay opens
    /// in selection mode so the first drag draws a region (the
    /// editor window's `Tool::Rect` default would instead commit a
    /// rectangle annotation on the first drag, which is not what a
    /// Flameshot user expects).
    pub(crate) fn new(
        mut state: EditorState,
        ctx: &Context,
        outcome: OutcomeSlot,
        clipboard_out: ClipboardSlot,
    ) -> Self {
        state.tool = Tool::Crop;
        // Same one-time theme application as `EditorApp::new`: the
        // floating toolbar reuses the editor's chips, so its
        // separators, tooltips, and spacing must read identically.
        crate::theme::apply(ctx);
        Self {
            state,
            canvas: EguiCanvas::new(ctx.clone()),
            bg: None,
            blur_cache: BlurCache::new(),
            overlay_rect: None,
            drag_active: false,
            gesture: Gesture::None,
            magnifier: Magnifier::new(),
            toolbar_rect: None,
            move_chrome: None,
            text_hover: None,
            dblclick: crate::dblclick::DoubleClickTracker::default(),
            toolbar_size: egui::Vec2::ZERO,
            clipboard: None,
            clipboard_out,
            outcome,
        }
    }

    /// Publish the outcome for a close decision and tear the window
    /// down. `Esc` publishes `Cancelled` (no side effects, Task
    /// 002); accept routes through the Enter=Copy pipeline (Task
    /// 006, plan decision 6).
    fn perform_close(&mut self, ctx: &Context, close: OverlayClose) {
        match close {
            OverlayClose::Cancel => {
                tracing::info!("overlay cancelled; no output");
                *self.outcome.borrow_mut() = Some(Ok(EditorOutcome::Cancelled));
            }
            OverlayClose::Accept => {
                self.accept_as_copy(ctx);
                return;
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// The accept pipeline (Enter / check chip / Copy chip / `Ctrl+C`):
    /// compose the region selection-aware onto the clipboard,
    /// publish `CopiedToClipboard`, close. A clipboard failure
    /// publishes `Err` and closes — the editor window's copy-failure
    /// policy.
    ///
    /// The overlay closes immediately (Flameshot Enter semantics);
    /// the X11 owner-clipboard problem is solved by handing the
    /// `arboard::Clipboard` handle to the shell's slot — the process
    /// lingers past window close holding it (see
    /// `run_overlay_blocking`).
    fn accept_as_copy(&mut self, ctx: &Context) {
        match copy_to_clipboard(&self.state, &mut self.clipboard) {
            Ok(()) => {
                tracing::info!("overlay accepted; region copied to clipboard");
                if let Some(handle) = self.clipboard.take() {
                    *self.clipboard_out.borrow_mut() = Some(handle);
                }
                *self.outcome.borrow_mut() = Some(Ok(EditorOutcome::CopiedToClipboard));
            }
            Err(err) => {
                tracing::error!(error = %err, "overlay copy failed; closing overlay session");
                *self.outcome.borrow_mut() = Some(Err(err));
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// Save chip / `Ctrl+S` handler: `rfd` dialog over a
    /// selection-aware compose (the region is what ships — Task
    /// 005's model), publish `Saved(path)` and close on success.
    /// Dialog-cancel keeps the overlay open; a write failure
    /// publishes `Err` and closes (the editor window's policy).
    fn handle_save(&mut self, ctx: &Context) {
        match save_region_to_disk(&self.state) {
            Ok(Some(path)) => {
                tracing::info!(path = %path.display(), "overlay saved");
                *self.outcome.borrow_mut() = Some(Ok(EditorOutcome::Saved(path)));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Ok(None) => {
                tracing::debug!("save dialog cancelled; staying in overlay");
            }
            Err(err) => {
                tracing::error!(error = %err, "save failed; closing overlay session");
                *self.outcome.borrow_mut() = Some(Err(err));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Upload chip handler: temp PNG + detached `flintshot upload`
    /// worker (the editor window's path verbatim), publish
    /// `Uploaded(String::new())` — the URL placeholder; the real one
    /// arrives asynchronously via the daemon event stream — and
    /// close. A **spawn** failure is recoverable: log at `error` and
    /// leave the overlay open so Save / Copy still work; a
    /// compose/write failure is terminal (publish `Err`, close).
    fn handle_upload(&mut self, ctx: &Context) {
        match compose_and_write_upload(&self.state) {
            Ok(path) => match spawn_upload_worker(&path) {
                Ok(()) => {
                    tracing::info!(
                        path = %path.display(),
                        "upload initiated; spawned upload worker"
                    );
                    *self.outcome.borrow_mut() = Some(Ok(EditorOutcome::Uploaded(String::new())));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Err(err) => {
                    tracing::error!(
                        error = %err,
                        "could not spawn upload worker; overlay stays open"
                    );
                }
            },
            Err(err) => {
                tracing::error!(
                    error = %err,
                    "upload compose/write failed; closing overlay session"
                );
                *self.outcome.borrow_mut() = Some(Err(err));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Pointer dispatch plus the Task 003 chrome interaction: gesture
    /// interception, gesture-driven selection mutation, and the hover
    /// cursor shape. Split out of `logic()` so each concern stays
    /// readable; runs only when a cached panel rect exists.
    fn pointer_and_chrome(&mut self, ctx: &Context, panel: egui::Rect, modifiers: Modifiers) {
        let ppp = ctx.pixels_per_point();
        let (pointer, primary_pressed, primary_down, primary_released, now) = ctx.input(|i| {
            (
                i.pointer.latest_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.primary_released(),
                i.time,
            )
        });
        let Some(pos) = pointer else {
            // No pointer this frame (left the window / touch lift):
            // stand down the hover affordance — the grab variant
            // clears the same way on the drag's release frame.
            self.move_chrome = None;
            self.text_hover = None;
            return;
        };
        let at = to_canvas_physical(pos, panel, ppp);

        // Chrome interception (Task 003): with Crop active and a
        // usable region present, a press on a handle or inside the
        // region starts an overlay-owned gesture instead of a
        // dispatcher `PointerDown` (which would restart the region
        // from a point — `begin_crop_region`). While the gesture
        // runs, drags mutate `state.crop_region` directly (the
        // documented exception; see `OverlayApp::gesture`) and
        // release normalizes, mirroring the dispatcher's
        // `finish_crop_region`.
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

        // Toolbar presses are button clicks, not canvas interaction:
        // the floating toolbar floats *inside* the panel rect, so it
        // needs its own guard or a chip click would also start a
        // drag / dispatch a `PointerDown`. Only the *press* is
        // swallowed — a canvas-started drag or gesture that wanders
        // over the toolbar keeps running (and its release lands).
        let over_toolbar = self.toolbar_rect.is_some_and(|r| r.contains(pos));

        let mut pointer_events: Vec<InputEvent> = Vec::new();
        if primary_pressed && panel.contains(pos) && !over_toolbar {
            match gesture_for_press(self.state.crop_region, at, self.state.tool, hit_radius) {
                Some(gesture) => {
                    self.drag_active = true;
                    self.gesture = gesture;
                }
                None if annotation_press_allowed(self.state.tool, self.state.crop_region, at) => {
                    self.drag_active = true;
                    pointer_events.push(InputEvent::PointerDown { pos: at, modifiers });
                    self.push_dblclick_if_due(ctx, now, pos, at, &mut pointer_events);
                }
                // Annotation-tool press outside the region (Task 005):
                // swallow it whole. `drag_active` stays false, so the
                // drags and release of this press never reach the
                // dispatcher either — nothing is drawn on the dimmed
                // area, matching what the clipped render would show.
                None => {}
            }
        } else if primary_released && self.drag_active {
            self.drag_active = false;
            if self.gesture.is_none() {
                pointer_events.push(InputEvent::PointerUp);
            } else {
                // Normalize the dragged rect so `min` is top-left —
                // the same contract the dispatcher's release path
                // (`finish_crop_region`) upholds for fresh drags.
                self.state.crop_region = self.state.crop_region.map(Rect::normalized);
                self.gesture = Gesture::None;
            }
        } else if primary_down && self.drag_active {
            match self.gesture {
                Gesture::None => pointer_events.push(InputEvent::PointerDrag { pos: at }),
                Gesture::Move { offset } => {
                    if let Some(sel) = usable_selection(self.state.crop_region) {
                        self.state.crop_region =
                            Some(moved_rect(sel, clamped, offset, image_bounds));
                    }
                }
                Gesture::Resize { handle } => {
                    if let Some(sel) = usable_selection(self.state.crop_region) {
                        self.state.crop_region = Some(resize_from(handle, sel, clamped));
                    }
                }
            }
        }
        // Pointer events always return `Continue` today; future
        // pointer-side outcomes (Task 006+) surface as a compile
        // error here rather than silently dropping.
        for event in pointer_events {
            let _ = self.state.dispatch_input(&event);
        }

        // Cursor shape: reflects the active gesture while dragging,
        // otherwise the hover target. Select-only outside gestures —
        // annotation tools keep the default cursor, matching the
        // editor window. Over the toolbar the default arrow wins:
        // buttons are not selection affordances.
        if panel.contains(pos) && !over_toolbar {
            let icon = match self.gesture {
                Gesture::Resize { handle } => Some(cursor_for_handle(handle)),
                Gesture::Move { .. } => Some(egui::CursorIcon::Move),
                Gesture::None => self.hover_cursor(at, hit_radius),
            };
            if let Some(icon) = icon {
                ctx.set_cursor_icon(icon);
            }
        }

        // Hand-tool object chrome (prominent-move spec), gated exactly
        // like a press that would grab an annotation: no drag or
        // gesture in flight, not over the toolbar, and the position
        // itself must be press-allowed. A live dispatcher move drag
        // (checked inside `move_chrome`) overrides the hover either
        // way and keeps the grab outline tracking through the drag.
        let hover_at = (!self.drag_active
            && self.gesture.is_none()
            && !over_toolbar
            && annotation_press_allowed(Tool::Hand, self.state.crop_region, at))
        .then_some(at);
        self.move_chrome = crate::chrome::move_chrome(&self.state, hover_at);

        // Text-tool hover affordance (text-select-edit), gated exactly
        // like a Text-tool press that would select: the query itself
        // checks the tool and Text-only hits.
        let text_hover_at = (!self.drag_active
            && self.gesture.is_none()
            && !over_toolbar
            && annotation_press_allowed(Tool::Text, self.state.crop_region, at))
        .then_some(at);
        self.text_hover = crate::chrome::text_hover_chrome(&self.state, text_hover_at);
    }

    /// Double-click synthesis (text-select-edit): fed only on
    /// dispatching presses, fired in the same frame right after the
    /// press — core's ordering contract.
    fn push_dblclick_if_due(
        &mut self,
        ctx: &Context,
        now: f64,
        pos: egui::Pos2,
        at: Point,
        pointer_events: &mut Vec<InputEvent>,
    ) {
        let (max_delay, max_dist) = ctx.options(|o| {
            (
                o.input_options.max_double_click_delay,
                o.input_options.max_click_dist,
            )
        });
        if self.dblclick.on_press(now, pos, max_delay, max_dist) {
            pointer_events.push(InputEvent::DoubleClick { pos: at });
        }
    }

    /// Cursor icon for the current hover position when no gesture is
    /// active. Select tool: a per-handle resize cursor over a handle,
    /// `Move` inside the region, `Crosshair` elsewhere. Hand tool:
    /// `Move` over a committed annotation (a press grabs it for a
    /// drag), default elsewhere. `None` leaves the default cursor
    /// (annotation tools).
    fn hover_cursor(&self, at: Point, hit_radius: f32) -> Option<egui::CursorIcon> {
        if self.state.tool == Tool::Hand {
            return self
                .state
                .hit_test(at)
                .is_some()
                .then_some(egui::CursorIcon::Move);
        }
        if self.state.tool == Tool::Text {
            // Text-tool hover affordance (text-select-edit): the
            // I-beam announces click-to-select / double-click-to-edit.
            return self.state.text_hit_test(at).map(|_| egui::CursorIcon::Text);
        }
        if self.state.tool != Tool::Crop {
            return None;
        }
        let sel = usable_selection(self.state.crop_region);
        if let Some(sel) = sel {
            if let Some(handle) = handle_at(sel, at, hit_radius) {
                return Some(cursor_for_handle(handle));
            }
        }
        if sel.is_some_and(|s| s.contains(at)) {
            return Some(egui::CursorIcon::Move);
        }
        Some(egui::CursorIcon::Crosshair)
    }

    /// Draw the floating toolbar anchored to the selection (Task 004)
    /// and act on its clicks. Visible only while a usable selection
    /// exists and no fresh dispatcher-driven region drag is in
    /// progress — the same guard as the handles — and follows the
    /// live region through resize / move gestures. The area's rect is
    /// cached for the next frame's press / cursor / magnifier guards.
    ///
    /// Layout (plan decision 4: compact rail + accept row, the
    /// editor rail's pod islands): row 1 walks the shared tool pods
    /// plus a history pod; row 2 reads as two islands — check accept
    /// (Enter semantics) / ✕ cancel (Esc semantics), then the
    /// outputs Save / Upload and the forest Copy primary. Copy
    /// is Enter-equivalent (spec: Enter = Copy).
    fn show_toolbar(
        &mut self,
        ctx: &Context,
        panel_rect: egui::Rect,
        sel_logical: Option<egui::Rect>,
    ) {
        let fresh_drag = self.drag_active && self.gesture.is_none();
        let Some(sel) = sel_logical.filter(|_| !fresh_drag) else {
            self.toolbar_rect = None;
            return;
        };
        let anchor = toolbar_anchor(sel, panel_rect, self.toolbar_size);
        let mut action = ToolbarAction::None;
        let mut close: Option<OverlayClose> = None;
        let area = egui::Area::new(egui::Id::new("overlay_toolbar"))
            .order(egui::Order::Foreground)
            .fixed_pos(anchor)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(crate::theme::palette::PAPER_CREAM)
                    .inner_margin(egui::Margin::symmetric(8, 5))
                    .corner_radius(egui::CornerRadius::same(crate::theme::CUTOUT_RADIUS))
                    .stroke(crate::theme::HAIRLINE_STROKE)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            action = overlay_rail_row(ui, &mut self.state);
                        });
                        ui.horizontal(|ui| {
                            let (row_action, row_close) = overlay_accept_row(ui);
                            if action == ToolbarAction::None {
                                action = row_action;
                            }
                            close = row_close;
                        });
                    });
            });
        self.toolbar_rect = Some(area.response.rect);
        let measured = area.response.rect.size();
        // Repaint discipline (Task 008): the anchor ran one frame
        // behind the measured size, so a size *change* while visible
        // (DPI move between monitors, future layout change) needs one
        // follow-up frame to re-anchor correctly. Requests fire only
        // on that transition — never per frame — keeping the overlay
        // idle-quiet at rest. First appearance needs no help: egui
        // runs a new area's invisible sizing pass and requests the
        // repaint itself (`containers/area.rs` `visible_last_frame`).
        if measured != self.toolbar_size {
            ctx.request_repaint();
        }
        self.toolbar_size = measured;

        if let Some(close) = close {
            self.perform_close(ctx, close);
            return;
        }
        self.apply_toolbar_action(ctx, action);
    }

    /// Act on a floating-toolbar action. History + Delete chips
    /// replay the same dispatches a Ctrl+Z / Ctrl+Y / Del keypress
    /// would produce (the editor window's rail handlers, verbatim);
    /// Copy is Enter-equivalent (the same accept-copy pipeline Enter
    /// triggers); Save / Upload run their output pipelines
    /// (Task 006) — the editor window's handlers' overlay twins.
    fn apply_toolbar_action(&mut self, ctx: &Context, action: ToolbarAction) {
        match action {
            ToolbarAction::Copy => self.perform_close(ctx, OverlayClose::Accept),
            ToolbarAction::Undo => {
                let _ = self.state.dispatch_input(&InputEvent::Key {
                    key: Key::Char('z'),
                    modifiers: Modifiers {
                        ctrl: true,
                        ..Modifiers::default()
                    },
                });
            }
            ToolbarAction::Redo => {
                let _ = self.state.dispatch_input(&InputEvent::Key {
                    key: Key::Char('y'),
                    modifiers: Modifiers {
                        ctrl: true,
                        ..Modifiers::default()
                    },
                });
            }
            ToolbarAction::Delete => {
                let _ = self.state.dispatch_input(&InputEvent::Key {
                    key: Key::Delete,
                    modifiers: Modifiers::default(),
                });
            }
            ToolbarAction::Save => self.handle_save(ctx),
            ToolbarAction::Upload => self.handle_upload(ctx),
            ToolbarAction::None => {}
        }
    }

    /// Magnifier on hover (overlay UX: no modifier hold, per the
    /// spec). Suppressed during text editing, same as the editor
    /// window's loupe, and over the floating toolbar — the loupe
    /// would cover the buttons the user is aiming at.
    fn paint_magnifier(&mut self, ctx: &Context, painter: &egui::Painter, panel_rect: egui::Rect) {
        if self.state.is_editing_text() {
            return;
        }
        let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) else {
            return;
        };
        let over_toolbar = self.toolbar_rect.is_some_and(|r| r.contains(pos));
        if !panel_rect.contains(pos) || over_toolbar {
            return;
        }
        let ppp = ctx.pixels_per_point();
        let cursor_phys = to_canvas_physical(pos, panel_rect, ppp);
        // The overlay is always fullscreen-fit: the view is the plain
        // 1:1 mapping (no fit scale — the image *is* the viewport).
        self.magnifier.show(
            painter,
            ctx,
            &self.state.image,
            cursor_phys,
            crate::canvas::CanvasView::over(panel_rect, ppp),
        );
    }
}

impl OverlayApp {
    /// The frame logic, minus eframe's unconstructible `Frame` (both
    /// `logic`/`ui` ignore it) so the idle-repaint guard can drive
    /// the real frame loop headlessly (see
    /// `overlay_idle_frames_request_no_further_repaint`).
    fn logic_impl(&mut self, ctx: &Context) {
        let modifiers = translate_modifiers(ctx.input(|i| i.modifiers));

        // If the user switched tools via the floating toolbar while a
        // text draft was active, commit the draft before anything
        // else. The toolbar sets `state.tool` directly (bypassing
        // the dispatcher), so the modal text-editing guard below
        // would otherwise trap the user in text mode forever.
        // Mirrors `EditorApp::logic`'s guard.
        if self.state.is_editing_text() && self.state.tool != Tool::Text {
            self.state.commit_text_draft();
        }

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
        // Esc layering (annotation-delete spec): with a persistent
        // selection, the first Esc clears it and consumes the frame —
        // no close, the outcome slot untouched. Only with nothing
        // selected does Esc keep today's Cancel. The layering lives
        // here, at the call site; `close_from_input`'s pure contract
        // is unchanged.
        let (esc, enter) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Enter),
            )
        });
        if esc && !self.state.selected.is_empty() {
            self.state.selected.clear();
            return;
        }
        if let Some(close) = close_from_input(esc, enter) {
            self.perform_close(ctx, close);
            return;
        }

        // History + output chords through the dispatcher (Ctrl+Z
        // undo, Ctrl+Shift+Z / Ctrl+Y redo, Ctrl+S save, Ctrl+C
        // copy), plus Del / Backspace deleting the persistent
        // selection. The dispatcher is the single chord recognizer —
        // the shell reacts to the returned outcome exactly as
        // `EditorApp::logic` does; unmodified letters return
        // `Continue` and fall through to tool selection below. The
        // modifiers ride along verbatim, so the dispatcher's
        // no-modifier gate spoils Ctrl+Del itself. Backspace only
        // reaches this block outside text mode — the modal branch
        // above owns it during a draft.
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
            } else if i.key_pressed(egui::Key::S) {
                Some(InputEvent::Key {
                    key: Key::Char('s'),
                    modifiers,
                })
            } else if i.key_pressed(egui::Key::C) {
                Some(InputEvent::Key {
                    key: Key::Char('c'),
                    modifiers,
                })
            } else if i.key_pressed(egui::Key::Delete) {
                Some(InputEvent::Key {
                    key: Key::Delete,
                    modifiers,
                })
            } else if i.key_pressed(egui::Key::Backspace) {
                Some(InputEvent::Key {
                    key: Key::Backspace,
                    modifiers,
                })
            } else {
                None
            }
        });
        if let Some(event) = key_event {
            // Ctrl+Z / Ctrl+Y (and plain letters) always return
            // `Continue`; only the Ctrl+S / Ctrl+C chords map to
            // shell-side pipelines, short-circuiting the rest of the
            // frame (a Ctrl+S mid-drag pre-empts the pointer-up that
            // follows — the editor window's ordering). The wildcard
            // no-op is required (`DispatchOutcome` is
            // `#[non_exhaustive]`) and covers `Cancel` / `Continue`
            // / future variants.
            match self.state.dispatch_input(&event) {
                DispatchOutcome::Save => {
                    self.handle_save(ctx);
                    return;
                }
                DispatchOutcome::Copy => {
                    self.accept_as_copy(ctx);
                    return;
                }
                _ => {}
            }
        }

        // Single-key tool selection (unmodified S R E A L M P T B N),
        // guarded on the event's own modifiers so chords keep their
        // dispatcher meanings. Selection-first lock: the floating
        // rail only renders and annotation presses only flow once a
        // usable region exists, so arming an annotation tool before
        // that would be invisible state; Select — the region-drawing
        // tool — stays armable at all times. Switches route through
        // core `set_tool`: leaving Hand drops the persistent
        // selection.
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
            if tool == Tool::Crop || usable_selection(self.state.crop_region).is_some() {
                self.state.set_tool(tool);
            }
        }

        // Pointer dispatch + selection-chrome interaction, gated on
        // the cached panel rect (the first frame has none — the
        // screenshot isn't visible yet, so no drag can start).
        if let Some(panel) = self.overlay_rect {
            self.pointer_and_chrome(ctx, panel, modifiers);
        }
    }

    /// The frame paint pass, minus eframe's `Frame` (see
    /// [`Self::logic_impl`] for why this split exists).
    fn ui_impl(&mut self, ui: &mut egui::Ui) {
        // Annotation confinement (Task 005): clip the shared render
        // pass's annotation draws to the usable region. Strokes may
        // still be dragged past the edge (the dispatcher sees raw
        // positions, keeping tool semantics byte-identical to the
        // editor window) — the edge just stops painting, exactly
        // where the region-cropped output will cut too.
        let sel_phys = usable_selection(self.state.crop_region);
        self.canvas.set_clip(sel_phys);
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
        // The clip is a per-frame render concern; clear it so any
        // draw outside this pairing (none today) can't inherit it.
        self.canvas.set_clip(None);
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
        // dimmed area. `sel_phys` was computed before the render pass
        // (it also drives the annotation clip).
        let panel_rect = panel.response.rect;
        let painter = ui.painter().clone();
        let ctx = ui.ctx().clone();
        let ppp = ctx.pixels_per_point();
        let to_logical =
            |p: Point| egui::pos2(panel_rect.min.x + p.x / ppp, panel_rect.min.y + p.y / ppp);

        // Dim mask: four rects framing the region (the full panel
        // when nothing is selected) — the shared mask math
        // (`mask_rects` + `mask_color`), reused verbatim by the
        // editor window's selection-first phase.
        let sel_logical =
            sel_phys.map(|s| egui::Rect::from_two_pos(to_logical(s.min), to_logical(s.max)));
        for band in mask_rects(panel_rect, sel_logical) {
            painter.rect_filled(band, egui::CornerRadius::ZERO, mask_color());
        }

        // Hand-tool object chrome (prominent-move spec): dashed hover
        // outline / solid forest grab outline, painted under the
        // selection handles and badge so the region chrome stays
        // legible over it. Computed in `pointer_and_chrome` this
        // frame; `None` paints nothing.
        if let Some((rects, kind)) = &self.move_chrome {
            for r in rects {
                let rect = egui::Rect::from_two_pos(to_logical(r.min), to_logical(r.max));
                crate::chrome::paint_move_chrome(&painter, rect, *kind);
            }
        }

        // Text-tool hover affordance (text-select-edit): dashed
        // outline on the move chrome's rhythm, same paint helper.
        if let Some(r) = self.text_hover {
            let rect = egui::Rect::from_two_pos(to_logical(r.min), to_logical(r.max));
            crate::chrome::paint_move_chrome(&painter, rect, crate::chrome::MoveChromeKind::Hover);
        }

        // Persistent-selection outlines (annotation-delete spec,
        // pluralized by multi-select-properties): one dashed forest
        // rect per selected annotation, right after the move chrome
        // in the overlay's chrome order — the editor window's
        // ordering. Chrome-only by construction (the shell's paint
        // pass, never the clipped render / composed output). A
        // selection with no paintable bbox at all while a
        // selection-owning tool is active (degenerate annotations /
        // stale indices) deselects instead of rendering.
        let selected_rects = crate::chrome::selected_chrome(&self.state);
        if !selected_rects.is_empty() {
            for r in selected_rects {
                let rect = egui::Rect::from_two_pos(to_logical(r.min), to_logical(r.max));
                crate::chrome::paint_selected_chrome(&painter, rect);
            }
        } else if matches!(self.state.tool, Tool::Hand | Tool::Text)
            && !self.state.selected.is_empty()
        {
            self.state.selected.clear();
        }

        // Grab handles: visible only while they're actually
        // interactive — Select tool active and no fresh
        // dispatcher-driven selection drag in progress (during a
        // fresh drag the midpoint handles would jitter as the rect
        // grows; Flameshot hides them until release too).
        if self.state.tool == Tool::Crop && !(self.drag_active && self.gesture.is_none()) {
            if let Some(s) = sel_phys {
                let edge = HANDLE_EDGE_LOGICAL;
                for center in handle_centers(s) {
                    let rect =
                        egui::Rect::from_center_size(to_logical(center), egui::vec2(edge, edge));
                    painter.rect_filled(rect, egui::CornerRadius::same(2), egui::Color32::WHITE);
                    painter.rect_stroke(
                        rect,
                        egui::CornerRadius::same(2),
                        egui::Stroke::new(1.0, crate::theme::palette::readability_black(200)),
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
                crate::theme::palette::readability_black(200),
            );
            painter.galley(rect.min, galley, egui::Color32::WHITE);
        }

        // Floating toolbar (Task 004), drawn before the magnifier so
        // this frame's rect already steers the loupe away from it.
        // Foreground order keeps it above the chrome regardless.
        // `sel_logical` was computed at the mask site above.
        self.show_toolbar(&ctx, panel_rect, sel_logical);

        // Magnifier on hover (overlay UX: no modifier hold, per the
        // spec), steered away from the floating toolbar.
        self.paint_magnifier(&ctx, &painter, panel_rect);
    }
}

impl App for OverlayApp {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.logic_impl(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui_impl(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::Annotation;
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
    fn overlay_app_opens_in_crop_mode() {
        let app = OverlayApp::new(
            blank_state(8, 8),
            &egui::Context::default(),
            Rc::new(RefCell::new(None)),
            Rc::new(RefCell::new(None)),
        );
        assert_eq!(
            app.state.tool,
            Tool::Crop,
            "the first drag must draw a crop region, not a rect annotation"
        );
    }

    // --- Selection-first keyboard gate (select-first-tool-lock) ---

    /// One overlay frame carrying a single unmodified key press —
    /// the exact path a real keystroke takes through `logic_impl`.
    fn key_frame(ctx: &egui::Context, app: &mut OverlayApp, t: f64, key: egui::Key) {
        pointer_frame(
            ctx,
            app,
            t,
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }

    /// Every `(key, tool)` pair `tool_for_key` admits besides Crop.
    fn annotation_key_pairs() -> Vec<(egui::Key, Tool)> {
        vec![
            (egui::Key::V, Tool::Select),
            (egui::Key::H, Tool::Hand),
            (egui::Key::R, Tool::Rect),
            (egui::Key::E, Tool::Ellipse),
            (egui::Key::A, Tool::Arrow),
            (egui::Key::L, Tool::Line),
            (egui::Key::M, Tool::Marker),
            (egui::Key::P, Tool::Pencil),
            (egui::Key::T, Tool::Text),
            (egui::Key::B, Tool::Blur),
            (egui::Key::N, Tool::Number),
        ]
    }

    #[test]
    fn overlay_tool_keys_inert_without_region() {
        let ctx = egui::Context::default();
        let mut app = OverlayApp::new(
            blank_state(64, 64),
            &ctx,
            Rc::new(RefCell::new(None)),
            Rc::new(RefCell::new(None)),
        );
        let mut t = 0.0;
        for (key, tool) in annotation_key_pairs() {
            t += 1.0 / 60.0;
            key_frame(&ctx, &mut app, t, key);
            assert_eq!(
                app.state.tool,
                Tool::Crop,
                "{key:?} must not arm {tool:?} before any region exists"
            );
        }
    }

    #[test]
    fn overlay_tool_keys_inert_with_degenerate_region() {
        let ctx = egui::Context::default();
        let mut app = OverlayApp::new(
            blank_state(64, 64),
            &ctx,
            Rc::new(RefCell::new(None)),
            Rc::new(RefCell::new(None)),
        );
        // Sub-pixel width: present but not usable — the rail and the
        // press gate already treat this as "no selection".
        app.state.crop_region = Some(Rect::from_xywh(10.0, 10.0, 0.5, 50.0));
        let mut t = 0.0;
        for (key, tool) in annotation_key_pairs() {
            t += 1.0 / 60.0;
            key_frame(&ctx, &mut app, t, key);
            assert_eq!(
                app.state.tool,
                Tool::Crop,
                "{key:?} must not arm {tool:?} on a degenerate region"
            );
        }
    }

    #[test]
    fn overlay_tool_keys_arm_once_region_usable() {
        let ctx = egui::Context::default();
        let mut app = OverlayApp::new(
            blank_state(64, 64),
            &ctx,
            Rc::new(RefCell::new(None)),
            Rc::new(RefCell::new(None)),
        );
        app.state.crop_region = Some(Rect::from_xywh(4.0, 4.0, 40.0, 30.0));
        let mut pairs = annotation_key_pairs();
        pairs.push((egui::Key::C, Tool::Crop));
        let mut t = 0.0;
        for (key, tool) in pairs {
            t += 1.0 / 60.0;
            key_frame(&ctx, &mut app, t, key);
            assert_eq!(
                app.state.tool, tool,
                "usable region: {key:?} must arm {tool:?} exactly as before the gate"
            );
        }
    }

    #[test]
    fn overlay_crop_key_always_arms() {
        let ctx = egui::Context::default();
        let mut app = OverlayApp::new(
            blank_state(64, 64),
            &ctx,
            Rc::new(RefCell::new(None)),
            Rc::new(RefCell::new(None)),
        );
        // Direct mutation: pre-region no input path can leave
        // Crop (the rail is unrendered, other keys gated) — yet C
        // must keep arming Crop, so the tool has to start elsewhere
        // for the press to be observable.
        app.state.tool = Tool::Pencil;
        key_frame(&ctx, &mut app, 0.0, egui::Key::C);
        assert_eq!(
            app.state.tool,
            Tool::Crop,
            "C is the region-drawing tool and stays armable without a region"
        );
    }

    // --- Hand-tool hover / grab chrome (prominent-move) ---

    /// Overlay state on the Hand tool with one committed rect at
    /// (10,10)-(50,40). Construction forces Select, so the fixture
    /// re-arms Hand like a user pressing `V` would.
    fn move_overlay_app() -> OverlayApp {
        let mut state = blank_state(64, 64);
        state.pending = Some(Annotation::Rect {
            rect: Rect::from_xywh(10.0, 10.0, 40.0, 30.0),
            stroke: flintshot_core::Stroke::new(flintshot_core::Color::RED, 3.0),
            fill: None,
        });
        state.commit_annotation();
        let mut app = OverlayApp::new(
            state,
            &egui::Context::default(),
            Rc::new(RefCell::new(None)),
            Rc::new(RefCell::new(None)),
        );
        app.state.tool = Tool::Hand;
        app
    }

    /// One overlay frame with the given pointer events (screen ==
    /// canvas-physical; ppp is 1 in a synthetic context).
    fn pointer_frame(ctx: &egui::Context, app: &mut OverlayApp, t: f64, events: Vec<egui::Event>) {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(64.0, 64.0),
            )),
            time: Some(t),
            events,
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            app.logic_impl(ui.ctx());
            app.ui_impl(ui);
        });
    }

    #[test]
    fn overlay_move_hover_yields_hover_chrome() {
        let ctx = egui::Context::default();
        let mut app = move_overlay_app();
        // Frame 1: layout (caches the panel rect); frame 2: hover.
        idle_frame(&ctx, &mut app, 0.0);
        pointer_frame(
            &ctx,
            &mut app,
            1.0 / 60.0,
            vec![egui::Event::PointerMoved(egui::pos2(30.0, 25.0))],
        );
        let (rects, kind) = app.move_chrome.expect("hover hit must yield chrome");
        assert_eq!(kind, crate::chrome::MoveChromeKind::Hover);
        assert_eq!(rects.len(), 1, "hover outlines exactly the hit annotation");
        assert_eq!(
            rects[0].min,
            Point::new(10.0, 10.0),
            "chrome tracks the bbox"
        );
        assert_eq!(rects[0].max, Point::new(50.0, 40.0));
    }

    #[test]
    fn overlay_move_hover_miss_clears_chrome() {
        let ctx = egui::Context::default();
        let mut app = move_overlay_app();
        idle_frame(&ctx, &mut app, 0.0);
        pointer_frame(
            &ctx,
            &mut app,
            1.0 / 60.0,
            vec![egui::Event::PointerMoved(egui::pos2(5.0, 5.0))],
        );
        assert!(app.move_chrome.is_none(), "empty canvas → no chrome");
    }

    #[test]
    fn overlay_move_grab_press_yields_grab_chrome() {
        let ctx = egui::Context::default();
        let mut app = move_overlay_app();
        idle_frame(&ctx, &mut app, 0.0);
        let press = egui::pos2(30.0, 25.0);
        pointer_frame(
            &ctx,
            &mut app,
            1.0 / 60.0,
            vec![
                egui::Event::PointerMoved(press),
                egui::Event::PointerButton {
                    pos: press,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        );
        assert!(app.state.moving.is_some(), "press grabbed the annotation");
        let (rects, kind) = app.move_chrome.expect("live drag must yield chrome");
        assert_eq!(kind, crate::chrome::MoveChromeKind::Grab);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].min, Point::new(10.0, 10.0));
        assert_eq!(rects[0].max, Point::new(50.0, 40.0));
    }

    #[test]
    fn overlay_move_grab_chrome_tracks_through_drag() {
        let ctx = egui::Context::default();
        let mut app = move_overlay_app();
        idle_frame(&ctx, &mut app, 0.0);
        let press = egui::pos2(30.0, 25.0);
        pointer_frame(
            &ctx,
            &mut app,
            1.0 / 60.0,
            vec![
                egui::Event::PointerMoved(press),
                egui::Event::PointerButton {
                    pos: press,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        );
        // Drag to (36,29) — the grabbed annotation follows and the
        // chrome rect must track it (dispatcher moved the ann by
        // (6,4)).
        pointer_frame(
            &ctx,
            &mut app,
            2.0 / 60.0,
            vec![egui::Event::PointerMoved(egui::pos2(36.0, 29.0))],
        );
        assert!(app.state.moving.is_some(), "drag is still live");
        let (rects, kind) = app.move_chrome.expect("drag frame keeps chrome");
        assert_eq!(kind, crate::chrome::MoveChromeKind::Grab);
        assert_eq!(rects.len(), 1);
        assert_eq!(
            rects[0].min,
            Point::new(16.0, 14.0),
            "chrome tracks the drag"
        );
        assert_eq!(rects[0].max, Point::new(56.0, 44.0));
    }

    // --- Double-click synthesis (text-select-edit Task 004) ---

    /// A 800×600 variant of `pointer_frame` / `idle_frame`: the
    /// floating toolbar measures ~687×104 logical px regardless of
    /// screen size, so on the stock 64×64 harness it blankets the
    /// whole panel and every press reads as a toolbar click. The
    /// wide screen gives it somewhere harmless to live.
    fn wide_frame(
        ctx: &egui::Context,
        app: &mut OverlayApp,
        t: f64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(800.0, 600.0),
            )),
            time: Some(t),
            events,
            ..egui::RawInput::default()
        };
        ctx.run_ui(raw, |ui| {
            app.logic_impl(ui.ctx());
            app.ui_impl(ui);
        })
    }

    /// Overlay state on the Text tool with one committed text "hello"
    /// at (10,10) and a usable region covering it. Construction
    /// forces Select; direct mutation is the fixture pattern (see
    /// `move_overlay_app`).
    fn text_overlay_app(ctx: &egui::Context) -> OverlayApp {
        let mut state = blank_state(64, 64);
        // Text sits LOW in the region: the floating toolbar anchors
        // below the selection (or pins to the panel top when there's
        // no room below), so a high text would sit under the toolbar
        // and its presses would be swallowed as button clicks.
        state.anns.push(Annotation::text(
            Point::new(10.0, 40.0),
            "hello",
            flintshot_core::text::TextStyle::simple(16.0, flintshot_core::Color::RED),
        ));
        let mut app = OverlayApp::new(
            state,
            ctx,
            Rc::new(RefCell::new(None)),
            Rc::new(RefCell::new(None)),
        );
        app.state.tool = Tool::Text;
        app.state.crop_region = Some(Rect::from_xywh(4.0, 34.0, 56.0, 26.0));
        app
    }

    /// A context with fonts live: egui creates its font atlas at the
    /// first pass's `end_pass`, and the Text annotation's paint /
    /// measure path needs it from frame one.
    fn prime_fonts() -> egui::Context {
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput::default());
        let _ = ctx.end_pass();
        ctx
    }

    /// Primary press/release events at `pos`.
    fn primary_at(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        }
    }

    /// Two rapid presses inside the region synthesize
    /// `PointerDown` + `DoubleClick`: select, then lift out for
    /// re-edit.
    #[test]
    fn overlay_rapid_double_press_opens_reedit() {
        let ctx = prime_fonts();
        let mut app = text_overlay_app(&ctx);
        let _ = wide_frame(&ctx, &mut app, 0.0, vec![]);
        let press = egui::pos2(20.0, 45.0);
        wide_frame(
            &ctx,
            &mut app,
            1.0,
            vec![egui::Event::PointerMoved(press), primary_at(press, true)],
        );
        assert_eq!(app.state.selected, vec![0], "first press selects");
        let _ = wide_frame(&ctx, &mut app, 1.05, vec![primary_at(press, false)]);
        wide_frame(
            &ctx,
            &mut app,
            1.1,
            vec![egui::Event::PointerMoved(press), primary_at(press, true)],
        );
        assert!(
            app.state.text_reedit.is_some(),
            "rapid second press opened the re-edit session"
        );
        assert!(app.state.anns.is_empty(), "the text is lifted into pending");
    }

    /// Slow presses stay a plain re-select: no session, no pending.
    #[test]
    fn overlay_slow_double_press_stays_selection_only() {
        let ctx = prime_fonts();
        let mut app = text_overlay_app(&ctx);
        let _ = wide_frame(&ctx, &mut app, 0.0, vec![]);
        let press = egui::pos2(20.0, 45.0);
        wide_frame(
            &ctx,
            &mut app,
            1.0,
            vec![egui::Event::PointerMoved(press), primary_at(press, true)],
        );
        let _ = wide_frame(&ctx, &mut app, 1.05, vec![primary_at(press, false)]);
        wide_frame(
            &ctx,
            &mut app,
            3.0,
            vec![egui::Event::PointerMoved(press), primary_at(press, true)],
        );
        assert_eq!(app.state.selected, vec![0], "still just selected");
        assert!(app.state.text_reedit.is_none(), "slow pair ⇒ no session");
        assert!(app.state.pending.is_none());
    }

    /// Presses outside the usable region are swallowed whole — and a
    /// swallowed press must not count toward a double-click either:
    /// even a rapid pair outside the region selects nothing and opens
    /// no session.
    #[test]
    fn overlay_swallowed_presses_never_fire_doubleclick() {
        let ctx = prime_fonts();
        let mut app = text_overlay_app(&ctx);
        // Shrink the region so the text hit point falls outside it.
        app.state.crop_region = Some(Rect::from_xywh(4.0, 34.0, 10.0, 6.0));
        let _ = wide_frame(&ctx, &mut app, 0.0, vec![]);
        let press = egui::pos2(20.0, 45.0);
        wide_frame(
            &ctx,
            &mut app,
            1.0,
            vec![egui::Event::PointerMoved(press), primary_at(press, true)],
        );
        let _ = wide_frame(&ctx, &mut app, 1.05, vec![primary_at(press, false)]);
        wide_frame(
            &ctx,
            &mut app,
            1.1,
            vec![egui::Event::PointerMoved(press), primary_at(press, true)],
        );
        assert!(
            app.state.selected.is_empty(),
            "swallowed press never selected"
        );
        assert!(
            app.state.text_reedit.is_none(),
            "no session from swallowed presses"
        );
        assert!(app.state.pending.is_none());
    }

    /// Hovering a committed text under the Text tool shows the text
    /// cursor (the dashed outline paints through the same query).
    #[test]
    fn overlay_text_hover_sets_text_cursor() {
        let ctx = prime_fonts();
        let mut app = text_overlay_app(&ctx);
        let _ = wide_frame(&ctx, &mut app, 0.0, vec![]);
        let hover_raw = |t: f64, at: egui::Pos2| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(800.0, 600.0),
            )),
            time: Some(t),
            events: vec![egui::Event::PointerMoved(at)],
            ..egui::RawInput::default()
        };
        let out = ctx.run_ui(hover_raw(1.0, egui::pos2(20.0, 45.0)), |ui| {
            app.logic_impl(ui.ctx());
            app.ui_impl(ui);
        });
        assert_eq!(
            out.platform_output.cursor_icon,
            egui::CursorIcon::Text,
            "hover on the text shows the text cursor"
        );
        let out = ctx.run_ui(hover_raw(1.1, egui::pos2(500.0, 500.0)), |ui| {
            app.logic_impl(ui.ctx());
            app.ui_impl(ui);
        });
        assert_eq!(
            out.platform_output.cursor_icon,
            egui::CursorIcon::Default,
            "hover on empty canvas shows no affordance"
        );
    }

    #[test]
    fn overlay_app_construction_requests_no_repaint() {
        // Repaint discipline (Task 008): constructing the app must not
        // itself schedule work — the first frame comes from the window
        // system, later frames from input. A fresh egui context starts
        // with a built-in `outstanding: 1` ("run a couple of frames at
        // the start"), so settle it with two empty passes first; a
        // settled context reports no pending repaint.
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput::default());
        let _ = ctx.end_pass();
        ctx.begin_pass(egui::RawInput::default());
        let _ = ctx.end_pass();
        assert!(
            !ctx.has_requested_repaint(),
            "test fixture: the settled context must start idle"
        );
        let _app = OverlayApp::new(
            blank_state(8, 8),
            &ctx,
            Rc::new(RefCell::new(None)),
            Rc::new(RefCell::new(None)),
        );
        assert!(
            !ctx.has_requested_repaint(),
            "construction must leave the context idle"
        );
    }

    /// One eframe-shaped frame: `logic` then `ui` inside a single
    /// egui pass, with empty input (no pointer, no keys) and a stable
    /// screen rect — the idle overlay. `time` advances like eframe's
    /// clock so anything time-dependent still sees a moving world.
    fn idle_frame(ctx: &egui::Context, app: &mut OverlayApp, t: f64) {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(64.0, 64.0),
            )),
            time: Some(t),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            app.logic_impl(ui.ctx());
            app.ui_impl(ui);
        });
    }

    #[test]
    fn overlay_idle_frames_request_no_further_repaint() {
        // The repaint-discipline guard (Task 008): once the overlay's
        // startup frames are burned (bg texture upload, first layout,
        // egui's built-in `outstanding: 1`), idle frames must not
        // request more — a per-frame `request_repaint` anywhere in
        // `logic_impl`/`ui_impl` would spin the loop at full speed
        // (spec exit criterion 6: no continuous CPU burn while idle).
        let ctx = egui::Context::default();
        let mut app = OverlayApp::new(
            blank_state(64, 64),
            &ctx,
            Rc::new(RefCell::new(None)),
            Rc::new(RefCell::new(None)),
        );
        for i in 0..6 {
            idle_frame(&ctx, &mut app, f64::from(i) / 60.0);
        }
        assert!(
            !ctx.has_requested_repaint(),
            "idle frames must not schedule further repaints"
        );
        // And a stationary hover (pointer parked over the fullscreen
        // panel — the magnifier's every-frame path) must stay quiet
        // too: painting per frame is fine, *requesting* frames is not.
        for i in 6..10 {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::pos2(0.0, 0.0),
                    egui::vec2(64.0, 64.0),
                )),
                time: Some(f64::from(i) / 60.0),
                events: vec![egui::Event::PointerMoved(egui::pos2(32.0, 32.0))],
                ..egui::RawInput::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                app.logic_impl(ui.ctx());
                app.ui_impl(ui);
            });
        }
        // egui's designed decay: an event-bearing frame schedules one
        // extra settle pass (`request_repaint_after(ZERO)` inside
        // `begin_pass` when input has events), and `repaint_delay`
        // only resets on the second quiet pass. A true per-frame
        // requester in our code never goes quiet, so two settle
        // frames still catch it.
        idle_frame(&ctx, &mut app, 10.0 / 60.0);
        idle_frame(&ctx, &mut app, 11.0 / 60.0);
        assert!(
            !ctx.has_requested_repaint(),
            "a parked pointer must not schedule repaints"
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
            gesture_for_press(Some(sel()), Point::new(100.0, 100.0), Tool::Crop, 10.0),
            Some(Gesture::Resize { handle: Handle::Nw }),
            "a press on a handle resizes, even though it's inside the rect too"
        );
    }

    #[test]
    fn gesture_for_press_inside_is_move_with_offset() {
        assert_eq!(
            gesture_for_press(Some(sel()), Point::new(150.0, 140.0), Tool::Crop, 10.0),
            Some(Gesture::Move {
                offset: Point::new(50.0, 40.0)
            }),
            "interior press grabs at cursor minus selection origin"
        );
    }

    #[test]
    fn gesture_for_press_outside_falls_through() {
        assert_eq!(
            gesture_for_press(Some(sel()), Point::new(50.0, 50.0), Tool::Crop, 10.0),
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
            gesture_for_press(None, Point::new(150.0, 140.0), Tool::Crop, 10.0),
            None
        );
        // Degenerate selections are treated as absent.
        assert_eq!(
            gesture_for_press(
                Some(Rect::from_xywh(10.0, 10.0, 0.5, 0.5)),
                Point::new(10.0, 10.0),
                Tool::Crop,
                10.0
            ),
            None
        );
    }

    /// A Select press on a committed annotation *inside* the region
    /// grabs the region (Select owns region manipulation; annotation
    /// moves belong to `Tool::Hand`, which never reaches the gesture
    /// layer).
    #[test]
    fn gesture_for_press_on_annotation_grabs_region() {
        assert_eq!(
            gesture_for_press(Some(sel()), Point::new(150.0, 145.0), Tool::Crop, 10.0),
            Some(Gesture::Move {
                offset: Point::new(50.0, 45.0)
            }),
            "interior press moves the region even over an annotation"
        );
    }

    /// Hand-tool presses never become region gestures — they flow to
    /// the dispatcher, which hit-tests for the annotation grab.
    #[test]
    fn gesture_for_press_ignores_move_tool() {
        assert_eq!(
            gesture_for_press(Some(sel()), Point::new(150.0, 140.0), Tool::Hand, 10.0),
            None,
            "Hand presses belong to the dispatcher, even on a handle"
        );
        assert_eq!(
            gesture_for_press(None, Point::new(150.0, 145.0), Tool::Hand, 10.0),
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

    // --- Floating toolbar (Task 004) ---

    /// A 1920×1080 logical panel — the shared fixture for the anchor
    /// tests.
    fn panel() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1920.0, 1080.0))
    }

    /// The measured toolbar size used by the anchor tests (two chip
    /// rows + margins — close to the real rail + accept row).
    fn toolbar_size() -> egui::Vec2 {
        egui::vec2(400.0, 72.0)
    }

    #[test]
    fn toolbar_anchor_sits_below_left_with_room() {
        let sel = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(300.0, 200.0));
        assert_eq!(
            toolbar_anchor(sel, panel(), toolbar_size()),
            egui::pos2(100.0, 300.0 + TOOLBAR_GAP),
            "below the bottom edge, left-aligned, one gap out"
        );
    }

    #[test]
    fn toolbar_anchor_flips_above_without_room_below() {
        // Selection hugging the bottom edge: below doesn't fit, so
        // the toolbar sits above the top edge.
        let sel = egui::Rect::from_min_size(egui::pos2(100.0, 900.0), egui::vec2(300.0, 170.0));
        assert_eq!(
            toolbar_anchor(sel, panel(), toolbar_size()),
            egui::pos2(100.0, 900.0 - TOOLBAR_GAP - 72.0),
            "flipped above, bottom one gap above the selection top"
        );
    }

    #[test]
    fn toolbar_anchor_clamps_x_at_the_right_edge() {
        let sel = egui::Rect::from_min_size(egui::pos2(1800.0, 100.0), egui::vec2(100.0, 200.0));
        assert_eq!(
            toolbar_anchor(sel, panel(), toolbar_size()),
            egui::pos2(1920.0 - 400.0, 300.0 + TOOLBAR_GAP),
            "a 400px toolbar at the right edge must slide left to stay on screen"
        );
    }

    #[test]
    fn toolbar_anchor_pins_to_top_when_neither_side_fits() {
        // Full-height selection: no room below or above — pin to the
        // panel top (overlapping the region) rather than off-screen.
        let sel = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1920.0, 1080.0));
        assert_eq!(
            toolbar_anchor(sel, panel(), toolbar_size()),
            egui::pos2(0.0, 0.0)
        );
    }

    #[test]
    fn toolbar_anchor_tolerates_zero_size_seed() {
        // First-ever frame has no measured size; placement must still
        // be the (correct) below-left default, not NaN or inverted.
        let sel = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(300.0, 200.0));
        assert_eq!(
            toolbar_anchor(sel, panel(), egui::Vec2::ZERO),
            egui::pos2(100.0, 300.0 + TOOLBAR_GAP)
        );
    }

    /// Closeout (toolbar-groups Task 003): the pod-padded rail is far
    /// wider than the old flat row — the manual matrix's 500 px
    /// selection on a narrow panel must still land the whole toolbar
    /// on-panel. The width isn't hardcoded: it is measured from the
    /// real rail row, so pod-padding growth keeps stressing the
    /// clamp.
    #[test]
    fn toolbar_anchor_keeps_wide_pod_rail_on_panel_beside_narrow_selection() {
        let (rects, _) = overlay_rail_chip_rects(&mut blank_state(64, 64));
        let rail_width = rects
            .iter()
            .map(|r| r.max.x)
            .fold(f32::MIN, f32::max)
            .max(egui::Vec2::ZERO.x);
        assert!(
            rail_width > 500.0,
            "the pod rail is meaningfully wide (measured {rail_width})"
        );
        let size = egui::vec2(rail_width, 72.0);
        let panel = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 800.0));
        let sel = egui::Rect::from_min_size(egui::pos2(20.0, 620.0), egui::vec2(500.0, 140.0));
        let at = toolbar_anchor(sel, panel, size);
        let toolbar = egui::Rect::from_min_size(at, size);
        assert!(
            panel.contains_rect(toolbar),
            "wide pod toolbar {toolbar:?} must stay fully on {panel:?} beside a 500 px selection"
        );
        // A panel narrower than the toolbar documents spec OQ2's
        // answer: no wrap — the clamp pins to the panel's left and
        // the rail overflows right, not off both edges.
        let narrow = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(600.0, 800.0));
        let at = toolbar_anchor(sel, narrow, size);
        assert!(
            (at.x - narrow.min.x).abs() < 0.01,
            "pinned left, got {at:?}"
        );
    }

    // --- Floating toolbar pods (toolbar-groups Task 002) ---

    /// Horizontal gap between two chip rects laid out left to right.
    fn chip_gap(a: egui::Rect, b: egui::Rect) -> f32 {
        b.min.x - a.max.x
    }

    /// Whether two chip rects share a centre line (the same-row
    /// guarantee; layout math is deterministic, so a tight epsilon
    /// suffices).
    fn same_row(a: egui::Rect, b: egui::Rect) -> bool {
        (a.center().y - b.center().y).abs() < 0.01
    }

    /// Count the pod-island fills a frame painted: rounded white
    /// cutout rects at the theme's pod corner radius — the visual
    /// signature of [`panel::pod_island`]. Frames with shadows
    /// wrap their shapes in `Shape::Vec`, so the collect recurses.
    fn island_count(output: &egui::FullOutput) -> usize {
        fn collect<'a>(
            shapes: impl Iterator<Item = &'a egui::Shape>,
            out: &mut Vec<&'a egui::epaint::RectShape>,
        ) {
            for shape in shapes {
                match shape {
                    egui::Shape::Rect(r) => out.push(r),
                    egui::Shape::Vec(v) => collect(v.iter(), out),
                    _ => {}
                }
            }
        }
        let mut rects = Vec::new();
        collect(output.shapes.iter().map(|cs| &cs.shape), &mut rects);
        rects
            .iter()
            .filter(|r| {
                r.fill == crate::theme::palette::PAPER_CREAM
                    && r.corner_radius == egui::CornerRadius::same(crate::theme::CUTOUT_RADIUS)
            })
            .count()
    }

    /// Lay the overlay rail row out once and return its chip rects
    /// in visual order, derived from the shared pod table (tool
    /// pods, then the history pod's undo / redo / delete).
    fn overlay_rail_chip_rects(state: &mut EditorState) -> (Vec<egui::Rect>, usize) {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.horizontal(|ui| {
                let _ = overlay_rail_row(ui, state);
            });
        });
        let rects = rail::TOOL_PODS
            .iter()
            .flat_map(|p| p.tools.iter())
            .map(|spec| {
                ctx.read_response(egui::Id::new(("overlay_tool", spec.key)))
                    .expect("rail layout registers every tool chip")
                    .rect
            })
            .chain(
                ["overlay_undo", "overlay_redo", "overlay_delete"].map(|id| {
                    ctx.read_response(egui::Id::new(id))
                        .expect("rail layout registers the history pod")
                        .rect
                }),
            )
            .collect();
        (rects, island_count(&output))
    }

    #[test]
    fn overlay_rail_groups_pods_as_islands() {
        let (rects, islands) = overlay_rail_chip_rects(&mut blank_state(64, 64));
        // Five tool pods + the history pod, each one painted island.
        assert_eq!(
            islands,
            rail::TOOL_PODS.len() + 1,
            "every pod renders as its own white cutout island"
        );
        // One row: shared centre line, strictly left to right.
        for w in rects.windows(2) {
            assert!(same_row(w[0], w[1]), "chips share a row");
            assert!(w[0].max.x < w[1].min.x, "chips run left to right");
        }
        // Pod boundaries sit after each pod's last chip (the history
        // pod trails the tool pods).
        let mut sizes: Vec<usize> = rail::TOOL_PODS.iter().map(|p| p.tools.len()).collect();
        sizes.push(3);
        let mut boundary = vec![false; rects.len() - 1];
        let mut i = 0;
        for &n in &sizes {
            i += n;
            if i < rects.len() {
                boundary[i - 1] = true;
            }
        }
        let mut intra = Vec::new();
        let mut inter = Vec::new();
        for (i, b) in boundary.iter().enumerate() {
            let gap = chip_gap(rects[i], rects[i + 1]);
            if *b {
                inter.push(gap);
            } else {
                intra.push(gap);
            }
        }
        assert!(!intra.is_empty() && !inter.is_empty());
        let max_intra = intra.iter().copied().fold(f32::MIN, f32::max);
        let min_inter = inter.iter().copied().fold(f32::MAX, f32::min);
        let max_inter = inter.iter().copied().fold(f32::MIN, f32::max);
        assert!(
            min_inter > max_intra + 4.0,
            "island gaps ({min_inter}..{max_inter}) must dominate intra-pod gaps (≤{max_intra})"
        );
        // Every pod boundary shares the one island rhythm — a stray
        // separator or missing pad at one boundary would stand out.
        assert!(
            max_inter - min_inter <= 2.0,
            "uniform island gaps, got {inter:?}"
        );
    }

    /// The Pin affordance is gone from the overlay's outputs island
    /// (remove-pin-to-desktop): no chip may register under the
    /// retired id.
    #[test]
    fn overlay_accept_row_has_no_pin_chip() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.horizontal(|ui| {
                let _ = overlay_accept_row(ui);
            });
        });
        assert!(
            ctx.read_response(egui::Id::new("overlay_pin")).is_none(),
            "the overlay pin chip must not register a response"
        );
    }

    #[test]
    fn overlay_accept_row_reads_as_two_islands() {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.horizontal(|ui| {
                let _ = overlay_accept_row(ui);
            });
        });
        assert_eq!(
            island_count(&output),
            2,
            "the accept row paints exactly two islands: check/cross and outputs"
        );
        let rect = |id: &str| {
            ctx.read_response(egui::Id::new(id))
                .expect("accept row registers its chips")
                .rect
        };
        let accept = rect("overlay_accept");
        let cancel = rect("overlay_cancel");
        let save = rect("overlay_save");
        let upload = rect("overlay_upload");
        let copy = ctx
            .read_response(egui::Id::new("copy_action"))
            .expect("the forest Copy primary stays in the outputs island")
            .rect;
        // One row, Copy trailing the outputs.
        for w in [accept, cancel, save, upload, copy].windows(2) {
            assert!(same_row(w[0], w[1]), "chips share a row");
            assert!(w[0].max.x < w[1].min.x, "chips run left to right");
        }
        // The check/cross pair is one island; the outputs island starts one
        // pod gap later (island pads + POD_GAP make the boundary at
        // least double the intra-island chip spacing).
        let tight = chip_gap(accept, cancel);
        let split = chip_gap(cancel, save);
        assert!(
            split >= 20.0 && split > tight + 4.0,
            "two islands: check-to-cross gap {tight} must be tight, cross-to-Save gap {split} must be a pod gap"
        );
        assert!(
            chip_gap(upload, copy) <= tight + 2.0,
            "outputs sit tight inside their island, Copy last"
        );
    }

    /// Click-through parity for the pod-walked rail: a mid-rail chip
    /// (Pencil, inside the freehand pod) still switches tools with
    /// the editor rail's semantics.
    #[test]
    fn overlay_tool_chip_click_switches_tool() {
        let ctx = egui::Context::default();
        let mut state = blank_state(64, 64);
        state.tool = Tool::Crop;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.horizontal(|ui| {
                let _ = overlay_rail_row(ui, &mut state);
            });
        });
        let pen = ctx
            .read_response(egui::Id::new(("overlay_tool", "P")))
            .expect("pass 1 registered the overlay Pencil chip");
        let at = pen.rect.center();
        let click = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(click, |ui| {
            ui.horizontal(|ui| {
                let _ = overlay_rail_row(ui, &mut state);
            });
        });
        assert_eq!(
            state.tool,
            Tool::Pencil,
            "the pod walk must keep chip clicks switching tools"
        );
    }

    // --- Annotation confinement (Task 005) ---

    #[test]
    fn annotation_press_allowed_inside_selection() {
        assert!(
            annotation_press_allowed(Tool::Rect, Some(sel()), Point::new(150.0, 150.0)),
            "interior press with a draw tool must dispatch"
        );
        // Edges are inclusive (Rect::contains contract).
        assert!(
            annotation_press_allowed(Tool::Rect, Some(sel()), Point::new(100.0, 100.0)),
            "boundary press counts as inside"
        );
    }

    #[test]
    fn annotation_press_denied_outside_selection() {
        assert!(
            !annotation_press_allowed(Tool::Rect, Some(sel()), Point::new(50.0, 50.0)),
            "a press outside the region must not start an annotation"
        );
        assert!(
            !annotation_press_allowed(Tool::Arrow, Some(sel()), Point::new(350.0, 250.0)),
            "just past the far corner is still outside"
        );
    }

    #[test]
    fn annotation_press_denied_without_usable_selection() {
        assert!(
            !annotation_press_allowed(Tool::Marker, None, Point::new(150.0, 150.0)),
            "no selection yet → nothing to draw on"
        );
        assert!(
            !annotation_press_allowed(
                Tool::Marker,
                Some(Rect::from_xywh(10.0, 10.0, 0.5, 0.5)),
                Point::new(10.0, 10.0)
            ),
            "degenerate selection counts as absent"
        );
    }

    #[test]
    fn select_and_move_presses_always_flow() {
        assert!(
            annotation_press_allowed(Tool::Crop, None, Point::new(0.0, 0.0)),
            "Select must keep working before any selection exists"
        );
        assert!(
            annotation_press_allowed(Tool::Crop, Some(sel()), Point::new(50.0, 50.0)),
            "a Select press outside the region starts a fresh selection drag"
        );
        assert!(
            annotation_press_allowed(Tool::Hand, None, Point::new(0.0, 0.0)),
            "Hand must reach the dispatcher to grab annotations"
        );
        assert!(
            annotation_press_allowed(Tool::Hand, Some(sel()), Point::new(50.0, 50.0)),
            "a Hand press outside the region is the dispatcher's (no-op) miss"
        );
    }

    // --- Tool parity (Task 005) ---
    //
    // Drives `EditorState` with the exact event sequences the overlay
    // emits (the same mapping `EditorApp::logic` produces) and asserts
    // each tool commits exactly one annotation of the right kind
    // inside the region. This is the plan's "verify text / number /
    // blur / marker parity with editor window" check — the dispatcher
    // is shared, so pinning the sequence shape pins parity.

    /// A fresh state with `sel()` as the region and `tool` active.
    fn state_with_region(tool: Tool) -> EditorState {
        let mut s = blank_state(1920, 1080);
        s.crop_region = Some(sel());
        s.tool = tool;
        s
    }

    fn press(s: &mut EditorState, at: Point) {
        let _ = s.dispatch_input(&InputEvent::PointerDown {
            pos: at,
            modifiers: Modifiers::default(),
        });
    }

    fn drag(s: &mut EditorState, at: Point) {
        let _ = s.dispatch_input(&InputEvent::PointerDrag { pos: at });
    }

    fn release(s: &mut EditorState) {
        let _ = s.dispatch_input(&InputEvent::PointerUp);
    }

    fn enter(s: &mut EditorState) {
        let _ = s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
    }

    /// Which annotation kind `ann` is, for assert messages.
    fn kind_of(ann: &flintshot_core::Annotation) -> &'static str {
        match ann {
            flintshot_core::Annotation::Rect { .. } => "rect",
            flintshot_core::Annotation::Ellipse { .. } => "ellipse",
            flintshot_core::Annotation::Line { .. } => "line",
            flintshot_core::Annotation::Arrow { .. } => "arrow",
            flintshot_core::Annotation::Marker { .. } => "marker",
            flintshot_core::Annotation::Pencil { .. } => "pencil",
            flintshot_core::Annotation::Text { .. } => "text",
            flintshot_core::Annotation::Blur { .. } => "blur",
            flintshot_core::Annotation::Number { .. } => "number",
            _ => "other",
        }
    }

    #[test]
    fn all_annotation_tools_commit_inside_the_region_via_the_overlay_sequence() {
        let cases: &[(Tool, &str)] = &[
            (Tool::Rect, "rect"),
            (Tool::Ellipse, "ellipse"),
            (Tool::Line, "line"),
            (Tool::Arrow, "arrow"),
            (Tool::Marker, "marker"),
            (Tool::Pencil, "pencil"),
            (Tool::Text, "text"),
            (Tool::Blur, "blur"),
            (Tool::Number, "number"),
        ];
        for &(tool, want) in cases {
            let mut s = state_with_region(tool);
            match tool {
                // Freehand strokes need >1 point to commit.
                Tool::Marker | Tool::Pencil => {
                    press(&mut s, Point::new(120.0, 120.0));
                    drag(&mut s, Point::new(150.0, 130.0));
                    drag(&mut s, Point::new(200.0, 170.0));
                    release(&mut s);
                }
                // Click-to-place: commits on the press itself.
                Tool::Number => press(&mut s, Point::new(150.0, 150.0)),
                // Place caret, type, commit with Enter (the overlay's
                // Enter-before-dispatch close guard routes Enter to the
                // draft while editing).
                Tool::Text => {
                    press(&mut s, Point::new(150.0, 150.0));
                    let _ = s.dispatch_input(&InputEvent::TextInput('h'));
                    let _ = s.dispatch_input(&InputEvent::TextInput('i'));
                    enter(&mut s);
                }
                // Drag-out tools.
                _ => {
                    press(&mut s, Point::new(120.0, 120.0));
                    drag(&mut s, Point::new(180.0, 160.0));
                    release(&mut s);
                }
            }
            assert_eq!(s.anns.len(), 1, "{want} must commit exactly one annotation");
            assert_eq!(
                kind_of(&s.anns[0]),
                want,
                "committed kind must match the tool"
            );
        }
    }

    #[test]
    fn select_tool_updates_region_and_commits_nothing() {
        let mut s = state_with_region(Tool::Crop);
        press(&mut s, Point::new(120.0, 120.0));
        drag(&mut s, Point::new(180.0, 160.0));
        release(&mut s);
        assert!(
            s.anns.is_empty(),
            "Select manipulates the region, never the annotation list"
        );
        assert_eq!(
            s.crop_region,
            Some(Rect::new(
                Point::new(120.0, 120.0),
                Point::new(180.0, 160.0)
            )),
            "a fresh drag replaces the region"
        );
    }

    /// Hand repositions a committed annotation through the overlay's
    /// event sequence and commits nothing new (the separation
    /// contract: Select owns the region, Hand owns the annotations).
    #[test]
    fn move_tool_repositions_annotation_and_commits_nothing() {
        let mut s = state_with_region(Tool::Rect);
        press(&mut s, Point::new(120.0, 120.0));
        drag(&mut s, Point::new(180.0, 160.0));
        release(&mut s);
        s.tool = Tool::Hand;
        // Press at the rect's centre: grab offset = (30, 20).
        press(&mut s, Point::new(150.0, 140.0));
        drag(&mut s, Point::new(160.0, 150.0));
        release(&mut s);
        assert_eq!(s.anns.len(), 1, "Hand repositions, never commits");
        match &s.anns[0] {
            flintshot_core::Annotation::Rect { rect, .. } => {
                assert_eq!(
                    rect.min,
                    Point::new(130.0, 130.0),
                    "moved by the drag delta with the grab point tracking"
                );
            }
            other => panic!("expected a Rect annotation, got {}", kind_of(other)),
        }
    }

    #[test]
    fn text_tool_parity_commits_typed_content() {
        let mut s = state_with_region(Tool::Text);
        press(&mut s, Point::new(150.0, 150.0));
        let _ = s.dispatch_input(&InputEvent::TextInput('h'));
        let _ = s.dispatch_input(&InputEvent::TextInput('i'));
        enter(&mut s);
        match &s.anns[0] {
            flintshot_core::Annotation::Text { text, pos, .. } => {
                assert_eq!(text, "hi", "typed characters must land in the draft");
                assert_eq!(*pos, Point::new(150.0, 150.0), "anchor is the press point");
            }
            other => panic!("expected a Text annotation, got {}", kind_of(other)),
        }
    }

    /// 1920×1080 image-bounds fixture for the move-clamp tests.
    fn image_bounds() -> Rect {
        Rect::from_xywh(0.0, 0.0, 1920.0, 1080.0)
    }

    // --- Output chords (Task 006) ---
    //
    // The overlay routes Ctrl+S / Ctrl+C through the dispatcher and
    // reacts to the returned outcome. These tests pin the event
    // sequences the new key branch emits, so wiring the chords can't
    // regress single-key tool selection or accidentally trigger an
    // output.

    fn key(s: &mut EditorState, ch: char, ctrl: bool) -> flintshot_core::DispatchOutcome {
        s.dispatch_input(&InputEvent::Key {
            key: Key::Char(ch),
            modifiers: Modifiers {
                ctrl,
                ..Modifiers::default()
            },
        })
    }

    #[test]
    fn ctrl_chords_dispatch_to_save_and_copy() {
        assert_eq!(
            key(&mut blank_state(8, 8), 's', true),
            flintshot_core::DispatchOutcome::Save,
            "Ctrl+S must ask the shell to save"
        );
        assert_eq!(
            key(&mut blank_state(8, 8), 'c', true),
            flintshot_core::DispatchOutcome::Copy,
            "Ctrl+C must ask the shell to copy"
        );
    }

    #[test]
    fn plain_letters_produce_no_output_outcome() {
        // The same letters flow through the key branch unmodified too
        // (the tool-switch path consumes them afterwards); the
        // dispatcher must never read them as output chords.
        for ch in ['s', 'c', 'z', 'y'] {
            assert_eq!(
                key(&mut blank_state(8, 8), ch, false),
                flintshot_core::DispatchOutcome::Continue,
                "unmodified {ch} must not trigger an output"
            );
        }
    }

    // --- Shared dim-mask math (region-selection-first Task 001) ---

    #[test]
    fn mask_covers_whole_panel_without_selection() {
        assert_eq!(
            mask_rects(panel(), None),
            vec![panel()],
            "no selection → everything is unselected"
        );
    }

    #[test]
    fn mask_bands_frame_the_selection() {
        let hole = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(200.0, 100.0));
        let bands = mask_rects(panel(), Some(hole));
        assert_eq!(bands.len(), 4, "top, bottom, left, right");
        assert_eq!(
            bands[0],
            egui::Rect::from_two_pos(egui::pos2(0.0, 0.0), egui::pos2(1920.0, 100.0)),
            "top band spans the panel above the hole"
        );
        assert_eq!(
            bands[1],
            egui::Rect::from_two_pos(egui::pos2(0.0, 200.0), egui::pos2(1920.0, 1080.0)),
            "bottom band spans the panel below the hole"
        );
        assert_eq!(
            bands[2],
            egui::Rect::from_two_pos(egui::pos2(0.0, 100.0), egui::pos2(100.0, 200.0)),
            "left band fills the hole's rows left of it"
        );
        assert_eq!(
            bands[3],
            egui::Rect::from_two_pos(egui::pos2(300.0, 100.0), egui::pos2(1920.0, 200.0)),
            "right band fills the hole's rows right of it"
        );
    }

    #[test]
    fn mask_bands_plus_hole_tile_the_panel() {
        let hole = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(200.0, 100.0));
        let bands = mask_rects(panel(), Some(hole));
        let mut covered = 0.0;
        for band in bands.iter().copied().chain([hole]) {
            assert!(
                panel().contains_rect(band),
                "every piece must stay inside the panel"
            );
            covered += band.width() * band.height();
        }
        let total = panel().width() * panel().height();
        assert!(
            (covered - total).abs() < 1e-3,
            "bands + hole must tile the panel exactly (no gap, no overlap): {covered} vs {total}"
        );
    }

    #[test]
    fn mask_bands_at_panel_edge_degenerate_not_inverted() {
        // A hole hugging the panel's top-left: the top and left bands
        // collapse to zero width/height (the painter skips degenerate
        // rects) instead of inverting past the panel.
        let hole = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(300.0, 200.0));
        for band in mask_rects(panel(), Some(hole)) {
            assert!(band.is_finite(), "no NaN bands");
            assert!(
                band.width() <= panel().width() && band.height() <= panel().height(),
                "degenerate bands must not grow past the panel"
            );
        }
    }

    // --- Delete chip (annotation-delete Task 002) ---

    /// Drive the overlay rail row through a synthetic context: pass 1
    /// lays out (registering the chip rects by id), pass 2 clicks the
    /// Delete chip's centre. Returns the row's collected action. The
    /// `ui.horizontal` wrap mirrors `show_toolbar`'s row layout —
    /// driving the row at the root ui directly would stack the pod
    /// islands vertically.
    fn overlay_action_after_delete_click(state: &mut EditorState) -> ToolbarAction {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.horizontal(|ui| {
                let _ = overlay_rail_row(ui, state);
            });
        });
        let del = ctx
            .read_response(egui::Id::new("overlay_delete"))
            .expect("pass 1 registered the overlay Delete chip");
        let at = del.rect.center();
        let click = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..egui::RawInput::default()
        };
        let mut action = ToolbarAction::None;
        let _ = ctx.run_ui(click, |ui| {
            ui.horizontal(|ui| {
                action = overlay_rail_row(ui, state);
            });
        });
        action
    }

    #[test]
    fn overlay_delete_chip_emits_delete_with_selection() {
        let mut app = move_overlay_app();
        app.state.selected = vec![0];
        assert_eq!(
            overlay_action_after_delete_click(&mut app.state),
            ToolbarAction::Delete,
            "a selected annotation makes the chip live and clicking it emits Delete"
        );
    }

    #[test]
    fn overlay_delete_chip_inert_without_selection() {
        let mut state = blank_state(64, 64);
        assert_eq!(
            overlay_action_after_delete_click(&mut state),
            ToolbarAction::None,
            "no selection: the chip must render greyed / hover-only and stay inert"
        );
    }

    #[test]
    fn overlay_delete_action_removes_selected_annotation() {
        let ctx = egui::Context::default();
        let mut app = move_overlay_app();
        app.state.selected = vec![0];
        app.apply_toolbar_action(&ctx, ToolbarAction::Delete);
        assert!(
            app.state.anns.is_empty(),
            "the Delete action's Del replay removes exactly the selected annotation"
        );
        assert!(
            app.state.undo.last().is_some_and(|s| s.anns.len() == 1 && s.image.is_none()),
            "the delete pushed an anns-only snapshot of the pre-delete state (fixture commit aside)"
        );
    }

    // --- Overlay delete UX (annotation-delete Task 004) ---

    /// The overlay's key block must forward `Del` to the dispatcher —
    /// the chip replay alone is not the keyboard path.
    #[test]
    fn overlay_del_key_deletes_selected_annotation() {
        let ctx = egui::Context::default();
        let mut app = move_overlay_app();
        app.state.selected = vec![0];
        key_frame(&ctx, &mut app, 0.0, egui::Key::Delete);
        assert!(app.state.anns.is_empty(), "Del through the overlay deletes");
        assert!(app.state.selected.is_empty(), "delete drops the selection");
        assert!(
            app.state
                .undo
                .last()
                .is_some_and(|s| s.anns.len() == 1 && s.image.is_none()),
            "the delete pushed an anns-only snapshot for Ctrl+Z"
        );
    }

    /// Backspace aliases Del (the Mac "delete" key); the text-editing
    /// modal branch owns it during a draft (pinned below).
    #[test]
    fn overlay_backspace_key_deletes_selected_annotation() {
        let ctx = egui::Context::default();
        let mut app = move_overlay_app();
        app.state.selected = vec![0];
        key_frame(&ctx, &mut app, 0.0, egui::Key::Backspace);
        assert!(
            app.state.anns.is_empty(),
            "Backspace through the overlay deletes the selection"
        );
    }

    /// Any modifier spoils the Del chord (Ctrl combos reserved) — the
    /// dispatcher's own gate, reached because the shell forwards the
    /// frame's modifiers verbatim.
    #[test]
    fn overlay_del_key_with_modifier_is_inert() {
        let ctx = egui::Context::default();
        let mut app = move_overlay_app();
        app.state.selected = vec![0];
        // egui tracks the held modifiers as a top-level RawInput
        // field (egui-winit fills it from the OS modifier state);
        // what real Ctrl-hold delivers on Linux — ctrl plus
        // `command`, the cross-platform chord field
        // `translate_modifiers` maps onto our `ctrl`.
        let raw = egui::RawInput {
            modifiers: egui::Modifiers {
                ctrl: true,
                command: true,
                ..egui::Modifiers::NONE
            },
            events: vec![egui::Event::Key {
                key: egui::Key::Delete,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            app.logic_impl(ui.ctx());
            app.ui_impl(ui);
        });
        assert_eq!(
            app.state.anns.len(),
            1,
            "Ctrl+Del must not delete — the chord is reserved"
        );
    }

    /// Esc layering: with a selection the first Esc clears it and the
    /// overlay stays open (outcome slot untouched); without one, Esc
    /// keeps today's Cancel. `close_from_input` itself stays pure —
    /// the layering lives at its call site.
    #[test]
    fn overlay_esc_with_selection_deselects_then_esc_cancels() {
        let ctx = egui::Context::default();
        let slot: OutcomeSlot = Rc::new(RefCell::new(None));
        let mut app = OverlayApp::new(
            {
                let mut s = blank_state(64, 64);
                s.pending = Some(Annotation::Rect {
                    rect: Rect::from_xywh(10.0, 10.0, 40.0, 30.0),
                    stroke: flintshot_core::Stroke::new(flintshot_core::Color::RED, 3.0),
                    fill: None,
                });
                s.commit_annotation();
                s.tool = Tool::Hand;
                s.selected = vec![0];
                s
            },
            &ctx,
            slot.clone(),
            Rc::new(RefCell::new(None)),
        );
        key_frame(&ctx, &mut app, 0.0, egui::Key::Escape);
        assert!(
            app.state.selected.is_empty(),
            "first Esc clears the selection"
        );
        assert!(
            slot.borrow().is_none(),
            "the outcome slot is untouched — the overlay stays open"
        );
        key_frame(&ctx, &mut app, 1.0 / 60.0, egui::Key::Escape);
        assert!(
            matches!(*slot.borrow(), Some(Ok(EditorOutcome::Cancelled))),
            "second Esc after the deselect cancels the overlay"
        );
    }

    /// Overlay hotkey tool switches route through core `set_tool`:
    /// leaving Hand drops the persistent selection.
    #[test]
    fn overlay_tool_switch_away_from_move_deselects() {
        let ctx = egui::Context::default();
        let mut app = move_overlay_app();
        // The overlay's selection-first lock: annotation tools arm
        // only with a usable region.
        app.state.crop_region = Some(Rect::from_xywh(4.0, 4.0, 40.0, 30.0));
        app.state.selected = vec![0];
        key_frame(&ctx, &mut app, 0.0, egui::Key::R);
        assert_eq!(app.state.tool, Tool::Rect, "fixture: R armed Rect");
        assert!(
            app.state.selected.is_empty(),
            "leaving Hand via hotkey drops the selection"
        );
    }

    /// The persistent selection survives idle frames (the outline
    /// keeps painting), while a selection with no paintable bbox
    /// deselects at paint time instead of rendering.
    #[test]
    fn overlay_selection_survives_frames_and_degenerate_deselects() {
        let ctx = egui::Context::default();
        let mut app = move_overlay_app();
        app.state.selected = vec![0];
        idle_frame(&ctx, &mut app, 0.0);
        idle_frame(&ctx, &mut app, 1.0 / 60.0);
        assert_eq!(
            app.state.selected,
            vec![0],
            "a healthy selection must persist across frames"
        );

        let mut degenerate = move_overlay_app();
        degenerate.state.anns[0] = Annotation::Pencil {
            points: vec![],
            color: flintshot_core::Color::RED,
            width: 2.0,
        };
        degenerate.state.selected = vec![0];
        idle_frame(&ctx, &mut degenerate, 0.0);
        assert!(
            degenerate.state.selected.is_empty(),
            "a degenerate bbox deselects at paint time"
        );
    }

    /// Backspace during a text draft edits the draft — the modal
    /// branch runs before the new key block, so a committed
    /// annotation is never deleted by a draft's Backspace.
    #[test]
    fn overlay_backspace_edits_draft_not_committed_anns() {
        let ctx = egui::Context::default();
        // Constructed with the frames' own ctx: the draft renders
        // through the canvas (font layout on its captured context),
        // unlike the chrome-only fixtures above.
        let mut app = OverlayApp::new(
            {
                let mut s = blank_state(64, 64);
                s.pending = Some(Annotation::Rect {
                    rect: Rect::from_xywh(10.0, 10.0, 40.0, 30.0),
                    stroke: flintshot_core::Stroke::new(flintshot_core::Color::RED, 3.0),
                    fill: None,
                });
                s.commit_annotation();
                s
            },
            &ctx,
            Rc::new(RefCell::new(None)),
            Rc::new(RefCell::new(None)),
        );
        // Place a caret and type "hi" (direct dispatch — fixture
        // setup, the path under test is the Backspace frame below).
        app.state.crop_region = Some(Rect::from_xywh(4.0, 4.0, 40.0, 30.0));
        app.state.tool = Tool::Text;
        let _ = app.state.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(20.0, 20.0),
            modifiers: Modifiers::default(),
        });
        let _ = app.state.dispatch_input(&InputEvent::TextInput('h'));
        let _ = app.state.dispatch_input(&InputEvent::TextInput('i'));
        key_frame(&ctx, &mut app, 0.0, egui::Key::Backspace);
        match &app.state.pending {
            Some(Annotation::Text { text, .. }) => {
                assert_eq!(text, "h", "Backspace must edit the draft");
            }
            other => panic!("draft still active after Backspace, got {other:?}"),
        }
        assert_eq!(
            app.state.anns.len(),
            1,
            "the committed annotation must survive a draft's Backspace"
        );
    }
}
