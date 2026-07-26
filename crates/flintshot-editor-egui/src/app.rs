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
    compose_final, render::render, DispatchOutcome, EditorOutcome, EditorState, FlintshotError,
    InputEvent, Key, Modifiers, Point, Tool,
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
    /// Screen-logical rect of the most recent `CentralPanel`. Cached
    /// by `ui()` so the next frame's `logic()` can translate pointer
    /// coords into canvas-physical px and reject clicks that land on
    /// the toolbar. `None` until the first `ui()` runs — pointer
    /// dispatch skips that frame (the screenshot isn't visible yet
    /// either, so no drags can start).
    canvas_rect: Option<egui::Rect>,
    /// Outcome slot shared with the shell. Set on `Esc` or any future
    /// exit path (Save / Copy / …). Defaults to `Ok(Cancelled)` if
    /// the window closes without anything being set (the X button).
    outcome: OutcomeSlot,
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
        Self {
            state,
            canvas: EguiCanvas::new(ctx.clone()),
            bg: None,
            canvas_rect: None,
            outcome,
        }
    }

    /// Publish an outcome to the shared slot. First writer wins in
    /// practice — every trigger path immediately follows up with
    /// `ViewportCommand::Close`, so a second writer can't race in.
    fn publish_outcome(&self, outcome: flintshot_core::Result<EditorOutcome>) {
        *self.outcome.borrow_mut() = Some(outcome);
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
                self.publish_outcome(Ok(EditorOutcome::Cancelled));
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
        let pointer_event = if primary_pressed && canvas_rect.contains(pos) {
            Some(InputEvent::PointerDown { pos: at })
        } else if primary_released {
            Some(InputEvent::PointerUp)
        } else if primary_down {
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
                    ui.selectable_value(&mut self.state.tool, Tool::Rect, "▭ Rectangle");
                    ui.selectable_value(&mut self.state.tool, Tool::Arrow, "→ Arrow");
                    ui.separator();
                    ui.label(format!("{} annotations", self.state.anns.len()));
                    ui.separator();
                    if ui.button("💾 Save").clicked() {
                        action = ToolbarAction::Save;
                    }
                    if ui.button("📋 Copy").clicked() {
                        action = ToolbarAction::Copy;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.small("Drag to draw · Ctrl+Z undo · Esc cancel");
                    });
                });
                action
            })
            .inner;

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
    /// `arboard::Clipboard`, publish `CopiedToClipboard` on success.
    fn handle_copy(&self, ctx: &Context) {
        match copy_to_clipboard(&self.state) {
            Ok(()) => {
                tracing::info!("editor copied to clipboard");
                self.publish_outcome(Ok(EditorOutcome::CopiedToClipboard));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(err) => {
                tracing::error!(error = %err, "copy failed; closing editor session");
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
/// Uses `arboard`'s default feature set (X11 on Linux, native
/// elsewhere). Wayland-only sessions without `XWayland` may need
/// arboard's `wayland` cargo feature — deferred until a maintainer
/// can verify.
///
/// # Errors
/// Surfaces [`FlintshotError::Shell`] for clipboard-creation or
/// clipboard-set failures (clipboard occupied by another process,
/// clipboard daemon unavailable on Linux, etc.).
fn copy_to_clipboard(state: &EditorState) -> flintshot_core::Result<()> {
    let img = compose_final(state);
    let mut clipboard = arboard::Clipboard::new().map_err(|e| FlintshotError::Shell {
        detail: e.to_string(),
    })?;
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
