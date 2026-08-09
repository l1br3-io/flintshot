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
    compose_final, compose_final_region,
    render::render,
    text::{FontFamily, FontStyle, FontWeight, TextAlign, TextOutline},
    BlurCache, Color, Config, DispatchOutcome, EditorOutcome, EditorState, FlintshotError,
    InputEvent, Key, Modifiers, Point, Rect, Tool,
};

use crate::canvas::{EguiCanvas, EguiTexture};
use crate::magnifier::Magnifier;

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
}

/// Which toolbar button the user clicked this frame (if any).
///
/// Collected inside the toolbar closure so the action can be handled
/// outside it, avoiding the borrow conflict between `&mut self.state`
/// (captured by the toolbar widgets) and `&self.outcome` (needed by
/// `publish_outcome`). Same shape as the existing render-error
/// pattern: closure returns data, control flow lives outside.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum ToolbarAction {
    /// No button clicked.
    #[default]
    None,
    /// Save button clicked — open file dialog.
    Save,
    /// Copy button clicked — write to clipboard.
    Copy,
    /// Pin button clicked — compose, write temp PNG, spawn pin worker.
    Pin,
}

impl EditorApp {
    /// Construct the app. The canvas captures a clone of the
    /// [`Context`] for texture loading outside the per-frame `ui` call.
    pub(crate) fn new(state: EditorState, ctx: &Context, outcome: OutcomeSlot) -> Self {
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
        }
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

    /// Text-tool-only toolbar controls. Text gets its own colour picker
    /// decoupled from the drawing-tools `state.color` (so e.g. red
    /// arrows and white labels coexist without re-picking on every tool
    /// switch) plus the font-size slider. Phase D appends the family /
    /// weight / style / align / outline / background controls here.
    ///
    /// Associated (not `&mut self`) so the call site can borrow
    /// `&mut self.state` without conflicting with the toolbar closure's
    /// other `self.state` field captures.
    fn draw_text_controls(state: &mut EditorState, ui: &mut egui::Ui) {
        ui.separator();
        // Dedicated text colour (decoupled from the drawing-tools colour).
        let mut rgb = [state.text_color.r, state.text_color.g, state.text_color.b];
        ui.color_edit_button_srgb(&mut rgb);
        if rgb != [state.text_color.r, state.text_color.g, state.text_color.b] {
            state.text_color = Color::rgb(rgb[0], rgb[1], rgb[2]);
        }
        ui.add(egui::Slider::new(&mut state.font_size, 8.0..=72.0).text("Font"));

        // Family / weight / style / alignment (Phase D).
        ui.separator();
        ui.selectable_value(&mut state.font_family, FontFamily::SansSerif, "Sans");
        ui.selectable_value(&mut state.font_family, FontFamily::Monospace, "Mono");
        ui.selectable_value(&mut state.font_weight, FontWeight::Normal, "Regular");
        ui.selectable_value(&mut state.font_weight, FontWeight::Bold, "Bold");
        ui.selectable_value(&mut state.font_style, FontStyle::Normal, "AA");
        ui.selectable_value(&mut state.font_style, FontStyle::Italic, "It");
        ui.selectable_value(&mut state.text_align, TextAlign::Left, "⬅");
        ui.selectable_value(&mut state.text_align, TextAlign::Center, "↔");
        ui.selectable_value(&mut state.text_align, TextAlign::Right, "➡");

        // Background highlight toggle + colour.
        ui.separator();
        let mut bg_on = state.text_background.is_some();
        ui.checkbox(&mut bg_on, "BG");
        if bg_on {
            let cur = state.text_background.unwrap_or(Color::rgb(255, 235, 0));
            let mut brgb = [cur.r, cur.g, cur.b];
            ui.color_edit_button_srgb(&mut brgb);
            state.text_background = Some(Color::rgb(brgb[0], brgb[1], brgb[2]));
        } else {
            state.text_background = None;
        }

        // Outline toggle + colour + width.
        let mut ol_on = state.text_outline.is_some();
        ui.checkbox(&mut ol_on, "Outline");
        if ol_on {
            let mut o = state.text_outline.unwrap_or(TextOutline {
                color: Color::rgb(0, 0, 0),
                width: 2.0,
            });
            let mut orgb = [o.color.r, o.color.g, o.color.b];
            ui.color_edit_button_srgb(&mut orgb);
            o.color = Color::rgb(orgb[0], orgb[1], orgb[2]);
            ui.add(egui::Slider::new(&mut o.width, 1.0..=8.0).text("OL"));
            state.text_outline = Some(o);
        } else {
            state.text_outline = None;
        }
    }

    /// Drain egui text-input events while in text-editing mode.
    ///
    /// egui reports typed characters via `Event::Text(String)` (which
    /// may carry several chars for fast typing or IME composition);
    /// `Backspace` / `Enter` arrive as `Event::Key`. We translate each
    /// to the toolkit-agnostic [`InputEvent`] and feed it to the
    /// dispatcher, which mutates the in-flight draft. The dispatcher's
    /// outcomes for text events are always `Continue` (no save / copy
    /// / cancel-editor mid-edit — Esc cancels the *draft* only), so we
    /// don't need to branch on the outcome here.
    fn dispatch_text_input(&mut self, ctx: &Context, modifiers: Modifiers) {
        let events: Vec<egui::Event> = ctx.input(|i| i.raw.events.clone());
        for event in events {
            match event {
                egui::Event::Text(text) => {
                    for ch in text.chars() {
                        self.state.dispatch_input(&InputEvent::TextInput(ch));
                    }
                }
                egui::Event::Key {
                    key: egui::Key::Backspace,
                    pressed: true,
                    ..
                } => {
                    self.state.dispatch_input(&InputEvent::Key {
                        key: Key::Backspace,
                        modifiers,
                    });
                }
                egui::Event::Key {
                    key: egui::Key::Enter,
                    pressed: true,
                    ..
                } => {
                    self.state.dispatch_input(&InputEvent::Key {
                        key: Key::Enter,
                        modifiers,
                    });
                }
                egui::Event::Key {
                    key: egui::Key::Escape,
                    pressed: true,
                    ..
                } => {
                    self.state.dispatch_input(&InputEvent::Key {
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
                        self.state
                            .dispatch_input(&InputEvent::Key { key: k, modifiers });
                    }
                }
                _ => {}
            }
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
}

impl App for EditorApp {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Translate egui's input events to the toolkit-agnostic
        // `InputEvent` and feed them through `state.dispatch_input`.
        // Per the Task 10 close-out's plan, this is the pure-dispatcher
        // extraction the Task 5 / 7 / 8 close-outs flagged forward:
        // the dispatcher does all state mutation; the shell just
        // performs the side-effect the returned `DispatchOutcome`
        // names. Tested end-to-end by the dispatcher unit tests in
        // `flintshot-core::editor`; the shell's only test surface is
        // the translation itself (which is too thin to unit-test
        // meaningfully without a synthetic `egui::Context`).
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
            self.dispatch_text_input(ctx, modifiers);
            return;
        }

        // One key event per frame: egui's `key_pressed` reports the
        // most recent press. If multiple keys fire on the same frame
        // (unusual), only the last one dispatches — matches the
        // existing Task 8 single-key behaviour.
        let key_event = ctx.input(|i| {
            // Order shortcuts by Phase 1 priority: Cancel > Save / Copy
            // > Undo / Redo. egui reports each key independently so we
            // can short-circuit on the first match.
            if i.key_pressed(egui::Key::Escape) {
                Some(InputEvent::Key {
                    key: Key::Escape,
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
            } else {
                None
            }
        });
        if let Some(event) = key_event {
            // `DispatchOutcome` is `#[non_exhaustive]`; future
            // variants fall through to `matches!` returning `false`,
            // which is the same behaviour as `Continue` for the shell
            // (do nothing this frame).
            let outcome = self.state.dispatch_input(&event);
            if matches!(outcome, DispatchOutcome::Cancel) {
                // Use `if_unset` so a prior copy (which published
                // `CopiedToClipboard` and kept the window open) is
                // honoured when the user eventually closes via Esc.
                self.publish_outcome_if_unset(Ok(EditorOutcome::Cancelled));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            if matches!(outcome, DispatchOutcome::Save) {
                self.handle_save(ctx);
                return;
            }
            if matches!(outcome, DispatchOutcome::Copy) {
                self.handle_copy(ctx);
                return;
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
        if let Some(event) = pointer_event {
            // Outcome is always `Continue` for pointer events today
            // (no current tool produces a side-effecting outcome via
            // pointer). Match exhaustively so future outcomes that
            // *can* arise from pointer events (e.g. a Select-tool
            // "crop-and-save" gesture) surface here as a compile error
            // rather than silently dropping.
            let _ = self.state.dispatch_input(&event);
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "egui immediate-mode toolbar is declaratively verbose; the panel/show closure captures many disjoint `&mut self.state` fields that a split-out helper would just re-borrow wholesale"
    )]
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Toolbar. Must come before CentralPanel so egui reserves the
        // horizontal strip at the top. Phase 1 Task 8 shipped the tool
        // selector; Task 9 adds Save / Copy buttons; Task 15 adds
        // color/stroke pickers. (egui 0.35 unified
        // `TopBottomPanel::top` and `SidePanel::left/right` into
        // `Panel::{top,bottom,left,right}`.)
        //
        // The closure collects a `ToolbarAction` rather than acting
        // inline so the action handler can borrow `&self.outcome` and
        // `&self.state` cleanly outside the closure (the closure body
        // already captures `&mut self.state.tool`).
        let action = egui::Panel::top("editor_toolbar")
            .show(ui, |ui| {
                let mut action = ToolbarAction::None;
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.state.tool, Tool::Select, "⬚ Select");
                    ui.selectable_value(&mut self.state.tool, Tool::Rect, "▭ Rectangle");
                    ui.selectable_value(&mut self.state.tool, Tool::Ellipse, "◯ Ellipse");
                    ui.selectable_value(&mut self.state.tool, Tool::Arrow, "→ Arrow");
                    ui.selectable_value(&mut self.state.tool, Tool::Line, "／ Line");
                    ui.selectable_value(&mut self.state.tool, Tool::Marker, "🖍 Marker");
                    ui.selectable_value(&mut self.state.tool, Tool::Pencil, "✏ Pencil");
                    ui.selectable_value(&mut self.state.tool, Tool::Text, "T Text");
                    ui.selectable_value(&mut self.state.tool, Tool::Blur, "░ Blur");
                    ui.selectable_value(&mut self.state.tool, Tool::Number, "# Number");
                    if self.state.tool == Tool::Blur {
                        ui.separator();
                        ui.label("Kind:");
                        ui.selectable_value(
                            &mut self.state.blur_kind,
                            flintshot_core::BlurKind::Gaussian,
                            "Gaussian",
                        );
                        ui.selectable_value(
                            &mut self.state.blur_kind,
                            flintshot_core::BlurKind::Pixelate,
                            "Pixelate",
                        );
                    }
                    // Color + stroke-width pickers (Task 15). Apply to
                    // all drawing tools (Rect / Arrow / Marker / Pencil
                    // / Text). Color uses egui's sRGB editor; the three
                    // u8 channels map directly to flintshot_core::Color.
                    ui.separator();
                    let mut rgb = [self.state.color.r, self.state.color.g, self.state.color.b];
                    ui.color_edit_button_srgb(&mut rgb);
                    if rgb != [self.state.color.r, self.state.color.g, self.state.color.b] {
                        self.state.color = Color::rgb(rgb[0], rgb[1], rgb[2]);
                    }
                    ui.add(
                        egui::Slider::new(&mut self.state.stroke_width, 1.0..=20.0).text("Width"),
                    );
                    // Text-tool-only controls live in a helper so the
                    // toolbar stays under clippy's line budget as text
                    // customisation grows (Phase D adds family / weight
                    // / style / align / outline / background).
                    if self.state.tool == Tool::Text {
                        Self::draw_text_controls(&mut self.state, ui);
                    }
                    // Select-tool: offer crop when a non-degenerate
                    // selection is active. Enter also confirms (the
                    // dispatcher handles that); the button is for
                    // pointer-driven users.
                    if self.state.tool == Tool::Select {
                        if let Some(sel) = self.state.selection {
                            let n = sel.normalized();
                            if n.width().abs() >= 1.0 && n.height().abs() >= 1.0 {
                                ui.separator();
                                if ui.button("✂ Crop to selection (↵)").clicked() {
                                    self.state.confirm_selection();
                                }
                            }
                        }
                    }
                    ui.separator();
                    ui.label(format!("{} annotations", self.state.anns.len()));
                    ui.separator();
                    if ui.button("💾 Save").clicked() {
                        action = ToolbarAction::Save;
                    }
                    let copy_label = if self.copied {
                        "✓ Copied!"
                    } else {
                        "📋 Copy"
                    };
                    if ui.button(copy_label).clicked() {
                        action = ToolbarAction::Copy;
                    }
                    if ui.button("📌 Pin").clicked() {
                        action = ToolbarAction::Pin;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.small("Drag to draw · Ctrl+Z undo · Esc cancel");
                    });
                });
                action
            })
            .inner;

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

        // Magnifier inset overlay (Task 005). Painted after render
        // succeeds, in screen-logical space (uses the cached
        // `canvas_rect`, not the canvas-bound painter). Suppressed
        // during text-editing so the trigger key doesn't pop the
        // loupe over the caret.
        self.paint_magnifier_if_active(ui.ctx(), ui.painter());

        // Handle toolbar button clicks outside the toolbar closure so
        // we can borrow `&self.outcome` cleanly. Each handler runs the
        // corresponding output pipeline (save / copy) and either
        // publishes the matching `EditorOutcome` + closes the window,
        // or logs + closes on failure. Dialog-cancel keeps the editor
        // open (no outcome published).
        match action {
            ToolbarAction::Save => self.handle_save(ui.ctx()),
            ToolbarAction::Copy => self.handle_copy(ui.ctx()),
            ToolbarAction::Pin => self.handle_pin(ui.ctx()),
            ToolbarAction::None => {}
        }
    }
}

impl EditorApp {
    /// Save button handler: compose the final image, prompt for a
    /// path via `rfd`, write PNG, publish `Saved(path)` on success.
    fn handle_save(&self, ctx: &Context) {
        match save_to_disk(&self.state) {
            Ok(Some(path)) => {
                tracing::info!(path = %path.display(), "editor saved");
                self.publish_outcome(Ok(EditorOutcome::Saved(path)));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
    /// clipboard before the user can paste.
    ///
    /// Therefore, on success we **do not close the window**. The
    /// `arboard::Clipboard` is stored in `self.clipboard` to keep the
    /// selection alive, and the Copy button shows "Copied!" feedback.
    /// The user pastes into the target application and then closes the
    /// editor (Esc / X button). The outcome was pre-published as
    /// `CopiedToClipboard`, so the shell returns the correct result
    /// even though the close was user-initiated.
    fn handle_copy(&mut self, ctx: &Context) {
        match copy_to_clipboard(&self.state, &mut self.clipboard) {
            Ok(()) => {
                tracing::info!("editor copied to clipboard");
                self.copied = true;
                self.publish_outcome(Ok(EditorOutcome::CopiedToClipboard));
            }
            Err(err) => {
                tracing::error!(error = %err, "copy failed");
                self.publish_outcome(Err(err));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Pin button handler: compose the final image (selection-aware,
    /// like Copy), write it to a temp PNG, spawn `flintshot pin <tmp>`
    /// as a detached worker (resolving the binary via `current_exe`,
    /// mirroring `worker::CommandSpawner`), then publish `Pinned` and
    /// close the editor.
    ///
    /// ## Error policy
    ///
    /// A compose or temp-write failure is terminal: the render/data
    /// path is broken, so we publish `Err` and close (mirroring
    /// `handle_save` / `handle_copy` and the render-error path).
    ///
    /// A **spawn** failure is recoverable. The editor is still fully
    /// usable — Save / Copy still work, and the spawn might succeed on
    /// retry. Rather than poisoning the outcome slot and closing, we
    /// log at `error` and leave the window open so the user can fall
    /// back. The outcome slot stays `None`-ish, so a later Save / Copy
    /// / Cancel publishes cleanly.
    fn handle_pin(&self, ctx: &Context) {
        match compose_and_write_pin(&self.state) {
            Ok(path) => match spawn_pin_worker(&path) {
                Ok(()) => {
                    tracing::info!(path = %path.display(), "pinned image; spawned pin worker");
                    self.publish_outcome(Ok(EditorOutcome::Pinned));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Err(err) => {
                    tracing::error!(
                        error = %err,
                        "could not spawn pin worker; editor stays open"
                    );
                }
            },
            Err(err) => {
                tracing::error!(
                    error = %err,
                    "pin compose/write failed; closing editor session"
                );
                self.publish_outcome(Err(err));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

/// Compose the final image and write it to a user-chosen PNG path.
///
/// Blocks on `rfd::FileDialog::save_file()` — the eframe event loop
/// is already blocking, and the native modal is what the user is
/// interacting with anyway, so there's no UI hitch. Returns
/// `Ok(None)` when the user cancels the dialog (no file written, no
/// error); callers should keep the editor open in that case.
///
/// # Errors
/// Surfaces [`FlintshotError::Image`] if PNG encoding or the
/// filesystem write fails, or [`FlintshotError::Shell`] if `arboard`
/// clipboard creation fails (it shouldn't — arboard isn't used here —
/// but the variant is kept for symmetry with `copy_to_clipboard`).
fn save_to_disk(state: &EditorState) -> flintshot_core::Result<Option<std::path::PathBuf>> {
    let path = rfd::FileDialog::new()
        .add_filter("PNG image", &["png"])
        .set_file_name("flintshot.png")
        .save_file();
    let Some(path) = path else {
        return Ok(None);
    };
    let img = compose_final(state);
    img.save_with_format(&path, image::ImageFormat::Png)
        .map_err(FlintshotError::from)?;
    Ok(Some(path))
}

/// Compose the final image and place it on the system clipboard.
///
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
fn copy_to_clipboard(
    state: &EditorState,
    slot: &mut Option<arboard::Clipboard>,
) -> flintshot_core::Result<()> {
    // Selection-aware output: crop the composed image to the active
    // selection rect when one is present and non-degenerate. The
    // 1.0-px threshold matches the render path's selection-rect guard
    // (`render::render`) and `EditorState::crop`'s degenerate no-op,
    // so "what the user sees outlined" is exactly "what gets copied".
    let img = match state.selection.map(Rect::normalized) {
        Some(n) if n.width().abs() >= 1.0 && n.height().abs() >= 1.0 => {
            compose_final_region(state, n)
        }
        _ => compose_final(state),
    };
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

/// Compose the image to pin, honouring an active `Tool::Select`
/// selection exactly like [`copy_to_clipboard`]: a non-degenerate
/// selection pins just the selected region; otherwise the whole
/// composed image is pinned. Pure / non-mutating.
fn compose_pin_image(state: &EditorState) -> image::RgbaImage {
    // Same 1.0-px threshold and `Rect::normalized` call as the Copy
    // path, so "what the user sees outlined" is "what gets pinned".
    match state.selection.map(Rect::normalized) {
        Some(n) if n.width().abs() >= 1.0 && n.height().abs() >= 1.0 => {
            compose_final_region(state, n)
        }
        _ => compose_final(state),
    }
}

/// Compose the pin image and write it to a temp PNG. Returns the path
/// the spawned `flintshot pin` worker should read. The file is **left
/// on disk** — the pin worker opens it once at startup; we never reap
/// it ourselves (the OS temp reaper handles stale files).
///
/// Factored out of [`EditorApp::handle_pin`](EditorApp::handle_pin) so
/// the compose + write path is headless-testable without a display or
/// the binary.
///
/// # Errors
/// Surfaces [`FlintshotError::Image`] if PNG encoding or the
/// filesystem write fails.
fn compose_and_write_pin(state: &EditorState) -> flintshot_core::Result<std::path::PathBuf> {
    let img = compose_pin_image(state);
    write_pin_tempfile(&img)
}

/// Write `img` to a PNG under the OS temp dir with a recognizable,
/// unique name `flintshot-pin-<pid>-<n>.png` and leave it on disk for
/// the pin worker to read. `<pid>` separates concurrent editor
/// sessions; a process-local counter (`<n>`) disambiguates repeated
/// pin clicks within one session. The file is never reaped by us —
/// the OS temp reaper handles stale files. (Write-then-spawn ordering
/// guarantees the worker always reads a complete file.)
///
/// # Errors
/// Surfaces [`FlintshotError::Image`] on PNG encode failure.
fn write_pin_tempfile(img: &image::RgbaImage) -> flintshot_core::Result<std::path::PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    // Process-local counter: the editor normally closes after one pin,
    // but a brief retry window or a spawn failure leaves it open, so a
    // second click must not overwrite the first pin's file.
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("flintshot-pin-{pid}-{n}.png"));
    img.save_with_format(&path, image::ImageFormat::Png)
        .map_err(FlintshotError::from)?;
    Ok(path)
}

/// Spawn `flintshot pin <path>` as a detached child of the current
/// binary (resolved via [`std::env::current_exe`], mirroring
/// `flintshot_app::worker::CommandSpawner`). stdin / stdout are
/// null'd so worker output can't interleave with the editor's;
/// stderr is inherited so tracing logs survive for debugging. A
/// lightweight reaper thread waits on the child so it doesn't linger
/// as a zombie.
///
/// # Errors
/// Surfaces [`FlintshotError::Shell`] if the binary can't be resolved,
/// the child can't be spawned, or the reaper thread can't start.
fn spawn_pin_worker(path: &std::path::Path) -> flintshot_core::Result<()> {
    let exe = std::env::current_exe().map_err(|e| FlintshotError::Shell {
        detail: format!("resolving current_exe for pin: {e}"),
    })?;
    let mut child = std::process::Command::new(&exe)
        .arg("pin")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .map_err(|e| FlintshotError::Shell {
            detail: format!(
                "spawning pin worker {} pin {}: {e}",
                exe.display(),
                path.display()
            ),
        })?;
    let _reaper = std::thread::Builder::new()
        .name("flintshot-pin-reaper".into())
        .spawn(move || {
            let _ = child.wait();
        })
        .map_err(|e| FlintshotError::Shell {
            detail: format!("spawning pin reaper thread: {e}"),
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
fn translate_modifiers(egui_mods: egui::Modifiers) -> Modifiers {
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

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::EditorState;

    /// Construct an all-white `EditorState` of the given pixel size.
    fn blank_state(w: u32, h: u32) -> EditorState {
        let img = image::RgbaImage::from_raw(w, h, vec![255; (w as usize) * (h as usize) * 4])
            .expect("raw buffer matches w*h*4");
        EditorState::new(img)
    }

    /// The compose + write path that `handle_pin` runs must yield a
    /// file whose bytes decode as a valid PNG at the source dimensions.
    /// This is the headless half of Task 5's verification: the spawn
    /// itself needs the binary + a display and is `#[ignore]`'d.
    #[test]
    fn compose_and_write_pin_writes_decodable_png() {
        let state = blank_state(8, 6);
        let path = compose_and_write_pin(&state).expect("compose + write succeeds");
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

    /// A selection-active pin must emit just the selected region
    /// (parity with the Copy selection fix at `copy_to_clipboard`).
    #[test]
    fn compose_and_write_pin_honours_selection() {
        let mut state = blank_state(10, 10);
        state.selection = Some(Rect::from_xywh(2.0, 3.0, 4.0, 3.0));
        let path = compose_and_write_pin(&state).expect("compose + write succeeds");
        let bytes = std::fs::read(&path).expect("temp file is readable");
        let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
            .expect("written bytes decode as PNG");
        assert_eq!(
            decoded.to_rgba8().dimensions(),
            (4, 3),
            "selection crops the pinned image"
        );
        let _ = std::fs::remove_file(&path);
    }
}
