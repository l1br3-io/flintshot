//! The single window's router: **one eframe lifetime per window
//! generation**, pages as internal routes (editor-capture-ux Task
//! 005).
//!
//! [`WindowApp`] holds the live [`Route`] and delegates eframe's
//! `logic` / `ui` to the route's app. A transition swaps the route
//! in place and issues a one-time viewport command batch
//! (`InnerSize` + `Resizable` + `Title`) derived from
//! [`route_geometry`] — no window close/recreate, so the surface
//! identity (taskbar entry, pinning, compositor animations) survives
//! every non-capture transition.
//!
//! Window destruction stays exactly where it is mandatory: captures
//! (the not-in-the-shot guarantee, ADR-0047) and user quit. Those
//! flows keep closing the window through the app crate's outer loop,
//! which reopens the next *generation* through
//! [`EguiShell::run_window`](crate::EguiShell::run_window).
//!
//! Task 005 ships the skeleton with the Home route live; the Editor
//! route joins in Task 006 and Loading in Task 008.

use std::cell::RefCell;
use std::rc::Rc;

use eframe::App;
use egui::Context;

use crate::home::{HomeApp, HomeBridge};
use crate::shell::{EDITOR_PAGE_DEFAULT_SIZE, HOME_WINDOW_TITLE, WINDOW_TITLE};

/// Which page the single window shows.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Route {
    /// The three-button home page (compact, fixed geometry — OQ-4's
    /// lean: its three buttons want a small window).
    Home,
    /// The editor page over an image, with its selection-first
    /// landing flag. Stub until Task 006 wires the in-window swap.
    Editor(Box<flintshot_core::EditorState>, bool),
    /// The capture-progress page (spinner + status while the portal
    /// delivers). Stub until Task 008.
    Loading,
}

/// Per-route viewport geometry — the one-time command batch a route
/// transition issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RouteGeometry {
    /// Window inner size, logical px.
    pub(crate) size: egui::Vec2,
    /// Whether the WM may resize the window.
    pub(crate) resizable: bool,
    /// Titlebar text (and WM class hint).
    pub(crate) title: &'static str,
}

/// The geometry each route wants. Pure — the transition tests and
/// the shell's `run_window` builder share it.
///
/// The Editor and Loading routes deliberately share the stable
/// geometry (Task 004): a Loading → Editor image swap issues **no**
/// viewport churn at all — the batch is empty and the surface stays
/// byte-identical.
#[must_use]
pub(crate) fn route_geometry(route: &Route) -> RouteGeometry {
    match route {
        Route::Home => RouteGeometry {
            size: egui::vec2(360.0, 240.0),
            resizable: false,
            title: HOME_WINDOW_TITLE,
        },
        Route::Editor(..) | Route::Loading => RouteGeometry {
            size: EDITOR_PAGE_DEFAULT_SIZE,
            resizable: true,
            title: WINDOW_TITLE,
        },
    }
}

/// The route the window closed on — shared between the shell (alive
/// after `run_native` returns) and the [`WindowApp`] (dropped on
/// close). Same single-threaded rationale as the editor's
/// `OutcomeSlot`: eframe's native loop owns the whole lifetime.
pub(crate) type RouteSlot = Rc<RefCell<Route>>;

/// The single window's eframe app: hosts the routes over one
/// lifetime, delegating to the active route's app.
pub(crate) struct WindowApp {
    /// The live route; transitions swap in place.
    route: Route,
    /// The home page's app (live while `route` is [`Route::Home`]).
    home: HomeApp,
    /// Shared with the shell; the final route survives the window.
    // Read by the routing transitions that join in Task 006; the
    // slot write path lands tested ahead of them here.
    #[cfg_attr(not(test), allow(dead_code))]
    route_slot: RouteSlot,
}

impl WindowApp {
    /// Construct the window app on the Home route over the home
    /// bridge.
    pub(crate) fn new(
        bridge: Box<dyn HomeBridge>,
        initial_status: String,
        route_slot: RouteSlot,
    ) -> Self {
        Self {
            route: Route::Home,
            home: HomeApp::new(bridge, initial_status),
            route_slot,
        }
    }

    /// The route the window currently shows.
    // Task 006's editor-page adoption reads it; pinned now.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn route(&self) -> &Route {
        &self.route
    }

    /// Swap the route in place and issue the one-time transition
    /// batch (`InnerSize` + `Resizable` + `Title`). Same-*kind*
    /// swaps (e.g. an image swap inside the editor route) are
    /// no-ops — the batch must never churn a window that keeps its
    /// page kind.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn request_route(&mut self, ctx: &Context, route: Route) {
        if std::mem::discriminant(&self.route) == std::mem::discriminant(&route) {
            return;
        }
        let geometry = route_geometry(&route);
        self.route = route.clone();
        *self.route_slot.borrow_mut() = route;
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(geometry.size));
        ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(geometry.resizable));
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(geometry.title.into()));
    }
}

impl App for WindowApp {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.logic_impl(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui_impl(ui);
    }
}

impl WindowApp {
    /// Headless-callable core of [`App::logic`] — eframe's `Frame`
    /// is unconstructible in tests, mirroring the other apps' split.
    pub(crate) fn logic_impl(&mut self, ctx: &Context) {
        match &self.route {
            Route::Home => self.home.logic_impl(ctx),
            // Editor joins the router in Task 006, Loading in Task
            // 008; the router opens Home-only until then.
            Route::Editor(..) | Route::Loading => {}
        }
    }

    /// Headless-callable core of [`App::ui`].
    pub(crate) fn ui_impl(&mut self, ui: &mut egui::Ui) {
        match &self.route {
            Route::Home => self.home.ui_impl(ui),
            Route::Editor(..) | Route::Loading => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A no-op home bridge — the router tests never click through to
    /// the outside world.
    struct IdleBridge;

    impl HomeBridge for IdleBridge {
        fn request_capture(&mut self) {}

        fn open_image(&mut self, _path: std::path::PathBuf) -> bool {
            false
        }

        fn request_paste(&mut self) -> bool {
            false
        }

        fn poll_directive(&mut self) -> Option<crate::home::HomeDirective> {
            None
        }
    }

    fn window_app() -> WindowApp {
        let slot: RouteSlot = Rc::new(RefCell::new(Route::Home));
        WindowApp::new(Box::new(IdleBridge), "connected".into(), slot)
    }

    #[test]
    fn home_route_keeps_its_compact_fixed_geometry() {
        // OQ-4's lean: home stays the small non-resizable window.
        let g = route_geometry(&Route::Home);
        assert_eq!(g.size, egui::vec2(360.0, 240.0));
        assert!(!g.resizable);
        assert_eq!(g.title, "flintshot");
    }

    #[test]
    fn editor_route_uses_the_stable_viewport_geometry() {
        let g = route_geometry(&Route::Editor(Box::new(editor_state()), false));
        assert_eq!(g.size, egui::vec2(1024.0, 640.0));
        assert!(g.resizable);
        assert_eq!(g.title, "flintshot — annotate");
    }

    /// Loading == Editor geometry byte-for-byte: the progress →
    /// image swap is an in-place state change, zero viewport churn.
    #[test]
    fn loading_route_geometry_equals_editor() {
        assert_eq!(
            route_geometry(&Route::Loading),
            route_geometry(&Route::Editor(Box::new(editor_state()), true))
        );
    }

    fn editor_state() -> flintshot_core::EditorState {
        let img = image::RgbaImage::from_raw(4, 4, vec![255; 4 * 4 * 4])
            .expect("raw buffer matches 4*4*4");
        flintshot_core::EditorState::new(img)
    }

    /// The viewport command batch a full output carries, in order.
    fn transition_commands(full: &egui::FullOutput) -> Vec<egui::ViewportCommand> {
        full.viewport_output
            .values()
            .flat_map(|v| v.commands.iter())
            .cloned()
            .collect()
    }

    /// A route swap issues exactly the geometry batch — `InnerSize`
    /// + `Resizable` + `Title` — and latches the slot.
    #[test]
    fn route_swap_issues_the_transition_batch_once() {
        let ctx = egui::Context::default();
        let mut app = window_app();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.request_route(ui.ctx(), Route::Loading);
        });
        let commands = transition_commands(&full);
        assert!(commands.contains(&egui::ViewportCommand::InnerSize(egui::vec2(1024.0, 640.0))));
        assert!(commands.contains(&egui::ViewportCommand::Resizable(true)));
        assert!(
            commands.contains(&egui::ViewportCommand::Title(
                "flintshot — annotate".to_owned()
            )),
            "the batch swaps the title too: {commands:?}"
        );
        assert!(
            matches!(app.route(), Route::Loading),
            "the route latched in place"
        );
        assert_eq!(commands.len(), 3, "no other viewport churn: {commands:?}");
    }

    /// A same-kind swap (an image swap inside the editor route,
    /// once Task 008 delivers into it) issues nothing.
    #[test]
    fn same_kind_route_swap_is_a_noop_batch() {
        let ctx = egui::Context::default();
        let mut app = window_app();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.request_route(ui.ctx(), Route::Editor(Box::new(editor_state()), false));
        });
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.request_route(ui.ctx(), Route::Editor(Box::new(editor_state()), true));
        });
        assert!(
            transition_commands(&full).is_empty(),
            "an image swap inside the editor route must not disturb the window"
        );
    }

    /// A repeated identical request is idempotent (a spurious
    /// re-request must not re-issue the batch).
    #[test]
    fn repeated_route_request_is_idempotent() {
        let ctx = egui::Context::default();
        let mut app = window_app();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.request_route(ui.ctx(), Route::Loading);
        });
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.request_route(ui.ctx(), Route::Loading);
        });
        assert!(
            transition_commands(&full).is_empty(),
            "re-requesting the live route kind is a no-op"
        );
    }

    /// The router delegates to the home route: a frame with the home
    /// active runs its logic (the repaint heartbeat is observable in
    /// the output's repaint time).
    #[test]
    fn home_route_delegates_frames_to_the_home_app() {
        let ctx = egui::Context::default();
        let mut app = window_app();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
            app.ui_impl(ui);
        });
        assert!(
            matches!(app.route(), Route::Home),
            "fixture: the window still shows home"
        );
        let _ = full;
    }
}
