//! The single window's router: **one eframe lifetime per window
//! generation**, pages as internal routes (editor-capture-ux Tasks
//! 005–006).
//!
//! [`WindowApp`] holds the live [`Route`] and delegates eframe's
//! `logic` / `ui` to the route's app. A transition swaps the route
//! in place and issues a one-time viewport command batch
//! (`InnerSize` + `Resizable` + `Title`) derived from
//! [`route_geometry`] — no window close/recreate, so the surface
//! identity (taskbar entry, pinning, compositor animations) survives
//! every non-capture transition.
//!
//! ## What still closes the window
//!
//! Window destruction stays exactly where it is mandatory: **every
//! capture** (the not-in-the-shot guarantee, ADR-0047 — surface
//! destruction is the only portable hide on Wayland) and user quit.
//! The route apps keep sending those closes themselves; everything
//! else routes:
//!
//! - Open / Paste (home or editor) → the bridge arms a routed
//!   editor; the router polls [`RouteHost::take_routed_editor`] and
//!   swaps to (or replaces) the Editor route in place.
//! - Esc in the editor publishes `Cancelled`; the router consumes it
//!   per frame and swaps Home — no close.
//! - A render failure in the editor page publishes `Err`; the router
//!   surfaces it through the page bridge's status and swaps Home —
//!   no close.
//! - A capture intent publishes `NewCapture` **and** closes; the
//!   router deliberately leaves that outcome in the slot so
//!   [`EguiShell::run_window`](crate::EguiShell::run_window) returns
//!   it — the app crate's outer loop runs the grab flow after the
//!   window dies.
//!
//! Task 008 adds the Loading route (progress while the portal
//! delivers) and in-window grab delivery.

use std::cell::RefCell;
use std::rc::Rc;

use eframe::App;
use egui::Context;

use crate::app::{EditorApp, OutcomeSlot};
use crate::home::{HomeApp, HomeBridge};
use crate::shell::{PageBridgeSlot, EDITOR_PAGE_DEFAULT_SIZE, HOME_WINDOW_TITLE, WINDOW_TITLE};
use flintshot_core::{EditorOutcome, EditorState};

/// The loading route's status line (spec copy).
const LOADING_STATUS: &str = "retrieving capture…";

/// The loading route's repaint heartbeat: the spinner animates and
/// the delivery polls (routed editor / route home) must run without
/// input. Short enough that the post-delivery swap lands well inside
/// the spec's perceivable window.
const LOADING_REPAINT: std::time::Duration = std::time::Duration::from_millis(50);

/// Which page the single window shows.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Route {
    /// The capture home page (topbar + hero + hcards + recents) —
    /// shares the stable page geometry with the other routes.
    Home,
    /// The editor page over a routed image, with its selection-first
    /// landing flag.
    Editor(Box<RoutedEditor>),
    /// The capture-progress page (spinner + status while the portal
    /// delivers). Stub until Task 008.
    Loading,
    /// The settings page (Task 010): reached from every route's
    /// `⋯` overflow; Escape returns to Home.
    Settings,
}

/// An editor route waiting to run: the editor state plus its
/// selection-first landing phase.
#[derive(Debug, Clone)]
pub struct RoutedEditor {
    /// Source screenshot + annotation model.
    pub state: EditorState,
    /// Whether the editor route opens selection-first.
    pub selection_first: bool,
}

/// What a window generation opens on (the app crate's outer loop
/// decides from its routing state).
#[derive(Debug, Clone)]
pub enum WindowStart {
    /// The three-button home.
    Home,
    /// The editor over a routed image (a delivered grab reopening a
    /// closed window). Boxed: `RoutedEditor` carries the image
    /// buffer inline.
    Editor(Box<RoutedEditor>),
    /// The capture-progress route while a portal delivery is pending
    /// (the grab confirmed; the clipboard write landing).
    Loading,
}

/// How one window generation ended — the post-close input the app
/// crate's outer loop classifies.
#[derive(Debug)]
pub struct WindowExit {
    /// The route showing at close — the same-page restore input (an
    /// external capture's close reopens on the route that was open).
    pub route: Route,
    /// The editor outcome left unpublished at close: `NewCapture`
    /// (a capture intent — the window is closing for the grab) is
    /// the expected value. `Cancelled` never survives (Esc routes
    /// home in-window); side effects never publish in page mode.
    pub editor_outcome: Option<flintshot_core::Result<EditorOutcome>>,
}

/// The router's seam to the app crate's shared routing state: the
/// routed-editor events the in-window router consumes **without** a
/// close, plus the editor-opened report. Implemented over the same
/// state the [`HomeBridge`] / [`EditorPageBridge`](crate::shell::EditorPageBridge)
/// impls hold; deliberately a separate trait so those contracts stay
/// unchanged (editor-capture-ux Task 006).
pub trait RouteHost {
    /// Take a routed editor image, if one waits (an Open/Paste
    /// control's decode arming the swap, or a grab's delivery —
    /// Task 008).
    fn take_routed_editor(&mut self) -> Option<RoutedEditor>;

    /// The editor route went live (the daemon report parity: one
    /// `EditorOpened` per editor session, wherever the editor lives).
    fn editor_route_opened(&mut self);

    /// Take a pending "route home" intent, if one waits (a failed or
    /// quiet grab while the window was open on the Loading route —
    /// nothing is coming to show, and the spinner must not strand).
    fn take_route_home(&mut self) -> bool;
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
/// Every route deliberately shares the stable geometry: a route
/// swap (Loading → Editor's image swap included) issues **no**
/// viewport churn beyond the title — the batch carries identical
/// sizes, so the surface stays byte-identical.
#[must_use]
pub(crate) fn route_geometry(route: &Route) -> RouteGeometry {
    match route {
        Route::Home => RouteGeometry {
            size: EDITOR_PAGE_DEFAULT_SIZE,
            resizable: true,
            title: HOME_WINDOW_TITLE,
        },
        Route::Editor(..) | Route::Loading | Route::Settings => RouteGeometry {
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
    /// The home page's app (constructed once — its status line and
    /// bridges survive editor round-trips).
    home: HomeApp,
    /// The live editor app, when the Editor route is active. A new
    /// image replaces it wholesale (Replace semantics).
    editor: Option<EditorApp>,
    /// The settings page, when the Settings route is active. Built
    /// on route entry from the bridge's config snapshot.
    settings: Option<crate::settings::SettingsApp>,
    /// The page bridge shared with every editor construction.
    page_bridge: PageBridgeSlot,
    /// The app crate's routing state (routed-editor events).
    host: Box<dyn RouteHost>,
    /// The editor outcome slot, shared with every editor
    /// construction and with the shell (the exit's `editor_outcome`).
    editor_outcome: OutcomeSlot,
    /// Shared with the shell; the final route survives the window.
    route_slot: RouteSlot,
}

impl WindowApp {
    /// Construct the window app over the home + page bridges and the
    /// route host, starting on `start`.
    // Construction-only plumbing: the bridges, host, and shared
    // slots arrive as flat args once, at the shell's builder.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        ctx: &Context,
        start: WindowStart,
        home_bridge: Box<dyn HomeBridge>,
        page_bridge: PageBridgeSlot,
        host: Box<dyn RouteHost>,
        initial_status: String,
        editor_outcome: OutcomeSlot,
        route_slot: RouteSlot,
    ) -> Self {
        // One-time theme application for the routed window: the
        // home route has no editor app to do it, so the router owns
        // the window's skin from its first frame (the editor
        // constructor's apply is idempotent for editor-started
        // windows).
        crate::theme::apply(ctx);
        let (route, editor) = match start {
            WindowStart::Home => (Route::Home, None),
            WindowStart::Loading => (Route::Loading, None),
            WindowStart::Editor(routed) => {
                let editor = Self::build_editor(&routed, ctx, &page_bridge, &editor_outcome);
                (Route::Editor(routed), Some(editor))
            }
        };
        *route_slot.borrow_mut() = route.clone();
        Self {
            route,
            home: HomeApp::new(home_bridge, initial_status),
            settings: None,
            editor,
            page_bridge,
            host,
            editor_outcome,
            route_slot,
        }
    }

    /// Construct the editor app for a routed editor over this
    /// window's shared slots.
    fn build_editor(
        routed: &RoutedEditor,
        ctx: &Context,
        page_bridge: &PageBridgeSlot,
        outcome: &OutcomeSlot,
    ) -> EditorApp {
        EditorApp::new_page(
            routed.state.clone(),
            ctx,
            Rc::clone(outcome),
            routed.selection_first,
            Rc::clone(page_bridge),
        )
    }

    /// Swap the route in place: construct / drop the route's app and
    /// issue the one-time geometry batch when the route *kind*
    /// changes. Same-kind swaps (an image swap inside the editor
    /// route) replace the editor app without touching the window.
    pub(crate) fn request_route(&mut self, ctx: &Context, route: Route) {
        let kind_changed = std::mem::discriminant(&self.route) != std::mem::discriminant(&route);
        match &route {
            Route::Editor(routed) => {
                if kind_changed {
                    self.host.editor_route_opened();
                }
                self.editor = Some(Self::build_editor(
                    routed,
                    ctx,
                    &self.page_bridge,
                    &self.editor_outcome,
                ));
                // A fresh editor session starts with a clean outcome
                // slot — a stale Cancelled must not route the new
                // page home on its first frame.
                *self.editor_outcome.borrow_mut() = None;
            }
            Route::Home | Route::Loading => {
                self.editor = None;
                self.settings = None;
            }
            Route::Settings => {
                self.editor = None;
                self.settings = Some(crate::settings::SettingsApp::new(
                    self.home.load_config(),
                    self.home.display_server(),
                    self.home.version(),
                ));
            }
        }
        self.route = route.clone();
        *self.route_slot.borrow_mut() = route;
        if kind_changed {
            let geometry = route_geometry(&self.route);
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(geometry.size));
            ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(geometry.resizable));
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(geometry.title.into()));
        }
    }

    /// The routed-editor poll: an Open/Paste decode (or a grab
    /// delivery — Task 008) arms the shared state; the router swaps
    /// without a close.
    fn poll_routed_editor(&mut self, ctx: &Context) {
        if let Some(routed) = self.host.take_routed_editor() {
            self.request_route(ctx, Route::Editor(Box::new(routed)));
        }
    }

    /// The Loading route's directive poll — no route app is live
    /// there, so the router owns the queue: `Hide` still closes (the
    /// not-in-the-shot guarantee holds mid-delivery) and `Show` is
    /// the focus ping; the status line stays home's.
    fn poll_loading_directives(&mut self, ctx: &Context) {
        while let Some(directive) = self.home.poll_directive() {
            match directive {
                crate::home::HomeDirective::Hide => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                crate::home::HomeDirective::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                crate::home::HomeDirective::SetStatus(_) => {}
            }
        }
    }

    /// The route-home poll: a failed or quiet grab while the window
    /// was open on the Loading route arms the intent — the spinner
    /// must not strand. Never fires on Home (a no-swap there).
    fn poll_route_home(&mut self, ctx: &Context) {
        if self.host.take_route_home() && !matches!(self.route, Route::Home) {
            self.request_route(ctx, Route::Home);
        }
    }

    /// The editor outcome poll: outcomes that route in-place are
    /// consumed here; a capture intent is left in the slot for the
    /// window's exit (its close is already landing).
    fn poll_editor_outcome(&mut self, ctx: &Context) {
        let outcome = self.editor_outcome.borrow_mut().take();
        match outcome {
            None => {}
            Some(Ok(EditorOutcome::Cancelled)) => {
                // Esc from the editor: home, same window.
                self.request_route(ctx, Route::Home);
            }
            Some(Err(e)) => {
                // Render failure: surface why through the status
                // line and route home — the window survives.
                self.page_bridge
                    .borrow_mut()
                    .set_status(&format!("editor closed on error: {e}"));
                self.request_route(ctx, Route::Home);
            }
            // A capture intent publishes-and-closes; the outcome
            // rides the window's exit for the outer loop.
            Some(other) => *self.editor_outcome.borrow_mut() = Some(other),
        }
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
            // Editor: the editor app's own logic handles its input;
            // its directives (Hide → close) stay its own business.
            Route::Editor(..) | Route::Settings => {
                // Editor: the editor app's own logic handles its
                // input; its directives stay its own business.
                // Settings: Escape + the debounce drain run in
                // ui_impl, which owns the page borrow.
            }
            Route::Loading => {
                // The spinner animates and the delivery polls run on
                // the heartbeat, not input.
                ctx.request_repaint_after(LOADING_REPAINT);
                self.poll_loading_directives(ctx);
            }
        }
        if let Some(editor) = self.editor.as_mut() {
            editor.logic_impl(ctx);
        }
        // Routing polls run after the active route's logic: an
        // Open/Paste click arms the routed editor here, the editor's
        // outcome (Esc / error / capture) settles now, and a failed
        // or quiet grab routes home from under the spinner.
        self.poll_routed_editor(ctx);
        self.poll_editor_outcome(ctx);
        self.poll_route_home(ctx);
    }

    /// Headless-callable core of [`App::ui`].
    pub(crate) fn ui_impl(&mut self, ui: &mut egui::Ui) {
        // The topbar spans every route (home, editor, loading) —
        // one identity + action line across the window's whole life.
        let surface = match &self.route {
            Route::Home => crate::topbar::Surface::Home,
            Route::Editor(..) => crate::topbar::Surface::Editor,
            Route::Loading => crate::topbar::Surface::Loading,
            Route::Settings => crate::topbar::Surface::Settings,
        };
        let status = if matches!(self.route, Route::Home) {
            Some(self.home.status_line())
        } else {
            None
        };
        let enable = crate::topbar::enable_for(surface);
        // The Copy hero's "Copied" state queries the live editor
        // (routed pages); other surfaces stay unarmed.
        let copied = self
            .editor
            .as_ref()
            .is_some_and(|e| e.copied_visible(ui.input(|i| i.time)));
        let topbar_action = crate::topbar::show(ui, surface, status.as_deref(), enable, copied);
        // Route the fired action: editor pages forward everything to
        // the editor's dispatcher; Home dispatches Capture / Open to
        // the home app's own paths; Loading is fully disabled and
        // Settings stays inert until its route lands.
        match topbar_action {
            Some(crate::topbar::TopbarAction::Capture) if matches!(self.route, Route::Home) => {
                self.home.start_capture(ui.ctx());
            }
            Some(crate::topbar::TopbarAction::Open) if matches!(self.route, Route::Home) => {
                self.home.handle_open(ui.ctx());
            }
            // The ⋯ overflow's Settings entry opens the settings
            // route from every route (spec §Settings route).
            Some(crate::topbar::TopbarAction::Settings) => {
                self.settings = Some(crate::settings::SettingsApp::new(
                    self.home.load_config(),
                    self.home.display_server(),
                    self.home.version(),
                ));
                self.request_route(ui.ctx(), Route::Settings);
            }
            // The breadcrumb's Home link: back to the home route
            // from any stage that can navigate (mirrors the editor
            // Esc path — the page swaps in place, same window).
            Some(crate::topbar::TopbarAction::NavigateHome)
                if !matches!(self.route, Route::Home) =>
            {
                self.request_route(ui.ctx(), Route::Home);
            }
            Some(a) if matches!(self.route, Route::Editor(..)) => {
                if let Some(editor) = self.editor.as_mut() {
                    editor.handle_topbar_action(a, ui.ctx());
                }
            }
            _ => {}
        }
        match &mut self.route {
            Route::Home => self.home.ui_impl(ui),
            Route::Editor(..) => {}
            Route::Loading => draw_loading(ui),
            Route::Settings => {
                if let Some(page) = self.settings.as_mut() {
                    let now = ui.ctx().input(|i| i.time);
                    let escape = crate::settings::SettingsPage::show(ui, page, now);
                    // Debounce drain: persist once the window has
                    // passed. GOTCHA: the config watcher reloads the
                    // app's config on every write — the page's
                    // SAVE_DEBOUNCE coalesces rapid edits, and the
                    // final write always lands.
                    if let Some(config) = page.drain_save(now) {
                        // Live-apply to the routed editor session,
                        // if one is open under this route.
                        if let Some(editor) = self.editor.as_mut() {
                            editor.apply_config(config);
                        }
                        self.home.save_config(config);
                    }
                    if escape {
                        self.request_route(ui.ctx(), Route::Home);
                    }
                }
            }
        }
        if let Some(editor) = self.editor.as_mut() {
            editor.ui_impl(ui);
        }
    }
}

/// The Loading route's body: a centered spinner over the status
/// line — the editor's stable geometry, so the image swap into place
/// moves nothing.
fn draw_loading(ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.4);
            ui.add(egui::Spinner::new());
            ui.label(
                egui::RichText::new(LOADING_STATUS)
                    .color(crate::theme::palette::INK_SOFT)
                    .small(),
            );
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::HomeDirective;
    use crate::shell::EditorPageBridge;

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

        fn poll_directive(&mut self) -> Option<HomeDirective> {
            None
        }
    }

    /// A home bridge counting Capture requests — the topbar's
    /// Capture dispatch contract on the Home route.
    #[derive(Clone, Default)]
    struct CountingBridge {
        captures: Rc<std::cell::Cell<usize>>,
    }

    impl HomeBridge for CountingBridge {
        fn request_capture(&mut self) {
            self.captures.set(self.captures.get() + 1);
        }
        fn open_image(&mut self, _path: std::path::PathBuf) -> bool {
            false
        }
        fn request_paste(&mut self) -> bool {
            false
        }
        fn poll_directive(&mut self) -> Option<HomeDirective> {
            None
        }
    }

    /// A home bridge replaying pre-loaded directives (the pump's
    /// queue stand-in) — the Loading route's directive contract.
    #[derive(Clone, Default)]
    struct DirectiveBridge {
        pending: Rc<RefCell<std::collections::VecDeque<HomeDirective>>>,
    }

    impl HomeBridge for DirectiveBridge {
        fn request_capture(&mut self) {}
        fn open_image(&mut self, _path: std::path::PathBuf) -> bool {
            false
        }
        fn request_paste(&mut self) -> bool {
            false
        }
        fn poll_directive(&mut self) -> Option<HomeDirective> {
            self.pending.borrow_mut().pop_front()
        }
    }

    fn loading_app_with_directives(directives: Vec<HomeDirective>) -> WindowApp {
        let bridge = DirectiveBridge::default();
        *bridge.pending.borrow_mut() = directives.into();
        let slot: RouteSlot = Rc::new(RefCell::new(Route::Home));
        let outcome: OutcomeSlot = Rc::new(RefCell::new(None));
        WindowApp::new(
            &egui::Context::default(),
            WindowStart::Loading,
            Box::new(bridge),
            Rc::new(RefCell::new(Box::new(StubPageBridge::default()))),
            Box::new(RecordingHost::default()),
            "connected".into(),
            outcome,
            slot,
        )
    }

    /// The topbar's Capture hero dispatches to the home bridge on
    /// the Home route — one click, exactly one capture request.
    #[test]
    fn home_topbar_capture_fires_the_bridge_request() {
        let bridge = CountingBridge::default();
        let slot: RouteSlot = Rc::new(RefCell::new(Route::Home));
        let outcome: OutcomeSlot = Rc::new(RefCell::new(None));
        let mut app = WindowApp::new(
            &egui::Context::default(),
            WindowStart::Home,
            Box::new(bridge.clone()),
            Rc::new(RefCell::new(Box::new(StubPageBridge::default()))),
            Box::new(RecordingHost::default()),
            "connected".into(),
            outcome,
            slot,
        );
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1024.0, 640.0));
        let frame = |ctx: &egui::Context, app: &mut WindowApp, events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..egui::RawInput::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                // logic_impl only polls route state; the click
                // contract is pure UI.
                app.ui_impl(ui);
            });
        };
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        let capture = ctx
            .read_response(egui::Id::new("topbar-capture"))
            .expect("the topbar Capture hero registers on Home")
            .rect;
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(capture.center())],
        );
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(capture.center()),
                egui::Event::PointerButton {
                    pos: capture.center(),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: capture.center(),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert_eq!(
            bridge.captures.get(),
            1,
            "one topbar Capture click = one bridge capture request"
        );
    }

    /// The breadcrumb's Home link routes the editor page home
    /// in-place — the same swap Esc performs, asserted through the
    /// route slot (the geometry batch follows the kind change).
    #[test]
    fn navigate_home_from_the_editor_routes_home() {
        let (mut app, _bridge, _host) = window_app_start(
            WindowStart::Editor(Box::new(RoutedEditor {
                state: editor_state(),
                selection_first: false,
            })),
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1024.0, 640.0));
        let frame = |ctx: &egui::Context, app: &mut WindowApp, events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..egui::RawInput::default()
            };
            let _ = ctx.run_ui(raw, |ui| app.ui_impl(ui));
        };
        // Settle: the topbar lays out, then the crumb link
        // hit-tests a pass behind it.
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        let crumb = ctx
            .read_response(egui::Id::new(("topbar-crumb", 0)))
            .expect("the Home crumb registers as a link on the editor route")
            .rect;
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(crumb.center())],
        );
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(crumb.center()),
                egui::Event::PointerButton {
                    pos: crumb.center(),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: crumb.center(),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(
            matches!(app.route, Route::Home),
            "the Home crumb routes the editor page home"
        );
        assert!(app.editor.is_none(), "the editor page is dropped");
    }

    /// The external-capture seam holds mid-delivery: a Hide directive
    /// on the Loading route still closes the window (not-in-the-shot).
    #[test]
    fn loading_hide_directive_closes_the_window() {
        let mut app = loading_app_with_directives(vec![HomeDirective::Hide]);
        let ctx = egui::Context::default();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        assert!(
            transition_commands(&full)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close)),
            "Hide closes even while the capture delivers"
        );
    }

    /// A Show on the Loading route is the focus ping — no close.
    #[test]
    fn loading_show_directive_focuses_without_closing() {
        let mut app = loading_app_with_directives(vec![HomeDirective::Show]);
        let ctx = egui::Context::default();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        assert!(
            !transition_commands(&full)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close)),
            "Show focuses; it never closes"
        );
    }

    /// A recording page bridge: statuses are the observable half of
    /// the error-route contract.
    #[derive(Clone, Default)]
    struct StubPageBridge {
        statuses: Rc<RefCell<Vec<String>>>,
    }

    impl EditorPageBridge for StubPageBridge {
        fn report_outcome(&mut self, _outcome: &EditorOutcome) {}

        fn set_status(&mut self, status: &str) {
            self.statuses.borrow_mut().push(status.into());
        }

        fn poll_directive(&mut self) -> Option<HomeDirective> {
            None
        }

        fn open_image(&mut self, _path: std::path::PathBuf) -> bool {
            false
        }

        fn request_paste(&mut self) -> bool {
            false
        }
    }

    /// A recording route host: pre-loaded routed editors replay one
    /// per poll (the arming the app crate's Shared does). Shared
    /// interior so the test keeps a handle past the move into the
    /// app.
    #[derive(Clone, Default)]
    struct RecordingHost {
        inner: Rc<RefCell<RecordingInner>>,
    }

    #[derive(Default)]
    struct RecordingInner {
        queued: std::collections::VecDeque<RoutedEditor>,
        opened: usize,
        route_home: Option<()>,
    }

    impl RouteHost for RecordingHost {
        fn take_routed_editor(&mut self) -> Option<RoutedEditor> {
            self.inner.borrow_mut().queued.pop_front()
        }

        fn editor_route_opened(&mut self) {
            self.inner.borrow_mut().opened += 1;
        }

        fn take_route_home(&mut self) -> bool {
            self.inner.borrow_mut().route_home.take().is_some()
        }
    }

    fn editor_state() -> EditorState {
        let img = image::RgbaImage::from_raw(4, 4, vec![255; 4 * 4 * 4])
            .expect("raw buffer matches 4*4*4");
        EditorState::new(img)
    }

    fn window_app() -> WindowApp {
        window_app_start(
            WindowStart::Home,
            StubPageBridge::default(),
            RecordingHost::default(),
        )
        .0
    }

    /// The app plus the handles its recordings are readable through.
    fn window_app_start(
        start: WindowStart,
        bridge: StubPageBridge,
        host: RecordingHost,
    ) -> (WindowApp, StubPageBridge, RecordingHost) {
        let slot: RouteSlot = Rc::new(RefCell::new(Route::Home));
        let outcome: OutcomeSlot = Rc::new(RefCell::new(None));
        let app = WindowApp::new(
            &egui::Context::default(),
            start,
            Box::new(IdleBridge),
            Rc::new(RefCell::new(Box::new(bridge.clone()))),
            Box::new(host.clone()),
            "connected".into(),
            outcome,
            slot,
        );
        (app, bridge, host)
    }

    #[test]
    fn home_route_uses_the_stable_geometry() {
        // The redesigned home (topbar + hero + hcards) cannot fit the
        // pre-redesign compact window — Home shares the stable page
        // geometry with Editor / Loading / Settings.
        let g = route_geometry(&Route::Home);
        assert_eq!(g.size, egui::vec2(1024.0, 640.0));
        assert!(g.resizable);
        assert_eq!(g.title, "flintshot");
    }

    #[test]
    fn editor_route_uses_the_stable_viewport_geometry() {
        let g = route_geometry(&Route::Editor(Box::new(RoutedEditor {
            state: editor_state(),
            selection_first: false,
        })));
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
            route_geometry(&Route::Editor(Box::new(RoutedEditor {
                state: editor_state(),
                selection_first: true,
            })))
        );
    }

    /// The viewport command batch a full output carries.
    fn transition_commands(full: &egui::FullOutput) -> Vec<egui::ViewportCommand> {
        full.viewport_output
            .values()
            .flat_map(|v| v.commands.iter())
            .cloned()
            .collect()
    }

    /// A Home → Editor swap issues exactly the geometry batch —
    /// `InnerSize` + `Resizable` + `Title` — and latches the slot.
    #[test]
    fn route_swap_issues_the_transition_batch_once() {
        let ctx = egui::Context::default();
        let mut app = window_app();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.request_route(
                ui.ctx(),
                Route::Editor(Box::new(RoutedEditor {
                    state: editor_state(),
                    selection_first: false,
                })),
            );
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
        assert_eq!(commands.len(), 3, "no other viewport churn: {commands:?}");
        assert!(
            matches!(app.route, Route::Editor(..)),
            "the route latched in place"
        );
    }

    /// A same-kind swap (a second Open/Paste replacing the editor's
    /// image) issues nothing — the editor app is replaced wholesale
    /// behind the same surface.
    #[test]
    fn image_swap_replaces_the_editor_without_viewport_churn() {
        let ctx = egui::Context::default();
        let (mut app, _bridge, _host) = window_app_start(
            WindowStart::Editor(Box::new(RoutedEditor {
                state: editor_state(),
                selection_first: false,
            })),
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.request_route(
                ui.ctx(),
                Route::Editor(Box::new(RoutedEditor {
                    state: editor_state(),
                    selection_first: true,
                })),
            );
        });
        assert!(
            transition_commands(&full).is_empty(),
            "an image swap inside the editor route must not disturb the window"
        );
        assert!(app.editor.is_some(), "a fresh editor app took over");
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

    /// The router polls the host: an armed routed editor swaps the
    /// route in-place (the Open control's decode path), and the
    /// editor-opened report fires once.
    #[test]
    fn routed_editor_poll_swaps_to_the_editor_route() {
        let ctx = egui::Context::default();
        let host = RecordingHost::default();
        let (mut app, _bridge, host) =
            window_app_start(WindowStart::Home, StubPageBridge::default(), host);
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            // The host queue is empty this frame — no swap.
            app.logic_impl(ui.ctx());
        });
        assert!(
            transition_commands(&full).is_empty(),
            "fixture: nothing routed yet"
        );
        host.inner.borrow_mut().queued.push_back(RoutedEditor {
            state: editor_state(),
            selection_first: false,
        });
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        assert!(
            matches!(app.route, Route::Editor(..)),
            "the armed routed editor swapped the route in-place"
        );
        assert!(
            transition_commands(&full).contains(&egui::ViewportCommand::Title(
                "flintshot — annotate".to_owned()
            )),
            "the transition batch issued"
        );
    }

    /// Esc's `Cancelled` routes home in-place — consumed by the
    /// router, never left for the exit.
    #[test]
    fn cancelled_outcome_routes_home_in_place() {
        let ctx = egui::Context::default();
        let (mut app, _bridge, _host) = window_app_start(
            WindowStart::Editor(Box::new(RoutedEditor {
                state: editor_state(),
                selection_first: false,
            })),
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        *app.editor_outcome.borrow_mut() = Some(Ok(EditorOutcome::Cancelled));
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        assert!(
            matches!(app.route, Route::Home),
            "Esc swaps home without closing"
        );
        assert!(
            !transition_commands(&full)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close)),
            "no close — the window survives the Esc route"
        );
        assert!(app.editor_outcome.borrow().is_none(), "consumed");
    }

    /// A render failure surfaces through the page bridge's status
    /// and routes home — the window survives.
    #[test]
    fn error_outcome_reports_status_and_routes_home() {
        let ctx = egui::Context::default();
        let bridge = StubPageBridge::default();
        let (mut app, bridge, _host) = window_app_start(
            WindowStart::Editor(Box::new(RoutedEditor {
                state: editor_state(),
                selection_first: false,
            })),
            bridge,
            RecordingHost::default(),
        );
        *app.editor_outcome.borrow_mut() = Some(Err(flintshot_core::FlintshotError::Shell {
            detail: "render blew up".into(),
        }));
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        assert!(
            matches!(app.route, Route::Home),
            "a render error routes home, same window"
        );
        let statuses = bridge.statuses.borrow();
        assert_eq!(
            statuses.len(),
            1,
            "the error surfaced through the status line"
        );
        assert!(
            statuses[0].contains("render blew up"),
            "the status carries the error detail: {}",
            statuses[0]
        );
    }

    /// A capture intent stays in the slot for the window's exit (its
    /// close is landing — the outer loop runs the grab after).
    #[test]
    fn capture_outcome_survives_for_the_exit() {
        let ctx = egui::Context::default();
        let (mut app, _bridge, _host) = window_app_start(
            WindowStart::Editor(Box::new(RoutedEditor {
                state: editor_state(),
                selection_first: false,
            })),
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        *app.editor_outcome.borrow_mut() = Some(Ok(EditorOutcome::NewCapture {
            mode: flintshot_core::NewCaptureMode::Region,
        }));
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        assert!(
            matches!(app.route, Route::Editor(..)),
            "the capture close is the editor's own; the router does not re-route"
        );
        assert!(
            matches!(
                app.editor_outcome.borrow().as_ref(),
                Some(Ok(EditorOutcome::NewCapture { .. }))
            ),
            "the capture intent rides the exit"
        );
    }

    /// Entering a fresh editor clears a stale outcome — a leftover
    /// Cancelled must not bounce the new page straight home.
    #[test]
    fn fresh_editor_starts_with_a_clean_outcome_slot() {
        let ctx = egui::Context::default();
        let mut app = window_app();
        *app.editor_outcome.borrow_mut() = Some(Ok(EditorOutcome::Cancelled));
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.request_route(
                ui.ctx(),
                Route::Editor(Box::new(RoutedEditor {
                    state: editor_state(),
                    selection_first: false,
                })),
            );
        });
        assert!(
            app.editor_outcome.borrow().is_none(),
            "route entry clears the slot"
        );
    }

    // --- The Loading route (editor-capture-ux Task 008) ---

    #[test]
    fn loading_start_opens_the_loading_route() {
        let (app, _bridge, _host) = window_app_start(
            WindowStart::Loading,
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        assert!(matches!(app.route, Route::Loading));
        assert!(app.editor.is_none(), "no editor until the image lands");
    }

    /// The delivery contract: a grab's image armed while the window
    /// shows Loading swaps the editor route in-place — no close, no
    /// recreation.
    #[test]
    fn loading_delivers_the_routed_editor_in_place() {
        let host = RecordingHost::default();
        let (mut app, _bridge, host) =
            window_app_start(WindowStart::Loading, StubPageBridge::default(), host);
        host.inner.borrow_mut().queued.push_back(RoutedEditor {
            state: editor_state(),
            selection_first: false,
        });
        let ctx = egui::Context::default();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        assert!(
            matches!(app.route, Route::Editor(..)),
            "the delivered image swaps the editor in-place"
        );
        assert!(
            !transition_commands(&full)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close)),
            "the delivery never recreates the window"
        );
    }

    /// A failed or quiet grab while Loading routes home — the
    /// spinner must not strand.
    #[test]
    fn route_home_intent_ends_the_loading_route() {
        let host = RecordingHost::default();
        let (mut app, _bridge, host) =
            window_app_start(WindowStart::Loading, StubPageBridge::default(), host);
        host.inner.borrow_mut().route_home = Some(());
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        assert!(
            matches!(app.route, Route::Home),
            "the route-home intent ends the spinner"
        );
    }

    /// A home-route window ignores a stale route-home intent (the
    /// no-swap guard).
    #[test]
    fn route_home_intent_is_ignored_on_home() {
        let host = RecordingHost::default();
        let (mut app, _bridge, host) =
            window_app_start(WindowStart::Home, StubPageBridge::default(), host);
        host.inner.borrow_mut().route_home = Some(());
        let ctx = egui::Context::default();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        assert!(
            matches!(app.route, Route::Home) && transition_commands(&full).is_empty(),
            "home stays home with no churn"
        );
    }

    /// The router delegates to the home route on Home and drops the
    /// editor app (a Home-route frame never touches an editor).
    #[test]
    fn home_route_delegates_and_holds_no_editor() {
        let app = window_app();
        assert!(app.editor.is_none());
        assert!(matches!(app.route, Route::Home));
    }
}
