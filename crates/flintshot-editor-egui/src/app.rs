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
use flintshot_core::{render::render, EditorOutcome, EditorState, FlintshotError};

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
    /// Outcome slot shared with the shell. Set on `Esc` or any future
    /// exit path (Save / Copy / …). Defaults to `Ok(Cancelled)` if
    /// the window closes without anything being set (the X button).
    outcome: OutcomeSlot,
}

impl EditorApp {
    /// Construct the app. The canvas captures a clone of the
    /// [`Context`] for texture loading outside the per-frame `ui` call.
    pub(crate) fn new(state: EditorState, ctx: &Context, outcome: OutcomeSlot) -> Self {
        Self {
            state,
            canvas: EguiCanvas::new(ctx.clone()),
            bg: None,
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
        // Esc cancels. The window's X button also produces Cancelled
        // by defaulting at the call site — see `shell::run_editor`.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.publish_outcome(Ok(EditorOutcome::Cancelled));
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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
        let render_result = egui::CentralPanel::default()
            .show(ui, |ui| {
                self.canvas.bind_painter(ui.painter());
                render(&self.state, &mut self.canvas, &mut self.bg)
            })
            .inner;

        if let Err(err) = render_result {
            tracing::error!(
                error = %err,
                "background render failed; closing editor session"
            );
            self.publish_outcome(Err(err));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
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
