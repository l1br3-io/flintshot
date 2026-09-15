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
    compose_final, compose_final_region, render::render, BlurCache, Config, DispatchOutcome,
    EditorOutcome, EditorState, FlintshotError, InputEvent, Key, Modifiers, NewCaptureMode, Point,
    Rect, Tool,
};

use crate::canvas::{EguiCanvas, EguiTexture};
use crate::home::HomeDirective;
use crate::magnifier::Magnifier;
use crate::overlay::{mask_color, mask_rects, usable_selection};
use crate::toolbar::{self, ToolbarAction};

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
    /// Whether the current primary-button press began inside the
    /// canvas. Set when `PointerDown` fires (which only happens on
    /// canvas hits), cleared on `PointerUp`. Without this guard,
    /// `primary_down` alone would emit `PointerDrag` for every frame
    /// the button is held — including presses that began on the
    /// toolbar. A toolbar press would then feed `PointerDrag`s
    /// straight into the dispatcher, corrupting in-flight state:
    /// e.g. an active `Tool::Select` selection would be extended to
    /// the toolbar's (negative canvas-y) position while the user is
    /// clicking Copy, and Copy would then read the mangled rect.
    canvas_drag_active: bool,
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
    /// Whether a copy has succeeded this session. Drives the
    /// "Copied!" button feedback so the user knows the data is on the
    /// clipboard and the editor is intentionally still open.
    copied: bool,
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
}

impl EditorApp {
    /// Construct the app. The canvas captures a clone of the
    /// [`Context`] for texture loading outside the per-frame `ui` call.
    ///
    /// `selection_first` seeds the region-capture landing phase:
    /// `Tool::Select` is pre-armed (mirroring `OverlayApp::new`, so
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
            state.tool = Tool::Select;
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
            state,
            canvas: EguiCanvas::new(ctx.clone()),
            bg: None,
            img_dims,
            canvas_rect: None,
            canvas_drag_active: false,
            outcome,
            clipboard: None,
            copied: false,
            blur_cache: BlurCache::new(),
            magnifier: Magnifier::new(),
            magnifier_always_on,
            magnifier_key,
            phase: if selection_first {
                SelectionPhase::Live
            } else {
                SelectionPhase::Off
            },
            page,
        }
    }

    /// Whether the selection-first phase is live: requested at
    /// construction and not yet ended by a crop.
    fn phase_active(&self) -> bool {
        self.phase == SelectionPhase::Live
    }

    /// The rail's tool gate for the current frame: the live
    /// selection-first phase locks every non-Select chip (selection
    /// is the only job), and the first crop restores the full set.
    fn rail_gate(&self) -> toolbar::ToolGate {
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
    /// keeps the phase ended. On the latch, a still-`Select` tool
    /// becomes `Move` — the phase force-armed `Select` at
    /// construction, so this restores the edit-phase default (the
    /// shell's own override, not the config-gated commit flip). The
    /// still-`Select` guard is defensive: the input gates (rail
    /// lock, hotkey, pointer) keep the tool `Select` for the phase's
    /// whole lifetime, so a non-Select tool at latch time can only
    /// arrive via direct state mutation — and it stays put rather
    /// than being silently flipped. The flip routes through core
    /// `set_tool` so the persistent selection (if any state arrived
    /// that way) obeys the same lifecycle as every other switch.
    fn end_phase_on_crop(&mut self) {
        if self.phase == SelectionPhase::Live && self.state.undo.iter().any(|s| s.image.is_some()) {
            self.phase = SelectionPhase::Ended;
            if self.state.tool == Tool::Select {
                self.state.set_tool(Tool::Move);
            }
        }
    }

    /// Paint the selection-first phase chrome over the shared render
    /// output: the dim mask outside the live (usable, normalized)
    /// selection — the overlay's mask math verbatim — and the hint
    /// line in a badge-style pill at the canvas's top centre. No-op
    /// outside the phase; see [`Self::phase_active`].
    fn paint_selection_phase(&self, ctx: &Context, painter: &egui::Painter) {
        let Some(panel) = self.canvas_rect else {
            return;
        };
        let ppp = ctx.pixels_per_point();
        let to_logical = |p: Point| egui::pos2(panel.min.x + p.x / ppp, panel.min.y + p.y / ppp);
        let sel_logical = usable_selection(self.state.selection)
            .map(|s| egui::Rect::from_two_pos(to_logical(s.min), to_logical(s.max)));
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
            egui::Color32::from_black_alpha(200),
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

    /// Detect image-dimension changes (crop) and respond by
    /// invalidating the bg texture cache and resizing the window.
    ///
    /// Without the resize, a cropped image renders at its true pixel
    /// size in the corner of the original (larger) window — the user
    /// sees a tiny screenshot. Resizing to `image_dims + toolbar_h`
    /// makes the canvas area exactly fit the new image.
    #[allow(
        clippy::cast_precision_loss,
        reason = "image dims ≤ 2^24 are exact in f32"
    )]
    fn handle_image_resize(&mut self, ctx: &Context) {
        let current_dims = self.state.image.dimensions();
        // Target panel size in logical px = image physical dims / ppp.
        let ppp = ctx.pixels_per_point();
        let target_w = current_dims.0 as f32 / ppp;
        let target_h = current_dims.1 as f32 / ppp;
        // Current canvas panel width/height (logical). When the resize
        // has taken effect, these match the target.
        let (cur_w, cur_h) = self
            .canvas_rect
            .map_or((0.0, 0.0), |r| (r.width(), r.height()));
        if (cur_w - target_w).abs() < 1.0 && (cur_h - target_h).abs() < 1.0 {
            return;
        }
        // Image dimensions changed OR the prior resize hasn't landed yet:
        // invalidate the bg texture cache (image may have changed) and
        // keep issuing InnerSize until the panel reports the right size.
        if current_dims != self.img_dims {
            self.bg = None;
            self.blur_cache.clear();
            self.img_dims = current_dims;
        }
        let toolbar_h = self.canvas_rect.map_or(0.0, |r| r.min.y);
        let new_size = egui::vec2(target_w, target_h + toolbar_h);
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(new_size));
        ctx.request_repaint();
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
        let ppp = ctx.pixels_per_point();
        let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) else {
            return;
        };
        // Only when the cursor is inside the canvas panel: showing the
        // loupe while the pointer is over the toolbar would cover the
        // buttons the user is interacting with.
        if !canvas_rect.contains(pos) {
            return;
        }
        // Screen-logical → canvas-physical, matching the pointer
        // translation in `logic()`.
        let local = pos - canvas_rect.min;
        let cursor_phys = Point::new(local.x * ppp, local.y * ppp);
        self.magnifier.show(
            painter,
            ctx,
            &self.state.image,
            cursor_phys,
            canvas_rect,
            ppp,
        );
    }

    /// Paint the Move-tool hover / grab outline (see
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
        let ppp = ctx.pixels_per_point();
        let pos = ctx.input(|i| i.pointer.latest_pos());
        let hover_at = pos
            .filter(|p| canvas_rect.contains(*p) && !self.canvas_drag_active)
            .map(|p| {
                // Screen-logical → canvas-physical, matching `logic_impl`.
                let local = p - canvas_rect.min;
                Point::new(local.x * ppp, local.y * ppp)
            });
        let Some((rect, kind)) = crate::chrome::move_chrome(&self.state, hover_at) else {
            return;
        };
        let to_logical =
            |p: Point| egui::pos2(canvas_rect.min.x + p.x / ppp, canvas_rect.min.y + p.y / ppp);
        crate::chrome::paint_move_chrome(
            painter,
            egui::Rect::from_two_pos(to_logical(rect.min), to_logical(rect.max)),
            kind,
        );
    }

    /// Paint the persistent-selection outline (annotation-delete
    /// spec): the selected annotation's bbox as a dashed ember rect
    /// over the shared render output, painted after the move chrome
    /// so a grab's solid ember outline visually wins over the dashed
    /// selection outline of the same annotation. A selection that
    /// yields no chrome while Move is active (degenerate bbox /
    /// stale index) deselects instead of rendering — the selection
    /// must never point at something unoutlinable.
    fn paint_selected_chrome_if_any(&mut self, ctx: &Context, painter: &egui::Painter) {
        let Some(rect) = crate::chrome::selected_chrome(&self.state) else {
            if self.state.tool == Tool::Move && self.state.selected_index.is_some() {
                self.state.selected_index = None;
            }
            return;
        };
        let Some(canvas_rect) = self.canvas_rect else {
            return;
        };
        let ppp = ctx.pixels_per_point();
        let to_logical =
            |p: Point| egui::pos2(canvas_rect.min.x + p.x / ppp, canvas_rect.min.y + p.y / ppp);
        crate::chrome::paint_selected_chrome(
            painter,
            egui::Rect::from_two_pos(to_logical(rect.min), to_logical(rect.max)),
        );
    }
}

impl EditorApp {
    /// Headless-callable core of [`App::logic`](EditorApp) — the
    /// `eframe::Frame` parameter is unused, so the split (mirroring
    /// `OverlayApp`'s) lets the shell-side event translation be
    /// unit-tested with a synthetic `egui::Context`.
    fn logic_impl(&mut self, ctx: &Context) {
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

        // Selection-first phase: the dispatcher's Enter-crop is one
        // of the two phase-ending sites (the other is `ui`'s toolbar
        // Crop button). Checked immediately after the dispatch so the
        // mask never paints on the crop's own frame.
        self.end_phase_on_crop();

        // Single-key tool selection: unmodified S R E A L M P T B N
        // switch tools (advertised in the rail tooltips). Guarded on
        // the *event's own* modifiers so Ctrl+S / Ctrl+Shift+Z chords
        // keep their dispatcher meanings; text-editing mode returned
        // early above, so typed letters can't switch tools mid-draft.
        // While the selection-first phase is live, every mapping
        // except Select is ignored — selection is the phase's only
        // job (S stays harmless: Select is what the phase armed).
        // Post-crop the keys work again. Switches route through core
        // `set_tool`: leaving Move drops the persistent selection.
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
            if !self.phase_active() || tool == Tool::Select {
                self.state.set_tool(tool);
            }
        }

        // Pointer dispatch: translate egui's screen-logical coords to
        // canvas-physical and feed the state machine. `canvas_rect` is
        // cached by the previous frame's `ui()`; on the first frame
        // it's `None` and we skip — see `EditorApp::canvas_rect`.
        let Some(canvas_rect) = self.canvas_rect else {
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

        // Screen-logical → canvas-physical: subtract the canvas origin
        // (so pointer (0,0) means canvas top-left, not screen top-left)
        // then scale by `pixels_per_point` to land in the physical-px
        // space `EditorState` operates on.
        let local = pos - canvas_rect.min;
        let at = Point::new(local.x * ppp, local.y * ppp);

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
        // release, all routed into the dispatcher. For `Tool::Select`
        // this updates `selection.max` to the toolbar's (negative
        // canvas-y) position, so by the time the toolbar's click
        // handler runs `handle_copy` / `confirm_selection`, the rect
        // no longer matches what the user dragged.
        let pointer_event = if primary_pressed && canvas_rect.contains(pos) {
            self.canvas_drag_active = true;
            Some(InputEvent::PointerDown { pos: at })
        } else if primary_released && self.canvas_drag_active {
            self.canvas_drag_active = false;
            Some(InputEvent::PointerUp)
        } else if primary_down && self.canvas_drag_active {
            Some(InputEvent::PointerDrag { pos: at })
        } else {
            None
        };
        // Selection-first phase drag gate: a live phase with a
        // non-Select tool skips pointer dispatch entirely — the
        // phase is annotation-proof by drag as well as by chip and
        // hotkey. Unreachable through input today (the rail lock and
        // the hotkey gate above keep the tool Select); this is the
        // backstop for state that got to a non-Select tool by other
        // means. `canvas_drag_active` above still tracks press /
        // release, so no stale drag state survives the gate.
        let annotation_gated = self.phase_active() && self.state.tool != Tool::Select;
        if let Some(event) = pointer_event.filter(|_| !annotation_gated) {
            // Outcome is always `Continue` for pointer events today
            // (no current tool produces a side-effecting outcome via
            // pointer). Match exhaustively so future outcomes that
            // *can* arise from pointer events (e.g. a Select-tool
            // "crop-and-save" gesture) surface here as a compile error
            // rather than silently dropping.
            let _ = self.state.dispatch_input(&event);
        }

        // Move-tool hover / grab cursor: the editor window's first
        // hover cursor (prominent-move spec). Follows the same
        // condition as the chrome paint — a hover hit or a live move
        // drag — so cursor and outline can never disagree. Empty
        // canvas under Move sets nothing (egui falls back to the
        // default arrow for the frame).
        let hover_at = (canvas_rect.contains(pos) && !self.canvas_drag_active).then_some(at);
        if crate::chrome::move_chrome(&self.state, hover_at).is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::Move);
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
        if ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && self.state.selected_index.take().is_some()
        {
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
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
    fn ui_impl(&mut self, ui: &mut egui::Ui) {
        // Toolbar. Must come before CentralPanel so egui reserves the
        // horizontal strip at the top. The rail + bay layout, chips,
        // swatches, and popups live in `crate::toolbar`; the plain
        // call (no closure) keeps the borrow of `&mut self.state`
        // disjoint from `self.copied`.
        let gate = self.rail_gate();
        let action = toolbar::show(ui, &mut self.state, self.copied, gate);

        // Selection-first phase: the toolbar's "Crop · Enter" button
        // crops through `confirm_selection`, bypassing `logic` — so
        // this is the second latch site, checked before this frame's
        // render paints the mask.
        self.end_phase_on_crop();

        // Invalidate the bg texture cache and resize the window when
        // the image dimensions change (crop replaces state.image with
        // a different-sized sub-image).
        self.handle_image_resize(ui.ctx());

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
        let panel = egui::CentralPanel::default().show(ui, |ui| {
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
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
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

        // Magnifier inset overlay (Task 005). Painted after render
        // succeeds, in screen-logical space (uses the cached
        // `canvas_rect`, not the canvas-bound painter). Suppressed
        // during text-editing so the trigger key doesn't pop the
        // loupe over the caret.
        self.paint_magnifier_if_active(ui.ctx(), ui.painter());

        // Move-tool object chrome (prominent-move spec): dashed hover
        // outline / solid ember grab outline, under the magnifier —
        // the overlay's chrome ordering.
        self.paint_move_chrome_if_any(ui.ctx(), ui.painter());

        // Persistent-selection outline (annotation-delete spec):
        // dashed ember bbox of the selected annotation, last among
        // the chrome. Chrome-only by construction — it lives in the
        // shell's paint pass, never in `render()` / composed output.
        self.paint_selected_chrome_if_any(ui.ctx(), ui.painter());

        // Handle toolbar actions outside the toolbar drawing code so
        // we can borrow `&self.outcome` cleanly. Each handler runs the
        // corresponding output pipeline (save / copy) and either
        // publishes the matching `EditorOutcome` + closes the window,
        // or logs + closes on failure. Dialog-cancel keeps the editor
        // open (no outcome published).
        match action {
            ToolbarAction::Save => self.handle_save(ui.ctx()),
            ToolbarAction::Copy => self.handle_copy(ui.ctx()),
            ToolbarAction::Upload => self.handle_upload(ui.ctx()),
            // Undo / Redo / Delete replay the same dispatch a
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
            ToolbarAction::NewCapture(mode) => self.handle_new_capture(mode, ui.ctx()),
            ToolbarAction::None => {}
        }
    }
}

impl EditorApp {
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
        !self.is_page() && close_after_copy(std::env::var_os("WAYLAND_DISPLAY").is_some())
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
        match save_to_disk(&self.state, self.phase_active()) {
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
    /// `arboard::Clipboard`, and publish `CopiedToClipboard`.
    ///
    /// On X11 the clipboard is **owner-based**: the process that sets
    /// the selection must stay alive and keep its `Clipboard` handle
    /// alive, or the data is lost the moment the process exits. Closing
    /// the editor window would terminate the process and wipe the
    /// clipboard before the user can paste — so on X11 we **do not
    /// close the window**; the handle lives in `self.clipboard`, the
    /// Copy button shows "Copied!" feedback, and the user closes the
    /// editor (Esc / X button) after pasting. The outcome is
    /// pre-published either way, so a user-initiated close still
    /// reports the copy.
    ///
    /// On Wayland (and by construction macOS/Windows, where the OS
    /// owns the clipboard) the selection survives the process:
    /// `arboard` drives the `zwlr_data_control` protocol there and
    /// the compositor takes ownership when the selection is set. The
    /// window closes immediately after the copy — Flameshot/Win11
    /// semantics — instead of lingering for an X11 rule the session
    /// never needed.
    fn handle_copy(&mut self, ctx: &Context) {
        match copy_to_clipboard(&self.state, &mut self.clipboard) {
            Ok(()) => {
                tracing::info!("editor copied to clipboard");
                self.copied = true;
                // Page mode keeps the window open on every platform —
                // the single window is long-lived, so the X11
                // owner-clipboard hold is satisfied by the window
                // itself (`self.clipboard`), not a lingering process.
                self.publish_side_effect_outcome(EditorOutcome::CopiedToClipboard);
                if self.close_after_copy_window() {
                    tracing::debug!("compositor-owned clipboard; closing after copy");
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

/// Prompt for a PNG save path via `rfd`. Blocks on the native modal
/// — the eframe event loop is already blocking, and the dialog is
/// what the user is interacting with anyway. `None` when the user
/// cancels.
fn prompt_png_path() -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .add_filter("PNG image", &["png"])
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

/// Prompt for a PNG path and write the [`compose_for_save`] image.
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
) -> flintshot_core::Result<Option<std::path::PathBuf>> {
    let Some(path) = prompt_png_path() else {
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
/// case).
///
/// Returns `Ok(None)` when the user cancels the dialog.
///
/// # Errors
/// Surfaces [`FlintshotError::Image`] if PNG encoding or the
/// filesystem write fails.
pub(crate) fn save_region_to_disk(
    state: &EditorState,
) -> flintshot_core::Result<Option<std::path::PathBuf>> {
    save_to_disk(state, true)
}

/// Compose the final image honouring an active `Tool::Select`
/// selection: a non-degenerate selection emits just the selected
/// region (`compose_final_region`); otherwise the whole composed
/// image (`compose_final`). Pure / non-mutating.
///
/// The single selection-aware compose for every region-defining
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
    match state.selection.map(Rect::normalized) {
        Some(n) if n.width().abs() >= 1.0 && n.height().abs() >= 1.0 => {
            compose_final_region(state, n)
        }
        _ => compose_final(state),
    }
}

/// Compose the final image and place it on the system clipboard.
///
/// Whether a successful Copy should close the editor window, given
/// that the session sets `WAYLAND_DISPLAY`. Pure seam over the env
/// read at the call site so the platform policy is pinned by tests:
/// the X11 owner-clipboard rule is the *only* reason to linger (the
/// process must keep serving the selection), and it does not apply
/// under Wayland's compositor-owned `zwlr_data_control`.
fn close_after_copy(wayland_display_set: bool) -> bool {
    wayland_display_set
}

/// When a non-degenerate `state.selection` is active, Copy emits just
/// the selected region (Flameshot-style): the user expects the active
/// selection to define what gets copied, separate from the Crop tool
/// which permanently narrows the editor. The editor state is **not**
/// mutated — `state.image`, `state.anns`, and `state.selection` are
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
/// live phase locks the rail to `Select`; an ended (or never
/// requested) phase renders the full set. Pure so the phase→gate
/// contract is testable without a live `Ui`.
fn rail_gate(phase_active: bool) -> toolbar::ToolGate {
    if phase_active {
        toolbar::ToolGate::SelectOnly
    } else {
        toolbar::ToolGate::All
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
        egui::Key::S => Some(Tool::Select),
        egui::Key::V => Some(Tool::Move),
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
    use flintshot_core::{Annotation, EditorState};

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
            (egui::Key::S, Tool::Select),
            (egui::Key::V, Tool::Move),
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
        // dispatcher, and unrelated letters fall through.
        for key in [
            egui::Key::C,
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
        state.selection = Some(Rect::from_xywh(2.0, 3.0, 4.0, 3.0));
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
        state.selection = Some(Rect::from_xywh(2.0, 3.0, 4.0, 3.0));
        let img = compose_selection_aware(&state);
        assert_eq!(img.dimensions(), (4, 3), "selection crops the output");
    }

    #[test]
    fn compose_selection_aware_falls_back_on_degenerate_selection() {
        let mut state = blank_state(10, 8);
        state.selection = Some(Rect::from_xywh(2.0, 3.0, 0.5, 3.0));
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
        });
        let _ = state.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(max.0, max.1),
        });
        let _ = state.dispatch_input(&InputEvent::PointerUp);
    }

    /// The rail gate follows the phase: live phase → Select-only
    /// rail; ended / never-requested phase → full rail.
    #[test]
    fn rail_gate_tracks_phase() {
        assert_eq!(rail_gate(true), toolbar::ToolGate::SelectOnly);
        assert_eq!(rail_gate(false), toolbar::ToolGate::All);
    }

    #[test]
    fn selection_first_opens_select_armed_and_phase_active() {
        let app = phase_app(true);
        assert_eq!(
            app.state.tool,
            Tool::Select,
            "the first drag must draw a selection, not a rect annotation"
        );
        assert!(app.phase_active(), "no crop yet → phase is live");
    }

    #[test]
    fn plain_editor_keeps_default_tool_and_no_phase() {
        let app = phase_app(false);
        assert_eq!(
            app.state.tool,
            Tool::Move,
            "the editor default is untouched"
        );
        assert!(!app.phase_active(), "no request → no phase");
    }

    // --- Move-tool hover / grab chrome (prominent-move) ---

    /// One eframe-shaped frame for the editor window: `logic_impl`
    /// then `ui_impl` inside a single egui pass over the given extra
    /// input events. Screen is 200×200 so the toolbar strip leaves
    /// the 64×64 canvas room to breathe.
    fn editor_frame(ctx: &egui::Context, app: &mut EditorApp, events: Vec<egui::Event>) {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(200.0, 200.0),
            )),
            events,
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            app.logic_impl(ui.ctx());
            app.ui_impl(ui);
        });
    }

    /// An editor state on the default Move tool with one committed
    /// rect annotation at (10,10)-(50,40) — something to hover.
    fn move_state_with_ann() -> EditorState {
        let mut state = blank_state(64, 64);
        state.pending = Some(Annotation::Rect {
            rect: Rect::from_xywh(10.0, 10.0, 40.0, 30.0),
            stroke: flintshot_core::Stroke::new(flintshot_core::Color::RED, 3.0),
            fill: None,
        });
        state.commit_annotation();
        assert_eq!(state.tool, Tool::Move, "fixture: tool stays Move");
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
            cursor_after_hover(&ctx, &mut app, (30.0, 25.0)),
            egui::CursorIcon::Move,
            "hovering the annotation under Move must show the grab cursor"
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
            "empty canvas under Move must leave the default cursor"
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
        let press = egui::pos2(canvas.min.x + 30.0, canvas.min.y + 25.0);
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
                canvas.min.y + 29.0,
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
    fn phase_end_crop_flips_select_to_move() {
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
            Tool::Move,
            "the edit phase opens on Move, not the phase's armed Select"
        );
    }

    #[test]
    fn phase_end_keeps_non_select_tool_against_flip() {
        let mut app = phase_app(true);
        // Direct state mutation — with the input gates live, no
        // keyboard / chip / drag path can leave Select mid-phase.
        // The latch's still-Select guard is defensive for exactly
        // this state, and must not clobber it.
        app.state.tool = Tool::Rect;
        app.state.selection = Some(Rect::from_xywh(8.0, 8.0, 32.0, 16.0));
        app.state.confirm_selection(); // the "Crop · Enter" button's call
        app.end_phase_on_crop();
        assert!(!app.phase_active(), "fixture: the crop ended the phase");
        assert_eq!(
            app.state.tool,
            Tool::Rect,
            "a non-Select tool at latch (direct mutation only) must survive the flip"
        );
    }

    #[test]
    fn toolbar_crop_button_path_ends_phase() {
        let mut app = phase_app(true);
        app.state.selection = Some(Rect::from_xywh(8.0, 8.0, 32.0, 16.0));
        app.state.confirm_selection(); // the "Crop · Enter" button's call
        app.end_phase_on_crop();
        assert!(
            !app.phase_active(),
            "the toolbar crop must end the phase too"
        );
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
        assert!(app.state.selection.is_none());
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
    fn copy_closes_on_wayland_and_lingers_on_x11() {
        // The X11 owner-clipboard rule is the only reason the editor
        // ever stayed open after a copy; a Wayland session (where the
        // env var is set) closes immediately.
        assert!(close_after_copy(true), "Wayland: close after copy");
        assert!(!close_after_copy(false), "X11: keep serving the selection");
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

    /// Whether the frame's full output asked the viewport to close.
    fn close_command_issued(full: &egui::FullOutput) -> bool {
        full.viewport_output
            .values()
            .flat_map(|v| v.commands.iter())
            .any(|c| matches!(c, egui::ViewportCommand::Close))
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
    fn worker_copy_still_closes_on_wayland() {
        let app = phase_app(false);
        assert_eq!(
            app.close_after_copy_window(),
            close_after_copy(std::env::var_os("WAYLAND_DISPLAY").is_some()),
            "worker mode keeps the platform-conditional close"
        );
    }

    #[test]
    fn page_esc_publishes_cancelled_and_issues_close() {
        // Esc is the exit-to-home intent: publish Cancelled (the
        // shell maps it to EditorPageExit::Home) + close the epoch.
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
            "Esc must publish the exit signal"
        );
        assert!(
            close_command_issued(&full),
            "Esc must issue the viewport close"
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
    fn save_during_phase_honours_live_selection() {
        let mut app = phase_app(true);
        app.state.selection = Some(Rect::from_xywh(4.0, 8.0, 20.0, 12.0));
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
        app.state.selection = Some(Rect::from_xywh(0.0, 0.0, 10.0, 6.0));
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
        state.selection = Some(Rect::from_xywh(2.0, 2.0, 20.0, 16.0));
        let live = compose_for_save(true, &state);
        let mut cropped = state.clone();
        cropped.confirm_selection();
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
    /// delta (observed: 4px between Move and Rect).
    fn canvas_drag(ctx: &egui::Context, app: &mut EditorApp, from: (f32, f32), to: (f32, f32)) {
        let canvas = app.canvas_rect.expect("fixture: canvas rect cached");
        let press = egui::pos2(canvas.min.x + from.0, canvas.min.y + from.1);
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
        let drag = egui::pos2(canvas.min.x + to.0, canvas.min.y + to.1);
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
            Tool::Select,
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
    fn every_non_select_tool_key_is_gated_during_phase() {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(blank_state(64, 64), &ctx, Rc::new(RefCell::new(None)), true);
        for key in [
            egui::Key::V,
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
                Tool::Select,
                "{key:?} is gated while the phase is live"
            );
        }
    }

    #[test]
    fn select_key_is_harmless_during_phase() {
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(blank_state(64, 64), &ctx, Rc::new(RefCell::new(None)), true);
        editor_frame(&ctx, &mut app, vec![key_press(egui::Key::S)]);
        assert_eq!(
            app.state.tool,
            Tool::Select,
            "S keeps arming Select — the phase's own armed tool"
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
        // Select drags are the phase's one job and keep flowing.
        app.state.tool = Tool::Select;
        editor_frame(&ctx, &mut app, vec![]); // settle at Select's rail
        canvas_drag(&ctx, &mut app, (5.0, 5.0), (30.0, 25.0));
        let sel = app
            .state
            .selection
            .expect("the Select drag drew a selection")
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

    // --- Delete chip (annotation-delete Task 002) ---

    #[test]
    fn delete_chip_removes_selected_annotation_through_shell() {
        let ctx = egui::Context::default();
        let mut state = move_state_with_ann();
        state.selected_index = Some(0);
        let mut app = EditorApp::new(state, &ctx, Rc::new(RefCell::new(None)), false);
        // The rail is ~470 px of chips inside a horizontal ScrollArea
        // that only exists for narrow captures — a 200 px screen (the
        // usual editor_frame size) scrolls the Delete chip out of the
        // viewport, where no pointer can reach it. Wide screen so the
        // whole rail is visible; positions are still canvas-derived.
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 200.0));
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
            .read_response(egui::Id::new("delete"))
            .expect("first frame registered the Delete chip");
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

    /// An editor app on the Move tool with one committed, selected
    /// annotation — the delete UX's starting state.
    fn app_with_selected_ann() -> (egui::Context, EditorApp) {
        let ctx = egui::Context::default();
        let mut state = move_state_with_ann();
        state.selected_index = Some(0);
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
        assert_eq!(app.state.selected_index, None, "delete drops the selection");
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
        assert_eq!(
            app.state.selected_index, None,
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
            app.state.selected_index.is_none(),
            "nothing is selectable mid-phase"
        );
    }

    /// Hotkey tool switches route through core `set_tool`: leaving
    /// Move must drop the persistent selection (the invariant lives
    /// in the core method, not at each shell call site).
    #[test]
    fn tool_switch_away_from_move_deselects_through_shell() {
        let (ctx, mut app) = app_with_selected_ann();
        editor_frame(&ctx, &mut app, vec![key_press(egui::Key::R)]);
        assert_eq!(app.state.tool, Tool::Rect, "fixture: R armed Rect");
        assert_eq!(
            app.state.selected_index, None,
            "leaving Move via hotkey drops the selection"
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
        state.selected_index = Some(0);
        let mut app = EditorApp::new(state, &ctx, Rc::new(RefCell::new(None)), false);
        editor_frame(&ctx, &mut app, vec![]);
        assert_eq!(
            app.state.selected_index, None,
            "a degenerate bbox deselects instead of rendering"
        );
    }
}
