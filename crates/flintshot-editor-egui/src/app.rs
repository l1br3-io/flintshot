//! The eframe app that drives the editor inside the window.
//!
//! [`EditorApp`] owns the [`EditorState`], the [`EguiCanvas`] it
//! renders through, and the cached background texture handle. The
//! shell constructs one per `run_editor` call; eframe owns it for the
//! window's lifetime and drops it on close.
//!
//! ## egui 0.35 trait split
//!
//! `eframe::App` in 0.35 has no `update` method — it's split into
//! [`App::logic`] (input + state transitions, called even when the UI
//! is hidden) and [`App::ui`] (painting). We follow that split here:
//! Esc handling lives in `logic`, the shared `render()` call lives in
//! `ui`. Future tools (Tasks 7+) keep the same shape — pointer/state
//! events in `logic`, drawing in `ui`.

use std::cell::RefCell;
use std::rc::Rc;

use eframe::App;
use egui::Context;
use flintshot_core::{
    chrono_date, chrono_time, compose_final, compose_final_region, parse_default_mode,
    parse_output_format, parse_size_mode, render::render, render_filename_template,
    save_capture_image, BlurCache, Config, DefaultMode, DispatchOutcome, EditorOutcome,
    EditorState, FlintshotError, InputEvent, Key, Modifiers, NewCaptureMode, Rect, SizeMode, Tool,
};

use crate::canvas::{EguiCanvas, EguiTexture};
use crate::floating;
use crate::home::HomeDirective;
use crate::magnifier::Magnifier;
use crate::overlay::{mask_color, mask_rects, usable_selection};
use crate::panel::{self, ToolbarAction};
use crate::rail;
use crate::topbar;
use crate::topbar::TopbarAction;
use crate::widgets;

/// The selection-first phase's hint line (spec copy). Visible while
/// the phase is live; every clause stays truthful for the whole
/// phase (Esc cancels the editor session both before and after a
/// selection exists).
const SELECTION_HINT: &str = "drag to select · Enter crop · Esc cancel";

/// Hint font size, logical px. Monospace + the overlay badge's size
/// so the phase chrome shares the overlay's rhythm.
const HINT_FONT_SIZE: f32 = 12.0;

/// Gap between the canvas top and the hint pill, logical px —
/// matches the overlay's `BADGE_GAP` rhythm.
const HINT_GAP: f32 = 6.0;

/// Outcome slot shared between the shell (which lives after
/// `eframe::run_native` returns) and the [`EditorApp`] (which is
/// dropped when the window closes). `Rc<RefCell<…>>` is fine here —
/// eframe's native event loop is single-threaded, so we don't need
/// `Arc<Mutex>`'s cross-thread story.
///
/// Carries `Result<EditorOutcome, FlintshotError>` (not just
/// `EditorOutcome`) so render failures inside `ui()` propagate as
/// `Err(_)` through the slot rather than being silently dropped.
/// Window-close-via-X leaves the slot `None`, which the shell treats
/// as `Ok(EditorOutcome::Cancelled)` at the read site.
pub(crate) type OutcomeSlot = Rc<RefCell<Option<flintshot_core::Result<EditorOutcome>>>>;

/// Slot for the `arboard::Clipboard` handle a copy leaves behind,
/// shared between [`OverlayApp`] (which produces it on accept) and
/// `run_overlay_blocking` (which holds it through the post-close
/// linger). Same single-threaded rationale as [`OutcomeSlot`]: the
/// X11 selection owner must outlive the app (eframe drops the app
/// when the window closes), so the handle is handed to the shell
/// before close and kept alive there.
///
/// The editor window needs no such slot: its copy keeps the window
/// (and therefore the app) open, so `EditorApp::clipboard` holds the
/// selection until the user closes the window themselves.
pub(crate) type ClipboardSlot = Rc<RefCell<Option<arboard::Clipboard>>>;

/// The selection-first phase lifecycle (region-selection-first Task
/// 001). An enum rather than two bools so the states can't combine
/// illegally (there is no "ended without being requested").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionPhase {
    /// Not requested — the plain editor (`fullscreen` / `open`
    /// paths).
    Off,
    /// Requested and live: Select pre-armed, dim mask + hint line
    /// painting. Runs from construction until the first crop.
    Live,
    /// A crop landed; the phase is over permanently for this
    /// session (undo does not restart it).
    Ended,
}

/// The eframe app: owns editor state + canvas + cached background
/// handle, runs `render()` each frame, and translates `Esc` (and any
/// future shortcut) into an [`EditorOutcome`] that the shell reads
/// back after the window closes.
// The four bools are independent per-frame lifecycle flags, not a
// combinational state machine — a bitflags dependency would not
// clarify anything.
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct EditorApp {
    /// Source screenshot + annotation model.
    state: EditorState,
    /// Egui-side canvas; painter is bound per-frame.
    canvas: EguiCanvas,
    /// Cached background texture handle (uploaded lazily by
    /// [`render`]). `None` until the first successful upload, and
    /// whenever the shell invalidates the cache (e.g. crop, Phase
    /// 1.F's Task 14).
    bg: Option<EguiTexture>,
    /// Last-seen image dimensions. Detects when crop (Task 14)
    /// replaces `state.image` so the bg texture cache is dropped
    /// and the new image is re-uploaded on the next `render()` call.
    img_dims: (u32, u32),
    /// Screen-logical rect of the most recent `CentralPanel`. Cached
    /// by `ui()` so the next frame's `logic()` can translate pointer
    /// coords into canvas-physical px and reject clicks that land on
    /// the toolbar. `None` until the first `ui()` runs — pointer
    /// dispatch skips that frame (the screenshot isn't visible yet
    /// either, so no drags can start).
    canvas_rect: Option<egui::Rect>,
    /// User view-zoom multiplier over the fit scale (the floating
    /// +/− chips and the mouse wheel step it; 1.0 = fit-to-view).
    /// See [`VIEW_ZOOM_MIN`] / [`VIEW_ZOOM_MAX`].
    view_zoom: f32,
    /// View pan offset (screen-logical px) over the fit letterbox —
    /// the hand-tool / middle-drag / wheel-anchor state. Composed
    /// into the canvas's `content_offset` as `letterbox + pan` by
    /// [`Self::update_fit`] each frame.
    ///
    /// GOTCHA: never set the canvas's `content_offset` directly
    /// outside that composition — the pointer mapping, the chrome,
    /// and the magnifier all read it, and a stale pan would skew
    /// every mapping site. Mutate this field instead (clamped via
    /// [`crate::canvas::clamp_pan`]); the fit pass re-composes.
    pan: egui::Vec2,
    /// An in-flight pan drag (middle button from any tool, or a
    /// primary drag under Hand): the press anchor and the pan at
    /// press, so the drag moves the view by the pointer delta.
    pan_drag: Option<PanDrag>,
    /// Whether the current primary-button press began inside the
    /// canvas. Set when `PointerDown` fires (which only happens on
    /// canvas hits), cleared on `PointerUp`. Without this guard,
    /// `primary_down` alone would emit `PointerDrag` for every frame
    /// the button is held — including presses that began on the
    /// side panel. A panel press would then feed `PointerDrag`s
    /// straight into the dispatcher, corrupting in-flight state:
    /// e.g. an active `Tool::Crop` region would be extended to
    /// a panel-side position while the user is clicking Copy, and
    /// Copy would then read the mangled rect. GOTCHA: the retired
    /// top-rail era described this as "negative canvas-y" — with
    /// the right-docked panel the poisoned coords are positive-x
    /// past `canvas_rect.max.x`, but the `contains` gate needs no
    /// axis-specific logic either way.
    canvas_drag_active: bool,
    /// Whether the active drag began as an intercepted in-text press
    /// (text-selection-highlight): drags then feed
    /// `text_drag_to_char` instead of dispatching `PointerDrag`, and
    /// the release clears the flag without a `PointerUp` (the
    /// selection persists). Cleared on every non-intercepted press.
    drag_in_pending_text: bool,
    /// Press-time double-click detector (text-select-edit): fed
    /// exactly when a `PointerDown` dispatches, fires the synthesized
    /// `DoubleClick` right after it. See `crate::dblclick`.
    dblclick: crate::dblclick::DoubleClickTracker,
    /// Outcome slot shared with the shell. Set on `Esc` or any future
    /// exit path (Save / Copy / …). Defaults to `Ok(Cancelled)` if
    /// the window closes without anything being set (the X button).
    outcome: OutcomeSlot,
    /// Clipboard handle kept alive after a copy so the X11 selection
    /// owner (our process) doesn't release the data before the user
    /// pastes. On X11 the clipboard is owner-based: dropping the
    /// `Clipboard` or exiting the process clears the selection.
    /// `None` until the first successful copy.
    clipboard: Option<arboard::Clipboard>,
    /// Instant (egui input time) of the last successful copy, driving
    /// the time-windowed "Copied" hero state. `None` until the first
    /// copy. GOTCHA: the state is time-windowed, not latched — a
    /// fresh `EditorApp` (every editor route constructs one) starts
    /// unarmed for free, so no explicit clear exists and a stale
    /// "Copied" can never survive into the next session.
    copied_at: Option<f64>,
    /// Precomputed blur images for committed `Annotation::Blur` entries.
    /// Avoids recomputing the expensive Gaussian / pixelate pass every
    /// frame — without it, a few blur regions saturate the CPU at 60 fps
    /// and the editor freezes. Cleared when crop replaces `state.image`.
    blur_cache: BlurCache,
    /// Pixel-zoom magnifier inset state (cached texture handle). Shown
    /// when `magnifier_always_on` is set or `magnifier_key` is held.
    /// Toolkit-owned per ARCHITECTURE.md §What stays toolkit-specific;
    /// reads `state.image` directly, no core abstraction.
    magnifier: Magnifier,
    /// Whether the magnifier shows without a modifier hold (config:
    /// `[ui] magnifier-always-on`).
    magnifier_always_on: bool,
    /// Key whose hold reveals the magnifier (config: `[ui]
    /// magnifier-modifier`). Defaults to `Space`; unknown config
    /// values fall back to `Space` with a warning.
    magnifier_key: egui::Key,
    /// The selection-first phase state (the `region` capture landing
    /// experience: Select pre-armed, dim mask outside the live
    /// selection, hint line). Requested by the app crate via
    /// [`EguiShell::with_selection_first`](crate::EguiShell::with_selection_first);
    /// every other editor entry point leaves it off. See
    /// [`SelectionPhase`] for the lifecycle.
    phase: SelectionPhase,
    /// Single-window page mode: the bridge to the app crate's router
    /// plus the divergent outcome lifecycle (side effects report and
    /// the page stays; see [`EditorPageBridge`]). `None` = the worker
    /// window lifecycle of every pre-single-window path.
    page: Option<crate::shell::PageBridgeSlot>,
    /// Worker-window seam for the topbar overflow's **Settings**
    /// entry: worker windows never route to Settings in-window (spec
    /// §Settings route — there is no home behind a worker), so the
    /// shell hands in the app crate's delegate (the daemon's
    /// `OpenSettings` choreography). Page mode never lands on this —
    /// the router handles Settings before forwarding editor actions.
    /// `None` keeps the entry inert (headless tests).
    open_settings: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    /// Worker-window seam for the breadcrumb's **Home** link: worker
    /// windows have no home route behind them, so the link
    /// delegates the daemon's `OpenApp` choreography (focus a
    /// running app window / spawn one) through this hook and then
    /// leaves via the Esc contract. Page mode never lands on this —
    /// the router intercepts `NavigateHome` before forwarding
    /// editor actions. `None` keeps the link a plain Esc leave
    /// (headless tests).
    open_home: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    /// Whether the stable editor viewport still owes its one-shot
    /// open-time clamp to the monitor (page epochs only; see
    /// [`Self::update_fit`]).
    open_clamp_pending: bool,
    /// The docked side panel's cross-frame state (hover-lift
    /// progress, grain texture).
    panel: panel::PanelState,
    /// The left tool rail's persistent state (grain texture).
    rail: rail::RailState,
    /// The config snapshot this editor session runs on: workers read
    /// it once at construction (no watcher there); routed pages get
    /// live updates from the router after settings saves.
    config: Config,
    /// The exact-PNG byte estimate cache: (dims, bytes). Re-encoded
    /// only when the image's dimensions change.
    statusbar_size: Option<((u32, u32), u64)>,
    /// The floating cluster's chip rects from the last frame —
    /// presses that begin there are chrome, never canvas gestures
    /// (misses between chips pass through to the canvas).
    floating_rects: Vec<egui::Rect>,
}

/// The paper-cutout ring around the image: 8 px cream — the cutout
/// stroke's width — per the mockup's canvas dressing.
pub(crate) const CANVAS_RING: f32 = 8.0;

/// Breathing room between the cutout's ring box and the canvas
/// panel's edge (the surrounding zones): keeps the ring's hairline
/// and the paper shadow's penumbra off the panel border so the
/// capture never sits flush against the window chrome.
pub(crate) const CANVAS_PAD: f32 = 12.0;

/// The view-zoom floor (the − chip's clamp): half the fit size.
pub(crate) const VIEW_ZOOM_MIN: f32 = 0.5;
/// The view-zoom ceiling (the + chip's clamp): four× the fit size.
pub(crate) const VIEW_ZOOM_MAX: f32 = 4.0;
/// One +/− chip step. A power of two, so repeated addition from
/// 1.0 stays exact in f32 and the "back to 100%" path is lossless.
const VIEW_ZOOM_STEP: f32 = 0.25;

/// An in-flight view-pan drag: the pointer anchor and the pan value
/// captured at press, so the drag applies `start_pan + (pointer −
/// anchor)` each frame.
#[derive(Debug, Clone, Copy)]
struct PanDrag {
    /// Press position (screen-logical).
    anchor: egui::Pos2,
    /// Pan offset at press.
    start_pan: egui::Vec2,
}

impl EditorApp {
    /// Construct the app. The canvas captures a clone of the
    /// [`Context`] for texture loading outside the per-frame `ui` call.
    ///
    /// `selection_first` seeds the region-capture landing phase:
    /// `Tool::Crop` is pre-armed (mirroring `OverlayApp::new`, so
    /// the first drag draws a region instead of a rect annotation)
    /// and the dim mask + hint line render until the first crop.
    /// Core stays untouched — the phase is shell-owned state, the
    /// state only seeded with a value the core API already exposes.
    pub(crate) fn new(
        state: EditorState,
        ctx: &Context,
        outcome: OutcomeSlot,
        selection_first: bool,
    ) -> Self {
        Self::new_lifecycle(state, ctx, outcome, selection_first, None)
    }

    /// Page-mode constructor (single-window): the same editor over a
    /// [`crate::shell::EditorPageBridge`] — side-effect outcomes
    /// report through the bridge and the page stays open; directives
    /// (external captures) poll each frame. See [`Self::new`].
    pub(crate) fn new_page(
        state: EditorState,
        ctx: &Context,
        outcome: OutcomeSlot,
        selection_first: bool,
        page: crate::shell::PageBridgeSlot,
    ) -> Self {
        Self::new_lifecycle(state, ctx, outcome, selection_first, Some(page))
    }

    /// The one real constructor behind [`Self::new`] /
    /// [`Self::new_page`].
    fn new_lifecycle(
        mut state: EditorState,
        ctx: &Context,
        outcome: OutcomeSlot,
        selection_first: bool,
        page: Option<crate::shell::PageBridgeSlot>,
    ) -> Self {
        if selection_first {
            state.tool = Tool::Crop;
        }
        // One-time theme application: the graphite + ember styling
        // (see `crate::theme`) persists for the window's lifetime.
        crate::theme::apply(ctx);
        let img_dims = state.image.dimensions();
        // Magnifier config is read once at construction (filesystem
        // read per-frame would be wasteful, and the trigger key /
        // always-on flag don't change mid-session).
        let cfg = Config::load().unwrap_or_default();
        let magnifier_always_on = cfg.ui.magnifier_always_on;
        let magnifier_key = parse_magnifier_key(&cfg.ui.magnifier_modifier);
        Self {
            statusbar_size: None,
            config: cfg.clone(),
            state,
            canvas: EguiCanvas::new(ctx.clone()),
            bg: None,
            img_dims,
            canvas_rect: None,
            view_zoom: 1.0,
            pan: egui::Vec2::ZERO,
            pan_drag: None,
            canvas_drag_active: false,
            dblclick: crate::dblclick::DoubleClickTracker::default(),
            drag_in_pending_text: false,
            outcome,
            clipboard: None,
            copied_at: None,
            blur_cache: BlurCache::new(),
            magnifier: Magnifier::new(),
            magnifier_always_on,
            magnifier_key,
            phase: if selection_first {
                SelectionPhase::Live
            } else {
                SelectionPhase::Off
            },
            open_clamp_pending: page.is_some(),
            page,
            open_settings: None,
            open_home: None,
            panel: panel::PanelState::default(),
            rail: rail::RailState::default(),
            floating_rects: Vec::new(),
        }
    }

    /// Whether the selection-first phase is live: requested at
    /// construction and not yet ended by a crop.
    fn phase_active(&self) -> bool {
        self.phase == SelectionPhase::Live
    }

    /// Whether the view sits at the 1.0 fit zoom — the worker
    /// resize policy's trigger (only the zoomed view letterboxes;
    /// 1.0 keeps the image-sized window contract).
    #[allow(
        clippy::float_cmp,
        reason = "0.25 steps are powers of two; 1.0 stays exact"
    )]
    fn at_fit_zoom(&self) -> bool {
        self.view_zoom == 1.0
    }

    /// The rail's tool gate for the current frame: the live
    /// selection-first phase locks every non-Select chip (selection
    /// is the only job), and the first crop restores the full set.
    fn rail_gate(&self) -> rail::ToolGate {
        rail_gate(self.phase_active())
    }

    /// Latch the selection-first phase over once a crop lands. Crop
    /// detection rides the undo stack: [`EditorState::crop`] is the
    /// only mutation that pushes a `Snapshot` carrying an image, so a
    /// full snapshot on the stack ⇔ a crop happened this session —
    /// image dimensions alone would miss a same-size crop. Called
    /// from both crop sites (`logic`'s Enter dispatch and `ui`'s
    /// toolbar action) so the mask never paints on the crop's own
    /// frame. One-way by design: undo pops the snapshot, the latch
    /// keeps the phase ended. On the latch, a still-`Crop` tool
    /// becomes `Select` — the phase force-armed `Crop` at
    /// construction, so this restores the edit-phase default (the
    /// shell's own override, not the config-gated commit flip). The
    /// still-`Crop` guard is defensive: the input gates (rail
    /// lock, hotkey, pointer) keep the tool `Crop` for the phase's
    /// whole lifetime, so a non-Crop tool at latch time can only
    /// arrive via direct state mutation — and it stays put rather
    /// than being silently flipped. The flip routes through core
    /// `set_tool` so the persistent selection (if any state arrived
    /// that way) obeys the same lifecycle as every other switch.
    fn end_phase_on_crop(&mut self) {
        if self.phase == SelectionPhase::Live && self.state.undo.iter().any(|s| s.image.is_some()) {
            self.phase = SelectionPhase::Ended;
            if self.state.tool == Tool::Crop {
                self.state.set_tool(Tool::Select);
            }
        }
    }

    /// Paint the selection-first phase chrome over the shared render
    /// output: the dim mask outside the live (usable, normalized)
    /// selection — the overlay's mask math verbatim — and the hint
    /// line in a badge-style pill at the canvas's top centre. No-op
    /// outside the phase; see [`Self::phase_active`].
    fn paint_selection_phase(&self, _ctx: &Context, painter: &egui::Painter) {
        let Some(panel) = self.canvas_rect else {
            return;
        };
        let view = self.canvas.view_over(panel);
        let sel_logical = usable_selection(self.state.crop_region).map(|s| {
            egui::Rect::from_two_pos(view.image_to_logical(s.min), view.image_to_logical(s.max))
        });
        for band in mask_rects(panel, sel_logical) {
            painter.rect_filled(band, egui::CornerRadius::ZERO, mask_color());
        }
        let galley = painter.layout_no_wrap(
            SELECTION_HINT.to_owned(),
            egui::FontId::monospace(HINT_FONT_SIZE),
            egui::Color32::WHITE,
        );
        let anchor = egui::pos2(panel.center().x, panel.min.y + HINT_GAP);
        let mut rect = egui::Align2::CENTER_TOP.anchor_size(anchor, galley.size());
        rect.min.x = rect
            .min
            .x
            .clamp(panel.min.x, (panel.max.x - rect.width()).max(panel.min.x));
        painter.rect_filled(
            rect.expand(4.0),
            egui::CornerRadius::same(3),
            crate::theme::palette::readability_black(200),
        );
        painter.galley(rect.min, galley, egui::Color32::WHITE);
    }

    /// Publish an outcome to the shared slot. First writer wins in
    /// practice — every trigger path immediately follows up with
    /// `ViewportCommand::Close`, so a second writer can't race in.
    ///
    /// Also persists the current UI defaults (color / stroke / font)
    /// to config before publishing, so the user's last-used values
    /// survive across sessions (Task 15). Persistence failures are
    /// logged as warnings and never block the primary outcome.
    fn publish_outcome(&self, outcome: flintshot_core::Result<EditorOutcome>) {
        self.persist_ui_config();
        *self.outcome.borrow_mut() = Some(outcome);
    }

    /// Like [`publish_outcome`](Self::publish_outcome) but does not
    /// overwrite a previously-published outcome. Used by the Esc / X
    /// close path so that a prior copy (which publishes
    /// `CopiedToClipboard` and keeps the window open) is honoured
    /// rather than being clobbered by `Cancelled`.
    fn publish_outcome_if_unset(&self, outcome: flintshot_core::Result<EditorOutcome>) {
        self.persist_ui_config();
        let mut slot = self.outcome.borrow_mut();
        if slot.is_none() {
            *slot = Some(outcome);
        }
    }

    /// Persist the current `color` / `stroke_width` / `font_size` to
    /// the user's config file (Task 15).
    ///
    /// Loads the existing config first so unrelated sections
    /// (`[capture]`, future tables) are preserved, then overwrites
    /// just the `[ui]` defaults. Failures are surfaced as
    /// `tracing::warn!` — non-blocking, matching the acceptance
    /// criterion that annotation continues to work even when the
    /// config file is unwritable.
    fn persist_ui_config(&self) {
        let mut cfg = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "could not load config for persistence; skipping save"
                );
                return;
            }
        };
        cfg.ui.default_color = self.state.color;
        cfg.ui.default_text_color = self.state.text_color;
        cfg.ui.default_stroke_width = self.state.stroke_width;
        cfg.ui.default_font_size = self.state.font_size;
        if let Err(e) = cfg.save() {
            tracing::warn!(
                error = %e,
                "could not save config; UI defaults not persisted this session"
            );
        }
    }

    /// Detect image-dimension changes (crop / open / paste swaps)
    /// and drop the bg texture + blur caches: the stale pixels must
    /// not survive the image they were derived from.
    fn invalidate_caches_on_dims_change(&mut self) {
        let current_dims = self.state.image.dimensions();
        if current_dims != self.img_dims {
            self.bg = None;
            self.blur_cache.clear();
            self.img_dims = current_dims;
        }
    }

    /// The view-pan + wheel-zoom input pass (any tool): the wheel
    /// steps the view zoom one quantized step anchored at the
    /// cursor, a middle-button drag pans, and a live pan drag
    /// (started by middle or by Hand's primary press) follows the
    /// pointer. Pure math lives in [`crate::canvas`]
    /// (`zoom_anchored_topleft`, `clamp_pan`, `letterbox_offset`);
    /// this owns only the input translation and the pan/zoom state.
    fn handle_view_input(&mut self, ctx: &Context) {
        // Wheel zoom: one notch = one step, toward the cursor. Only
        // over the canvas — the side panel / toolbar keep their own
        // scroll semantics.
        let wheel = ctx.input(|i| {
            i.raw.events.iter().rev().find_map(|e| match e {
                egui::Event::MouseWheel { delta, .. } => Some(*delta),
                _ => None,
            })
        });
        if let (Some(delta), Some(cursor), Some(rect)) = (
            wheel,
            ctx.input(|i| i.pointer.latest_pos()),
            self.canvas_rect,
        ) {
            if rect.contains(cursor) && delta.y != 0.0 {
                let step = if delta.y > 0.0 {
                    VIEW_ZOOM_STEP
                } else {
                    -VIEW_ZOOM_STEP
                };
                self.zoom_at_cursor(step, cursor, rect, ctx.pixels_per_point());
            }
        }

        // Middle-button pan (any tool): press anchors, drag applies
        // `start_pan + (pointer − anchor)`, release ends — applying
        // the final position first, so a press-move-release flick
        // with no intermediate frame still lands the delta.
        let (middle_pressed, middle_down, middle_released) = ctx.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Middle),
                i.pointer.button_down(egui::PointerButton::Middle),
                i.pointer.button_released(egui::PointerButton::Middle),
            )
        });
        let cursor = ctx.input(|i| i.pointer.latest_pos());
        if middle_pressed && cursor.is_some_and(|c| self.canvas_rect.is_some_and(|r| r.contains(c)))
        {
            let pos = cursor.unwrap_or_default();
            self.pan_drag = Some(PanDrag {
                anchor: pos,
                start_pan: self.pan,
            });
        }
        // A live pan drag is the middle button held, or a Hand
        // primary press held (the press itself is intercepted in the
        // dispatch block below).
        let (primary_down, primary_released) =
            ctx.input(|i| (i.pointer.primary_down(), i.pointer.primary_released()));
        let pan_live = middle_down || primary_down;
        let pan_ended = middle_released || primary_released;
        if (pan_ended || pan_live) && self.pan_drag.is_some() {
            if let (Some(drag), Some(pos)) = (self.pan_drag, cursor) {
                self.apply_pan(drag.start_pan + (pos - drag.anchor), ctx);
            }
        }
        if pan_ended || !pan_live {
            // Release — or the button vanishing without one (focus
            // loss): the drag ends after the final apply above.
            self.pan_drag = None;
        }
    }

    /// Step the view zoom by `step` (signed), keeping the image
    /// point under `cursor` fixed. Returning to the 1.0 fit zoom
    /// resets pan to zero (the image fits again; the worker resize
    /// policy's trigger stays exact). No-op at the clamp bounds.
    #[allow(
        clippy::cast_precision_loss,
        reason = "image dims ≤ 2^24 are exact in f32"
    )]
    fn zoom_at_cursor(&mut self, step: f32, cursor: egui::Pos2, rect: egui::Rect, ppp: f32) {
        let new_zoom = (self.view_zoom + step).clamp(VIEW_ZOOM_MIN, VIEW_ZOOM_MAX);
        // The 0.25 steps are powers-of-two fractions — exact in f32 —
        // so equality is the quantization contract, not drift.
        #[expect(clippy::float_cmp, reason = "0.25-stepped zoom values are f32-exact")]
        let at_bound = new_zoom == self.view_zoom;
        if at_bound {
            return;
        }
        if (new_zoom - 1.0).abs() < f32::EPSILON {
            self.view_zoom = new_zoom;
            self.pan = egui::Vec2::ZERO;
            return;
        }
        // Recover the fit from the live scale so the anchor math
        // matches what `update_fit` will compose next frame.
        let fit = self.canvas.scale() / self.view_zoom;
        let scale_before = self.canvas.scale();
        let scale_after = fit * new_zoom;
        let (w, h) = self.state.image.dimensions();
        let (w, h) = (w as f32, h as f32);
        // Cursor + current image top-left in canvas-local logical
        // px; scales convert to logical-per-image-px.
        let topleft_before = self.canvas.content_offset();
        let cursor_local = cursor - rect.min;
        let topleft_after = crate::canvas::zoom_anchored_topleft(
            cursor_local,
            topleft_before,
            scale_before / ppp,
            scale_after / ppp,
        );
        // The letterbox `update_fit` will compute at the new scale,
        // so pan lands as (anchored top-left − letterbox).
        let ring_l_after = CANVAS_RING * scale_after / ppp;
        let img_after = egui::vec2(w * scale_after / ppp, h * scale_after / ppp);
        let box_after = img_after + egui::vec2(2.0 * ring_l_after, 2.0 * ring_l_after);
        let pad_l = CANVAS_PAD / ppp;
        let letterbox_after =
            crate::canvas::letterbox_offset(rect.size(), box_after, pad_l, ring_l_after + pad_l);
        self.view_zoom = new_zoom;
        self.pan =
            crate::canvas::clamp_pan(topleft_after - letterbox_after, img_after, rect.size());
    }

    /// Set the pan (clamped to keep the image intersecting the
    /// viewport) — the single write path for drag-driven pans.
    #[allow(
        clippy::cast_precision_loss,
        reason = "image dims ≤ 2^24 are exact in f32"
    )]
    fn apply_pan(&mut self, pan: egui::Vec2, ctx: &Context) {
        let Some(rect) = self.canvas_rect else {
            return;
        };
        let (w, h) = self.state.image.dimensions();
        let scale = self.canvas.scale() / ctx.pixels_per_point();
        let img_logical = egui::vec2(w as f32 * scale, h as f32 * scale);
        self.pan = crate::canvas::clamp_pan(pan, img_logical, rect.size());
    }

    /// The worker window's image-sized viewport policy (unchanged,
    /// spec: out of scope): at the 1.0 view zoom, resize until the
    /// panel exactly fits the image; any other zoom belongs to
    /// [`Self::update_fit`]'s letterbox path (never resize — the
    /// window's size is stable, only the view transform moves).
    #[allow(
        clippy::cast_precision_loss,
        reason = "image dims ≤ 2^24 are exact in f32"
    )]
    fn handle_image_resize(&mut self, ctx: &Context) {
        self.invalidate_caches_on_dims_change();
        let current_dims = self.state.image.dimensions();
        // Target panel size in logical px = image physical dims / ppp,
        // plus the cutout ring's margin and the panel padding on
        // every side so the dressing (ring + shadow penumbra) stays
        // inside the canvas with air to spare.
        let ppp = ctx.pixels_per_point();
        let target_w = current_dims.0 as f32 / ppp + 2.0 * CANVAS_RING + 2.0 * CANVAS_PAD;
        let target_h = current_dims.1 as f32 / ppp + 2.0 * CANVAS_RING + 2.0 * CANVAS_PAD;
        // Current canvas panel width/height (logical). When the resize
        // has taken effect, these match the target.
        let (cur_w, cur_h) = self
            .canvas_rect
            .map_or((0.0, 0.0), |r| (r.width(), r.height()));
        if (cur_w - target_w).abs() < 1.0 && (cur_h - target_h).abs() < 1.0 {
            return;
        }
        // The prior resize hasn't landed yet: keep issuing InnerSize
        // until the panel reports the right size. GOTCHA: the old
        // top-rail layout added the toolbar's height below
        // `canvas_rect.min.y` here; the docked side panel instead
        // reserves width to the *right* of the canvas, the topbar
        // reserves a strip above and the statusbar below — the
        // window is image + the panel width and image +
        // [`topbar::TOPBAR_HEIGHT`] + [`statusbar::STATUSBAR_HEIGHT`]
        // tall.
        let new_size = egui::vec2(
            target_w + panel::PANEL_WIDTH,
            target_h + crate::topbar::TOPBAR_HEIGHT + crate::statusbar::STATUSBAR_HEIGHT,
        );
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(new_size));
        ctx.request_repaint();
    }

    /// The per-frame fit (both lifecycles): compute the fit-to-view
    /// scale from the live canvas rect, multiply by the user's view
    /// zoom, and center the ring box inside the padded canvas
    /// (letterbox). Never issues a resize — the window's size is the
    /// user's (page epochs) or [`Self::handle_image_resize`]'s
    /// (workers at 1.0); only the view transform moves. Also owns
    /// the one-shot open-time clamp of the default window size to a
    /// small monitor (egui reports `monitor_size` only once the loop
    /// runs; the default fits real panels).
    #[allow(
        clippy::cast_precision_loss,
        reason = "image dims ≤ 2^24 are exact in f32"
    )]
    fn update_fit(&mut self, ctx: &Context) {
        self.clamp_open_to_monitor_once(ctx);
        self.invalidate_caches_on_dims_change();
        let Some(rect) = self.canvas_rect else {
            return;
        };
        let (w, h) = self.state.image.dimensions();
        let ppp = ctx.pixels_per_point();
        // The fit unit is the ring box (image + the cutout's cream
        // ring on every side), fitted inside the panel minus the
        // padding — the dressing stays inside the canvas with air to
        // spare instead of sitting flush against the window chrome.
        let (bw, bh) = (w as f32 + 2.0 * CANVAS_RING, h as f32 + 2.0 * CANVAS_RING);
        let pad_l = CANVAS_PAD / ppp;
        let fit = crate::canvas::fit_scale(
            (bw, bh),
            (
                (rect.width() - 2.0 * pad_l).max(0.0),
                (rect.height() - 2.0 * pad_l).max(0.0),
            ),
        );
        let scale = match fit {
            Some(fit) => fit * self.view_zoom,
            // Degenerate viewport: keep the previous transform
            // rather than collapsing the canvas.
            None => self.canvas.scale(),
        };
        let ring_l = CANVAS_RING * scale / ppp;
        let img_logical = egui::vec2(w as f32 * scale / ppp, h as f32 * scale / ppp);
        let box_logical = img_logical + egui::vec2(2.0 * ring_l, 2.0 * ring_l);
        // Pan composes over the letterbox: the fit centers (or
        // anchors) the ring box, pan slides it. Clamped every frame
        // so a window resize or a zoom change can never strand the
        // image off-viewport; at the 1.0 fit zoom the image always
        // fits, so the clamp pins pan at zero (the resize policy's
        // trigger stays exact).
        self.pan = crate::canvas::clamp_pan(self.pan, img_logical, rect.size());
        let offset =
            crate::canvas::letterbox_offset(rect.size(), box_logical, pad_l, ring_l + pad_l);
        // "Changed" tolerates float jitter: sub-epsilon drift must
        // not request repaints every frame, and a quarter-pixel
        // letterbox shift is invisible.
        let scale_drifted = (scale - self.canvas.scale()).abs() > 1e-4;
        let offset_drifted =
            (offset + self.pan - self.canvas.content_offset()).length_sq() > 0.25_f32 * 0.25;
        if scale_drifted || offset_drifted {
            self.canvas.set_scale(scale);
            self.canvas.set_content_offset(offset + self.pan);
            // The correcting frame must come immediately (window
            // resizes and crop landings otherwise wait for input).
            ctx.request_repaint();
        }
    }

    /// Pull the freshly-opened editor window inside a small monitor
    /// (VM / embedded), once. Never fights a user resize: the flag
    /// clears the first frame the monitor is known, and a maximized
    /// or fullscreen state is left alone.
    fn clamp_open_to_monitor_once(&mut self, ctx: &Context) {
        if !self.open_clamp_pending {
            return;
        }
        let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) else {
            return;
        };
        self.open_clamp_pending = false;
        if ctx.input(|i| i.viewport().maximized == Some(true))
            || ctx.input(|i| i.viewport().fullscreen == Some(true))
        {
            return;
        }
        let clamped =
            crate::shell::clamp_default_size(crate::shell::EDITOR_PAGE_DEFAULT_SIZE, Some(monitor));
        if clamped != crate::shell::EDITOR_PAGE_DEFAULT_SIZE {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(clamped));
        }
    }

    /// The paper shadow under the capture (the dressing's lowest
    /// layer). Painted inside the canvas panel *before* the render
    /// pass so the image covers the shadow shape's interior —
    /// `Shadow::as_shape` fills its whole rect with a translucent
    /// tint, which over the blit would read as an overlay. Hidden
    /// during the selection-first phase (the dim mask owns the
    /// moment). One frame behind the layout (the cached canvas rect).
    fn paint_paper_shadow(&self, painter: &egui::Painter) {
        if self.phase_active() {
            return;
        }
        let Some(image_rect) = self.image_screen_rect() else {
            return;
        };
        let radius = egui::CornerRadius::same(crate::theme::CUTOUT_RADIUS);
        painter.add(crate::theme::SHADOW_PAPER.as_shape(image_rect.expand(CANVAS_RING), radius));
    }

    /// The canvas paper dressing: the capture as a paper cutout —
    /// cream ring and forest hairline around the image. Chrome-only:
    /// painted over the composed output, never in `render()` /
    /// saves. Hidden during the selection-first phase (the dim mask
    /// owns the moment). Clipped to the canvas panel so the ring (and
    /// its hairline) never bleeds over the surrounding zones.
    fn paint_canvas_dressing(&self, painter: &egui::Painter) {
        use crate::theme::palette as p;
        if self.phase_active() {
            return;
        }
        let Some(image_rect) = self.image_screen_rect() else {
            return;
        };
        let Some(canvas_rect) = self.canvas_rect else {
            return;
        };
        let painter = painter.with_clip_rect(canvas_rect);
        let radius = egui::CornerRadius::same(crate::theme::CUTOUT_RADIUS);
        let ring = image_rect.expand(CANVAS_RING);
        // The cream ring around the image: a stroked band strictly
        // outside the image rect. A fill over `ring` would cover the
        // capture entirely, and an inside stroke would eat its border
        // (spec: the dressing never goes over the image). The band
        // also masks the blit's square corners into the cutout's
        // rounded ones.
        painter.rect(
            image_rect,
            radius,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(CANVAS_RING, p::PAPER_CREAM),
            egui::StrokeKind::Outside,
        );
        // The forest hairline at the ring's outer edge.
        painter.rect(
            ring,
            radius,
            egui::Color32::TRANSPARENT,
            crate::theme::HAIRLINE_STROKE,
            egui::StrokeKind::Middle,
        );
    }

    /// The image's on-screen logical rect (the fit transform
    /// applied). `None` before the first layout pass.
    fn image_screen_rect(&self) -> Option<egui::Rect> {
        let canvas_rect = self.canvas_rect?;
        let view = self.canvas.view_over(canvas_rect);
        let (w, h) = self.state.image.dimensions();
        #[allow(
            clippy::cast_precision_loss,
            reason = "image dims ≤ 2^24 are exact in f32"
        )]
        #[expect(
            clippy::cast_precision_loss,
            reason = "image dims ≤ 2^24 are exact in f32"
        )]
        let corners = {
            let (w, h) = (w as f32, h as f32);
            (
                view.image_to_logical(flintshot_core::Point::new(0.0, 0.0)),
                view.image_to_logical(flintshot_core::Point::new(w, h)),
            )
        };
        Some(egui::Rect::from_two_pos(corners.0, corners.1))
    }

    /// Paint the magnifier inset when its trigger is active and the
    /// cursor is over the canvas. No-op otherwise. Reads
    /// `state.image` directly (the magnifier is toolkit-owned per
    /// ARCHITECTURE.md §What stays toolkit-specific).
    ///
    /// Suppressed during text-editing so holding the trigger key while
    /// typing doesn't pop the loupe over the caret.
    fn paint_magnifier_if_active(&mut self, ctx: &Context, painter: &egui::Painter) {
        if self.state.is_editing_text() {
            return;
        }
        let active = self.magnifier_always_on || ctx.input(|i| i.key_down(self.magnifier_key));
        if !active {
            return;
        }
        let Some(canvas_rect) = self.canvas_rect else {
            return;
        };
        let view = self.canvas.view_over(canvas_rect);
        let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) else {
            return;
        };
        // Only when the cursor is inside the canvas panel: showing the
        // loupe while the pointer is over the toolbar would cover the
        // buttons the user is interacting with.
        if !canvas_rect.contains(pos) {
            return;
        }
        // Screen-logical → image-physical through the shared view,
        // matching the pointer translation in `logic()`.
        let cursor_phys = view.logical_to_image(pos);
        self.magnifier
            .show(painter, ctx, &self.state.image, cursor_phys, view);
    }

    /// Paint the Hand-tool hover / grab outline (see
    /// [`crate::chrome`]) over the shared render output. The hover
    /// variant is gated exactly like the cursor in [`Self::logic_impl`]
    /// — pointer inside the canvas (never the toolbar: it sits outside
    /// `canvas_rect`), no drag in flight — so outline and cursor can
    /// never disagree. The grab variant needs no gate: a live
    /// `moving` drag is its own condition and must keep tracking
    /// through the whole drag.
    fn paint_move_chrome_if_any(&self, ctx: &Context, painter: &egui::Painter) {
        let Some(canvas_rect) = self.canvas_rect else {
            return;
        };
        let view = self.canvas.view_over(canvas_rect);
        let pos = ctx.input(|i| i.pointer.latest_pos());
        let hover_at = pos
            .filter(|p| canvas_rect.contains(*p) && !self.canvas_drag_active)
            // Screen-logical → image-physical, matching `logic_impl`.
            .map(|p| view.logical_to_image(p));
        let Some((rects, kind)) = crate::chrome::move_chrome(&self.state, hover_at) else {
            // Text-tool hover affordance (text-select-edit): same gate,
            // same dashed rhythm, Hover kind — one query, one paint
            // helper, zero new visuals.
            if let Some(rect) = crate::chrome::text_hover_chrome(&self.state, hover_at) {
                crate::chrome::paint_move_chrome(
                    painter,
                    egui::Rect::from_two_pos(
                        view.image_to_logical(rect.min),
                        view.image_to_logical(rect.max),
                    ),
                    crate::chrome::MoveChromeKind::Hover,
                );
            }
            return;
        };
        for rect in rects {
            crate::chrome::paint_move_chrome(
                painter,
                egui::Rect::from_two_pos(
                    view.image_to_logical(rect.min),
                    view.image_to_logical(rect.max),
                ),
                kind,
            );
        }
    }

    /// Paint the persistent-selection outlines (annotation-delete
    /// spec, pluralized by multi-select-properties): one dashed
    /// ember rect per selected annotation over the shared render
    /// output, painted after the move chrome so a grab's solid ember
    /// outline visually wins over the dashed selection outline of
    /// the same annotation. A selection that yields no chrome at
    /// all while a selection-owning tool is active (degenerate
    /// bboxes / stale indices) deselects instead of rendering — the
    /// selection must never point at something unoutlinable.
    fn paint_selected_chrome_if_any(&mut self, _ctx: &Context, painter: &egui::Painter) {
        let rects = crate::chrome::selected_chrome(&self.state);
        if rects.is_empty() {
            if matches!(self.state.tool, Tool::Select | Tool::Text)
                && !self.state.selected.is_empty()
            {
                self.state.selected.clear();
            }
            return;
        }
        let Some(canvas_rect) = self.canvas_rect else {
            return;
        };
        let view = self.canvas.view_over(canvas_rect);
        for rect in rects {
            crate::chrome::paint_selected_chrome(
                painter,
                egui::Rect::from_two_pos(
                    view.image_to_logical(rect.min),
                    view.image_to_logical(rect.max),
                ),
            );
        }
    }
}

impl EditorApp {
    /// Headless-callable core of [`App::logic`](EditorApp) — the
    /// `eframe::Frame` parameter is unused, so the split (mirroring
    /// `OverlayApp`'s) lets the shell-side event translation be
    /// unit-tested with a synthetic `egui::Context`. `pub(crate)`:
    /// the window router delegates here on the Editor route.
    pub(crate) fn logic_impl(&mut self, ctx: &Context) {
        // Page mode first: directives (external captures) must land
        // even mid-text-draft, and idle pages keep a slow heartbeat so
        // they do.
        self.poll_page_directives(ctx);

        // Translate egui's input events to the toolkit-agnostic
        // `InputEvent` and feed them through `state.dispatch_input`.
        // Per the Task 10 close-out's plan, this is the pure-dispatcher
        // extraction the Task 5 / 7 / 8 close-outs flagged forward:
        // the dispatcher does all state mutation; the shell just
        // performs the side-effect the returned `DispatchOutcome`
        // names. Tested end-to-end by the dispatcher unit tests in
        // `flintshot-core::editor` plus the translation tests below
        // (the Enter-crop gap shipped because only the dispatcher
        // side was tested — see `enter_key_crops_through_shell`).
        //
        // Key order: keyboard first, then pointer. A `Ctrl+S` mid-drag
        // should pre-empt the pointer-up that follows; the dispatcher's
        // `Save` outcome short-circuits via `return` below before the
        // pointer branch runs.

        let modifiers = translate_modifiers(ctx.input(|i| i.modifiers));

        // If the user switched tools via the toolbar while a text draft
        // was active, commit the draft before anything else. The toolbar
        // sets `state.tool` directly (bypassing the dispatcher), so the
        // modal text-editing guard below would otherwise trap the user
        // in text mode forever with a non-Text tool selected.
        if self.state.is_editing_text() && self.state.tool != Tool::Text {
            self.state.commit_text_draft();
        }

        // Text-editing modal mode (Task 12): when the user has placed
        // a caret (Tool::Text click → pending Text annotation), route
        // only text-affecting events and suppress the shortcut / pointer
        // dispatch. The dispatcher does the same filtering on its side
        // (`EditorState::is_editing_text`); we replicate the branch
        // here so we don't steal an `s` keystroke for the Save shortcut
        // while the user is typing.
        if self.state.is_editing_text() {
            dispatch_text_input(&mut self.state, ctx, modifiers);
            // Pointer input stays live mid-edit (text-selection-highlight):
            // the pump intercepts in-text presses for the selection
            // APIs and dispatches outside presses into the modal
            // commit-and-move-on arm. Everything else (shortcuts,
            // tool keys, pan) stays suppressed by this early return.
            self.handle_pointer_input(ctx);
            return;
        }

        // One key event per frame: egui's `key_pressed` reports the
        // most recent press. If multiple keys fire on the same frame
        // (unusual), only the last one dispatches — matches the
        // existing Task 8 single-key behaviour. Outcome side effects
        // (close / save / copy) end the frame.
        if self.dispatch_shortcut_key(ctx, modifiers) {
            return;
        }

        // Ctrl+N (new capture) — after the dispatcher's chords (it
        // doesn't translate N) and before the tool keys, so plain N
        // still switches tools while the chord starts a capture.
        if self.new_capture_shortcut(ctx, modifiers) {
            return;
        }

        // Page chrome accelerators (single-window): plain C / O
        // mirror the home's actions; P stays the Pencil tool key.
        if self.page_action_shortcut(ctx, modifiers) {
            return;
        }

        // Selection-first phase: the dispatcher's Enter-crop is one
        // of the two phase-ending sites (the other is `ui`'s toolbar
        // Crop button). Checked immediately after the dispatch so the
        // mask never paints on the crop's own frame.
        self.end_phase_on_crop();

        // View pan + wheel zoom (any tool; Hand's primary drag pans —
        // the shell owns panning and never dispatches Hand presses).
        self.handle_view_input(ctx);

        // Single-key tool selection: unmodified S R E A L M P T B N
        // switch tools (advertised in the rail tooltips). Guarded on
        // the *event's own* modifiers so Ctrl+S / Ctrl+Shift+Z chords
        // keep their dispatcher meanings; text-editing mode returned
        // early above, so typed letters can't switch tools mid-draft.
        // While the selection-first phase is live, every mapping
        // except Crop is ignored — the region is the phase's only
        // job (C stays harmless: Crop is what the phase armed).
        // Post-crop the keys work again. Switches route through core
        // `set_tool`: leaving Hand drops the persistent selection.
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
            if !self.phase_active() || tool == Tool::Crop {
                self.state.set_tool(tool);
            }
        }

        self.handle_pointer_input(ctx);
    }

    /// In-text press interception (text-selection-highlight): resolve
    /// `at` against the pending text draft and, on a hit, reposition
    /// the caret via [`EditorState::text_press_at_char`] instead of
    /// dispatching `PointerDown`. Returns `true` when the press was
    /// consumed. The tracker is still fed (on the logical press
    /// position) so consecutive in-text clicks count — a double-click
    /// then dispatches [`InputEvent::DoubleClick`] into the modal
    /// path, which selects the word around the just-collapsed cursor;
    /// count 3 likewise selects the line.
    fn intercept_text_press(
        &mut self,
        ctx: &Context,
        view: crate::canvas::CanvasView,
        at: flintshot_core::Point,
        pointer_events: &mut Vec<InputEvent>,
    ) -> bool {
        let Some(idx) = crate::texthit::pending_text_hit(&self.state, ctx, view, at) else {
            return false;
        };
        self.state.text_press_at_char(idx);
        self.drag_in_pending_text = true;
        let (max_delay, max_dist) = ctx.options(|o| {
            (
                o.input_options.max_double_click_delay,
                o.input_options.max_click_dist,
            )
        });
        let now = ctx.input(|i| i.time);
        let logical = view.image_to_logical(at);
        match self.dblclick.on_press(now, logical, max_delay, max_dist) {
            2 => pointer_events.push(InputEvent::DoubleClick { pos: at }),
            3 => pointer_events.push(InputEvent::TripleClick { pos: at }),
            _ => {}
        }
        true
    }

    /// Extend the selection to the drag position, clamped to the
    /// block edge when the pointer leaves the text (dragging off the
    /// block extends to its boundary, never cancels). No-op when the
    /// draft vanished mid-drag (a stray synthetic drag).
    fn drag_pending_text_to(
        &mut self,
        ctx: &Context,
        view: crate::canvas::CanvasView,
        at: flintshot_core::Point,
    ) {
        let idx = crate::texthit::pending_text_hit_or_edge(&self.state, ctx, view, at);
        self.state.text_drag_to_char(idx);
    }

    /// Pointer dispatch: the pump that translates egui's screen-
    /// logical pointer state into canvas-physical [`InputEvent`]s for
    /// the dispatcher — plus the in-text press interception and
    /// drag-select routing while a text draft is being edited
    /// (text-selection-highlight). Extracted from `logic_impl` so the
    /// text-editing modal branch can run it too: pointer input is the
    /// one input class that stays live mid-edit (caret placement,
    /// selection gestures, click-away commit), while shortcuts, tool
    /// keys, and view pan stay suppressed.
    fn handle_pointer_input(&mut self, ctx: &Context) {
        let modifiers = translate_modifiers(ctx.input(|i| i.modifiers));
        // Pointer dispatch: translate egui's screen-logical coords to
        // canvas-physical and feed the state machine. `canvas_rect` is
        // cached by the previous frame's `ui()`; on the first frame
        // it's `None` and we skip — see `EditorApp::canvas_rect`.
        let Some(canvas_rect) = self.canvas_rect else {
            return;
        };
        let view = self.canvas.view_over(canvas_rect);

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

        // Screen-logical → image-physical through the shared view:
        // subtract the canvas origin (so pointer (0,0) means canvas
        // top-left, not screen top-left), then scale by
        // `pixels_per_point` (and the fit scale once Task 004 wires
        // it) to land in the image-px space `EditorState` operates on.
        let at = view.logical_to_image(pos);

        // Begin only when the press lands inside the canvas — clicks on
        // the toolbar (or outside the window) shouldn't start a drag.
        // Once begun, updates and commit proceed even if the pointer
        // leaves the canvas: the rect just extends, and egui clips at
        // the panel edge on render.
        //
        // `canvas_drag_active` gates drag/up events on whether the
        // *press* began in the canvas. Without it, a toolbar click
        // (Copy / Crop / tool selectors / pickers) would emit
        // `PointerDrag`s for every held frame and a `PointerUp` on
        // release, all routed into the dispatcher. For `Tool::Crop`
        // this updates `crop_region.max` to the toolbar's (negative
        // canvas-y) position, so by the time the toolbar's click
        // handler runs `handle_copy` / `confirm_crop`, the rect
        // no longer matches what the user dragged.
        // The floating quick cluster floats over the canvas: its
        // chips sit inside `canvas_rect`, so the press gate must
        // exclude them explicitly or a chip press would also start
        // an annotation drag. Misses between chips pass through
        // (the pointer-miss passthrough contract).
        let on_floating = self.floating_rects.iter().any(|r| r.contains(pos));
        // Selection-first phase drag gate: a live phase with a
        // non-Crop tool skips pointer dispatch entirely — the
        // phase is annotation-proof by drag as well as by chip and
        // hotkey. Unreachable through input today (the rail lock and
        // the hotkey gate above keep the tool Crop); this is the
        // backstop for state that got to a non-Crop tool by other
        // means. `canvas_drag_active` below still tracks press /
        // release, so no stale drag state survives the gate.
        let annotation_gated = self.phase_active() && self.state.tool != Tool::Crop;
        let mut pointer_events: Vec<InputEvent> = Vec::new();
        if primary_pressed && canvas_rect.contains(pos) && !on_floating {
            if self.state.tool == Tool::Hand {
                // Pan-only contract: a Hand press starts a view pan —
                // no `PointerDown` ever reaches the dispatcher, so
                // annotations are untouchable under Hand (core no-ops
                // them defensively for synthetic state).
                self.pan_drag = Some(PanDrag {
                    anchor: pos,
                    start_pan: self.pan,
                });
            } else {
                self.canvas_drag_active = true;
                if !annotation_gated {
                    // Press-interception contract (text-selection-highlight):
                    // while a text draft is being edited, a press the
                    // resolver maps inside the pending text is consumed
                    // here — no `PointerDown` dispatches, so the
                    // commit-and-move-on path is bypassed and the caret
                    // repositions instead. Presses resolving outside
                    // behave exactly as before.
                    if !self.intercept_text_press(ctx, view, at, &mut pointer_events) {
                        self.drag_in_pending_text = false;
                        pointer_events.push(InputEvent::PointerDown { pos: at, modifiers });
                        // Double-click synthesis (text-select-edit): fed only
                        // on dispatching presses, fired in the same frame
                        // right after the press — core's ordering contract.
                        let (max_delay, max_dist) = ctx.options(|o| {
                            (
                                o.input_options.max_double_click_delay,
                                o.input_options.max_click_dist,
                            )
                        });
                        let now = ctx.input(|i| i.time);
                        match self.dblclick.on_press(now, pos, max_delay, max_dist) {
                            2 => pointer_events.push(InputEvent::DoubleClick { pos: at }),
                            3 => pointer_events.push(InputEvent::TripleClick { pos: at }),
                            _ => {}
                        }
                    }
                }
            }
        } else if primary_released && self.canvas_drag_active {
            self.canvas_drag_active = false;
            self.drag_in_pending_text = false;
            if !annotation_gated {
                pointer_events.push(InputEvent::PointerUp);
            }
        } else if primary_down && self.canvas_drag_active && !annotation_gated {
            if self.drag_in_pending_text {
                self.drag_pending_text_to(ctx, view, at);
            } else {
                pointer_events.push(InputEvent::PointerDrag { pos: at });
            }
        }
        for event in pointer_events {
            // Outcome is always `Continue` for pointer events today
            // (no current tool produces a side-effecting outcome via
            // pointer). Match exhaustively so future outcomes that
            // *can* arise from pointer events (e.g. a Select-tool
            // "crop-and-save" gesture) surface here as a compile error
            // rather than silently dropping.
            let _ = self.state.dispatch_input(&event);
        }

        self.set_hover_cursors(ctx, canvas_rect, pos, at, on_floating);
    }

    /// The frame's hover / drag cursor icons: Hand's pan affordance
    /// (grab on hover, grabbing mid-drag) and the Select-tool hover /
    /// grab cursor (prominent-move spec — same condition as the
    /// chrome paint, so cursor and outline can never disagree; empty
    /// canvas under Select sets nothing and egui falls back to the
    /// default arrow for the frame).
    fn set_hover_cursors(
        &self,
        ctx: &Context,
        canvas_rect: egui::Rect,
        pos: egui::Pos2,
        at: flintshot_core::Point,
        on_floating: bool,
    ) {
        // Hand cursor: the pan affordance — grab on hover, grabbing
        // mid-drag (over the canvas; the default arrow elsewhere).
        if self.state.tool == Tool::Hand && canvas_rect.contains(pos) && !on_floating {
            ctx.set_cursor_icon(if self.pan_drag.is_some() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
        } else if self.pan_drag.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }

        let hover_at = (canvas_rect.contains(pos) && !self.canvas_drag_active).then_some(at);
        if crate::chrome::move_chrome(&self.state, hover_at).is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::Move);
        } else if crate::chrome::text_hover_chrome(&self.state, hover_at).is_some() {
            // Text-tool hover affordance (text-select-edit): the I-beam
            // announces "click to select / double-click to re-edit".
            ctx.set_cursor_icon(egui::CursorIcon::Text);
        }
    }

    /// Translate this frame's shortcut keypress (one per frame) and
    /// perform its outcome side effect. Returns `true` when the
    /// outcome ended the frame (close / save / copy), so
    /// [`logic_impl`](Self::logic_impl) skips the pointer branch.
    ///
    /// Order: Cancel > Enter-crop / Save / Copy > Undo / Redo >
    /// Delete — egui reports each key independently, so the first
    /// match wins. Enter only forwards the key: the crop gate (Select
    /// tool + live selection) lives in the dispatcher. Without this
    /// branch the Enter-crop contract held only for the toolbar button (the
    /// keyboard path never reached the dispatcher — the gap
    /// `enter_key_crops_through_shell` pins).
    ///
    /// Esc layering (annotation-delete spec): with a persistent
    /// selection, the first Esc clears it and consumes the frame —
    /// no Cancel, the outcome slot untouched, the window stays open.
    /// Only with nothing selected does Esc forward to the dispatcher
    /// as today's Cancel. Shell-side by design: the dispatcher's Esc
    /// → `Cancel` contract stays stable for future shells.
    fn dispatch_shortcut_key(&mut self, ctx: &Context, modifiers: Modifiers) -> bool {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !self.state.selected.is_empty() {
            self.state.selected.clear();
            return true;
        }
        let key_event = ctx.input(|i| {
            if i.key_pressed(egui::Key::Escape) {
                Some(InputEvent::Key {
                    key: Key::Escape,
                    modifiers,
                })
            } else if i.key_pressed(egui::Key::Enter) {
                Some(InputEvent::Key {
                    key: Key::Enter,
                    modifiers,
                })
            } else if i.key_pressed(egui::Key::Z) {
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
                // Only reachable outside text-editing mode — the
                // modal text branch in `logic_impl` owns Backspace
                // during a draft and returns before this runs.
                Some(InputEvent::Key {
                    key: Key::Backspace,
                    modifiers,
                })
            } else {
                None
            }
        });
        let Some(event) = key_event else {
            return false;
        };
        // `DispatchOutcome` is `#[non_exhaustive]`; future variants
        // fall through to `matches!` returning `false`, which is the
        // same behaviour as `Continue` for the shell (no side effect
        // this frame).
        let outcome = self.state.dispatch_input(&event);
        if matches!(outcome, DispatchOutcome::Cancel) {
            // Use `if_unset` so a prior copy (which published
            // `CopiedToClipboard` and kept the window open) is
            // honoured when the user eventually closes via Esc.
            self.publish_outcome_if_unset(Ok(EditorOutcome::Cancelled));
            // Worker window: the close is the exit. Page epoch: the
            // router consumes the Cancelled and swaps Home in-place —
            // closing would recreate the window the router exists to
            // keep.
            if self.page.is_none() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return true;
        }
        if matches!(outcome, DispatchOutcome::Save) {
            self.handle_save(ctx);
            return true;
        }
        if matches!(outcome, DispatchOutcome::Copy) {
            self.handle_copy(ctx);
            return true;
        }
        false
    }
}

impl App for EditorApp {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.logic_impl(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui_impl(ui);
    }
}

impl EditorApp {
    /// Headless-callable core of [`App::ui`](EditorApp) — the split
    /// (mirroring `logic_impl` / `OverlayApp`) lets the frame body be
    /// exercised with a synthetic `egui::Context` in tests.
    /// `pub(crate)`: the window router delegates here on the Editor
    /// route.
    /// Statusbar: editor-only, gated by ui.show-statusbar; the byte
    /// estimate follows ui.statusbar-size-mode (the exact PNG encode
    /// is cached per image dims).
    fn draw_statusbar(&mut self, ui: &mut egui::Ui) {
        if !self.config.ui.show_statusbar {
            return;
        }
        let dims = self.state.image.dimensions();
        let mode = parse_size_mode(&self.config.ui.statusbar_size_mode);
        let cached_valid = self
            .statusbar_size
            .as_ref()
            .is_some_and(|(cached_dims, _)| *cached_dims == dims);
        let bytes = if mode == SizeMode::ExactPng {
            if cached_valid {
                self.statusbar_size.as_ref().map_or(0, |(_, bytes)| *bytes)
            } else {
                let composed = compose_for_save(false, &self.state);
                let bytes =
                    flintshot_core::estimate_size_bytes(dims, SizeMode::ExactPng, Some(&composed));
                self.statusbar_size = Some((dims, bytes));
                bytes
            }
        } else {
            flintshot_core::estimate_size_bytes(dims, SizeMode::Heuristic, None)
        };
        crate::statusbar::show(ui, dims, self.state.tool, bytes);
    }

    /// Worker editor windows draw the topbar themselves (the router
    /// draws it over routed pages).
    fn draw_worker_topbar(&mut self, ui: &mut egui::Ui) {
        if self.page.is_some() {
            return;
        }
        let copied = self.copied_visible(ui.input(|i| i.time));
        if let Some(a) = topbar::show(
            ui,
            topbar::Surface::Editor,
            None,
            topbar::Enable::WORKER,
            copied,
        ) {
            self.handle_topbar_action(a, ui.ctx());
        }
    }

    pub(crate) fn ui_impl(&mut self, ui: &mut egui::Ui) {
        // The settings route's animation gate drives every spring in
        // this window (sections, chips, rail pop, statusbar).
        widgets::set_animations(ui.ctx(), self.config.ui.animations);
        // The "Copied" revert must land without input: while the
        // window is open, repaint after at most the remaining time.
        if let Some(delay) = copied_repaint_delay(self.copied_at, ui.input(|i| i.time)) {
            ui.ctx().request_repaint_after(delay);
        }
        self.draw_worker_topbar(ui);
        // Statusbar: the bottom zone strips its 30 px before the
        // side panels reserve (draw order: top → bottom → left →
        // right). Editor-only — the router draws home / settings
        // without it, per the mockups.
        self.draw_statusbar(ui);
        // Tool rail: the left zone (draw order: top → bottom → left
        // → right). Tool switching mutates the shared state directly;
        // the gate reflects the selection-first phase.
        let gate = self.rail_gate();
        rail::show(ui, &mut self.state, gate, &mut self.rail);
        // Properties panel. Must come before CentralPanel so egui
        // reserves the right-hand strip. The plain call (no closure)
        // keeps the borrow of `&mut self.state` disjoint from the
        // panel's own state. Returns the Selection section's fired
        // Delete, if any.
        let panel_action = panel::show(ui, &mut self.state, &mut self.panel);

        // Selection-first phase: the toolbar's "Crop · Enter" button
        // crops through `confirm_crop`, bypassing `logic` — so
        // this is the second latch site, checked before this frame's
        // render paints the mask.
        self.end_phase_on_crop();

        // Viewport policy by lifecycle: worker windows keep the
        // image-sized window while the view zoom is 1.0 (resize
        // until the panel matches); page epochs — and workers under
        // a user zoom — never resize, the fit path letterboxes
        // inside the stable window instead.
        if self.page.is_none() && self.at_fit_zoom() {
            self.handle_image_resize(ui.ctx());
        }
        self.update_fit(ui.ctx());

        // Per-frame render: bind the painter, then walk the shared
        // render path. Render failure is terminal — publish `Err(_)`
        // and close. The only current failure path is
        // `canvas.upload_image`, which in the egui impl is infallible
        // (`load_texture` returns a handle directly), so this branch
        // is defensive against future Canvas impls / Task 13's blur
        // upload path.
        //
        // The closure returns the render `Result` so error handling
        // stays outside the closure body — keeps the borrow checker
        // happy (`&mut self.canvas` + `&mut self.bg` + `&self.state`
        // are captured disjointly by the closure, while the outcome
        // publication happens against the unborrowed `self`).
        // Zero inner margin: `Frame::central_panel` defaults to 8 px,
        // which anchors the canvas's draws at the panel's *inner*
        // rect while this shell maps pointers and chrome against
        // `panel.response.rect` (the outer one) — an 8 px offset
        // between where the cursor maps and where strokes land, and
        // between the image and its dressing. One rect for both.
        let panel = egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ui.style()).inner_margin(egui::Margin::same(0)))
            .show(ui, |ui| {
                // The paper shadow goes UNDER the render pass:
                // `Shadow::as_shape` fills its whole rect (a
                // translucent tint, blurred at the edges), so painted
                // over the image it reads as an overlay. Under the
                // blit, only the penumbra outside the cutout shows.
                self.paint_paper_shadow(ui.painter());
                self.canvas.bind_painter(ui.painter());
                render(
                    &self.state,
                    &mut self.canvas,
                    &mut self.bg,
                    &mut self.blur_cache,
                )
            });

        // Cache the canvas's screen-logical rect so the next frame's
        // `logic()` can do pointer hit-testing. `panel.response.rect`
        // is `Copy`, so this doesn't extend the borrow.
        self.canvas_rect = Some(panel.response.rect);

        if let Err(err) = panel.inner {
            tracing::error!(
                error = %err,
                "background render failed; closing editor session"
            );
            self.publish_outcome(Err(err));
            // Worker window: close (the outcome rides the slot).
            // Page epoch: the router consumes the Err — status
            // through the bridge, Home route, same window.
            if self.page.is_none() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            // Return early — Save / Copy handlers below would touch
            // state the render path has already abandoned.
            return;
        }

        // Selection-first phase chrome: the dim mask outside the
        // live selection plus the hint line, painted over the shared
        // render output and under the magnifier — the overlay's
        // chrome ordering. No-op once the phase has ended.
        if self.phase_active() {
            self.paint_selection_phase(ui.ctx(), ui.painter());
        }

        // Hand-tool object chrome (prominent-move spec): dashed hover
        // outline / solid ember grab outline, under the magnifier —
        // the overlay's chrome ordering.
        self.paint_move_chrome_if_any(ui.ctx(), ui.painter());

        // Persistent-selection outline (annotation-delete spec):
        // dashed forest bbox of the selected annotation, last among
        // the annotation chrome. Chrome-only by construction — it
        // lives in the shell's paint pass, never in `render()` /
        // composed output.
        self.paint_selected_chrome_if_any(ui.ctx(), ui.painter());

        // The paper-cutout dressing (shadow + ring + handles), after
        // the annotation chrome and under the magnifier, per the
        // mockup's stacking. Hidden during the selection-first phase.
        self.paint_canvas_dressing(ui.painter());

        // Magnifier inset overlay (Task 005). Painted after render
        // succeeds, in screen-logical space (uses the cached
        // `canvas_rect`, not the canvas-bound painter). Suppressed
        // during text-editing so the trigger key doesn't pop the
        // loupe over the caret.
        self.paint_magnifier_if_active(ui.ctx(), ui.painter());

        // Floating quick cluster (undo / redo / magnifier / view
        // zoom) over the canvas's top-left corner. The chip rects
        // feed the next frame's press gate.
        let canvas_rect = self.canvas_rect;
        let (floating_action, chip_rects) = canvas_rect.map_or((None, Vec::new()), |rect| {
            floating::show(ui.ctx(), rect, self.magnifier_always_on)
        });
        self.floating_rects = chip_rects;
        self.handle_floating_action(floating_action);

        // Handle toolbar actions outside the toolbar drawing code so
        // we can borrow `&self.outcome` cleanly. Each handler runs the
        // corresponding output pipeline (save / copy) and either
        // publishes the matching `EditorOutcome` + closes the window,
        // or logs + closes on failure. Dialog-cancel keeps the editor
        // open (no outcome published).
        match panel_action {
            // The panel only fires Delete today (the Selection
            // section); Save/Copy/Upload live on the topbar. Delete
            // replays the same dispatch a Del keypress would produce
            // (the dispatcher owns all state mutation).
            // Ctrl+Z / Ctrl+Y / Del keypress would produce (the
            // dispatcher owns all state mutation); their outcomes are
            // always `Continue`.
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
            // Save / Copy / Upload / NewCapture are unreachable from
            // the panel today — the topbar owns them.
            _ => {}
        }
    }
}

impl EditorApp {
    /// Apply a topbar action (worker windows; the router forwards
    /// routed-page actions here too). Open / Paste have no worker
    /// bridge and stay disabled; Settings delegates through the
    /// shell's hook — worker windows never route to Settings
    /// in-window (spec §Settings route).
    pub(crate) fn handle_topbar_action(&mut self, action: TopbarAction, ctx: &Context) {
        match action {
            TopbarAction::Capture => {
                // capture.default-mode seeds the grab: fullscreen
                // skips the selection; window falls back to region
                // (the grab pipeline's interactive default).
                let mode = match parse_default_mode(&self.config.capture.default_mode) {
                    DefaultMode::Fullscreen => NewCaptureMode::Fullscreen,
                    _ => NewCaptureMode::Region,
                };
                self.handle_new_capture(mode, ctx);
            }
            TopbarAction::Copy => self.handle_copy(ctx),
            TopbarAction::Save => self.handle_save(ctx),
            TopbarAction::Upload => self.handle_upload(ctx),
            TopbarAction::Open => self.handle_page_open(ctx),
            TopbarAction::Paste => self.handle_page_paste(ctx),
            // The shell's delegate (the daemon's `OpenSettings`
            // choreography: an open app window focuses + routes
            // itself to Settings; otherwise one is spawned on the
            // Settings route). Page mode never reaches this arm —
            // the router swaps routes before forwarding editor
            // actions — so the hook cannot double-open. The worker
            // then leaves via the Esc contract (the ADR-0065
            // NavigateHome choreography, mirrored): the handoff is
            // requested while the worker is still alive to ask, and
            // the editor never lingers beside the window it opened.
            TopbarAction::Settings => {
                if let Some(open) = &self.open_settings {
                    open();
                }
                self.publish_outcome_if_unset(Ok(EditorOutcome::Cancelled));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            // The breadcrumb's Home link: worker windows have no
            // home route behind them, so the link delegates the
            // daemon's `OpenApp` choreography (focus-or-spawn the
            // app window) first — the handoff must be requested
            // while the worker is still alive to ask — then leaves
            // via the Esc contract: publish-if-unset and close. The
            // app-window router intercepts `NavigateHome` before
            // editor actions flow, so this arm is worker-side; the
            // page path through here stays defensive.
            TopbarAction::NavigateHome => {
                if let Some(open) = &self.open_home {
                    open();
                }
                self.publish_outcome_if_unset(Ok(EditorOutcome::Cancelled));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Install the worker Settings entry's delegate (see the
    /// `open_settings` field). Called by the shell's app creator
    /// right after [`Self::new`].
    pub(crate) fn set_open_settings(&mut self, f: std::sync::Arc<dyn Fn() + Send + Sync>) {
        self.open_settings = Some(f);
    }

    /// Install the worker Home link's delegate (see the `open_home`
    /// field). Called by the shell's app creator right after
    /// [`Self::new`].
    pub(crate) fn set_open_home(&mut self, f: std::sync::Arc<dyn Fn() + Send + Sync>) {
        self.open_home = Some(f);
    }

    /// Live-apply a refreshed config snapshot (the router calls this
    /// after the settings route saves). UI-facing fields only — the
    /// magnifier snapshot stays at construction, matching its
    /// read-once precedent.
    pub(crate) fn apply_config(&mut self, config: &Config) {
        self.config = config.clone();
    }

    /// Apply a floating-cluster action: Undo / Redo replay the same
    /// dispatches their keyboard twins produce, the magnifier chip
    /// flips the always-on flag, and the zoom chips step the view
    /// zoom (the fit transform applies it on the next
    /// [`Self::update_fit`] pass).
    fn handle_floating_action(&mut self, action: Option<floating::FloatingAction>) {
        match action {
            Some(floating::FloatingAction::Undo) => {
                let _ = self.state.dispatch_input(&InputEvent::Key {
                    key: Key::Char('z'),
                    modifiers: Modifiers {
                        ctrl: true,
                        ..Modifiers::default()
                    },
                });
            }
            Some(floating::FloatingAction::Redo) => {
                let _ = self.state.dispatch_input(&InputEvent::Key {
                    key: Key::Char('y'),
                    modifiers: Modifiers {
                        ctrl: true,
                        ..Modifiers::default()
                    },
                });
            }
            Some(floating::FloatingAction::ToggleMagnifier) => {
                self.magnifier_always_on = !self.magnifier_always_on;
            }
            Some(floating::FloatingAction::ZoomIn) => {
                self.view_zoom = (self.view_zoom + VIEW_ZOOM_STEP).min(VIEW_ZOOM_MAX);
            }
            Some(floating::FloatingAction::ZoomOut) => {
                self.view_zoom = (self.view_zoom - VIEW_ZOOM_STEP).max(VIEW_ZOOM_MIN);
            }
            None => {}
        }
    }

    /// Poll the page bridge's directives (single-window page mode
    /// only). `Hide` closes the window — the external-capture seam
    /// (the daemon's pre-grab handshake; the page must never be in a
    /// shot); `Show` is the focus ping; `SetStatus` drains quietly
    /// (the editor page shows no status line — the home page owns
    /// one when the window routes back).
    fn poll_page_directives(&mut self, ctx: &Context) {
        let Some(bridge) = self.page.clone() else {
            return;
        };
        // Idle repaint heartbeat so directives arrive without input
        // (mirrors `HomeApp`'s pump).
        ctx.request_repaint_after(crate::home::REPOLL_INTERVAL);
        while let Some(directive) = bridge.borrow_mut().poll_directive() {
            match directive {
                HomeDirective::Hide => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                HomeDirective::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                HomeDirective::SetStatus(_) => {}
            }
        }
    }

    /// Whether the current page is a single-window page epoch.
    fn is_page(&self) -> bool {
        self.page.is_some()
    }

    /// The page chrome's Open action, dialog already answered:
    /// hand the picked path to the bridge and close on a route (the
    /// fresh editor epoch replaces this page's image). A failed
    /// decode keeps the page exactly as it was.
    fn apply_page_open(&self, _ctx: &Context, path: std::path::PathBuf) {
        let Some(bridge) = self.page.clone() else {
            return;
        };
        // A routed image no longer closes: the window router polls
        // the armed editor and swaps the route in-place (the fresh
        // editor app replaces this one behind the same surface).
        let _ = bridge.borrow_mut().open_image(path);
    }

    /// The page chrome's Open action: in-window file dialog, then
    /// [`Self::apply_page_open`]. Cancelling the dialog is a no-op.
    fn handle_page_open(&self, ctx: &Context) {
        if let Some(path) = crate::home::pick_image_path() {
            self.apply_page_open(ctx, path);
        }
    }

    /// The page chrome's Paste action: clipboard image via the
    /// bridge; a routed image closes the window for the fresh epoch,
    /// an empty clipboard keeps the page.
    fn handle_page_paste(&self, _ctx: &Context) {
        let Some(bridge) = self.page.clone() else {
            return;
        };
        // Same no-close contract as the page Open (see
        // [`Self::apply_page_open`]).
        let _ = bridge.borrow_mut().request_paste();
    }

    /// The page chrome's keyboard accelerators: `C` capture, `O`
    /// open — matching the home's letters (both are unclaimed by
    /// tool keys). `P` stays the Pencil tool key (tool keys win over
    /// home parity; Paste is button-only in the editor). Chords
    /// (Ctrl+C copy) are untouched — the plain-key guard.
    fn page_action_shortcut(&mut self, ctx: &Context, modifiers: Modifiers) -> bool {
        if self.page.is_none() || modifiers != Modifiers::default() {
            return false;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::C)) {
            // The `+` chip's exit: publishes NewCapture (mode label
            // intact) and closes — the router runs the grab flow.
            self.handle_new_capture(NewCaptureMode::Region, ctx);
            return true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::O)) {
            self.handle_page_open(ctx);
            return true;
        }
        false
    }

    /// Publish a side-effect outcome (Save / Copy / Upload success)
    /// through the owning lifecycle: the worker window publishes to
    /// the slot and closes; the single-window page reports through
    /// the bridge and stays open (spec: outcomes keep the page).
    fn publish_side_effect_outcome(&self, outcome: EditorOutcome) {
        match &self.page {
            Some(bridge) => bridge.borrow_mut().report_outcome(&outcome),
            None => self.publish_outcome(Ok(outcome)),
        }
    }

    /// Whether a side-effect arm should follow its publish with a
    /// close: worker windows close (today's lifecycle); the page
    /// never does.
    fn closes_after_side_effect(&self) -> bool {
        self.page.is_none()
    }

    /// The copy arm's close decision: page mode never closes (the
    /// clipboard handle lives on in `self.clipboard` — the X11 owner
    /// rule satisfied by the long-lived window instead of a lingering
    /// process); worker mode keeps the platform rule.
    fn close_after_copy_window(&self) -> bool {
        !self.is_page() && close_after_copy(os_owns_clipboard())
    }

    /// Whether the topbar Copy hero should paint its "Copied" state
    /// at `now` (egui input time). Both lifecycles query this: the
    /// worker's own topbar and the router over routed pages.
    pub(crate) fn copied_visible(&self, now: f64) -> bool {
        copied_window_open(self.copied_at, now)
    }

    /// `+` chip / Ctrl+N handler: publish `NewCapture` and close —
    /// the closing window is the not-in-the-shot guarantee for the
    /// next grab. Uses `publish_outcome` (not `if_unset`): a prior
    /// copy's outcome must not mask an explicit "start over" — the
    /// clipboard side effect already happened either way.
    fn handle_new_capture(&self, mode: NewCaptureMode, ctx: &Context) {
        tracing::debug!(?mode, "new capture requested; closing editor");
        self.publish_outcome(Ok(EditorOutcome::NewCapture { mode }));
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// Ctrl+N — new region capture, Win11's chord. Shell-side by
    /// design: the core dispatcher owns annotation edits, not
    /// session lifecycle. Must run past the text-draft branch (which
    /// returns earlier in `logic_impl`), so a draft can't swallow the
    /// chord.
    fn new_capture_shortcut(&self, ctx: &Context, modifiers: Modifiers) -> bool {
        let ctrl_only = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        if modifiers == ctrl_only && ctx.input(|i| i.key_pressed(egui::Key::N)) {
            self.handle_new_capture(NewCaptureMode::Region, ctx);
            return true;
        }
        false
    }

    /// Save button / Ctrl+S handler: compose the image per the
    /// phase contract ([`compose_for_save`] — selection-aware while
    /// the selection-first phase is live), prompt for a path via
    /// `rfd`, write PNG, publish `Saved(path)` on success.
    fn handle_save(&self, ctx: &Context) {
        // Direct save when a filename template is configured; the
        // file dialog otherwise.
        if !self.config.output.filename_template.is_empty() {
            match save_direct(&self.state, self.phase_active(), &self.config.output) {
                Ok(path) => {
                    tracing::info!(path = %path.display(), "editor direct-saved");
                    self.publish_side_effect_outcome(EditorOutcome::Saved(path));
                    if self.closes_after_side_effect() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                Err(err) => {
                    tracing::error!(error = %err, "direct save failed; closing editor session");
                    self.publish_outcome(Err(err));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            return;
        }
        match save_to_disk(&self.state, self.phase_active(), &self.config.output) {
            Ok(Some(path)) => {
                tracing::info!(path = %path.display(), "editor saved");
                // GOTCHA (single-window): the page epoch stays open
                // after a save — the side effect already happened and
                // the bridge carries the report. Only the worker
                // window closes here.
                self.publish_side_effect_outcome(EditorOutcome::Saved(path));
                if self.closes_after_side_effect() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Ok(None) => {
                tracing::debug!("save dialog cancelled; staying in editor");
            }
            Err(err) => {
                tracing::error!(error = %err, "save failed; closing editor session");
                self.publish_outcome(Err(err));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Copy button handler: compose the final image, hand it to
    /// `arboard::Clipboard`, and publish `CopiedToClipboard`. When
    /// `output.save-clipboard-copies` is on (default), the same
    /// image is persisted to the resolved save directory and
    /// recorded into recents — a copy is no longer invisible
    /// history. A persist failure never fails the copy.
    ///
    /// GOTCHA (probe evidence, ux-redesign-regressions spec §2): the
    /// clipboard contents survive this window's close only where the
    /// OS owns the clipboard (macOS/Windows). On Linux — **both**
    /// display servers — the selection is served from an in-process
    /// thread (arboard's wl-clipboard-rs model; `wl-copy` forks for
    /// exactly this reason), and compositor adoption of an exited
    /// client's `zwlr_data_control` selection is not guaranteed
    /// (COSMIC rejects it; the old "the compositor takes ownership
    /// when the selection is set" assumption is false). So a worker
    /// Copy never auto-closes on Linux: the Copy hero shows the
    /// "Copied" state and the user closes via Esc / X after pasting
    /// (the outcome is pre-published, so the eventual close still
    /// reports the copy). Page mode keeps the single window open on
    /// every platform — the handle lives in `self.clipboard`.
    fn handle_copy(&mut self, ctx: &Context) {
        match copy_to_clipboard(&self.state, &mut self.clipboard) {
            Ok(()) => {
                tracing::info!("editor copied to clipboard");
                self.copied_at = Some(ctx.input(|i| i.time));
                if let Some(path) = persist_clipboard_copy_side_effect(&self.state, &self.config) {
                    flintshot_core::recents::record(
                        &path,
                        self.config.ui.recent_captures_limit_clamped(),
                    );
                }
                self.publish_side_effect_outcome(EditorOutcome::CopiedToClipboard);
                if self.close_after_copy_window() {
                    tracing::debug!("OS-owned clipboard; closing after copy");
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Err(err) => {
                tracing::error!(error = %err, "copy failed");
                self.publish_outcome(Err(err));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Upload button handler: compose the final image (selection-aware,
    /// like Copy), write it to a temp PNG, spawn `flintshot upload <tmp>`
    /// as a detached worker (resolving the binary via `current_exe`),
    /// then publish `Uploaded` and close the editor.
    ///
    /// The URL is **not** available synchronously: the spawned worker
    /// performs the HTTP request and reports the result back to the
    /// daemon (Task 008), which fans it out as `Event::Uploaded`. The
    /// editor publishes `Uploaded(String::new())` — an empty placeholder
    /// — purely to signal "upload was initiated" so the shell closes
    /// cleanly. Subscribers see the real URL asynchronously via the
    /// daemon event stream.
    ///
    /// ## Error policy
    ///
    /// A compose or temp-write failure is terminal (publish `Err`,
    /// close); a spawn failure is recoverable (log at `error`, leave
    /// the window open so Save / Copy still work).
    fn handle_upload(&self, ctx: &Context) {
        match compose_and_write_upload(&self.state) {
            Ok(path) => match spawn_upload_worker(&path) {
                Ok(()) => {
                    tracing::info!(
                        path = %path.display(),
                        "upload initiated; spawned upload worker"
                    );
                    // Same page/worker split as Save (see the GOTCHA
                    // there): the worker closes; the page reports and
                    // stays.
                    self.publish_side_effect_outcome(EditorOutcome::Uploaded(String::new()));
                    if self.closes_after_side_effect() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                Err(err) => {
                    tracing::error!(
                        error = %err,
                        "could not spawn upload worker; editor stays open"
                    );
                }
            },
            Err(err) => {
                tracing::error!(
                    error = %err,
                    "upload compose/write failed; closing editor session"
                );
                self.publish_outcome(Err(err));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

/// Persist the copy side effect when `output.save-clipboard-copies`
/// is on (default): the selection-aware image through core's
/// clipboard-copy rules (template-or-builtin naming, the resolved
/// save directory, collision suffixing, `[output] format`
/// encoding). Returns the written path for the caller's recents
/// record; `None` when the flag is off or the save failed — a
/// failure logs and never fails the copy (the clipboard write
/// already succeeded).
fn persist_clipboard_copy_side_effect(
    state: &EditorState,
    config: &Config,
) -> Option<std::path::PathBuf> {
    if !config.output.save_clipboard_copies {
        return None;
    }
    let img = compose_selection_aware(state);
    match flintshot_core::persist_clipboard_copy(&img, &config.output) {
        Ok(path) => {
            tracing::info!(path = %path.display(), "clipboard copy persisted to the save location");
            Some(path)
        }
        Err(e) => {
            tracing::warn!(error = %e, "persisting the clipboard copy; the copy itself succeeded");
            None
        }
    }
}

/// The save dialog's prefill directory: the resolved save dir —
/// the configured `save-dir`, or the platform pictures default
/// (`~/Pictures/Flintshot`) when unset. The dialog lands somewhere
/// sane even on the dialog-first path.
fn save_dialog_dir(output: &flintshot_core::OutputConfig) -> std::path::PathBuf {
    output.resolve_save_dir()
}

/// Prompt for a PNG save path via `rfd`, prefilled with `dir`.
/// Blocks on the native modal — the eframe event loop is already
/// blocking, and the dialog is what the user is interacting with
/// anyway. `None` when the user cancels.
fn prompt_png_path(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .add_filter("PNG image", &["png"])
        .set_directory(dir)
        .set_file_name("flintshot.png")
        .save_file()
}

/// Compose the image a Save writes: selection-aware while the
/// selection-first phase is live, full-image otherwise.
///
/// The Save half of the outputs-with-live-selection contract
/// (region-selection-first Task 003): Copy / Upload are
/// selection-aware unconditionally via
/// [`compose_selection_aware`], but Save only while the phase is
/// live — the post-phase and plain-editor (`fullscreen` / `open`)
/// sessions keep the Phase 1 full-image contract, pinned unregressed
/// by tests.
///
/// Pure decision point so the phase-scoped contract is
/// headless-testable; the rfd prompt wrapped around it in
/// [`save_to_disk`] is not.
fn compose_for_save(selection_aware: bool, state: &EditorState) -> image::RgbaImage {
    if selection_aware {
        compose_selection_aware(state)
    } else {
        compose_final(state)
    }
}

/// The direct-save path: compose, encode per the configured format
/// (JPEG at the configured quality; PNG otherwise), write to the
/// **resolved** save directory ([`OutputConfig::resolve_save_dir`]
/// — `~/Pictures/Flintshot` when `save-dir` is unset) as
/// `template.ext` with `-2`-style collision suffixes. The directory
/// is created on first write. Returns the written path.
///
/// # Errors
/// Surfaces [`FlintshotError::Image`] if encoding or the
/// filesystem write fails.
fn save_direct(
    state: &EditorState,
    selection_aware: bool,
    output: &flintshot_core::OutputConfig,
) -> flintshot_core::Result<std::path::PathBuf> {
    let rendered = render_filename_template(
        &output.filename_template,
        env!("CARGO_PKG_VERSION"),
        &chrono_date(),
        &chrono_time(),
    );
    let format = parse_output_format(&output.format);
    let image = compose_for_save(selection_aware, state);
    save_capture_image(
        &output.resolve_save_dir(),
        &rendered,
        &image,
        format,
        output.jpeg_quality,
    )
}

/// Prompt for a PNG path (prefilled from `output`'s resolved save
/// dir) and write the [`compose_for_save`] image.
///
/// `selection_aware` comes from the caller's phase state — the only
/// call site, [`EditorApp::handle_save`], passes
/// `self.phase_active()`; the overlay's
/// [`save_region_to_disk`] passes `true` (its model is "the region
/// defines what ships").
///
/// Returns `Ok(None)` when the user cancels the dialog (no file
/// written, no error); callers should keep the editor open in that
/// case.
///
/// # Errors
/// Surfaces [`FlintshotError::Image`] if PNG encoding or the
/// filesystem write fails.
fn save_to_disk(
    state: &EditorState,
    selection_aware: bool,
    output: &flintshot_core::OutputConfig,
) -> flintshot_core::Result<Option<std::path::PathBuf>> {
    let Some(path) = prompt_png_path(&save_dialog_dir(output)) else {
        return Ok(None);
    };
    compose_for_save(selection_aware, state)
        .save_with_format(&path, image::ImageFormat::Png)
        .map_err(FlintshotError::from)?;
    Ok(Some(path))
}

/// The overlay's Save (Phase 5 Task 006):
/// [`save_to_disk`] with `selection_aware = true` — the overlay's
/// model is "the region defines what ships in the output" (Task
/// 005), so a usable selection saves just the region; without one
/// it saves the full composed frame (the `Ctrl+S`-before-any-drag
/// case). The dialog prefill reads the persisted config (the
/// overlay carries no live session config; a load failure warns
/// and prefills the defaults).
///
/// Returns `Ok(None)` when the user cancels the dialog.
///
/// # Errors
/// Surfaces [`FlintshotError::Image`] if PNG encoding or the
/// filesystem write fails.
pub(crate) fn save_region_to_disk(
    state: &EditorState,
) -> flintshot_core::Result<Option<std::path::PathBuf>> {
    let output = match flintshot_core::Config::load() {
        Ok(c) => c.output,
        Err(e) => {
            tracing::warn!(error = %e, "loading config for the save dialog prefill; using defaults");
            flintshot_core::OutputConfig::default()
        }
    };
    save_to_disk(state, true, &output)
}

/// Compose the final image honouring an active `Tool::Crop`
/// region: a non-degenerate region emits just its pixels
/// (`compose_final_region`); otherwise the whole composed
/// image (`compose_final`). Pure / non-mutating.
///
/// The single region-aware compose for every region-defining
/// output (Copy / Upload / the Phase 5 overlay's Save, and
/// the editor window's Save while the selection-first phase is
/// live — see [`compose_for_save`]), so "what the user sees
/// outlined" is "what ships", in one place. Outside the phase,
/// the editor window's `Ctrl+S` / Save button deliberately does
/// **not** use this — its Phase 1 contract saves the full image.
pub(crate) fn compose_selection_aware(state: &EditorState) -> image::RgbaImage {
    // The 1.0-px threshold matches the render path's selection-rect
    // guard (`render::render`) and `EditorState::crop`'s degenerate
    // no-op; a degenerate selection counts as absent.
    match state.crop_region.map(Rect::normalized) {
        Some(n) if n.width().abs() >= 1.0 && n.height().abs() >= 1.0 => {
            compose_final_region(state, n)
        }
        _ => compose_final(state),
    }
}

/// Whether a successful worker Copy should close the editor window,
/// given the platform's clipboard ownership model. Pure seam over a
/// cfg constant so the table is pinned by tests on any platform.
///
/// GOTCHA (probe evidence, ux-redesign-regressions spec §2): the old
/// assumption — "the compositor takes ownership when the selection is
/// set" — is false for arboard's wl-clipboard-rs thread-serve model
/// (`wl-copy` forks for exactly this reason), and COSMIC does not
/// adopt an exited client's `zwlr_data_control` selection. On Linux
/// the serve thread must outlive the user's paste, on **both**
/// display servers (X11 was already owner-clipboard). macOS/Windows
/// own the clipboard OS-side, so closing is safe there.
fn close_after_copy(os_owns_clipboard: bool) -> bool {
    os_owns_clipboard
}

/// The "Copied" hero state's window: visible for this long after a
/// successful copy, then the hero reverts.
const COPIED_WINDOW_SECS: f64 = 2.0;

/// Whether the "Copied" window is open at `now`: true only within
/// [`COPIED_WINDOW_SECS`] of the copy instant. Pure seam so the
/// timing is pinned headlessly.
fn copied_window_open(copied_at: Option<f64>, now: f64) -> bool {
    copied_at.is_some_and(|t| (0.0..=COPIED_WINDOW_SECS).contains(&(now - t)))
}

/// The repaint delay that lands the "Copied" revert without input:
/// `Some(remaining)` while the window is open, `None` otherwise.
fn copied_repaint_delay(copied_at: Option<f64>, now: f64) -> Option<std::time::Duration> {
    if !copied_window_open(copied_at, now) {
        return None;
    }
    let remaining = COPIED_WINDOW_SECS - (now - copied_at?);
    Some(std::time::Duration::from_secs_f64(remaining.max(0.001)))
}

/// Whether this platform's OS owns the clipboard contents (the
/// source process may exit freely). True on macOS/Windows; false on
/// Linux, where the serving thread is in-process only.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn os_owns_clipboard() -> bool {
    true
}

/// Linux (X11 and Wayland): the selection is served from an
/// in-process thread — see [`close_after_copy`]'s probe note.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn os_owns_clipboard() -> bool {
    false
}

/// When a non-degenerate `state.crop_region` is active, Copy emits just
/// the selected region (Flameshot-style): the user expects the active
/// selection to define what gets copied, separate from the Crop tool
/// which permanently narrows the editor. The editor state is **not**
/// mutated — `state.image`, `state.anns`, and `state.crop_region` are
/// untouched, so the user can keep adjusting the selection and copy
/// again. A degenerate selection (< 1 px) falls back to the full
/// image, matching the no-selection behaviour.
///
/// The `arboard::Clipboard` handle is stored in `slot` rather than
/// dropped at the end of this function. On X11 the clipboard is
/// owner-based: dropping the handle (or exiting the process) clears
/// the selection. The caller keeps the handle alive until the editor
/// window closes, giving the user time to paste.
///
/// # Errors
/// Surfaces [`FlintshotError::Shell`] for clipboard-creation or
/// clipboard-set failures (clipboard occupied by another process,
/// clipboard daemon unavailable on Linux, etc.).
pub(crate) fn copy_to_clipboard(
    state: &EditorState,
    slot: &mut Option<arboard::Clipboard>,
) -> flintshot_core::Result<()> {
    // Selection-aware output: crop the composed image to the active
    // selection rect when one is present and non-degenerate.
    let img = compose_selection_aware(state);
    // Reuse the existing Clipboard handle, or create one if this is
    // the first copy this session.
    let mut clipboard = match slot.take() {
        Some(c) => c,
        None => arboard::Clipboard::new().map_err(|e| FlintshotError::Shell {
            detail: e.to_string(),
        })?,
    };
    let (w, h) = img.dimensions();
    // `arboard::ImageData::bytes` is RGBA in row-major order, which
    // matches `image::RgbaImage::into_raw` exactly — no swizzling.
    let data = std::borrow::Cow::from(img.into_raw());
    let image_data = arboard::ImageData {
        width: w as usize,
        height: h as usize,
        bytes: data,
    };
    clipboard
        .set_image(image_data)
        .map_err(|e| FlintshotError::Shell {
            detail: e.to_string(),
        })?;
    // Store the handle back so the caller keeps the X11 selection
    // alive until the editor window closes.
    *slot = Some(clipboard);
    Ok(())
}

/// Compose the upload image (selection-aware via
/// [`compose_selection_aware`]) and write it to a temp PNG. Returns
/// the path the spawned `flintshot upload` worker should read. The
/// file is left on disk — the upload worker opens it once at startup;
/// the OS temp reaper handles stale files.
///
/// Factored out of [`EditorApp::handle_upload`](EditorApp::handle_upload)
/// so the compose + write path is headless-testable without a display,
/// network, or the binary.
///
/// # Errors
/// Surfaces [`FlintshotError::Image`] if PNG encoding or the filesystem
/// write fails.
pub(crate) fn compose_and_write_upload(
    state: &EditorState,
) -> flintshot_core::Result<std::path::PathBuf> {
    write_upload_tempfile(&compose_selection_aware(state))
}

/// Write `img` to a PNG under the OS temp dir with a recognizable,
/// unique name `flintshot-upload-<pid>-<n>.png` and leave it on disk
/// for the upload worker to read. `<pid>` separates concurrent editor
/// sessions; a process-local counter (`<n>`) disambiguates repeated
/// upload clicks within one session. The file is never reaped by us —
/// the OS temp reaper handles stale files. (Write-then-spawn ordering
/// guarantees the worker always reads a complete file.)
///
/// # Errors
/// Surfaces [`FlintshotError::Image`] on PNG encode failure.
fn write_upload_tempfile(img: &image::RgbaImage) -> flintshot_core::Result<std::path::PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("flintshot-upload-{pid}-{n}.png"));
    img.save_with_format(&path, image::ImageFormat::Png)
        .map_err(FlintshotError::from)?;
    Ok(path)
}

/// Spawn `flintshot upload <path>` as a detached child of the current
/// binary (resolved via [`std::env::current_exe`], mirroring
/// `flintshot_app::worker::CommandSpawner`). stdin / stdout are null'd
/// so worker output can't interleave with the editor's; stderr is
/// inherited so tracing logs survive for debugging. A lightweight
/// reaper thread waits on the child so it doesn't linger as a zombie.
///
/// # Errors
/// Surfaces [`FlintshotError::Shell`] if the binary can't be resolved,
/// the child can't be spawned, or the reaper thread can't start.
pub(crate) fn spawn_upload_worker(path: &std::path::Path) -> flintshot_core::Result<()> {
    let exe = std::env::current_exe().map_err(|e| FlintshotError::Shell {
        detail: format!("resolving current_exe for upload: {e}"),
    })?;
    let mut child = std::process::Command::new(&exe)
        .arg("upload")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .map_err(|e| FlintshotError::Shell {
            detail: format!(
                "spawning upload worker {} upload {}: {e}",
                exe.display(),
                path.display()
            ),
        })?;
    let _reaper = std::thread::Builder::new()
        .name("flintshot-upload-reaper".into())
        .spawn(move || {
            let _ = child.wait();
        })
        .map_err(|e| FlintshotError::Shell {
            detail: format!("spawning upload reaper thread: {e}"),
        })?;
    Ok(())
}

/// Convert an `eframe::Error` into a [`FlintshotError::Shell`].
///
/// Free-standing so the shell module can call it without re-stating
/// the variant mapping inline.
pub(crate) fn eframe_err_to_flintshot(err: &eframe::Error) -> FlintshotError {
    FlintshotError::Shell {
        detail: err.to_string(),
    }
}

/// Drain egui text-input events into the dispatcher while a text
/// draft is active (shared by the editor window and the Phase 5
/// overlay — plan decision 5's factored translation helper).
///
/// egui reports typed characters via `Event::Text(String)` (which
/// may carry several chars for fast typing or IME composition);
/// `Backspace` / `Enter` arrive as `Event::Key`. Each is translated
/// to the toolkit-agnostic [`InputEvent`] and fed to the dispatcher,
/// which mutates the in-flight draft. The dispatcher's outcomes for
/// text events are always `Continue` (no save / copy / cancel-editor
/// mid-edit — Esc cancels the *draft* only), so there is no outcome
/// branching here.
pub(crate) fn dispatch_text_input(state: &mut EditorState, ctx: &Context, modifiers: Modifiers) {
    let events: Vec<egui::Event> = ctx.input(|i| i.raw.events.clone());
    for event in events {
        match event {
            egui::Event::Text(text) => {
                for ch in text.chars() {
                    state.dispatch_input(&InputEvent::TextInput(ch));
                }
            }
            egui::Event::Key {
                key: egui::Key::Backspace,
                pressed: true,
                ..
            } => {
                state.dispatch_input(&InputEvent::Key {
                    key: Key::Backspace,
                    modifiers,
                });
            }
            egui::Event::Key {
                key: egui::Key::Enter,
                pressed: true,
                ..
            } => {
                state.dispatch_input(&InputEvent::Key {
                    key: Key::Enter,
                    modifiers,
                });
            }
            egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                ..
            } => {
                state.dispatch_input(&InputEvent::Key {
                    key: Key::Escape,
                    modifiers,
                });
            }
            egui::Event::Key {
                key, pressed: true, ..
            } => {
                // Arrow / Home / End feed the text cursor; the
                // dispatcher's text-editing branch moves the
                // insertion point when a draft is active.
                let mapped = match key {
                    egui::Key::ArrowLeft => Some(Key::ArrowLeft),
                    egui::Key::ArrowRight => Some(Key::ArrowRight),
                    egui::Key::ArrowUp => Some(Key::ArrowUp),
                    egui::Key::ArrowDown => Some(Key::ArrowDown),
                    egui::Key::Home => Some(Key::Home),
                    egui::Key::End => Some(Key::End),
                    _ => None,
                };
                if let Some(k) = mapped {
                    state.dispatch_input(&InputEvent::Key { key: k, modifiers });
                }
            }
            _ => {}
        }
    }
}

/// Translate egui's modifier state to the toolkit-agnostic
/// [`Modifiers`] at the event boundary.
///
/// egui exposes both `ctrl` (the raw Ctrl key) and `command` (the
/// cross-platform chord modifier — Ctrl on Linux / Windows, ⌘ on
/// macOS). We map `command` onto our `Modifiers::ctrl` so the
/// dispatcher's chord-matching (`ctrl_only`, `ctrl_shift`) works
/// uniformly across platforms. The raw `ctrl` field is dropped: no
/// current Phase 1 shortcut binds to "Linux Ctrl but not macOS Cmd",
/// and adding one would require a separate `Modifiers::control_raw`
/// field — deferred until needed.
///
/// `egui::Modifiers::alt` and `:shift` map 1:1. `egui::Modifiers::
/// mac_cmd` is always `false` off macOS, so it maps cleanly onto
/// `Modifiers::super_key` without a platform `cfg!`.
///
/// Shared by the editor window and the Phase 5 overlay (plan
/// decision 5's factored translation helper).
pub(crate) fn translate_modifiers(egui_mods: egui::Modifiers) -> Modifiers {
    Modifiers {
        ctrl: egui_mods.command,
        shift: egui_mods.shift,
        alt: egui_mods.alt,
        super_key: egui_mods.mac_cmd,
    }
}

/// Parse the `[ui] magnifier-modifier` config string into an
/// [`egui::Key`]. The default is `Space`; unknown values fall back to
/// `Space` with a `tracing::warn!` so a typo doesn't silently disable
/// the feature.
///
/// Only a curated set of `egui::Key` variants that make ergonomic sense
/// as a *hold-to-reveal* trigger are mapped: ordinary character keys
/// (`S`, `C`, `Z`…) collide with the drawing shortcuts, and egui models
/// Alt / Ctrl / Shift / Super as modifier *flags* rather than `Key`
/// variants (so they can't be queried via `key_down`). Add a named
/// branch here when a new trigger key is needed.
fn parse_magnifier_key(name: &str) -> egui::Key {
    match name.trim() {
        "Space" => egui::Key::Space,
        "Tab" => egui::Key::Tab,
        "Insert" => egui::Key::Insert,
        other => {
            tracing::warn!(
                key = other,
                "unknown or unsupported magnifier-modifier; falling back to Space"
            );
            egui::Key::Space
        }
    }
}

/// The rail's tool gate from the selection-first phase state: a
/// live phase locks the rail to `Crop`; an ended (or never
/// requested) phase renders the full set. Pure so the phase→gate
/// contract is testable without a live `Ui`.
fn rail_gate(phase_active: bool) -> rail::ToolGate {
    if phase_active {
        rail::ToolGate::CropOnly
    } else {
        rail::ToolGate::All
    }
}

/// Map a single-key tool shortcut to its [`Tool`]. Only unmodified
/// presses reach this fn (see the guard in `logic`); the chords over
/// the same letters (Ctrl+S save, Ctrl+C copy, Ctrl+Z/Y history) are
/// handled by the core dispatcher and never collide.
///
/// Shared by the editor window and the Phase 5 overlay (plan
/// decision 5's factored translation helper).
pub(crate) fn tool_for_key(key: egui::Key) -> Option<Tool> {
    match key {
        egui::Key::V => Some(Tool::Select),
        egui::Key::H => Some(Tool::Hand),
        egui::Key::C => Some(Tool::Crop),
        egui::Key::R => Some(Tool::Rect),
        egui::Key::E => Some(Tool::Ellipse),
        egui::Key::A => Some(Tool::Arrow),
        egui::Key::L => Some(Tool::Line),
        egui::Key::M => Some(Tool::Marker),
        egui::Key::P => Some(Tool::Pencil),
        egui::Key::T => Some(Tool::Text),
        egui::Key::B => Some(Tool::Blur),
        egui::Key::N => Some(Tool::Number),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::Point;
    use flintshot_core::{Annotation, EditorState};

    // ── Clipboard-copy persistence (recent-captures-v2 Task 003) ──

    /// A config over a tempdir save location, with the persist flag.
    fn copy_persist_config(dir: &std::path::Path, on: bool) -> Config {
        let mut output = flintshot_core::OutputConfig::default();
        output.save_dir = dir.to_string_lossy().into_owned();
        output.save_clipboard_copies = on;
        let mut config = Config::default();
        config.output = output;
        config
    }

    /// Off-flag: no file, `None` — the copy stays invisible history.
    #[test]
    fn clipboard_persistence_off_flag_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let config = copy_persist_config(dir.path(), false);
        let out = persist_clipboard_copy_side_effect(&blank_state(4, 4), &config);
        assert!(out.is_none());
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            0,
            "no file written"
        );
    }

    /// On-flag: the copy lands in the resolved dir and the path
    /// names it (the caller records it into recents).
    #[test]
    fn clipboard_persistence_writes_the_resolved_dir() {
        let dir = tempfile::tempdir().unwrap();
        let config = copy_persist_config(dir.path(), true);
        let path = persist_clipboard_copy_side_effect(&blank_state(4, 4), &config)
            .expect("on-flag persists");
        assert!(path.exists(), "the file is on disk");
        assert!(
            path.starts_with(dir.path()),
            "lands in the configured dir: {path:?}"
        );
    }

    /// A persist failure (the save dir is a regular file) returns
    /// `None` without panicking — the clipboard write already
    /// succeeded and must never fail over the save.
    #[test]
    fn clipboard_persistence_failure_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"x").unwrap();
        let config = copy_persist_config(&blocker, true);
        let out = persist_clipboard_copy_side_effect(&blank_state(4, 4), &config);
        assert!(out.is_none(), "a failed save never fails the copy");
    }

    /// The save dialog prefills the resolved save dir (configured,
    /// or the platform pictures default when unset).
    #[test]
    fn save_dialog_dir_is_the_resolved_save_dir() {
        let mut output = flintshot_core::OutputConfig::default();
        output.save_dir = "/tmp/explicit".into();
        assert_eq!(
            save_dialog_dir(&output),
            std::path::PathBuf::from("/tmp/explicit")
        );
        assert!(save_dialog_dir(&flintshot_core::OutputConfig::default()).ends_with("Flintshot"));
    }

    /// Construct an all-white `EditorState` of the given pixel size.
    fn blank_state(w: u32, h: u32) -> EditorState {
        let img = image::RgbaImage::from_raw(w, h, vec![255; (w as usize) * (h as usize) * 4])
            .expect("raw buffer matches w*h*4");
        EditorState::new(img)
    }

    /// Every tool shortcut maps, and no two letters collapse onto the
    /// same tool (the rail tooltips advertise these letters).
    #[test]
    fn tool_shortcuts_are_complete_and_injective() {
        let pairs = [
            (egui::Key::V, Tool::Select),
            (egui::Key::H, Tool::Hand),
            (egui::Key::C, Tool::Crop),
            (egui::Key::R, Tool::Rect),
            (egui::Key::E, Tool::Ellipse),
            (egui::Key::A, Tool::Arrow),
            (egui::Key::L, Tool::Line),
            (egui::Key::M, Tool::Marker),
            (egui::Key::P, Tool::Pencil),
            (egui::Key::T, Tool::Text),
            (egui::Key::B, Tool::Blur),
            (egui::Key::N, Tool::Number),
        ];
        for (key, tool) in pairs {
            assert_eq!(tool_for_key(key), Some(tool));
        }
        // Keys that must never switch tools: chords are handled by the
        // dispatcher, and unrelated letters fall through. S is freed
        // (formerly Select's key, now unbound).
        for key in [
            egui::Key::S,
            egui::Key::Z,
            egui::Key::Y,
            egui::Key::Escape,
            egui::Key::Space,
            egui::Key::Enter,
        ] {
            assert_eq!(tool_for_key(key), None, "{key:?} must not select a tool");
        }
    }

    /// The compose + write path that `handle_upload` runs must yield a
    /// file whose bytes decode as a valid PNG at the source dimensions.
    /// Headless half of Task 009's verification (the spawn itself needs
    /// the binary + network + daemon and is `#[ignore]`'d by nature).
    #[test]
    fn compose_and_write_upload_writes_decodable_png() {
        let state = blank_state(8, 6);
        let path = compose_and_write_upload(&state).expect("compose + write succeeds");
        let bytes = std::fs::read(&path).expect("temp file is readable");
        let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
            .expect("written bytes decode as PNG");
        assert_eq!(
            decoded.to_rgba8().dimensions(),
            (8, 6),
            "decoded size matches source"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// A selection-active upload must emit just the selected region
    /// (parity with the Copy selection fix at `copy_to_clipboard`).
    #[test]
    fn compose_and_write_upload_honours_selection() {
        let mut state = blank_state(10, 10);
        state.crop_region = Some(Rect::from_xywh(2.0, 3.0, 4.0, 3.0));
        let path = compose_and_write_upload(&state).expect("compose + write succeeds");
        let bytes = std::fs::read(&path).expect("temp file is readable");
        let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
            .expect("written bytes decode as PNG");
        assert_eq!(
            decoded.to_rgba8().dimensions(),
            (4, 3),
            "selection crops the uploaded image"
        );
        let _ = std::fs::remove_file(&path);
    }

    // --- Selection-aware compose (Task 006) ---

    #[test]
    fn compose_selection_aware_without_selection_is_full_image() {
        let img = compose_selection_aware(&blank_state(10, 8));
        assert_eq!(img.dimensions(), (10, 8), "no selection → full image");
    }

    #[test]
    fn compose_selection_aware_crops_to_usable_selection() {
        let mut state = blank_state(10, 10);
        state.crop_region = Some(Rect::from_xywh(2.0, 3.0, 4.0, 3.0));
        let img = compose_selection_aware(&state);
        assert_eq!(img.dimensions(), (4, 3), "selection crops the output");
    }

    #[test]
    fn compose_selection_aware_falls_back_on_degenerate_selection() {
        let mut state = blank_state(10, 8);
        state.crop_region = Some(Rect::from_xywh(2.0, 3.0, 0.5, 3.0));
        let img = compose_selection_aware(&state);
        assert_eq!(
            img.dimensions(),
            (10, 8),
            "sub-1px selection is degenerate → full image, matching copy/upload"
        );
    }

    // --- Selection-first phase (Task 001) ---

    use std::cell::RefCell;
    use std::rc::Rc;

    /// Construct an `EditorApp` over a blank 64×64 state with the
    /// given selection-first request. Headless: `EditorApp::new` needs
    /// no display (the bg texture uploads lazily on the first `ui`).
    fn phase_app(selection_first: bool) -> EditorApp {
        EditorApp::new(
            blank_state(64, 64),
            &egui::Context::default(),
            Rc::new(RefCell::new(None)),
            selection_first,
        )
    }

    /// Drag a fresh selection via the dispatcher's exact event
    /// sequence, then release.
    fn drag_selection(state: &mut EditorState, min: (f32, f32), max: (f32, f32)) {
        let _ = state.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(min.0, min.1),
            modifiers: Modifiers::default(),
        });
        let _ = state.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(max.0, max.1),
        });
        let _ = state.dispatch_input(&InputEvent::PointerUp);
    }

    /// The rail gate follows the phase: live phase → Crop-only
    /// rail; ended / never-requested phase → full rail.
    #[test]
    fn rail_gate_tracks_phase() {
        assert_eq!(rail_gate(true), rail::ToolGate::CropOnly);
        assert_eq!(rail_gate(false), rail::ToolGate::All);
    }

    #[test]
    fn selection_first_opens_crop_armed_and_phase_active() {
        let app = phase_app(true);
        assert_eq!(
            app.state.tool,
            Tool::Crop,
            "the first drag must draw a crop region, not a rect annotation"
        );
        assert!(app.phase_active(), "no crop yet → phase is live");
    }

    #[test]
    fn plain_editor_keeps_default_tool_and_no_phase() {
        let app = phase_app(false);
        assert_eq!(
            app.state.tool,
            Tool::Select,
            "the editor default is untouched"
        );
        assert!(!app.phase_active(), "no request → no phase");
    }

    // --- Hand-tool hover / grab chrome (prominent-move) ---

    /// One eframe-shaped frame for the editor window: `logic_impl`
    /// then `ui_impl` inside a single egui pass over the given extra
    /// input events. Screen is 200×200 so the toolbar strip leaves
    /// the 64×64 canvas room to breathe. Returns the frame's full
    /// output so tests can inspect viewport commands.
    fn editor_frame(ctx: &egui::Context, app: &mut EditorApp, events: Vec<egui::Event>) {
        let _ = editor_frame_output(ctx, app, events);
    }

    /// [`editor_frame`], keeping the [`egui::FullOutput`].
    fn editor_frame_output(
        ctx: &egui::Context,
        app: &mut EditorApp,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        // Roomy enough for the docked side panel (260 px) plus a
        // real canvas; the retired top-rail era fit in 200×200.
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(640.0, 480.0),
            )),
            events,
            ..egui::RawInput::default()
        };
        ctx.run_ui(raw, |ui| {
            app.logic_impl(ui.ctx());
            app.ui_impl(ui);
        })
    }

    /// [`editor_frame`] with an explicit input clock — double-click
    /// detection is time-based, so timing-sensitive tests pin `t`
    /// instead of relying on the pass-to-pass `predicted_dt` step.
    fn editor_frame_at(ctx: &egui::Context, app: &mut EditorApp, t: f64, events: Vec<egui::Event>) {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(640.0, 480.0),
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

    /// Primary-button press/release event pair builder at `pos`.
    fn primary_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        }
    }

    /// An editor state on the default Select tool with one committed
    /// rect annotation — something to hover.
    fn move_state_with_ann() -> EditorState {
        // The annotation sits below the floating cluster's band (the
        // canvas's top-left ~40 px are chrome now).
        let mut state = blank_state(64, 64);
        state.pending = Some(Annotation::Rect {
            rect: Rect::from_xywh(10.0, 50.0, 40.0, 30.0),
            stroke: flintshot_core::Stroke::new(flintshot_core::Color::RED, 3.0),
            fill: None,
        });
        state.commit_annotation();
        assert_eq!(state.tool, Tool::Select, "fixture: tool stays Select");
        state
    }

    /// Run one logic pass with the pointer parked at canvas-local
    /// (phys) — ppp is 1 in a synthetic context — and return the
    /// frame's cursor icon.
    fn cursor_after_hover(
        ctx: &egui::Context,
        app: &mut EditorApp,
        phys: (f32, f32),
    ) -> egui::CursorIcon {
        let canvas = app.canvas_rect.expect("fixture: canvas rect cached");
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(200.0, 200.0),
            )),
            events: vec![egui::Event::PointerMoved(egui::pos2(
                canvas.min.x + phys.0,
                canvas.min.y + phys.1,
            ))],
            ..egui::RawInput::default()
        };
        let mut icon = egui::CursorIcon::Default;
        let _ = ctx.run_ui(raw, |ui| {
            app.logic_impl(ui.ctx());
            icon = ui.ctx().output(|o| o.cursor_icon);
        });
        icon
    }

    #[test]
    fn move_hover_over_annotation_sets_move_cursor() {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(
            move_state_with_ann(),
            &ctx,
            Rc::new(RefCell::new(None)),
            false,
        );
        editor_frame(&ctx, &mut app, vec![]);
        assert!(
            app.canvas_rect.is_some(),
            "fixture: first frame caches the canvas rect"
        );
        assert_eq!(
            cursor_after_hover(&ctx, &mut app, (30.0, 65.0)),
            egui::CursorIcon::Move,
            "hovering the annotation under Hand must show the grab cursor"
        );
    }

    #[test]
    fn move_hover_over_empty_canvas_sets_no_cursor() {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(
            move_state_with_ann(),
            &ctx,
            Rc::new(RefCell::new(None)),
            false,
        );
        editor_frame(&ctx, &mut app, vec![]);
        assert_eq!(
            cursor_after_hover(&ctx, &mut app, (5.0, 5.0)),
            egui::CursorIcon::Default,
            "empty canvas under Hand must leave the default cursor"
        );
    }

    #[test]
    fn move_grab_drag_keeps_move_cursor_and_live_chrome() {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(
            move_state_with_ann(),
            &ctx,
            Rc::new(RefCell::new(None)),
            false,
        );
        editor_frame(&ctx, &mut app, vec![]);
        let canvas = app.canvas_rect.expect("fixture: canvas rect cached");
        // Press on the annotation, drag a little, release not yet.
        let press = egui::pos2(canvas.min.x + 30.0, canvas.min.y + 65.0);
        editor_frame(
            &ctx,
            &mut app,
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
        let mut icon = egui::CursorIcon::Default;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(200.0, 200.0),
            )),
            events: vec![egui::Event::PointerMoved(egui::pos2(
                canvas.min.x + 36.0,
                canvas.min.y + 69.0,
            ))],
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            app.logic_impl(ui.ctx());
            icon = ui.ctx().output(|o| o.cursor_icon);
        });
        assert!(
            app.state.moving.is_some(),
            "drag keeps the grab live (no release yet)"
        );
        assert_eq!(icon, egui::CursorIcon::Move);
    }

    #[test]
    fn enter_crop_ends_phase() {
        let mut app = phase_app(true);
        drag_selection(&mut app.state, (10.0, 10.0), (40.0, 30.0));
        let _ = app.state.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        app.end_phase_on_crop();
        assert!(
            !app.phase_active(),
            "the first Enter-crop must end the phase"
        );
    }

    #[test]
    fn phase_end_crop_flips_crop_to_select() {
        let mut app = phase_app(true);
        drag_selection(&mut app.state, (10.0, 10.0), (40.0, 30.0));
        let _ = app.state.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        app.end_phase_on_crop();
        assert!(!app.phase_active(), "fixture: the crop ended the phase");
        assert_eq!(
            app.state.tool,
            Tool::Select,
            "the edit phase opens on Select, not the phase's armed Crop"
        );
    }

    #[test]
    fn phase_end_keeps_non_crop_tool_against_flip() {
        let mut app = phase_app(true);
        // Direct state mutation — with the input gates live, no
        // keyboard / chip / drag path can leave Crop mid-phase.
        // The latch's still-Crop guard is defensive for exactly
        // this state, and must not clobber it.
        app.state.tool = Tool::Rect;
        app.state.crop_region = Some(Rect::from_xywh(8.0, 8.0, 32.0, 16.0));
        app.state.confirm_crop(); // the "Crop · Enter" button's call
        app.end_phase_on_crop();
        assert!(!app.phase_active(), "fixture: the crop ended the phase");
        assert_eq!(
            app.state.tool,
            Tool::Rect,
            "a non-Select tool at latch (direct mutation only) must survive the flip"
        );
    }

    #[test]
    fn panel_crop_button_path_ends_phase() {
        let mut app = phase_app(true);
        app.state.crop_region = Some(Rect::from_xywh(8.0, 8.0, 32.0, 16.0));
        app.state.confirm_crop(); // the "Crop · Enter" button's call
        app.end_phase_on_crop();
        assert!(!app.phase_active(), "the panel crop must end the phase too");
    }

    #[test]
    fn enter_without_selection_keeps_phase() {
        let mut app = phase_app(true);
        let _ = app.state.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        app.end_phase_on_crop();
        assert!(
            app.phase_active(),
            "Enter with no selection is a no-op, not a crop"
        );
    }

    /// Regression: `logic`'s key translation never forwarded
    /// `egui::Key::Enter`, so the dispatcher's Enter-crop branch was
    /// reachable only through the toolbar button — with a live
    /// selection, pressing Enter did nothing (reported on the
    /// Wayland-demoted selection-first editor, 2026-09-03). This
    /// drives the *shell translation* through a synthetic
    /// `egui::Context`, not the dispatcher directly.
    /// Ctrl+N publishes `NewCapture { Region }` through the shell's
    /// translation (mirrors `enter_key_crops_through_shell`'s
    /// synthetic-context drive). Plain N must NOT: it stays a tool
    /// key (Text).
    /// egui-winit sets `command` alongside `ctrl` on Linux/Windows
    /// (egui's own `Modifiers::CTRL` leaves it false), and the shell's
    /// `translate_modifiers` reads `command` — synthesize the
    /// platform-shaped state.
    fn ctrl_as_winit_reports_it() -> egui::Modifiers {
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..egui::Modifiers::NONE
        }
    }

    #[test]
    fn ctrl_n_publishes_new_capture_and_plain_n_does_not() {
        let mut app = phase_app(false);
        let ctx = egui::Context::default();
        let key = |mods: egui::Modifiers| egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::N,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: mods,
            }],
            modifiers: mods,
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(key(egui::Modifiers::NONE), |ui| app.logic_impl(ui.ctx()));
        assert!(
            app.outcome.borrow().is_none(),
            "plain N is a tool key, not a session action"
        );
        assert_ne!(app.state.tool, Tool::Text, "plain N armed the Text tool");

        let _ = ctx.run_ui(key(ctrl_as_winit_reports_it()), |ui| {
            app.logic_impl(ui.ctx());
        });
        let published = app.outcome.borrow();
        assert!(
            matches!(
                published.as_ref(),
                Some(Ok(EditorOutcome::NewCapture {
                    mode: NewCaptureMode::Region
                }))
            ),
            "Ctrl+N must publish the region recapture intent"
        );
        drop(published);
    }

    #[test]
    fn enter_key_crops_through_shell() {
        let mut app = phase_app(true);
        drag_selection(&mut app.state, (8.0, 8.0), (40.0, 32.0));
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| app.logic_impl(ui.ctx()));
        assert!(
            !app.phase_active(),
            "Enter through the shell translation must crop and end the phase"
        );
        assert_eq!(
            app.state.image.dimensions(),
            (32, 24),
            "the crop actually replaced the image"
        );
    }

    #[test]
    fn annotation_commit_keeps_phase() {
        let mut app = phase_app(true);
        // Direct state mutation (input is phase-gated): a non-Select
        // drag through the dispatcher commits an anns-only snapshot,
        // which must not read as a crop.
        app.state.tool = Tool::Rect;
        drag_selection(&mut app.state, (5.0, 5.0), (20.0, 20.0));
        assert_eq!(app.state.anns.len(), 1, "fixture: one annotation committed");
        app.end_phase_on_crop();
        assert!(
            app.phase_active(),
            "anns-only snapshots must not read as crops"
        );
    }

    #[test]
    fn phase_end_survives_undo() {
        let mut app = phase_app(true);
        drag_selection(&mut app.state, (10.0, 10.0), (40.0, 30.0));
        let _ = app.state.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        app.end_phase_on_crop();
        // Undo pops the full snapshot (restoring the pre-crop image);
        // the latch must keep the phase ended.
        let _ = app.state.dispatch_input(&InputEvent::Key {
            key: Key::Char('z'),
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        });
        assert_eq!(
            app.state.image.dimensions(),
            (64, 64),
            "fixture: undo restored the full capture"
        );
        app.end_phase_on_crop();
        assert!(
            !app.phase_active(),
            "the latch is one-way: undo must not resurrect the phase"
        );
    }

    // --- Selection-first Esc + outputs (Task 003) ---

    #[test]
    fn esc_during_phase_cancels_without_side_effects() {
        let mut app = phase_app(true);
        // The exact event `logic` translates an Esc press into. With
        // no selection yet the dispatcher must answer Cancel — the
        // outcome `logic` publishes-if-unset as Cancelled and closes
        // on, which the app crate reports as "no file written" before
        // exiting 0 (the overlay's cancel contract).
        let outcome = app.state.dispatch_input(&InputEvent::Key {
            key: Key::Escape,
            modifiers: Modifiers::default(),
        });
        assert!(matches!(outcome, DispatchOutcome::Cancel));
        // No state side effects: nothing committed, no selection
        // fabricated — and a stray Esc must not end the phase (only
        // a crop does), so the landing chrome survives it.
        assert!(app.state.anns.is_empty());
        assert!(app.state.crop_region.is_none());
        assert!(app.phase_active());
    }

    #[test]
    fn esc_with_live_selection_keeps_standing_cancel() {
        let mut app = phase_app(true);
        drag_selection(&mut app.state, (5.0, 5.0), (30.0, 25.0));
        // Spec: after a selection exists, Esc keeps today's editor
        // behaviour — the standing close-as-Cancelled, not a
        // selection-clearing Esc.
        let outcome = app.state.dispatch_input(&InputEvent::Key {
            key: Key::Escape,
            modifiers: Modifiers::default(),
        });
        assert!(matches!(outcome, DispatchOutcome::Cancel));
    }

    #[test]
    fn esc_after_copy_keeps_copied_outcome() {
        // A copy pre-publishes CopiedToClipboard and keeps the window
        // open; the later Esc close must honour it. This is the
        // if-unset publish — the mechanism behind "keeps today's
        // editor behaviour" after an output has fired.
        let app = phase_app(true);
        *app.outcome.borrow_mut() = Some(Ok(EditorOutcome::CopiedToClipboard));
        app.publish_outcome_if_unset(Ok(EditorOutcome::Cancelled));
        assert!(
            matches!(
                *app.outcome.borrow(),
                Some(Ok(EditorOutcome::CopiedToClipboard))
            ),
            "the prior copy wins over the Esc cancel"
        );
    }

    #[test]
    fn copy_lingers_on_linux_closes_where_the_os_owns_the_clipboard() {
        // Probe evidence (ux-redesign-regressions spec §2): COSMIC
        // does not adopt an exited client's zwlr_data_control
        // selection, and arboard serves the selection from an
        // in-process thread — so a Linux worker copy must linger on
        // both display servers. macOS/Windows own the clipboard
        // OS-side, so the close is safe there.
        assert!(
            !close_after_copy(false),
            "Linux (X11 or Wayland): keep serving the selection"
        );
        assert!(
            close_after_copy(true),
            "OS-owned clipboard (macOS/Windows): close after copy"
        );
    }

    #[test]
    fn copied_visible_only_inside_the_two_second_window() {
        let mut app = phase_app(false);
        app.copied_at = Some(10.0);
        assert!(app.copied_visible(10.0), "at 0 s: visible");
        assert!(app.copied_visible(11.9), "at 1.9 s: still visible");
        assert!(!app.copied_visible(12.1), "at 2.1 s: reverted");
        assert!(!app.copied_visible(9.9), "before the instant: unarmed");
    }

    #[test]
    fn fresh_editor_starts_unarmed() {
        // The state is time-windowed, not latched: a fresh EditorApp
        // (every editor route constructs one) can never inherit a
        // stale "Copied".
        let app = phase_app(false);
        assert!(!app.copied_visible(0.0));
        assert!(!app.copied_visible(f64::MAX));
    }

    #[test]
    fn copied_repaint_delay_tracks_the_remaining_window() {
        assert_eq!(
            copied_repaint_delay(None, 0.0),
            None,
            "unarmed: no revert repaint"
        );
        assert_eq!(
            copied_repaint_delay(Some(10.0), 12.1),
            None,
            "past the window: no revert repaint"
        );
        let inside = copied_repaint_delay(Some(10.0), 11.0).expect("inside: repaints");
        assert!(
            inside <= std::time::Duration::from_secs_f64(1.0),
            "at 1 s elapsed the remaining window is 1 s: got {inside:?}"
        );
        let fresh = copied_repaint_delay(Some(10.0), 10.0).expect("at the instant: repaints");
        assert!(
            fresh <= std::time::Duration::from_secs_f64(2.0) && !fresh.is_zero(),
            "at 0 s elapsed the delay is at most the full window: got {fresh:?}"
        );
    }

    #[test]
    fn copied_state_requests_the_revert_repaint_in_frame() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut app = phase_app(false);
        // A first frame primes egui's input time (raw time None → 0.0
        // on the first pass); arming at 0.0 makes the state visible
        // with the full window remaining.
        let _ = editor_frame_output(&ctx, &mut app, vec![]);
        app.copied_at = Some(ctx.input(|i| i.time));
        let full = editor_frame_output(&ctx, &mut app, vec![]);
        let delay = full.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        assert!(
            delay <= std::time::Duration::from_secs(2),
            "the revert lands within the 2 s window, got {delay:?}"
        );
    }

    // --- Single-window page mode (single-window Task 003) ---

    use std::collections::VecDeque;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use crate::home::HomeDirective;
    use crate::shell::{EditorPageBridge, PageBridgeSlot};

    #[derive(Default)]
    struct PageRecorder {
        reported: Vec<EditorOutcome>,
        statuses: Vec<String>,
        /// Chrome-open calls (path, routed).
        opened: Vec<(PathBuf, bool)>,
        pastes: usize,
    }

    /// Bridge double: records reports/statuses, replays directives.
    #[derive(Default, Clone)]
    struct RecordingPageBridge {
        rec: Arc<Mutex<PageRecorder>>,
        pending: Arc<Mutex<VecDeque<HomeDirective>>>,
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
            self.pending.lock().expect("pending mutex").pop_front()
        }

        fn open_image(&mut self, path: PathBuf) -> bool {
            // Fail (false) for paths containing "bad" — one double,
            // both contract arms.
            let routed = !path.to_string_lossy().contains("bad");
            self.rec
                .lock()
                .expect("recorder mutex")
                .opened
                .push((path, routed));
            routed
        }

        fn request_paste(&mut self) -> bool {
            let mut rec = self.rec.lock().expect("recorder mutex");
            rec.pastes += 1;
            true
        }
    }

    /// A page-mode `EditorApp` over a recording bridge.
    fn page_app(bridge: RecordingPageBridge) -> EditorApp {
        let slot: PageBridgeSlot = Rc::new(RefCell::new(Box::new(bridge)));
        EditorApp::new_page(
            blank_state(64, 64),
            &egui::Context::default(),
            Rc::new(RefCell::new(None)),
            false,
            slot,
        )
    }

    // --- Stable viewport fit (editor-capture-ux Task 004) ---

    /// A page-mode app over an arbitrarily sized image.
    fn page_app_sized(w: u32, h: u32) -> EditorApp {
        let slot: PageBridgeSlot = Rc::new(RefCell::new(Box::new(RecordingPageBridge::default())));
        EditorApp::new_page(
            blank_state(w, h),
            &egui::Context::default(),
            Rc::new(RefCell::new(None)),
            false,
            slot,
        )
    }

    /// Whether the frame's output asked the viewport to resize.
    fn inner_size_command_issued(full: &egui::FullOutput) -> bool {
        full.viewport_output
            .values()
            .flat_map(|v| v.commands.iter())
            .any(|c| matches!(c, egui::ViewportCommand::InnerSize(_)))
    }

    /// A big image in the 640×480 synthetic screen must render
    /// scaled down (fit < 1) and centered in the canvas panel.
    #[test]
    fn page_large_image_fits_scaled_and_centered() {
        let ctx = egui::Context::default();
        let mut app = page_app_sized(400, 300);
        for _ in 0..3 {
            editor_frame(&ctx, &mut app, vec![]);
        }
        let scale = app.canvas.scale();
        assert!(
            scale > 0.0 && scale < 1.0,
            "a 400×300 image in a ~380-wide canvas must scale down, got {scale}"
        );
        let rect = app.canvas_rect.expect("frames cached the canvas rect");
        let view = app.canvas.view_over(rect);
        let center = view.image_to_logical(flintshot_core::Point::new(200.0, 150.0));
        assert!(
            (center - rect.center()).length() < 1.0,
            "the fitted image centres in the panel: {center:?} vs {:?}",
            rect.center()
        );
    }

    /// A tiny image must never upscale: it renders 1:1, letterboxed
    /// and centered inside the panel.
    #[test]
    fn page_tiny_image_stays_1_to_1_and_letterboxes() {
        let ctx = egui::Context::default();
        let mut app = page_app_sized(64, 64);
        for _ in 0..3 {
            editor_frame(&ctx, &mut app, vec![]);
        }
        assert!(
            (app.canvas.scale() - 1.0).abs() < 1e-6,
            "never upscale: scale stays 1.0, got {}",
            app.canvas.scale()
        );
        let offset = app.canvas.content_offset();
        assert!(
            offset.x > 0.0 && offset.y > 0.0,
            "a 64×64 image letterboxes in the panel, got offset {offset:?}"
        );
    }

    /// Crop keeps the window: the smaller image re-fits inside the
    /// unchanged viewport, caches invalidate, and no resize command
    /// ever fires in page mode.
    #[test]
    fn page_crop_refits_without_a_resize_command() {
        let ctx = egui::Context::default();
        let mut app = page_app_sized(400, 300);
        for _ in 0..3 {
            editor_frame(&ctx, &mut app, vec![]);
        }
        assert!(app.canvas.scale() < 1.0, "fixture: the big image fitted");
        // Simulate a crop: the image shrinks in place.
        app.state.image = blank_state(100, 80).image;
        let full = editor_frame_output(&ctx, &mut app, vec![]);
        assert!(
            !inner_size_command_issued(&full),
            "crop must not resize the stable editor window"
        );
        assert_eq!(
            app.img_dims,
            (100, 80),
            "the dims change still invalidates the bg + blur caches"
        );
        let scale = app.canvas.scale();
        assert!(
            scale > 0.99 && scale <= 1.0,
            "the smaller image re-fits (1:1 here), got {scale}"
        );
    }

    /// Worker windows keep the image-sized lifecycle: a big image
    /// still issues the resize command (out of scope by spec).
    #[test]
    fn worker_window_still_resizes_to_the_image() {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(
            blank_state(400, 300),
            &ctx,
            Rc::new(RefCell::new(None)),
            false,
        );
        let full = editor_frame_output(&ctx, &mut app, vec![]);
        assert!(
            inner_size_command_issued(&full),
            "the worker window keeps its image-sized resize behavior"
        );
    }

    /// The floating +/− chips zoom the canvas view (the regression
    /// behind "zoom + and - are not working": they used to step the
    /// loupe's invisible factor). One + click steps the view scale
    /// up on the next fit pass, and a zoomed worker view stops
    /// issuing image-sized resize commands — the fit letterboxes
    /// inside the stable window instead.
    #[test]
    fn zoom_chips_step_the_view_scale() {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(
            blank_state(400, 300),
            &ctx,
            Rc::new(RefCell::new(None)),
            false,
        );
        // Settle: the floating Area pins a pass behind its layout.
        for _ in 0..3 {
            editor_frame(&ctx, &mut app, vec![]);
        }
        let before = app.canvas.scale();
        let chip = ctx
            .read_response(egui::Id::new("zoom-in"))
            .expect("the zoom-in chip registers")
            .rect;
        editor_frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(chip.center())],
        );
        editor_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(chip.center()),
                egui::Event::PointerButton {
                    pos: chip.center(),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: chip.center(),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(
            (app.view_zoom - 1.25).abs() < 1e-6,
            "one + click steps the zoom to 1.25×, got {}",
            app.view_zoom
        );
        // The fit pass applies the zoom and no resize is issued
        // while zoomed (the window is stable; the transform moves).
        let full = editor_frame_output(&ctx, &mut app, vec![]);
        assert!(
            app.canvas.scale() > before,
            "the view scale steps up with the zoom ({} → {})",
            before,
            app.canvas.scale()
        );
        assert!(
            !inner_size_command_issued(&full),
            "a zoomed worker view must not chase the image-sized window"
        );
    }

    // --- View pan + wheel zoom (select-crop-hand Task 005) ---

    /// An editor app over an 800×600 image in a 600×380 canvas (the
    /// fit scales down; zooming in overflows the viewport so pan has
    /// room). The fixture pins `canvas_rect` and never runs the ui
    /// pass (it would re-cache the rect from the synthetic 640×480
    /// screen), so input frames drive `logic_impl` only and the fit
    /// composes on demand via `settle_fit`.
    fn pannable_app() -> (egui::Context, EditorApp) {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(
            blank_state(800, 600),
            &ctx,
            Rc::new(RefCell::new(None)),
            false,
        );
        app.canvas_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(600.0, 380.0),
        ));
        app.update_fit(&ctx);
        (ctx, app)
    }

    /// One logic-only frame over the pinned canvas (the events a
    /// real pointer would deliver), then the fit composition.
    fn logic_frame(ctx: &egui::Context, app: &mut EditorApp, events: Vec<egui::Event>) {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(600.0, 380.0),
            )),
            events,
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            app.logic_impl(ui.ctx());
        });
        app.update_fit(ctx);
    }

    fn wheel_up(at: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 50.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ]
    }

    fn wheel_down(at: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -50.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ]
    }

    /// Wheel notches step the zoom a quantized 0.25 each and keep
    /// the image point under the cursor fixed wherever pan is free
    /// (the pan absorbs the difference); at a zoom where the image
    /// overflows the viewport on both axes, the composed view maps
    /// the cursor to the same image pixel before and after.
    #[test]
    fn wheel_zoom_steps_and_keeps_cursor_anchored() {
        let (ctx, mut app) = pannable_app();
        let canvas = app.canvas_rect.expect("fixture");
        let cursor = canvas.min + egui::vec2(300.0, 190.0);
        logic_frame(&ctx, &mut app, wheel_up(cursor));
        assert!(
            (app.view_zoom - 1.25).abs() < 1e-6,
            "one wheel notch steps to 1.25×, got {}",
            app.view_zoom
        );
        // Settle, then snapshot the anchor. From here the image
        // overflows the viewport enough that the second notch (1.5×)
        // has pan free on both axes — the regime where the anchor
        // contract is exact (a clamped axis recenters instead).
        logic_frame(&ctx, &mut app, vec![egui::Event::PointerMoved(cursor)]);
        let anchor = app.canvas.view_over(canvas).logical_to_image(cursor);
        logic_frame(&ctx, &mut app, wheel_up(cursor));
        assert!(
            (app.view_zoom - 1.5).abs() < 1e-6,
            "the second notch steps to 1.5×, got {}",
            app.view_zoom
        );
        // Settle one frame so the fit pass composes the new pan.
        logic_frame(&ctx, &mut app, vec![egui::Event::PointerMoved(cursor)]);
        let mapped = app.canvas.view_over(canvas).logical_to_image(cursor);
        assert!(
            ((mapped.x - anchor.x).abs() < 1e-2) && ((mapped.y - anchor.y).abs() < 1e-2),
            "cursor anchor drifted: before {anchor:?} after {mapped:?}"
        );
    }

    /// Zooming back to the 1.0 fit zoom resets the pan to zero —
    /// the image fits again and the worker resize policy's trigger
    /// (`at_fit_zoom`) stays exact.
    #[test]
    fn zoom_back_to_fit_resets_pan() {
        let (ctx, mut app) = pannable_app();
        let canvas = app.canvas_rect.expect("fixture");
        let cursor = canvas.min + egui::vec2(300.0, 190.0);
        logic_frame(&ctx, &mut app, wheel_up(cursor));
        logic_frame(&ctx, &mut app, vec![]);
        assert!(app.view_zoom > 1.0, "fixture: zoomed in");
        // Force a non-zero pan and confirm the clamp allows it here.
        assert!(
            app.pan != egui::Vec2::ZERO || app.canvas.content_offset() != egui::Vec2::ZERO,
            "fixture: the zoomed view moved the offset"
        );
        logic_frame(&ctx, &mut app, wheel_down(cursor));
        assert!((app.view_zoom - 1.0).abs() < 1e-6, "back at the fit zoom");
        assert_eq!(
            app.pan,
            egui::Vec2::ZERO,
            "returning to the fit zoom resets pan"
        );
        assert!(app.at_fit_zoom(), "at_fit_zoom is exact again");
    }

    /// A middle-button drag pans from any tool — here Rect, an
    /// annotation tool — and never dispatches a pointer event (no
    /// draft starts).
    #[test]
    fn middle_drag_pans_from_any_tool() {
        let (ctx, mut app) = pannable_app();
        app.state.set_tool(Tool::Rect);
        let canvas = app.canvas_rect.expect("fixture");
        let from = canvas.min + egui::vec2(300.0, 190.0);
        let to = canvas.min + egui::vec2(250.0, 170.0);
        // Zoom in twice so the image overflows the viewport on both
        // axes (pan has room).
        logic_frame(&ctx, &mut app, wheel_up(from));
        logic_frame(&ctx, &mut app, wheel_up(from));
        logic_frame(&ctx, &mut app, vec![]);
        let start_pan = app.pan;
        logic_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(from),
                egui::Event::PointerButton {
                    pos: from,
                    button: egui::PointerButton::Middle,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        logic_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(to),
                egui::Event::PointerButton {
                    pos: to,
                    button: egui::PointerButton::Middle,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        // The delta applies and the clamp bounds it (both pinned by
        // the pure-fn tests; here the wiring is the contract).
        let scale = app.canvas.scale();
        let img_logical = egui::vec2(800.0 * scale, 600.0 * scale);
        let expected = crate::canvas::clamp_pan(
            start_pan + egui::vec2(-50.0, -20.0),
            img_logical,
            canvas.size(),
        );
        assert!(
            ((app.pan.x - expected.x).abs() < 1e-3) && ((app.pan.y - expected.y).abs() < 1e-3),
            "middle drag applies start_pan + delta (clamped), got {:?} want {expected:?}",
            app.pan
        );
        assert!(app.pan != start_pan, "the drag moved the view");
        assert!(
            app.state.pending.is_none() && app.state.anns.is_empty(),
            "the middle drag never reaches the dispatcher"
        );
        assert!(app.pan_drag.is_none(), "release ends the pan drag");
    }

    /// A Hand primary drag pans — the press never dispatches (the
    /// pan-only contract; annotations are untouchable under Hand) —
    /// and a committed annotation stays exactly where it was.
    #[test]
    fn hand_primary_drag_pans_and_never_touches_annotations() {
        let (ctx, mut app) = pannable_app();
        app.state.set_tool(Tool::Hand);
        app.state.anns.push(Annotation::Rect {
            rect: flintshot_core::Rect::from_xywh(10.0, 10.0, 40.0, 30.0),
            stroke: flintshot_core::Stroke::new(flintshot_core::Color::RED, 3.0),
            fill: None,
        });
        let canvas = app.canvas_rect.expect("fixture");
        let from = canvas.min + egui::vec2(300.0, 190.0);
        let to = canvas.min + egui::vec2(260.0, 150.0);
        logic_frame(&ctx, &mut app, wheel_up(from));
        logic_frame(&ctx, &mut app, wheel_up(from));
        logic_frame(&ctx, &mut app, vec![]);
        let start_pan = app.pan;
        logic_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(from),
                egui::Event::PointerButton {
                    pos: from,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(
            app.pan_drag.is_some(),
            "a Hand press starts a pan, not a dispatcher drag"
        );
        assert!(
            app.state.selected.is_empty() && app.state.moving.is_none(),
            "the Hand press never routes selection state"
        );
        logic_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(to),
                egui::Event::PointerButton {
                    pos: to,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let scale = app.canvas.scale();
        let img_logical = egui::vec2(800.0 * scale, 600.0 * scale);
        let expected = crate::canvas::clamp_pan(
            start_pan + egui::vec2(-40.0, -40.0),
            img_logical,
            canvas.size(),
        );
        assert!(
            ((app.pan.x - expected.x).abs() < 1e-3) && ((app.pan.y - expected.y).abs() < 1e-3),
            "the Hand drag applies start_pan + delta (clamped), got {:?} want {expected:?}",
            app.pan
        );
        assert!(app.pan != start_pan, "the drag moved the view");
        match &app.state.anns[0] {
            Annotation::Rect { rect, .. } => {
                assert_eq!(rect.min, flintshot_core::Point::new(10.0, 10.0));
            }
            other => panic!("expected Rect, got {other:?}"),
        }
        assert!(app.state.undo.is_empty(), "panning records no history");
        assert!(app.pan_drag.is_none(), "release ends the pan drag");
    }

    /// The Hand cursor is the pan affordance: `Grab` on hover,
    /// `Grabbing` mid-drag.
    #[test]
    fn hand_cursor_is_grab_then_grabbing() {
        let (ctx, mut app) = pannable_app();
        app.state.set_tool(Tool::Hand);
        let hover = cursor_after_hover(&ctx, &mut app, (300.0, 190.0));
        assert_eq!(hover, egui::CursorIcon::Grab, "hover shows the open hand");
        let canvas = app.canvas_rect.expect("fixture");
        let from = canvas.min + egui::vec2(300.0, 190.0);
        logic_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(from),
                egui::Event::PointerButton {
                    pos: from,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let mut icon = egui::CursorIcon::Default;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(600.0, 380.0),
            )),
            events: vec![egui::Event::PointerMoved(from)],
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            app.logic_impl(ui.ctx());
            icon = ui.ctx().output(|o| o.cursor_icon);
        });
        assert_eq!(
            icon,
            egui::CursorIcon::Grabbing,
            "mid-drag shows the closed hand"
        );
    }

    /// The zoom clamps at both ends and the 0.25 steps are exact:
    /// mashing the + chip tops out at 4×, mashing the − chip bottoms
    /// out at 0.5×, and stepping away from and back to 1.0 is
    /// lossless (the worker resize policy's trigger).
    #[test]
    fn view_zoom_steps_clamp_and_recenter_exactly() {
        let mut app = phase_app(false);
        for _ in 0..30 {
            app.handle_floating_action(Some(floating::FloatingAction::ZoomIn));
        }
        assert!(
            (app.view_zoom - VIEW_ZOOM_MAX).abs() < 1e-6,
            "mashing + clamps at {}×, got {}",
            VIEW_ZOOM_MAX,
            app.view_zoom
        );
        for _ in 0..60 {
            app.handle_floating_action(Some(floating::FloatingAction::ZoomOut));
        }
        assert!(
            (app.view_zoom - VIEW_ZOOM_MIN).abs() < 1e-6,
            "mashing − clamps at {}×, got {}",
            VIEW_ZOOM_MIN,
            app.view_zoom
        );
        app.handle_floating_action(Some(floating::FloatingAction::ZoomIn));
        app.handle_floating_action(Some(floating::FloatingAction::ZoomIn));
        assert!(
            (app.view_zoom - 1.0).abs() < 1e-6,
            "+2 from the floor returns to exactly 1.0, got {}",
            app.view_zoom
        );
        app.handle_floating_action(Some(floating::FloatingAction::ZoomIn));
        app.handle_floating_action(Some(floating::FloatingAction::ZoomIn));
        app.handle_floating_action(Some(floating::FloatingAction::ZoomOut));
        app.handle_floating_action(Some(floating::FloatingAction::ZoomOut));
        assert!(
            (app.view_zoom - 1.0).abs() < 1e-6,
            "+2/−2 around 1.0 lands back at exactly 1.0, got {}",
            app.view_zoom
        );
    }

    /// The fit reserves [`CANVAS_PAD`] of air between the ring box
    /// and the panel edge on every side, and the view zoom
    /// multiplies the fit: at 2× the letterbox collapses to the
    /// ring + pad inset and the panel clips the overflow.
    #[test]
    fn update_fit_reserves_padding_and_applies_zoom() {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(
            blank_state(400, 300),
            &ctx,
            Rc::new(RefCell::new(None)),
            false,
        );
        // ppp is 1 in a synthetic context; a panel the image fits
        // into (400+2·8+2·12 = 440 wide, 300+40 = 340 tall fits in
        // 600×380 with slack).
        app.canvas_rect = Some(egui::Rect::from_min_size(
            egui::pos2(48.0, 44.0),
            egui::vec2(600.0, 380.0),
        ));
        app.update_fit(&ctx);
        assert!(
            (app.canvas.scale() - 1.0).abs() < 1e-6,
            "no upscaling at 1× zoom"
        );
        // Slack = (600−24−416)/2 = 80, (380−24−316)/2 = 20; inset =
        // ring (8) + pad (12).
        let offset = app.canvas.content_offset();
        assert!(
            ((offset.x - 100.0).abs() < 1e-3) && ((offset.y - 40.0).abs() < 1e-3),
            "the ring box centers in the padded panel at ring+pad, got {offset:?}"
        );
        app.view_zoom = 2.0;
        app.update_fit(&ctx);
        assert!(
            (app.canvas.scale() - 2.0).abs() < 1e-6,
            "the zoom multiplies the fit, got {}",
            app.canvas.scale()
        );
        let offset = app.canvas.content_offset();
        assert!(
            ((offset.x - 28.0).abs() < 1e-3) && ((offset.y - 28.0).abs() < 1e-3),
            "an overflowing box anchors at ring (16) + pad (12), got {offset:?}"
        );
        // The padding invariant: the ring box's outer edge never
        // touches the panel border.
        let image = app.image_screen_rect().expect("canvas rect cached");
        let panel = app.canvas_rect.expect("fixture");
        assert!(
            (image.min.x - panel.min.x - 28.0).abs() < 1e-3,
            "the image anchor carries the ring + pad inset"
        );
    }

    /// The breadcrumb's Home link on a worker window is the Esc
    /// contract: publish-if-unset Cancelled, ready for the close.
    #[test]
    fn navigate_home_publishes_cancelled_on_worker_windows() {
        let outcome: OutcomeSlot = Rc::new(RefCell::new(None));
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(blank_state(64, 64), &ctx, Rc::clone(&outcome), false);
        app.handle_topbar_action(TopbarAction::NavigateHome, &ctx);
        assert!(
            matches!(&*outcome.borrow(), Some(Ok(EditorOutcome::Cancelled))),
            "the Home link leaves the worker editor like Esc"
        );
    }

    /// The breadcrumb's Home link with the shell's open-home
    /// delegate installed: the hook fires exactly once, then the
    /// worker still leaves via the Esc contract (publish-if-unset
    /// Cancelled + viewport close) — the handoff precedes the exit.
    #[test]
    fn navigate_home_with_hook_fires_it_then_cancels_and_closes() {
        let outcome: OutcomeSlot = Rc::new(RefCell::new(None));
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(blank_state(64, 64), &ctx, Rc::clone(&outcome), false);
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = std::sync::Arc::clone(&fired);
        app.set_open_home(std::sync::Arc::new(move || {
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }));
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.handle_topbar_action(TopbarAction::NavigateHome, ui.ctx());
        });
        assert_eq!(
            fired.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "the Home delegate fired exactly once"
        );
        assert!(
            matches!(&*outcome.borrow(), Some(Ok(EditorOutcome::Cancelled))),
            "the worker still leaves via the Esc contract after the handoff"
        );
        assert!(
            close_command_issued(&full),
            "the worker editor closes behind the handoff"
        );
    }

    /// Page mode with the hook installed keeps the same defensive
    /// contract: the router normally intercepts `NavigateHome`
    /// before editor actions flow, but the handler arm publishes +
    /// closes either way.
    #[test]
    fn page_navigate_home_with_hook_still_publishes_and_closes() {
        let mut app = page_app(RecordingPageBridge::default());
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = std::sync::Arc::clone(&fired);
        app.set_open_home(std::sync::Arc::new(move || {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }));
        let ctx = egui::Context::default();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.handle_topbar_action(TopbarAction::NavigateHome, ui.ctx());
        });
        assert!(
            matches!(&*app.outcome.borrow(), Some(Ok(EditorOutcome::Cancelled))),
            "the defensive arm still publishes the exit signal"
        );
        assert!(
            close_command_issued(&full),
            "the defensive arm still closes"
        );
    }

    /// Whether the frame's full output asked the viewport to close.
    fn close_command_issued(full: &egui::FullOutput) -> bool {
        full.viewport_output
            .values()
            .flat_map(|v| v.commands.iter())
            .any(|c| matches!(c, egui::ViewportCommand::Close))
    }

    /// The worker Settings entry without the shell's delegate: the
    /// arm keeps the Esc contract (publish-if-unset Cancelled +
    /// close) — never a dead click, never a lingering second window.
    #[test]
    fn settings_publishes_cancelled_on_worker_windows() {
        let outcome: OutcomeSlot = Rc::new(RefCell::new(None));
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(blank_state(64, 64), &ctx, Rc::clone(&outcome), false);
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.handle_topbar_action(TopbarAction::Settings, ui.ctx());
        });
        assert!(
            matches!(&*outcome.borrow(), Some(Ok(EditorOutcome::Cancelled))),
            "the Settings entry leaves the worker editor like Esc"
        );
        assert!(
            close_command_issued(&full),
            "the worker editor closes behind the Settings handoff"
        );
    }

    /// The worker Settings entry with the shell's open-settings
    /// delegate installed (the ADR-0065 `NavigateHome` twin): the hook
    /// fires exactly once, then the worker leaves via the Esc
    /// contract — the handoff precedes the exit, and the editor never
    /// lingers beside the window it just asked for.
    #[test]
    fn settings_with_hook_fires_it_then_cancels_and_closes() {
        let outcome: OutcomeSlot = Rc::new(RefCell::new(None));
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(blank_state(64, 64), &ctx, Rc::clone(&outcome), false);
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = std::sync::Arc::clone(&fired);
        app.set_open_settings(std::sync::Arc::new(move || {
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }));
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.handle_topbar_action(TopbarAction::Settings, ui.ctx());
        });
        assert_eq!(
            fired.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "the Settings delegate fired exactly once"
        );
        assert!(
            matches!(&*outcome.borrow(), Some(Ok(EditorOutcome::Cancelled))),
            "the worker still leaves via the Esc contract after the handoff"
        );
        assert!(
            close_command_issued(&full),
            "the worker editor closes behind the Settings handoff"
        );
    }

    /// Page mode with the hook installed keeps the same defensive
    /// contract: the router normally intercepts `Settings` before
    /// editor actions flow (routing in place), but the handler arm
    /// publishes + closes either way.
    #[test]
    fn page_settings_with_hook_still_publishes_and_closes() {
        let mut app = page_app(RecordingPageBridge::default());
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = std::sync::Arc::clone(&fired);
        app.set_open_settings(std::sync::Arc::new(move || {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }));
        let ctx = egui::Context::default();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.handle_topbar_action(TopbarAction::Settings, ui.ctx());
        });
        assert!(
            fired.load(std::sync::atomic::Ordering::Relaxed),
            "the defensive arm still requests the handoff"
        );
        assert!(
            matches!(&*app.outcome.borrow(), Some(Ok(EditorOutcome::Cancelled))),
            "the defensive arm still publishes the exit signal"
        );
        assert!(
            close_command_issued(&full),
            "the defensive arm still closes"
        );
    }

    #[test]
    fn page_save_side_effect_reports_through_bridge_and_stays() {
        // Page semantics (spec): Save applies its side effect, reports
        // through the bridge, and the page stays — no slot publish
        // (the slot carries exit signals only), no close.
        let bridge = RecordingPageBridge::default();
        let rec = bridge.rec.clone();
        let app = page_app(bridge);
        app.publish_side_effect_outcome(EditorOutcome::Saved(PathBuf::from("/tmp/shot.png")));
        let guard = rec.lock().expect("recorder mutex");
        assert!(
            matches!(&guard.reported[..], [EditorOutcome::Saved(p)] if *p == std::path::Path::new("/tmp/shot.png")),
            "the bridge must receive the save outcome"
        );
        drop(guard);
        assert!(
            app.outcome.borrow().is_none(),
            "page mode must not publish side effects to the exit slot"
        );
        assert!(
            !app.closes_after_side_effect(),
            "the page never closes after a side effect"
        );
    }

    #[test]
    fn worker_side_effect_publishes_to_slot_and_closes() {
        // Worker windows keep today's lifecycle: publish + close.
        let app = phase_app(false);
        app.publish_side_effect_outcome(EditorOutcome::Saved(PathBuf::from("/tmp/shot.png")));
        assert!(
            matches!(
                &*app.outcome.borrow(),
                Some(Ok(EditorOutcome::Saved(p))) if *p == std::path::Path::new("/tmp/shot.png")
            ),
            "worker mode must keep publishing to the slot"
        );
        assert!(
            app.closes_after_side_effect(),
            "worker windows close after a side effect"
        );
    }

    #[test]
    fn page_upload_reports_like_save() {
        let bridge = RecordingPageBridge::default();
        let rec = bridge.rec.clone();
        let app = page_app(bridge);
        app.publish_side_effect_outcome(EditorOutcome::Uploaded(String::new()));
        assert!(
            matches!(
                &rec.lock().expect("recorder mutex").reported[..],
                [EditorOutcome::Uploaded(_)]
            ),
            "upload rides the same page seam as save"
        );
    }

    #[test]
    fn page_copy_never_closes_on_any_platform() {
        // The single window is long-lived: a copy keeps it open even
        // on Wayland (where the worker closes) — the clipboard handle
        // lives on in `self.clipboard`, same as the X11 worker path.
        let app = page_app(RecordingPageBridge::default());
        assert!(
            !app.close_after_copy_window(),
            "page mode suppresses the post-copy close everywhere"
        );
    }

    #[test]
    fn worker_copy_never_closes_on_linux() {
        let app = phase_app(false);
        assert!(
            !app.close_after_copy_window(),
            "worker mode lingers on Linux — the serve thread must outlive the paste"
        );
    }

    #[test]
    fn page_esc_publishes_cancelled_and_keeps_the_window() {
        // Esc is the exit-to-home intent: publish Cancelled — the
        // window router consumes it and swaps Home in-place
        // (editor-capture-ux Task 006); the page itself never closes.
        let mut app = page_app(RecordingPageBridge::default());
        let ctx = egui::Context::default();
        let full = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| app.logic_impl(ui.ctx()),
        );
        assert!(
            matches!(&*app.outcome.borrow(), Some(Ok(EditorOutcome::Cancelled))),
            "Esc must publish the route signal"
        );
        assert!(
            !close_command_issued(&full),
            "the router swaps the route — no close"
        );
    }

    #[test]
    fn page_new_capture_publishes_and_issues_close() {
        // Ctrl+N in page mode: publish NewCapture + close — the shell
        // maps it to EditorPageExit::Capture(mode).
        let mut app = page_app(RecordingPageBridge::default());
        let ctx = egui::Context::default();
        let full = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::N,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: ctrl_as_winit_reports_it(),
                }],
                modifiers: ctrl_as_winit_reports_it(),
                ..Default::default()
            },
            |ui| app.logic_impl(ui.ctx()),
        );
        assert!(
            matches!(
                &*app.outcome.borrow(),
                Some(Ok(EditorOutcome::NewCapture {
                    mode: NewCaptureMode::Region
                }))
            ),
            "Ctrl+N must publish the recapture intent with its mode label"
        );
        assert!(
            close_command_issued(&full),
            "Ctrl+N must issue the viewport close"
        );
    }

    #[test]
    fn page_hide_directive_issues_close_command() {
        // The external-capture seam: the daemon's HideForCapture
        // handshake arrives as a Hide directive; the page closes so
        // it is never in the shot.
        let bridge = RecordingPageBridge::default();
        bridge
            .pending
            .lock()
            .expect("pending mutex")
            .push_back(HomeDirective::Hide);
        let mut app = page_app(bridge.clone());
        let ctx = egui::Context::default();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| app.logic_impl(ui.ctx()));
        assert!(
            close_command_issued(&full),
            "a Hide directive must close the page window"
        );
        assert!(
            bridge.pending.lock().expect("pending mutex").is_empty(),
            "the directive was consumed"
        );
    }

    #[test]
    fn page_set_status_directive_is_consumed_quietly() {
        // The editor page shows no status line; SetStatus directives
        // (pump → window) must drain, not pile up or panic.
        let bridge = RecordingPageBridge::default();
        bridge
            .pending
            .lock()
            .expect("pending mutex")
            .push_back(HomeDirective::SetStatus("daemon went away".into()));
        let mut app = page_app(bridge.clone());
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| app.logic_impl(ui.ctx()));
        assert!(
            bridge.pending.lock().expect("pending mutex").is_empty(),
            "SetStatus drains in page mode"
        );
    }

    #[test]
    fn page_capture_accelerator_fires_the_new_capture_exit() {
        // The chrome's Capture is the `+` chip's exit: plain C
        // publishes NewCapture with its mode label and closes.
        let mut app = page_app(RecordingPageBridge::default());
        let ctx = egui::Context::default();
        let full = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::C,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| app.logic_impl(ui.ctx()),
        );
        assert!(
            matches!(
                &*app.outcome.borrow(),
                Some(Ok(EditorOutcome::NewCapture {
                    mode: NewCaptureMode::Region
                }))
            ),
            "plain C must fire the capture exit with its mode label"
        );
        assert!(close_command_issued(&full));
    }

    #[test]
    fn page_paste_key_stays_the_pencil_tool() {
        // Collision resolution, pinned: P is the Pencil tool key and
        // tool keys win over home parity — Paste is button-only in
        // the editor.
        let bridge = RecordingPageBridge::default();
        let rec = bridge.rec.clone();
        let mut app = page_app(bridge);
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::P,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| app.logic_impl(ui.ctx()),
        );
        assert_eq!(
            rec.lock().expect("recorder mutex").pastes,
            0,
            "P must not fire the paste action"
        );
        assert_eq!(app.state.tool, Tool::Pencil, "P arms the Pencil tool");
    }

    #[test]
    fn page_open_apply_routes_without_close_and_stays_on_failure() {
        // The chrome Open's contract at the apply seam: the bridge
        // arms the routed editor; the window router swaps the route
        // in-place (no close — editor-capture-ux Task 006); a failed
        // decode keeps the page exactly as it was.
        let bridge = RecordingPageBridge::default();
        let rec = bridge.rec.clone();
        let app = page_app(bridge);
        let ctx = egui::Context::default();
        let routed = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.apply_page_open(ui.ctx(), PathBuf::from("/tmp/good.png"));
        });
        assert!(
            !close_command_issued(&routed),
            "a routed open keeps the window — the router swaps"
        );
        let kept = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.apply_page_open(ui.ctx(), PathBuf::from("/tmp/bad.png"));
        });
        assert!(
            !close_command_issued(&kept),
            "a failed decode keeps the page"
        );
        let guard = rec.lock().expect("recorder mutex");
        assert_eq!(
            guard.opened,
            vec![
                (PathBuf::from("/tmp/good.png"), true),
                (PathBuf::from("/tmp/bad.png"), false),
            ],
            "each action reaches the bridge exactly once"
        );
    }

    #[test]
    fn page_paste_routes_without_close() {
        let bridge = RecordingPageBridge::default();
        let rec = bridge.rec.clone();
        let app = page_app(bridge);
        let ctx = egui::Context::default();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.handle_page_paste(ui.ctx());
        });
        assert!(
            !close_command_issued(&full),
            "a routed paste keeps the window — the router swaps"
        );
        assert_eq!(rec.lock().expect("recorder mutex").pastes, 1);
    }

    #[test]
    fn worker_windows_have_no_page_accelerators() {
        // The chrome actions are single-window page furniture: a
        // worker window's plain C is nothing (it is not a tool key
        // there either — no accidental capture from the recapture
        // editor).
        let mut app = phase_app(false);
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::C,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| app.logic_impl(ui.ctx()),
        );
        assert!(
            app.outcome.borrow().is_none(),
            "plain C in a worker window does nothing"
        );
    }

    #[test]
    fn save_during_phase_honours_live_selection() {
        let mut app = phase_app(true);
        app.state.crop_region = Some(Rect::from_xywh(4.0, 8.0, 20.0, 12.0));
        let img = compose_for_save(app.phase_active(), &app.state);
        assert_eq!(
            img.dimensions(),
            (20, 12),
            "phase-live Save crops to the live selection"
        );
    }

    #[test]
    fn save_after_phase_ends_keeps_full_image_contract() {
        let mut app = phase_app(true);
        drag_selection(&mut app.state, (4.0, 8.0), (24.0, 20.0));
        let _ = app.state.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        app.end_phase_on_crop();
        assert!(!app.phase_active(), "fixture: the crop ended the phase");
        // A fresh selection after the crop must not scope Save — the
        // post-phase editor keeps the Phase 1 full-image contract.
        app.state.crop_region = Some(Rect::from_xywh(0.0, 0.0, 10.0, 6.0));
        let img = compose_for_save(app.phase_active(), &app.state);
        assert_eq!(img.dimensions(), (20, 12), "post-crop image saves whole");
    }

    #[test]
    fn save_during_phase_without_selection_falls_back_to_full() {
        let app = phase_app(true);
        let img = compose_for_save(app.phase_active(), &app.state);
        assert_eq!(
            img.dimensions(),
            (64, 64),
            "no selection → degenerate fallback saves the full shot"
        );
    }

    #[test]
    fn live_selection_save_matches_crop_then_save_pixels() {
        // Spec exit criterion 3: Copy/Save with an active (uncropped)
        // selection produce the selected region only — the same
        // pixels as cropping first and then outputting. Pinned with
        // an annotation fully inside the region, where render-then-
        // crop == crop-then-render; straddling annotations
        // legitimately differ (geometric clip vs pixel clip) and sit
        // outside the parity claim.
        let mut state = blank_state(40, 30);
        state.tool = Tool::Rect;
        drag_selection(&mut state, (6.0, 4.0), (18.0, 14.0));
        assert_eq!(state.anns.len(), 1, "fixture: annotation committed");
        state.crop_region = Some(Rect::from_xywh(2.0, 2.0, 20.0, 16.0));
        let live = compose_for_save(true, &state);
        let mut cropped = state.clone();
        cropped.confirm_crop();
        let after = compose_final(&cropped);
        assert_eq!(live.dimensions(), after.dimensions());
        assert_eq!(
            live.into_raw(),
            after.into_raw(),
            "live-selection Save is pixel-identical to crop-then-Save"
        );
    }

    // --- Editor input gates (select-first-tool-lock Task 002) ---

    /// An unmodified key press — the shape `logic_impl`'s
    /// tool-switch scan consumes.
    fn key_press(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Drive a full pointer gesture (press → drag → release) between
    /// canvas-local physical points `from` / `to`, one editor frame
    /// per gesture phase. Requires a prior frame so `canvas_rect` is
    /// cached; ppp is 1 in a synthetic context.
    ///
    /// GOTCHA: `logic_impl` translates pointer coords with the
    /// *current* frame's `canvas_rect`, and the toolbar's height is
    /// tool-dependent (annotation tools render a style bay). A test
    /// that mutates `state.tool` must run an empty settle frame
    /// before gesturing, or the drag lands skewed by the height
    /// delta (observed: 4px between Hand and Rect).
    fn canvas_drag(ctx: &egui::Context, app: &mut EditorApp, from: (f32, f32), to: (f32, f32)) {
        // Gestures are specified in image px: map them through the
        // app's live view (the fit letterbox + zoom move the anchor
        // off the panel corner), so fixtures stay view-independent.
        let screen_at = |app: &EditorApp, (x, y): (f32, f32)| {
            let canvas = app.canvas_rect.expect("fixture: canvas rect cached");
            app.canvas
                .view_over(canvas)
                .image_to_logical(flintshot_core::Point::new(x, y))
        };
        let press = screen_at(app, from);
        editor_frame(
            ctx,
            app,
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
        let drag = screen_at(app, to);
        editor_frame(ctx, app, vec![egui::Event::PointerMoved(drag)]);
        editor_frame(
            ctx,
            app,
            vec![egui::Event::PointerButton {
                pos: drag,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            }],
        );
    }

    #[test]
    fn tool_key_gated_during_phase_arms_after_crop() {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(blank_state(64, 64), &ctx, Rc::new(RefCell::new(None)), true);
        editor_frame(&ctx, &mut app, vec![key_press(egui::Key::P)]);
        assert_eq!(
            app.state.tool,
            Tool::Crop,
            "phase live: P must not arm Pencil"
        );
        // Crop through the shell translation — the phase-ending
        // path — then the same key must work.
        drag_selection(&mut app.state, (4.0, 4.0), (60.0, 50.0));
        editor_frame(&ctx, &mut app, vec![key_press(egui::Key::Enter)]);
        assert!(!app.phase_active(), "fixture: the crop ended the phase");
        editor_frame(&ctx, &mut app, vec![key_press(egui::Key::P)]);
        assert_eq!(
            app.state.tool,
            Tool::Pencil,
            "post-crop: the same P arms Pencil"
        );
    }

    #[test]
    fn every_non_crop_tool_key_is_gated_during_phase() {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(blank_state(64, 64), &ctx, Rc::new(RefCell::new(None)), true);
        for key in [
            egui::Key::V,
            egui::Key::H,
            egui::Key::R,
            egui::Key::E,
            egui::Key::A,
            egui::Key::L,
            egui::Key::M,
            egui::Key::P,
            egui::Key::T,
            egui::Key::B,
            egui::Key::N,
        ] {
            editor_frame(&ctx, &mut app, vec![key_press(key)]);
            assert_eq!(
                app.state.tool,
                Tool::Crop,
                "{key:?} is gated while the phase is live"
            );
        }
    }

    #[test]
    fn crop_key_is_harmless_during_phase() {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(blank_state(64, 64), &ctx, Rc::new(RefCell::new(None)), true);
        editor_frame(&ctx, &mut app, vec![key_press(egui::Key::C)]);
        assert_eq!(
            app.state.tool,
            Tool::Crop,
            "C keeps arming Crop — the phase's own armed tool"
        );
        assert!(app.phase_active());
    }

    #[test]
    fn annotation_pointer_commits_nothing_during_phase() {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(blank_state(64, 64), &ctx, Rc::new(RefCell::new(None)), true);
        editor_frame(&ctx, &mut app, vec![]); // first frame caches canvas_rect
                                              // Direct mutation: with the rail lock and the hotkey gate,
                                              // no input path can leave Select while the phase is live —
                                              // the pointer gate is the backstop that keeps the phase
                                              // drag-proof regardless of how state got here. The settle
                                              // frame lets the toolbar re-layout at Rect's (taller) bay
                                              // before the gesture — see the GOTCHA on `canvas_drag`.
        app.state.tool = Tool::Rect;
        editor_frame(&ctx, &mut app, vec![]);
        canvas_drag(&ctx, &mut app, (10.0, 10.0), (40.0, 30.0));
        assert!(
            app.state.anns.is_empty(),
            "no annotation committed through the gate"
        );
        assert!(
            app.state.undo.is_empty(),
            "no undo snapshot through the gate"
        );
        // Crop drags are the phase's one job and keep flowing.
        app.state.tool = Tool::Crop;
        editor_frame(&ctx, &mut app, vec![]); // settle at Crop's rail
        canvas_drag(&ctx, &mut app, (5.0, 5.0), (30.0, 25.0));
        let sel = app
            .state
            .crop_region
            .expect("the Crop drag drew a region")
            .normalized();
        assert_eq!(sel.min, Point::new(5.0, 5.0));
        assert_eq!(sel.max, Point::new(30.0, 25.0));
    }

    #[test]
    fn post_crop_pointer_flow_matches_plain_editor() {
        // Identical gestures into a post-crop session and a plain
        // (fullscreen-style) session must commit identical
        // annotations — the gates lift with the phase.
        let phased_ctx = egui::Context::default();
        let mut phased = EditorApp::new(
            blank_state(64, 64),
            &phased_ctx,
            Rc::new(RefCell::new(None)),
            true,
        );
        editor_frame(&phased_ctx, &mut phased, vec![]);
        drag_selection(&mut phased.state, (2.0, 2.0), (60.0, 52.0));
        editor_frame(&phased_ctx, &mut phased, vec![key_press(egui::Key::Enter)]);
        assert!(!phased.phase_active(), "fixture: the crop ended the phase");
        phased.state.tool = Tool::Rect;
        // Settle frame: the toolbar re-layouts at Rect's style bay
        // before the gesture (see the GOTCHA on `canvas_drag`).
        editor_frame(&phased_ctx, &mut phased, vec![]);

        let plain_ctx = egui::Context::default();
        let mut plain = EditorApp::new(
            blank_state(64, 64),
            &plain_ctx,
            Rc::new(RefCell::new(None)),
            false,
        );
        plain.state.tool = Tool::Rect;
        editor_frame(&plain_ctx, &mut plain, vec![]);

        canvas_drag(&phased_ctx, &mut phased, (10.0, 10.0), (40.0, 30.0));
        canvas_drag(&plain_ctx, &mut plain, (10.0, 10.0), (40.0, 30.0));

        let rect_of = |app: &EditorApp| match app.state.anns.as_slice() {
            [Annotation::Rect { rect, .. }] => Some(*rect),
            _ => None,
        };
        assert_eq!(
            rect_of(&phased),
            rect_of(&plain),
            "post-crop pointer input must flow like a fullscreen session"
        );
        assert_eq!(
            rect_of(&plain),
            Some(Rect::from_xywh(10.0, 10.0, 30.0, 20.0)),
            "fixture: the gesture committed the expected rect"
        );
    }

    // --- Side-panel input gating (pop-up book Task 005) ---

    /// A press that begins on the docked panel must never start a
    /// canvas gesture: the press-and-release stays inert even when
    /// the pointer drags across the canvas, and a follow-up drag
    /// with the button never held emits nothing (the
    /// `canvas_drag_active` gate holds for the right-edge panel the
    /// same way it held for the retired top rail).
    #[test]
    fn press_beginning_on_the_panel_never_starts_a_gesture() {
        let ctx = egui::Context::default();
        let mut state = move_state_with_ann();
        state.tool = Tool::Rect;
        state.selected.clear();
        let mut app = EditorApp::new(state, &ctx, Rc::new(RefCell::new(None)), false);
        editor_frame(&ctx, &mut app, vec![]);
        let canvas = app.canvas_rect.expect("fixture: canvas rect cached");
        // A point inside the side panel: right of the canvas rect.
        let panel_point = egui::pos2(
            f32::midpoint(canvas.max.x, 1024.0).min(canvas.max.x + 100.0),
            canvas.min.y + 40.0,
        );
        assert!(
            panel_point.x > canvas.max.x,
            "fixture: the press point must sit on the panel"
        );
        let into_canvas = egui::pos2(canvas.min.x + 30.0, canvas.min.y + 30.0);
        editor_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(panel_point),
                egui::Event::PointerButton {
                    pos: panel_point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        );
        editor_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(into_canvas),
                egui::Event::PointerButton {
                    pos: into_canvas,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        );
        assert!(
            app.state.pending.is_none() && app.state.anns.len() == 1,
            "a panel press must not begin a rect annotation (anns: {:?}, pending: {:?})",
            app.state.anns.len(),
            app.state.pending.is_some()
        );
        assert!(!app.canvas_drag_active, "no drag may survive the release");
    }

    /// The direct-save path composes dir + rendered template + ext.
    /// Collision handling: the second save at the same target gets
    /// a `-2` suffix before the extension.
    /// `ui.show-statusbar = false` hides the footer: the central
    /// panel reaches the window bottom.
    #[test]
    fn statusbar_setting_hides_the_footer() {
        let ctx = egui::Context::default();
        let state = blank_state(400, 300);
        let mut app = EditorApp::new(state, &ctx, Rc::new(RefCell::new(None)), false);
        app.apply_config(&{
            let mut c = Config::default();
            c.ui.show_statusbar = false;
            c
        });
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(640.0, 480.0),
            )),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| app.ui_impl(ui));
        let canvas = app
            .canvas_rect
            .expect("ui_impl caches the central panel's rect");
        assert!(
            (canvas.bottom() - 480.0).abs() < 2.0,
            "no footer: the canvas reaches the window bottom ({canvas:?})"
        );
    }

    /// Task 008 regression: the paper-cutout dressing is chrome
    /// AROUND the capture — no opaque shape may cover an interior
    /// point of the image. The first ring implementation filled the
    /// whole expanded rect with `PAPER_CREAM`, hiding the capture
    /// entirely.
    #[test]
    fn canvas_dressing_never_covers_the_image() {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(
            blank_state(200, 150),
            &ctx,
            Rc::new(RefCell::new(None)),
            false,
        );
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(640.0, 480.0),
            )),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw.clone(), |ui| app.ui_impl(ui));
        let full = ctx.run_ui(raw, |ui| app.ui_impl(ui));

        let bg_id = app
            .bg
            .as_ref()
            .expect("the bg texture uploads on frame 1")
            .0
            .id();
        assert!(
            full.shapes
                .iter()
                .any(|cs| matches!(&cs.shape, egui::Shape::Mesh(m) if m.texture_id == bg_id)),
            "the capture's texture blit must be in the frame"
        );

        let image = app
            .image_screen_rect()
            .expect("frame 1 caches the canvas rect");
        let [w, h] = [image.width(), image.height()];
        let probes = [
            image.center(),
            egui::pos2(image.left() + w * 0.25, image.center().y),
            egui::pos2(image.right() - w * 0.25, image.center().y),
            egui::pos2(image.center().x, image.top() + h * 0.25),
            egui::pos2(image.center().x, image.bottom() - h * 0.25),
        ];
        for cs in &full.shapes {
            if let egui::Shape::Rect(r) = &cs.shape {
                if r.fill == crate::theme::palette::PAPER_CREAM {
                    for probe in probes {
                        assert!(
                            !r.rect.contains(probe),
                            "dressing covers the capture at {probe:?} (rect {:?})",
                            r.rect
                        );
                    }
                }
            }
        }

        // The paper shadow paints UNDER the blit: `Shadow::as_shape`
        // fills its whole rect with a translucent tint, so painted
        // over the capture it reads as an overlay.
        let shadow_idx = full
            .shapes
            .iter()
            .position(|cs| {
                matches!(&cs.shape, egui::Shape::Rect(r)
                    if r.fill == crate::theme::palette::SHADOW_PAPER)
            })
            .expect("the paper shadow paints");
        let blit_idx = full
            .shapes
            .iter()
            .position(|cs| matches!(&cs.shape, egui::Shape::Mesh(m) if m.texture_id == bg_id))
            .expect("the capture's blit paints");
        assert!(
            shadow_idx < blit_idx,
            "the shadow must precede the capture (shadow {shadow_idx}, blit {blit_idx})"
        );

        // The dressing never bleeds into the surrounding zones: any
        // cream fill that touches the canvas is clipped to it.
        let canvas = app.canvas_rect.expect("canvas rect cached");
        for cs in &full.shapes {
            if let egui::Shape::Rect(r) = &cs.shape {
                if r.fill == crate::theme::palette::PAPER_CREAM && r.rect.intersects(canvas) {
                    assert!(
                        canvas.expand(1.0).contains_rect(cs.clip_rect),
                        "dressing clip {cs:?} escapes the canvas {canvas:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn capture_config_seeds_the_new_capture_mode() {
        let ctx = egui::Context::default();
        let state = blank_state(400, 300);
        let outcome: OutcomeSlot = Rc::new(RefCell::new(None));
        let mut app = EditorApp::new(state, &ctx, Rc::clone(&outcome), false);
        app.apply_config(&{
            let mut c = Config::default();
            c.capture.default_mode = "fullscreen".into();
            c
        });
        app.handle_topbar_action(TopbarAction::Capture, &ctx);
        let captured = outcome.borrow().as_ref().map(|r| match r {
            Ok(EditorOutcome::NewCapture { mode }) => *mode == NewCaptureMode::Fullscreen,
            _ => false,
        });
        assert_eq!(
            captured,
            Some(true),
            "fullscreen default-mode seeds the NewCapture exit"
        );
    }

    /// The worker overflow's Settings entry fires the shell's
    /// open-settings delegate end-to-end (popup open → row click →
    /// hook). Worker windows never route to Settings in-window — the
    /// app crate owns the choreography (spec §Settings route).
    #[test]
    fn worker_overflow_settings_click_fires_the_open_settings_hook() {
        let ctx = egui::Context::default();
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = std::sync::Arc::clone(&fired);
        let mut app = EditorApp::new(
            blank_state(64, 64),
            &ctx,
            Rc::new(RefCell::new(None)),
            false,
        );
        app.set_open_settings(std::sync::Arc::new(move || {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }));
        editor_frame(&ctx, &mut app, vec![]);
        editor_frame(&ctx, &mut app, vec![]);
        let ovf = ctx
            .read_response(egui::Id::new("topbar-overflow"))
            .expect("the worker topbar's overflow registers")
            .rect;
        let click = |pos: egui::Pos2| {
            vec![
                egui::Event::PointerMoved(pos),
                primary_button(pos, true),
                primary_button(pos, false),
            ]
        };
        // Open the popup, then the settle passes: the popup anchors
        // at the screen centre on its open pass and re-anchors below
        // the chip one pass later (the + popup tests' quirk).
        editor_frame(&ctx, &mut app, click(ovf.center()));
        editor_frame(&ctx, &mut app, vec![]);
        editor_frame(&ctx, &mut app, vec![]);
        let settings = ctx
            .read_response(egui::Id::new("topbar-settings"))
            .expect("the Settings entry renders in the worker overflow")
            .rect;
        editor_frame(&ctx, &mut app, click(settings.center()));
        assert!(
            fired.load(std::sync::atomic::Ordering::Relaxed),
            "the worker Settings entry fired the open-settings delegate"
        );
    }

    #[test]
    fn press_beginning_on_the_floating_cluster_never_starts_a_gesture() {
        let ctx = egui::Context::default();
        let mut state = move_state_with_ann();
        state.tool = Tool::Rect;
        state.selected.clear();
        let mut app = EditorApp::new(state, &ctx, Rc::new(RefCell::new(None)), false);
        editor_frame(&ctx, &mut app, vec![]);
        // Settle pass: egui's Area sizing pass places the cluster on
        // frame 1 and pins the exact rect on frame 2.
        editor_frame(&ctx, &mut app, vec![]);
        let chips = app.floating_rects.clone();
        assert!(!chips.is_empty(), "frames cached the floating chip rects");
        let canvas = app.canvas_rect.expect("fixture: canvas rect cached");
        assert!(
            chips.iter().all(|r| canvas.contains_rect(*r)),
            "fixture: the cluster floats over the canvas"
        );
        // Press on the magnifier-toggle chip (the cluster's middle).
        let at = chips[2].center();
        editor_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        );
        editor_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(egui::pos2(canvas.min.x + 200.0, canvas.min.y + 100.0)),
                egui::Event::PointerButton {
                    pos: egui::pos2(canvas.min.x + 200.0, canvas.min.y + 100.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        );
        assert!(
            app.state.pending.is_none() && app.state.anns.len() == 1,
            "a cluster press must not begin a rect annotation"
        );
        assert!(!app.canvas_drag_active);
    }

    // --- Delete chip (annotation-delete Task 002) ---

    #[test]
    fn delete_chip_removes_selected_annotation_through_shell() {
        let ctx = egui::Context::default();
        let mut state = move_state_with_ann();
        state.selected = vec![0];
        let mut app = EditorApp::new(state, &ctx, Rc::new(RefCell::new(None)), false);
        // Deterministic layout: `ui_impl` re-seeds the animation gate
        // from the config each frame (the inspector's section stack
        // must be fully unfolded or the Delete chip's rect is
        // clipped mid-spring).
        app.config.ui.animations = false;
        // The panel stacks ~450 px of pods + bay above the bottom
        // cluster; a short screen would clip the Delete chip out of
        // the viewport, where no pointer can reach it. Roomy screen
        // so the whole panel is visible — the inspector's section
        // stack (Stroke + Fill + Selection) is taller than the old
        // Hand-tool lone Selection card; positions are still
        // response-derived.
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1024.0, 768.0));
        let frame = |ctx: &egui::Context, app: &mut EditorApp, events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..egui::RawInput::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                app.logic_impl(ui.ctx());
                app.ui_impl(ui);
            });
        };
        frame(&ctx, &mut app, vec![]);
        let del = ctx
            .read_response(egui::Id::new("panel-delete"))
            .expect("first frame registered the Selection section's Delete button");
        let at = del.rect.center();
        frame(
            &ctx,
            &mut app,
            vec![
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
        );
        eprintln!(
            "del rect {:?} clicked {:?} anns {}",
            ctx.read_response(egui::Id::new("panel-delete"))
                .map(|r| r.rect),
            ctx.read_response(egui::Id::new("panel-delete"))
                .map(|r| r.clicked()),
            app.state.anns.len()
        );
        assert!(
            app.state.anns.is_empty(),
            "clicking Delete with a selection must remove the annotation via the Del replay"
        );
        assert!(
            app.state.undo.last().is_some_and(|s| s.anns.len() == 1 && s.image.is_none()),
            "the delete pushed an anns-only snapshot of the pre-delete state (fixture commit aside)"
        );
    }

    // --- Editor window delete UX (annotation-delete Task 003) ---

    /// An editor app on the Hand tool with one committed, selected
    /// annotation — the delete UX's starting state.
    fn app_with_selected_ann() -> (egui::Context, EditorApp) {
        let ctx = egui::Context::default();
        let mut state = move_state_with_ann();
        state.selected = vec![0];
        let app = EditorApp::new(state, &ctx, Rc::new(RefCell::new(None)), false);
        (ctx, app)
    }

    /// Regression shape: the Del key must flow through the *shell*
    /// translation (`dispatch_shortcut_key`), not just the core
    /// dispatcher — the Enter-crop gap's lesson applied upfront.
    #[test]
    fn del_key_through_shell_deletes_selected_annotation() {
        let (ctx, mut app) = app_with_selected_ann();
        editor_frame(&ctx, &mut app, vec![key_press(egui::Key::Delete)]);
        assert!(app.state.anns.is_empty(), "Del through the shell deletes");
        assert!(app.state.selected.is_empty(), "delete drops the selection");
        assert!(
            app.state
                .undo
                .last()
                .is_some_and(|s| s.anns.len() == 1 && s.image.is_none()),
            "the delete pushed an anns-only snapshot for Ctrl+Z"
        );
    }

    /// Backspace aliases Del (Mac keyboards label it "delete"); the
    /// text-editing modal mode owns it during a draft, so this
    /// translation is collision-free outside text mode.
    #[test]
    fn backspace_key_through_shell_deletes_selected_annotation() {
        let (ctx, mut app) = app_with_selected_ann();
        editor_frame(&ctx, &mut app, vec![key_press(egui::Key::Backspace)]);
        assert!(
            app.state.anns.is_empty(),
            "Backspace through the shell deletes the selection"
        );
    }

    /// Esc layering: with a selection, the first Esc clears it and
    /// consumes the key — the outcome slot is untouched (the window
    /// stays open); a second Esc, with nothing selected, keeps
    /// today's Cancel.
    #[test]
    fn esc_with_selection_deselects_then_esc_cancels() {
        let (ctx, mut app) = app_with_selected_ann();
        let slot = app.outcome.clone();
        editor_frame(&ctx, &mut app, vec![key_press(egui::Key::Escape)]);
        assert!(
            app.state.selected.is_empty(),
            "first Esc clears the selection"
        );
        assert!(
            slot.borrow().is_none(),
            "the outcome slot is untouched — the window stays open"
        );
        editor_frame(&ctx, &mut app, vec![key_press(egui::Key::Escape)]);
        assert!(
            matches!(*slot.borrow(), Some(Ok(EditorOutcome::Cancelled))),
            "second Esc after the deselect cancels the editor"
        );
    }

    /// Selection-first phase: no annotation selection can exist
    /// mid-phase (Select is the only tool), so Esc keeps the standing
    /// Cancel even with a live region selection.
    #[test]
    fn esc_during_selection_first_phase_cancels_through_shell() {
        let ctx = egui::Context::default();
        let slot: OutcomeSlot = Rc::new(RefCell::new(None));
        let mut app = EditorApp::new(blank_state(64, 64), &ctx, slot.clone(), true);
        editor_frame(&ctx, &mut app, vec![]);
        drag_selection(&mut app.state, (5.0, 5.0), (30.0, 25.0));
        editor_frame(&ctx, &mut app, vec![key_press(egui::Key::Escape)]);
        assert!(
            matches!(*slot.borrow(), Some(Ok(EditorOutcome::Cancelled))),
            "phase Esc cancels — the deselect layer can't intercept (nothing selectable)"
        );
    }

    /// Del mid-phase is inert: nothing is selectable while the
    /// selection-first phase is live, so the delete must not fire,
    /// snapshot, or change the phase's behaviour.
    #[test]
    fn del_during_selection_first_phase_is_inert() {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(blank_state(64, 64), &ctx, Rc::new(RefCell::new(None)), true);
        editor_frame(&ctx, &mut app, vec![]);
        drag_selection(&mut app.state, (5.0, 5.0), (30.0, 25.0));
        editor_frame(&ctx, &mut app, vec![key_press(egui::Key::Delete)]);
        assert!(app.phase_active(), "Del must not end the phase");
        assert!(app.state.undo.is_empty(), "an inert Del pushes no snapshot");
        assert!(
            app.state.selected.is_empty(),
            "nothing is selectable mid-phase"
        );
    }

    /// Hotkey tool switches route through core `set_tool`: leaving
    /// Hand must drop the persistent selection (the invariant lives
    /// in the core method, not at each shell call site).
    #[test]
    fn tool_switch_away_from_move_deselects_through_shell() {
        let (ctx, mut app) = app_with_selected_ann();
        editor_frame(&ctx, &mut app, vec![key_press(egui::Key::R)]);
        assert_eq!(app.state.tool, Tool::Rect, "fixture: R armed Rect");
        assert!(
            app.state.selected.is_empty(),
            "leaving Hand via hotkey drops the selection"
        );
    }

    /// A selected annotation whose bbox is degenerate (nothing to
    /// outline) deselects at paint time instead of rendering — the
    /// selection must never point at something unoutlinable.
    #[test]
    fn degenerate_selected_bbox_deselects_at_paint_time() {
        let ctx = egui::Context::default();
        let mut state = blank_state(64, 64);
        state.anns.push(Annotation::Pencil {
            points: vec![],
            color: flintshot_core::Color::RED,
            width: 2.0,
        });
        state.selected = vec![0];
        let mut app = EditorApp::new(state, &ctx, Rc::new(RefCell::new(None)), false);
        editor_frame(&ctx, &mut app, vec![]);
        assert!(
            app.state.selected.is_empty(),
            "a degenerate bbox deselects instead of rendering"
        );
    }

    // --- Double-click synthesis (text-select-edit Task 004) ---

    /// Editor-window state on the Text tool with one committed text
    /// "hello" at (10,10) — 16 px style, coarse bbox ≈ (10,10)-(50,29).
    fn text_app() -> (egui::Context, EditorApp) {
        let ctx = egui::Context::default();
        let mut state = blank_state(64, 64);
        state.tool = Tool::Text;
        state.anns.push(Annotation::text(
            flintshot_core::Point::new(10.0, 10.0),
            "hello",
            flintshot_core::text::TextStyle::simple(16.0, flintshot_core::Color::RED),
        ));
        let app = EditorApp::new(state, &ctx, Rc::new(RefCell::new(None)), false);
        (ctx, app)
    }

    /// Image-px point → the screen-logical pos the pump would see.
    fn screen_point(app: &EditorApp, at: (f32, f32)) -> egui::Pos2 {
        let canvas = app.canvas_rect.expect("fixture: canvas rect cached");
        let logical = app
            .canvas
            .view_over(canvas)
            .image_to_logical(flintshot_core::Point::new(at.0, at.1));
        egui::pos2(logical.x, logical.y)
    }

    /// Two rapid presses at the same canvas spot synthesize
    /// `PointerDown` + `DoubleClick` in the same frame: the first
    /// press selects the text, the second re-selects and upgrades to
    /// the lift-out re-edit session.
    #[test]
    fn rapid_double_press_opens_reedit_in_editor_window() {
        let (ctx, mut app) = text_app();
        editor_frame(&ctx, &mut app, vec![]); // caches canvas_rect
        editor_frame(&ctx, &mut app, vec![]); // settle Text tool layout
        let press = screen_point(&app, (20.0, 15.0));
        editor_frame_at(
            &ctx,
            &mut app,
            1.0,
            vec![
                egui::Event::PointerMoved(press),
                primary_button(press, true),
            ],
        );
        assert_eq!(app.state.selected, vec![0], "first press selects the text");
        editor_frame_at(&ctx, &mut app, 1.05, vec![primary_button(press, false)]);
        editor_frame_at(
            &ctx,
            &mut app,
            1.1,
            vec![
                egui::Event::PointerMoved(press),
                primary_button(press, true),
            ],
        );
        assert!(
            app.state.text_reedit.is_some(),
            "rapid second press synthesized a DoubleClick and opened the session"
        );
        match &app.state.pending {
            Some(Annotation::Text { text, .. }) => assert_eq!(text, "hello"),
            other => panic!("expected lifted-out Text, got {other:?}"),
        }
    }

    /// Slow presses at the same spot stay a plain re-select — no
    /// session, no pending.
    #[test]
    fn slow_double_press_stays_selection_only() {
        let (ctx, mut app) = text_app();
        editor_frame(&ctx, &mut app, vec![]);
        editor_frame(&ctx, &mut app, vec![]);
        let press = screen_point(&app, (20.0, 15.0));
        editor_frame_at(
            &ctx,
            &mut app,
            1.0,
            vec![
                egui::Event::PointerMoved(press),
                primary_button(press, true),
            ],
        );
        editor_frame_at(&ctx, &mut app, 1.05, vec![primary_button(press, false)]);
        editor_frame_at(
            &ctx,
            &mut app,
            3.0,
            vec![
                egui::Event::PointerMoved(press),
                primary_button(press, true),
            ],
        );
        assert_eq!(app.state.selected, vec![0], "still just selected");
        assert!(
            app.state.text_reedit.is_none(),
            "no session for slow presses"
        );
        assert!(app.state.pending.is_none(), "no caret on top of the text");
    }

    /// Hovering a committed text under the Text tool announces
    /// selectability: the text cursor icon (the dashed outline paints
    /// through the shared chrome query on the same gate).
    #[test]
    fn text_hover_over_committed_text_sets_text_cursor() {
        let (ctx, mut app) = text_app();
        editor_frame(&ctx, &mut app, vec![]);
        editor_frame(&ctx, &mut app, vec![]);
        let at = screen_point(&app, (20.0, 15.0));
        let out = editor_frame_output(&ctx, &mut app, vec![egui::Event::PointerMoved(at)]);
        assert_eq!(
            out.platform_output.cursor_icon,
            egui::CursorIcon::Text,
            "hover on the text shows the text cursor"
        );
        let miss = screen_point(&app, (70.0, 70.0));
        let out = editor_frame_output(&ctx, &mut app, vec![egui::Event::PointerMoved(miss)]);
        assert_eq!(
            out.platform_output.cursor_icon,
            egui::CursorIcon::Default,
            "hover on empty canvas shows no affordance"
        );
    }

    // --- In-text gestures (text-selection-highlight Task 009) ---

    /// A 200×200 editor on the Text tool with nothing committed —
    /// drafts open mid-canvas, away from both floating clusters.
    fn draft_app() -> (egui::Context, EditorApp) {
        let ctx = egui::Context::default();
        let mut state = blank_state(200, 200);
        state.tool = Tool::Text;
        let app = EditorApp::new(state, &ctx, Rc::new(RefCell::new(None)), false);
        (ctx, app)
    }

    /// Open a fresh draft at image point `at` through the real pump
    /// (miss press + release), then type `text`.
    fn open_draft_via_pump(
        ctx: &egui::Context,
        app: &mut EditorApp,
        at: (f32, f32),
        text: &str,
        t: f64,
    ) {
        let p = screen_point(app, at);
        editor_frame_at(
            ctx,
            app,
            t,
            vec![egui::Event::PointerMoved(p), primary_button(p, true)],
        );
        editor_frame_at(ctx, app, t + 0.05, vec![primary_button(p, false)]);
        editor_frame(ctx, app, vec![egui::Event::Text(text.to_string())]);
    }

    /// Advance width (image px) of `prefix` at the session's default
    /// 16 px annotation font — computed with the exact measurer the
    /// pump resolves hits through, so test targets can't drift from
    /// what the wiring resolves.
    fn boundary_x(ctx: &egui::Context, app: &EditorApp, prefix: &str) -> f32 {
        let view = app
            .canvas
            .view_over(app.canvas_rect.expect("fixture: canvas rect cached"));
        crate::texthit::egui_measurer(ctx, view, 16.0)(prefix)
    }

    /// Press (and hold) at image point `at` at time `t`.
    fn press_at(ctx: &egui::Context, app: &mut EditorApp, at: (f32, f32), t: f64) {
        let p = screen_point(app, at);
        editor_frame_at(
            ctx,
            app,
            t,
            vec![egui::Event::PointerMoved(p), primary_button(p, true)],
        );
    }

    /// Drag the held pointer to image point `at`.
    fn drag_to(ctx: &egui::Context, app: &mut EditorApp, at: (f32, f32)) {
        let p = screen_point(app, at);
        editor_frame(ctx, app, vec![egui::Event::PointerMoved(p)]);
    }

    /// Release the held pointer at image point `at` at time `t`.
    fn release_at(ctx: &egui::Context, app: &mut EditorApp, at: (f32, f32), t: f64) {
        let p = screen_point(app, at);
        editor_frame_at(ctx, app, t, vec![primary_button(p, false)]);
    }

    fn pending_text_of(app: &EditorApp) -> String {
        match &app.state.pending {
            Some(Annotation::Text { text, .. }) => text.clone(),
            other => panic!("expected pending Text, got {other:?}"),
        }
    }

    #[test]
    fn intext_press_repositions_caret_without_commit() {
        let (ctx, mut app) = draft_app();
        editor_frame(&ctx, &mut app, vec![]); // caches canvas_rect
        editor_frame(&ctx, &mut app, vec![]); // settle layout
        open_draft_via_pump(&ctx, &mut app, (100.0, 100.0), "hello", 1.0);
        assert!(app.state.is_editing_text(), "fixture: draft open");

        // Press just right of the third boundary (between 'l' and 'l').
        let bx = 100.0 + boundary_x(&ctx, &app, "hel") + 2.0;
        press_at(&ctx, &mut app, (bx, 105.0), 2.0);
        release_at(&ctx, &mut app, (bx, 105.0), 2.05);
        assert_eq!(
            app.state.text_cursor, 3,
            "caret moved to the press boundary"
        );
        assert_eq!(app.state.text_anchor, None, "a plain press selects nothing");
        assert!(
            app.state.is_editing_text(),
            "the press never committed the draft"
        );
        assert!(app.state.anns.is_empty(), "no commit, no new draft");
        assert!(!app.drag_in_pending_text, "flag cleared on release");
    }

    #[test]
    fn intext_drag_selects_and_release_keeps_selection() {
        let (ctx, mut app) = draft_app();
        editor_frame(&ctx, &mut app, vec![]);
        editor_frame(&ctx, &mut app, vec![]);
        open_draft_via_pump(&ctx, &mut app, (100.0, 100.0), "hello", 1.0);

        press_at(&ctx, &mut app, (101.0, 105.0), 2.0);
        assert_eq!(app.state.text_cursor, 0, "press collapsed onto boundary 0");
        assert!(app.drag_in_pending_text, "drag marked as in-text");

        let x4 = 100.0 + boundary_x(&ctx, &app, "hell") + 1.0;
        drag_to(&ctx, &mut app, (x4, 105.0));
        assert_eq!(
            app.state.selection_range(),
            Some(0..4),
            "selection grew live under the drag"
        );

        let x2 = 100.0 + boundary_x(&ctx, &app, "he") + 1.0;
        drag_to(&ctx, &mut app, (x2, 105.0));
        assert_eq!(
            app.state.selection_range(),
            Some(0..2),
            "anchor stays pinned"
        );

        release_at(&ctx, &mut app, (x2, 105.0), 2.2);
        assert_eq!(
            app.state.selection_range(),
            Some(0..2),
            "release keeps the selection"
        );
        assert!(!app.drag_in_pending_text);

        editor_frame(&ctx, &mut app, vec![egui::Event::Text("X".to_string())]);
        assert_eq!(
            pending_text_of(&app),
            "Xllo",
            "typing replaces the selection"
        );
    }

    #[test]
    fn intext_drag_off_the_block_clamps_to_its_edge() {
        let (ctx, mut app) = draft_app();
        editor_frame(&ctx, &mut app, vec![]);
        editor_frame(&ctx, &mut app, vec![]);
        open_draft_via_pump(&ctx, &mut app, (100.0, 100.0), "hello", 1.0);

        press_at(&ctx, &mut app, (101.0, 105.0), 2.0);
        drag_to(&ctx, &mut app, (190.0, 105.0));
        assert_eq!(
            app.state.selection_range(),
            Some(0..5),
            "drag past the block's right edge selects to the end"
        );
        drag_to(&ctx, &mut app, (150.0, 190.0));
        assert_eq!(
            app.state.selection_range(),
            Some(0..5),
            "drag below the block stays clamped at the end"
        );
        drag_to(&ctx, &mut app, (5.0, 105.0));
        assert_eq!(
            app.state.text_cursor, 0,
            "drag far left clamps to the boundary char"
        );
        assert_eq!(
            app.state.selection_range(),
            None,
            "collapsing onto the anchor empties the selection"
        );
    }

    #[test]
    fn intext_double_and_triple_click_select_word_then_line() {
        let (ctx, mut app) = draft_app();
        editor_frame(&ctx, &mut app, vec![]);
        editor_frame(&ctx, &mut app, vec![]);
        open_draft_via_pump(&ctx, &mut app, (100.0, 100.0), "hello world", 1.0);

        // Click inside "world".
        let wx = 100.0 + boundary_x(&ctx, &app, "hello w") + 2.0;
        press_at(&ctx, &mut app, (wx, 105.0), 1.5);
        release_at(&ctx, &mut app, (wx, 105.0), 1.55);
        press_at(&ctx, &mut app, (wx, 105.0), 1.6);
        assert_eq!(
            app.state.selection_range(),
            Some(6..11),
            "double-click selects the word under the cursor"
        );
        release_at(&ctx, &mut app, (wx, 105.0), 1.65);
        press_at(&ctx, &mut app, (wx, 105.0), 1.7);
        assert_eq!(
            app.state.selection_range(),
            Some(0..11),
            "triple-click selects the (whole single) line"
        );
        release_at(&ctx, &mut app, (wx, 105.0), 1.75);
    }

    #[test]
    fn press_outside_text_while_editing_commits_and_moves_on() {
        let (ctx, mut app) = draft_app();
        editor_frame(&ctx, &mut app, vec![]);
        editor_frame(&ctx, &mut app, vec![]);
        open_draft_via_pump(&ctx, &mut app, (100.0, 100.0), "hello", 1.0);

        // A press above the block (outside it) is NOT intercepted.
        press_at(&ctx, &mut app, (100.0, 20.0), 2.0);
        release_at(&ctx, &mut app, (100.0, 20.0), 2.05);
        assert_eq!(app.state.anns.len(), 1, "the draft committed");
        match &app.state.pending {
            Some(Annotation::Text { pos, text, .. }) => {
                assert_eq!(
                    *pos,
                    flintshot_core::Point::new(100.0, 20.0),
                    "fresh caret placed"
                );
                assert!(text.is_empty());
            }
            other => panic!("expected fresh pending Text, got {other:?}"),
        }
        assert_eq!(app.state.text_cursor, 0);
        assert!(!app.drag_in_pending_text);
    }

    /// Re-edit sessions share the pending path, so the gestures work
    /// there too: drag-select inside a lifted-out text.
    #[test]
    fn drag_select_works_inside_a_reedit_session() {
        let (ctx, mut app) = text_app();
        editor_frame(&ctx, &mut app, vec![]);
        editor_frame(&ctx, &mut app, vec![]);
        let press = screen_point(&app, (20.0, 15.0));
        editor_frame_at(
            &ctx,
            &mut app,
            1.0,
            vec![
                egui::Event::PointerMoved(press),
                primary_button(press, true),
            ],
        );
        editor_frame_at(&ctx, &mut app, 1.05, vec![primary_button(press, false)]);
        editor_frame_at(
            &ctx,
            &mut app,
            1.1,
            vec![
                egui::Event::PointerMoved(press),
                primary_button(press, true),
            ],
        );
        assert!(app.state.text_reedit.is_some(), "fixture: session open");

        // Press near the start of the lifted-out "hello" and drag to
        // its end.
        press_at(&ctx, &mut app, (11.0, 15.0), 2.0);
        let end_x = 10.0 + boundary_x(&ctx, &app, "hello") - 1.0;
        drag_to(&ctx, &mut app, (end_x, 15.0));
        release_at(&ctx, &mut app, (end_x, 15.0), 2.2);
        assert_eq!(
            app.state.selection_range(),
            Some(0..5),
            "drag-select inside a re-edit session"
        );
    }
}
