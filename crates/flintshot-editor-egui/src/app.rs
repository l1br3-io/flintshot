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
    compose_final, compose_final_region, render::render, Color, Config, DispatchOutcome,
    EditorOutcome, EditorState, FlintshotError, InputEvent, Key, Modifiers, Point, Rect, Tool,
};

use crate::canvas::{EguiCanvas, EguiTexture};

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
}

impl EditorApp {
    /// Construct the app. The canvas captures a clone of the
    /// [`Context`] for texture loading outside the per-frame `ui` call.
    pub(crate) fn new(state: EditorState, ctx: &Context, outcome: OutcomeSlot) -> Self {
        let img_dims = state.image.dimensions();
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
        cfg.ui.default_stroke_width = self.state.stroke_width;
        cfg.ui.default_font_size = self.state.font_size;
        if let Err(e) = cfg.save() {
            tracing::warn!(
                error = %e,
                "could not save config; UI defaults not persisted this session"
            );
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
            self.img_dims = current_dims;
        }
        let toolbar_h = self.canvas_rect.map_or(0.0, |r| r.min.y);
        let new_size = egui::vec2(target_w, target_h + toolbar_h);
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(new_size));
        ctx.request_repaint();
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
                    ui.selectable_value(&mut self.state.tool, Tool::Arrow, "→ Arrow");
                    ui.selectable_value(&mut self.state.tool, Tool::Text, "T Text");
                    ui.selectable_value(&mut self.state.tool, Tool::Blur, "░ Blur");
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
                    // Font-size control shows only for Text (matches
                    // the blur-kind-gated pattern).
                    if self.state.tool == Tool::Text {
                        ui.add(
                            egui::Slider::new(&mut self.state.font_size, 8.0..=72.0).text("Font"),
                        );
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
            render(&self.state, &mut self.canvas, &mut self.bg)
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

        // Handle toolbar button clicks outside the toolbar closure so
        // we can borrow `&self.outcome` cleanly. Each handler runs the
        // corresponding output pipeline (save / copy) and either
        // publishes the matching `EditorOutcome` + closes the window,
        // or logs + closes on failure. Dialog-cancel keeps the editor
        // open (no outcome published).
        match action {
            ToolbarAction::Save => self.handle_save(ui.ctx()),
            ToolbarAction::Copy => self.handle_copy(ui.ctx()),
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
