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

/// The toast a settings focus ping raises while Settings is already
/// the live route — the "nothing to swap to" branch stays audible
/// instead of silently consuming the intent.
const SETTINGS_ALREADY_OPEN: &str = "Settings is already open";

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
    /// dots overflow; Escape returns to Home.
    Settings,
    /// The AI settings page (Ask-the-shot): reached from the
    /// topbar overflow's **AI settings** row (every routed
    /// surface), Settings' link row, the transform dialog's
    /// **Configure AI…** link, or `app --route ai-config`.
    /// Escaping returns to the parked
    /// editor when an editor-route entry parked one (annotations
    /// intact), else Home.
    AiConfig {
        /// The editor session parked by an entry that routed from
        /// the editor (the dialog's Configure link or the
        /// overflow's AI settings row) — the live state (not the
        /// route's construction snapshot), restored on Esc /
        /// breadcrumb.
        parked_editor: Option<Box<RoutedEditor>>,
    },
}

/// An editor route waiting to run: the editor state plus its
/// selection-first landing phase.
#[derive(Debug, Clone)]
pub struct RoutedEditor {
    /// Source screenshot + annotation model.
    pub state: EditorState,
    /// Whether the editor route opens selection-first.
    pub selection_first: bool,
    /// The session-store entry tracking this capture, when armed
    /// through the app's choke point (`None` for untracked
    /// fallbacks — a bare-stash restore whose store entry is gone).
    pub session_id: Option<flintshot_core::SessionId>,
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
    /// The settings page (settings-improvement): a cold start on
    /// Settings (`app --route settings`) or the reopen after a
    /// `SettingsFocus` intent while the window was closed.
    Settings,
    /// The AI settings page: cold start (`app --route ai-config`).
    /// In-window entries route via [`Route::AiConfig`] parking.
    AiConfig,
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
    /// `session_id` names the session-store entry tracking the
    /// capture, so the host can bind its active-editor state.
    fn editor_route_opened(&mut self, session_id: Option<flintshot_core::SessionId>);

    /// The editor route is being left (an in-window route swap or a
    /// same-route image replacement) **with the live editor state**
    /// — not the route slot's construction snapshot, which went
    /// stale the moment the user edited. The host flushes the
    /// committed annotations into the session store here; this is
    /// the seam that ends the old annotation loss.
    fn editor_route_left(&mut self, state: &EditorState);

    /// Take a pending "route home" intent, if one waits (a failed or
    /// quiet grab while the window was open on the Loading route —
    /// nothing is coming to show, and the spinner must not strand —
    /// or an idle
    /// [`Event::AppFocus`](flintshot_core::ipc::Event::AppFocus) ping
    /// (tray / applet *Open app window*): the open window routes
    /// home from wherever it sat).
    fn take_route_home(&mut self) -> bool;

    /// Take a pending "route settings" intent, if one waits (an
    /// [`Event::SettingsFocus`](flintshot_core::ipc::Event::SettingsFocus)
    /// arriving while the window was open — tray / applet
    /// *Settings*). The router swaps the settings page in place, no
    /// close.
    fn take_route_settings(&mut self) -> bool;

    /// Take a pending "route AI settings" intent, if one waits (the
    /// transform dialog's **Configure AI…** link arming the swap
    /// through the page bridge's shared state). Default: never —
    /// only the app window's host arms it.
    fn take_route_ai_config(&mut self) -> bool {
        false
    }
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
        Route::Editor(..) | Route::Loading | Route::Settings | Route::AiConfig { .. } => {
            RouteGeometry {
                size: EDITOR_PAGE_DEFAULT_SIZE,
                resizable: true,
                title: WINDOW_TITLE,
            }
        }
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
    /// The AI settings page, when the `AiConfig` route is active.
    ai_config: Option<crate::ai_config::AiConfigApp>,
    /// The live toast, if one is up (the already-open Settings ping
    /// is the first raiser).
    toast: Option<crate::toast::Toast>,
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
            WindowStart::Settings => (Route::Settings, None),
            WindowStart::AiConfig => (
                Route::AiConfig {
                    parked_editor: None,
                },
                None,
            ),
            WindowStart::Editor(routed) => {
                let editor = Self::build_editor(&routed, ctx, &page_bridge, &editor_outcome);
                (Route::Editor(routed), Some(editor))
            }
        };
        *route_slot.borrow_mut() = route.clone();
        let mut home = HomeApp::new(home_bridge, initial_status);
        // A Settings start builds its page from the bridge's config
        // snapshot — the same construction `request_route` performs
        // on an in-window swap.
        let settings = if matches!(route, Route::Settings) {
            Some(Self::build_settings(&mut home))
        } else {
            None
        };
        let ai_config = if matches!(route, Route::AiConfig { .. }) {
            Some(Self::build_ai_config(&mut home, &page_bridge))
        } else {
            None
        };
        Self {
            route,
            home,
            editor,
            settings,
            ai_config,
            toast: None,
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

    /// Snapshot the settings page from the home bridge (the one
    /// construction every Settings entry shares: route start, in-window
    /// swap, and the topbar's Settings action).
    fn build_settings(home: &mut crate::home::HomeApp) -> crate::settings::SettingsApp {
        crate::settings::SettingsApp::new(
            home.load_config(),
            home.display_server(),
            home.version(),
            home.default_config_toml(),
            home.session_is_wayland(),
        )
    }

    /// Snapshot the AI settings page from the home bridge (every
    /// `AiConfig` entry shares it), seeding the key-status row from
    /// the page bridge.
    fn build_ai_config(
        home: &mut crate::home::HomeApp,
        bridge: &PageBridgeSlot,
    ) -> crate::ai_config::AiConfigApp {
        let mut app = crate::ai_config::AiConfigApp::new(home.load_config());
        let provider = app.config().ai.provider.clone();
        let saved = bridge.borrow_mut().ai_key_saved(&provider);
        app.set_key_saved(&provider, saved);
        app
    }

    /// Swap the route in place: construct / drop the route's app and
    /// issue the one-time geometry batch when the route *kind*
    /// changes. Same-kind swaps (an image swap inside the editor
    /// route) replace the editor app without touching the window.
    pub(crate) fn request_route(&mut self, ctx: &Context, route: Route) {
        let kind_changed = std::mem::discriminant(&self.route) != std::mem::discriminant(&route);
        // GOTCHA: flush from the live EditorApp, never from the
        // route payload — `self.route`'s copy is the construction
        // snapshot and predates every annotation the user made.
        // Covers both kind changes away from Editor and same-kind
        // image replacements (an outgoing editor ends either way).
        if matches!(self.route, Route::Editor(..)) {
            if let Some(editor) = self.editor.as_ref() {
                self.host.editor_route_left(editor.state());
            }
        }
        // AiConfig computes its parked payload from the live editor
        // before the match (the match's other arms read `&route`).
        // Parking rule: when the Configure link routed from a live
        // editor, the parked state is the *live* state (the same
        // freshness rule as the flush above — the route slot's copy
        // predates every annotation).
        let route = if let Route::AiConfig { .. } = route {
            let parked = match (&self.route, self.editor.as_ref()) {
                (Route::Editor(routed), Some(editor)) => {
                    let mut fresh = (**routed).clone();
                    fresh.state = editor.state().clone();
                    Some(Box::new(fresh))
                }
                _ => None,
            };
            Route::AiConfig {
                parked_editor: parked,
            }
        } else {
            route
        };
        match &route {
            Route::Editor(routed) => {
                self.host.editor_route_opened(routed.session_id);
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
                self.settings = None;
                self.ai_config = None;
            }
            Route::Home | Route::Loading => {
                self.editor = None;
                self.settings = None;
                self.ai_config = None;
            }
            Route::Settings => {
                self.editor = None;
                self.settings = Some(Self::build_settings(&mut self.home));
                self.ai_config = None;
            }
            Route::AiConfig { .. } => {
                self.editor = None;
                self.settings = None;
                self.ai_config = Some(Self::build_ai_config(&mut self.home, &self.page_bridge));
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

    /// The page-level directive poll for routes without a route app
    /// that polls them itself (Loading, Settings): the router owns
    /// the queue there — `Hide` still closes (the not-in-the-shot
    /// guarantee holds on every route), `Quit` closes with user-quit
    /// semantics (the tray toggle), and `Show` is the focus ping;
    /// the status line stays home's. Without this poll, a capture
    /// started while the Settings page was open would leave the
    /// window on screen — in the shot.
    fn poll_page_directives(&mut self, ctx: &Context) {
        while let Some(directive) = self.home.poll_directive() {
            match directive {
                crate::home::HomeDirective::Hide | crate::home::HomeDirective::Quit => {
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
    /// must not strand. An idle `AppFocus` ping (tray / applet
    /// *Open app window*) arms it too: the window routes home from
    /// Settings or an editor. Never fires on Home (a no-swap there).
    fn poll_route_home(&mut self, ctx: &Context) {
        if self.host.take_route_home() && !matches!(self.route, Route::Home) {
            self.request_route(ctx, Route::Home);
        }
    }

    /// The route-settings poll (settings-improvement): a
    /// `SettingsFocus` intent armed while the window was open swaps
    /// the settings page in place — no close, no viewport churn
    /// beyond the route-kind batch. A ping landing while Settings is
    /// *already* the live route has nothing to swap to: it raises the
    /// already-open toast instead of being silently consumed.
    fn poll_route_settings(&mut self, ctx: &Context) {
        if self.host.take_route_settings() {
            if matches!(self.route, Route::Settings) {
                let now = ctx.input(|i| i.time);
                self.toast = Some(crate::toast::Toast::new(SETTINGS_ALREADY_OPEN, now));
            } else {
                self.request_route(ctx, Route::Settings);
            }
        }
    }

    /// The `AiConfig` route poll: the transform dialog's **Configure
    /// AI…** link (page-mode editor) arms the intent through the page
    /// bridge's shared state; the router swaps in place — parking
    /// happens inside `request_route` from the live editor.
    fn poll_route_ai_config(&mut self, ctx: &Context) {
        if self.host.take_route_ai_config() && !matches!(self.route, Route::AiConfig { .. }) {
            self.request_route(
                ctx,
                Route::AiConfig {
                    parked_editor: None,
                },
            );
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
            Route::Editor(..) => {}
            Route::Loading => {
                // The spinner animates and the delivery polls run on
                // the heartbeat, not input.
                ctx.request_repaint_after(LOADING_REPAINT);
                self.poll_page_directives(ctx);
            }
            // Settings: Escape + the debounce drain run in ui_impl,
            // which owns the page borrow — but the directive queue
            // still needs a consumer here (a Hide during a capture
            // must close the window from the settings page too).
            // `AiConfig` shares the poll contract.
            Route::Settings | Route::AiConfig { .. } => {
                ctx.request_repaint_after(LOADING_REPAINT);
                self.poll_page_directives(ctx);
            }
        }
        if let Some(editor) = self.editor.as_mut() {
            editor.logic_impl(ctx);
        }
        // Routing polls run after the active route's logic: an
        // Open/Paste click arms the routed editor here, the editor's
        // outcome (Esc / error / capture) settles now, a failed or
        // quiet grab routes home from under the spinner, and a
        // settings focus intent swaps the settings page in.
        self.poll_routed_editor(ctx);
        self.poll_editor_outcome(ctx);
        self.poll_route_home(ctx);
        self.poll_route_settings(ctx);
        self.poll_route_ai_config(ctx);
    }

    /// Headless-callable core of [`App::ui`].
    pub(crate) fn ui_impl(&mut self, ui: &mut egui::Ui) {
        // The topbar spans every route (home, editor, loading) —
        // one identity + action line across the window's whole life.
        let surface = match &self.route {
            Route::Home => crate::topbar::Surface::Home,
            Route::Editor(..) => crate::topbar::Surface::Editor,
            Route::Loading => crate::topbar::Surface::Loading,
            Route::Settings | Route::AiConfig { .. } => crate::topbar::Surface::Settings,
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
        // The Resize item is live only on a shrinkable image (the
        // editor route; other surfaces render no editor items).
        let resize_live = self.editor.as_ref().is_some_and(EditorApp::can_resize);
        let topbar_action =
            crate::topbar::show(ui, surface, status.as_deref(), enable, copied, resize_live);
        // Route the fired action: editor pages forward everything to
        // the editor's dispatcher; Home dispatches Capture / Open to
        // the home app's own paths; Loading is fully disabled and
        // Settings stays inert until its route lands.
        match topbar_action {
            // The Capture hero fires on Home and Settings alike —
            // the enable matrix keeps it live on Settings, and the
            // settings route is a page over home (the bridge's
            // capture path + close choreography is route-agnostic).
            Some(crate::topbar::TopbarAction::Capture)
                if matches!(
                    self.route,
                    Route::Home | Route::Settings | Route::AiConfig { .. }
                ) =>
            {
                self.home.start_capture(ui.ctx());
            }
            Some(crate::topbar::TopbarAction::Open) if matches!(self.route, Route::Home) => {
                self.home.handle_open(ui.ctx());
            }
            // The dots overflow's Settings entry opens the settings
            // route from every route (spec §Settings route).
            Some(crate::topbar::TopbarAction::Settings) => {
                self.settings = Some(Self::build_settings(&mut self.home));
                self.request_route(ui.ctx(), Route::Settings);
            }
            // The overflow's AI settings entry: the Configure
            // link's routing from the topbar — `request_route`
            // parks the live editor when routing from Editor, and
            // Esc restores it (the existing AiConfig contract).
            Some(crate::topbar::TopbarAction::AiSettings) => {
                self.ai_config = Some(Self::build_ai_config(&mut self.home, &self.page_bridge));
                self.request_route(
                    ui.ctx(),
                    Route::AiConfig {
                        parked_editor: None,
                    },
                );
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
        // Deferred route intents: whole-`self` route swaps must wait
        // out the per-page borrows of the body pass.
        if let Some(route) = self.draw_route_body(ui) {
            self.request_route(ui.ctx(), route);
        }
        if let Some(editor) = self.editor.as_mut() {
            editor.ui_impl(ui);
        }
        crate::toast::show(ui, &mut self.toast);
    }

    /// The active route's body. Returns a deferred route intent
    /// (Esc / link clicks): whole-`self` route swaps must wait out
    /// this pass's per-page borrows, so the caller applies them
    /// after it returns.
    fn draw_route_body(&mut self, ui: &mut egui::Ui) -> Option<Route> {
        let mut route_after: Option<Route> = None;
        match &mut self.route {
            Route::Home => self.home.ui_impl(ui),
            Route::Editor(..) => {}
            Route::Loading => draw_loading(ui),
            Route::Settings => {
                if let Some(page) = self.settings.as_mut() {
                    let now = ui.ctx().input(|i| i.time);
                    // The body lives in a CentralPanel: without one,
                    // the page's ScrollArea drew directly on the
                    // window's root region and painted under the
                    // 44 px topbar (settings-improvement's layout
                    // bug — pinned by
                    // `settings_body_never_paints_under_the_topbar`).
                    let panel = egui::CentralPanel::default()
                        .frame(egui::Frame::new().fill(crate::theme::palette::PAPER_CREAM))
                        .show(ui, |ui| crate::settings::SettingsPage::show(ui, page, now));
                    let escape = panel.inner;
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
                    // The page's clicked links: the bridge services
                    // them (the editor spawn leaves the UI thread).
                    let mut actions = Vec::new();
                    while let Some(action) = page.drain_action() {
                        actions.push(action);
                    }
                    for action in actions {
                        match action {
                            crate::settings::SettingsAction::EditConfigFile => {
                                self.home.open_config_file();
                            }
                            crate::settings::SettingsAction::RebindShortcuts => {
                                self.home.request_rebind();
                            }
                            // The Ask-the-shot link row: route to the
                            // AI settings page (no parked editor —
                            // Settings never has one).
                            crate::settings::SettingsAction::OpenAiConfig => {
                                route_after = Some(Route::AiConfig {
                                    parked_editor: None,
                                });
                            }
                        }
                    }
                    if escape {
                        route_after = Some(Route::Home);
                    }
                }
            }
            Route::AiConfig { parked_editor } => {
                if let Some(page) = self.ai_config.as_mut() {
                    let now = ui.ctx().input(|i| i.time);
                    let panel = egui::CentralPanel::default()
                        .frame(egui::Frame::new().fill(crate::theme::palette::PAPER_CREAM))
                        .show(ui, |ui| crate::ai_config::AiConfigPage::show(ui, page, now));
                    let escape = panel.inner;
                    if let Some(config) = page.drain_save(now) {
                        // The editor bridge owns the save (same
                        // persist contract as the settings route)
                        // and the save-change vision probe that arms
                        // on it — the AI page's saves flow here, not
                        // through the settings route.
                        self.page_bridge.borrow_mut().save_config(&config);
                    }
                    // The page's actions: the bridge services keyring
                    // writes, test connections, and model fetches
                    // (the page stays pure data + queue).
                    Self::service_ai_config_actions(&self.page_bridge, page);
                    // The vision-probe arrival: the banner's
                    // persisted verdict (+ triple), and — when a
                    // manual Test armed this probe — its result
                    // line.
                    if let Some(arrival) = self.page_bridge.borrow_mut().poll_vision_probe() {
                        page.note_probe_outcome(&arrival.outcome);
                        if let Some(result) = arrival.manual_test {
                            page.set_test_result(result);
                        }
                    }
                    // The models fetch's arrival (the Browse
                    // lifecycle): dropped page-side when it no longer
                    // matches the working pair.
                    if let Some(arrival) = self.page_bridge.borrow_mut().poll_models() {
                        page.note_models_arrival(&arrival);
                    }
                    if escape {
                        // Esc / breadcrumb: the parked editor returns
                        // (annotations intact) when the Configure
                        // link routed from one, else Home.
                        match parked_editor.take() {
                            Some(parked) => route_after = Some(Route::Editor(parked)),
                            None => route_after = Some(Route::Home),
                        }
                    }
                }
            }
        }
        route_after
    }

    /// Drain the AI settings page's action queue through the page
    /// bridge (keyring writes, key-status queries, test connections,
    /// model fetches — the page stays pure data + this loop). Takes
    /// the bridge slot, not `self`: the page is borrowed out of
    /// `self.ai_config` for the route body's whole draw.
    fn service_ai_config_actions(
        bridge: &PageBridgeSlot,
        page: &mut crate::ai_config::AiConfigApp,
    ) {
        let mut actions = Vec::new();
        while let Some(action) = page.drain_action() {
            actions.push(action);
        }
        for action in actions {
            match action {
                crate::ai_config::AiConfigAction::SaveKey { provider, key } => {
                    let stored = bridge.borrow_mut().ai_save_key(&provider, &key);
                    if stored {
                        page.set_key_saved(&provider, true);
                    } else {
                        page.set_key_store_failed(&provider);
                    }
                }
                crate::ai_config::AiConfigAction::ProviderChanged { provider } => {
                    let saved = bridge.borrow_mut().ai_key_saved(&provider);
                    page.set_key_saved(&provider, saved);
                }
                crate::ai_config::AiConfigAction::TestConnection {
                    provider,
                    model,
                    base_url,
                } => {
                    bridge
                        .borrow_mut()
                        .ai_test_connection(&provider, &model, &base_url);
                }
                crate::ai_config::AiConfigAction::ListModels { provider, base_url } => {
                    // The Browse click is the user action — the only
                    // path that fires the fetch.
                    bridge.borrow_mut().ai_list_models(&provider, &base_url);
                }
            }
        }
    }
}

impl Drop for WindowApp {
    /// Window death while the editor is live (a `NewCapture` close,
    /// a hidden-by-event capture close, user quit): the live state
    /// dies with the window, so this is the last hand-off point —
    /// route swaps already flushed through `request_route` (the
    /// editor slot is `None` / the route has moved on, so there is
    /// no double flush).
    fn drop(&mut self) {
        if matches!(self.route, Route::Editor(..)) {
            if let Some(editor) = self.editor.as_ref() {
                self.host.editor_route_left(editor.state());
            }
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

    /// The Settings route's Capture hero is no dead control: the
    /// same click fires the bridge exactly once from Settings too
    /// (task 005 — the enable matrix kept Capture live there, the
    /// dispatch guard just never matched).
    #[test]
    fn settings_topbar_capture_fires_the_bridge_request() {
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
        app.request_route(&ctx, Route::Settings);
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1024.0, 640.0));
        let frame = |ctx: &egui::Context, app: &mut WindowApp, events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..egui::RawInput::default()
            };
            let _ = ctx.run_ui(raw, |ui| app.ui_impl(ui));
        };
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        let capture = ctx
            .read_response(egui::Id::new("topbar-capture"))
            .expect("the topbar Capture hero registers on Settings")
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
            "one topbar Capture click on Settings = one bridge capture request"
        );
    }

    /// The dots overflow's Settings entry routes the editor page to
    /// Settings in place — the router handles the entry itself, so
    /// editor pages never see the action (spec §Settings route).
    #[test]
    fn editor_overflow_settings_routes_to_the_settings_route() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let (mut app, _bridge, _host) = window_app_start(
            WindowStart::Editor(Box::new(RoutedEditor {
                state: editor_state(),
                selection_first: false,
                session_id: None,
            })),
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1024.0, 640.0));
        let frame = |ctx: &egui::Context, app: &mut WindowApp, events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..egui::RawInput::default()
            };
            let _ = ctx.run_ui(raw, |ui| app.ui_impl(ui));
        };
        let click = |pos: egui::Pos2| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        let ovf = ctx
            .read_response(egui::Id::new("topbar-overflow"))
            .expect("the routed editor's overflow registers")
            .rect;
        // Open the popup, then the settle passes (the popup anchors
        // at the screen centre on its open pass and re-anchors below
        // the chip one pass later — the + popup tests' quirk).
        frame(&ctx, &mut app, click(ovf.center()));
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        let settings = ctx
            .read_response(egui::Id::new("topbar-settings"))
            .expect("the Settings entry renders in the editor overflow")
            .rect;
        frame(&ctx, &mut app, click(settings.center()));
        assert!(
            matches!(app.route, Route::Settings),
            "the overflow's Settings entry routed the editor page to Settings"
        );
    }

    /// The overflow's **AI settings** entry routes the editor page to
    /// the AI config page with the live editor parked — the Configure
    /// link's parking semantics from the topbar.
    #[test]
    fn overflow_ai_settings_routes_and_parks_the_editor() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let (mut app, _bridge, _host) = window_app_start(
            WindowStart::Editor(Box::new(RoutedEditor {
                state: editor_state(),
                selection_first: false,
                session_id: None,
            })),
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1024.0, 640.0));
        let frame = |ctx: &egui::Context, app: &mut WindowApp, events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..egui::RawInput::default()
            };
            let _ = ctx.run_ui(raw, |ui| app.ui_impl(ui));
        };
        let click = |pos: egui::Pos2| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        let ovf = ctx
            .read_response(egui::Id::new("topbar-overflow"))
            .expect("the routed editor's overflow registers")
            .rect;
        frame(&ctx, &mut app, click(ovf.center()));
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        let row = ctx
            .read_response(egui::Id::new("topbar-ai-settings"))
            .expect("the AI settings entry renders in the editor overflow")
            .rect;
        frame(&ctx, &mut app, click(row.center()));
        assert!(
            matches!(
                &app.route,
                Route::AiConfig {
                    parked_editor: Some(_)
                }
            ),
            "the overflow's AI settings entry parks the live editor"
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
                session_id: None,
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

    /// The tray toggle's hide direction (tray-show-hide-toggle):
    /// a Quit directive closes from the Loading route too.
    #[test]
    fn loading_quit_directive_closes_the_window() {
        let mut app = loading_app_with_directives(vec![HomeDirective::Quit]);
        let ctx = egui::Context::default();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        assert!(
            transition_commands(&full)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close)),
            "Quit closes even while the capture delivers"
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
        saved_keys: Rc<RefCell<Vec<(String, String)>>>,
        key_saved_answers: Rc<RefCell<std::collections::HashMap<String, bool>>>,
        key_queries: Rc<RefCell<Vec<String>>>,
        tests: Rc<RefCell<Vec<(String, String, String)>>>,
        models_fetches: Rc<RefCell<Vec<(String, String)>>>,
        models_answer: Rc<RefCell<Option<crate::shell::ModelsArrival>>>,
    }

    impl StubPageBridge {
        /// Seed a key-status answer for a provider (the bridge-side
        /// keyring truth the page queries).
        #[allow(dead_code, reason = "a convenience seam for future AI-page tests")]
        fn set_key_saved(&self, provider: &str, saved: bool) {
            self.key_saved_answers
                .borrow_mut()
                .insert(provider.to_string(), saved);
        }
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

        fn ai_key_saved(&mut self, provider: &str) -> bool {
            self.key_queries.borrow_mut().push(provider.to_string());
            self.key_saved_answers
                .borrow()
                .get(provider)
                .copied()
                .unwrap_or(false)
        }

        fn ai_save_key(&mut self, provider: &str, key: &str) -> bool {
            self.saved_keys
                .borrow_mut()
                .push((provider.to_string(), key.to_string()));
            // The stub "stores" into its map: answer the follow-up
            // key-status query truthfully.
            self.key_saved_answers
                .borrow_mut()
                .insert(provider.to_string(), true);
            true
        }

        fn ai_test_connection(&mut self, provider: &str, model: &str, base_url: &str) {
            self.tests
                .borrow_mut()
                .push((provider.into(), model.into(), base_url.into()));
        }

        fn ai_list_models(&mut self, provider: &str, base_url: &str) {
            self.models_fetches
                .borrow_mut()
                .push((provider.into(), base_url.into()));
        }

        fn poll_models(&mut self) -> Option<crate::shell::ModelsArrival> {
            self.models_answer.borrow_mut().take()
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
        opened_sessions: Vec<Option<flintshot_core::SessionId>>,
        left: Vec<Vec<flintshot_core::Annotation>>,
        route_home: Option<()>,
        route_settings: Option<()>,
        route_ai_config: Option<()>,
    }

    impl RouteHost for RecordingHost {
        fn take_routed_editor(&mut self) -> Option<RoutedEditor> {
            self.inner.borrow_mut().queued.pop_front()
        }

        fn editor_route_opened(&mut self, session_id: Option<flintshot_core::SessionId>) {
            let mut inner = self.inner.borrow_mut();
            inner.opened += 1;
            inner.opened_sessions.push(session_id);
        }

        fn editor_route_left(&mut self, state: &EditorState) {
            self.inner.borrow_mut().left.push(state.anns.clone());
        }

        fn take_route_home(&mut self) -> bool {
            self.inner.borrow_mut().route_home.take().is_some()
        }

        fn take_route_settings(&mut self) -> bool {
            self.inner.borrow_mut().route_settings.take().is_some()
        }

        fn take_route_ai_config(&mut self) -> bool {
            self.inner.borrow_mut().route_ai_config.take().is_some()
        }
    }

    fn editor_state() -> EditorState {
        let img = image::RgbaImage::from_raw(4, 4, vec![255; 4 * 4 * 4])
            .expect("raw buffer matches 4*4*4");
        EditorState::new(img)
    }

    fn line_ann_fixture() -> flintshot_core::Annotation {
        flintshot_core::Annotation::Line {
            a: flintshot_core::Point::new(0.0, 0.0),
            b: flintshot_core::Point::new(9.0, 9.0),
            stroke: flintshot_core::Stroke::new(flintshot_core::Color::RED, 3.0),
        }
    }

    /// An editor start whose live state carries one committed
    /// annotation — the state a user leaves behind mid-edit.
    fn annotated_editor_start() -> WindowStart {
        let mut state = editor_state();
        state.anns.push(line_ann_fixture());
        WindowStart::Editor(Box::new(RoutedEditor {
            state,
            selection_first: false,
            session_id: None,
        }))
    }

    /// Editor → Home: the leave-flush hands the host the live
    /// editor's annotations — the construction snapshot's empty
    /// list must not win (the documented loss this effort fixes).
    #[test]
    fn leaving_the_editor_flushes_live_annotations() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        // Start clean, then annotate *after* construction so the
        // route payload and the live state provably differ.
        let (mut app, _bridge, host) = window_app_start(
            WindowStart::Editor(Box::new(RoutedEditor {
                state: editor_state(),
                selection_first: false,
                session_id: None,
            })),
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        app.editor
            .as_mut()
            .expect("editor start builds the editor app")
            .state_mut()
            .anns
            .push(line_ann_fixture());

        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.request_route(ui.ctx(), Route::Home);
        });

        let left = host.inner.borrow().left.clone();
        assert_eq!(left.len(), 1, "one flush per editor session end");
        assert_eq!(
            left[0].len(),
            1,
            "the flush carries the live annotation, not the snapshot's"
        );
    }

    /// Window death with the editor live (`NewCapture` /
    /// hidden-by-event closes): Drop is the last hand-off point.
    #[test]
    fn drop_flushes_the_live_editor_at_window_death() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let (mut app, _bridge, host) = window_app_start(
            WindowStart::Editor(Box::new(RoutedEditor {
                state: editor_state(),
                selection_first: false,
                session_id: None,
            })),
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        app.editor
            .as_mut()
            .expect("editor start builds the editor app")
            .state_mut()
            .anns
            .push(line_ann_fixture());
        drop(app);

        let left = host.inner.borrow().left.clone();
        assert_eq!(left.len(), 1, "window death flushes exactly once");
        assert_eq!(left[0].len(), 1);
    }

    /// The editor route opening reports the session-store id it is
    /// bound to, so the host can track the active entry.
    #[test]
    fn editor_route_opened_carries_the_session_id() {
        let ctx = egui::Context::default();
        let (mut app, _bridge, host) = window_app_start(
            annotated_editor_start(),
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        let id = flintshot_core::SessionId::from_raw(7);
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.request_route(
                ui.ctx(),
                Route::Editor(Box::new(RoutedEditor {
                    state: editor_state(),
                    selection_first: false,
                    session_id: Some(id),
                })),
            );
        });
        let sessions = host.inner.borrow().opened_sessions.clone();
        assert_eq!(
            sessions,
            vec![Some(id)],
            "the host sees which store entry went live"
        );
    }

    /// A same-kind image replacement ends the outgoing editor too —
    /// its annotations flush before the replacement arms.
    #[test]
    fn replacing_the_editor_image_flushes_the_outgoing_annotations() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let (mut app, _bridge, host) = window_app_start(
            WindowStart::Editor(Box::new(RoutedEditor {
                state: editor_state(),
                selection_first: false,
                session_id: None,
            })),
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        app.editor
            .as_mut()
            .expect("editor start builds the editor app")
            .state_mut()
            .anns
            .push(line_ann_fixture());
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.request_route(
                ui.ctx(),
                Route::Editor(Box::new(RoutedEditor {
                    state: editor_state(),
                    selection_first: false,
                    session_id: None,
                })),
            );
        });
        let left = host.inner.borrow().left.clone();
        assert_eq!(
            left.len(),
            1,
            "the outgoing editor flushes even without a kind change"
        );
    }

    /// Editor → `AiConfig` (the Configure link's route): the editor
    /// parks with its **live** annotations, the leave-flush fires
    /// once, and Esc restores the editor with the annotations intact
    /// — the configure-and-back round-trip contract.
    #[test]
    fn aiconfig_parks_the_editor_and_esc_restores_it_annotated() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let (mut app, _bridge, host) = window_app_start(
            WindowStart::Editor(Box::new(RoutedEditor {
                state: editor_state(),
                selection_first: false,
                session_id: None,
            })),
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        // Annotate after construction so the live state provably
        // differs from the route's snapshot.
        app.editor
            .as_mut()
            .expect("editor start builds the editor app")
            .state_mut()
            .anns
            .push(line_ann_fixture());
        app.request_route(
            &ctx,
            Route::AiConfig {
                parked_editor: None,
            },
        );

        assert!(matches!(app.route, Route::AiConfig { .. }));
        assert!(
            app.editor.is_none(),
            "the editor app drops while parked (the route payload holds it)"
        );
        assert_eq!(
            host.inner.borrow().left.len(),
            1,
            "leaving Editor for AiConfig flushes once"
        );
        match &app.route {
            Route::AiConfig { parked_editor } => {
                let parked = parked_editor
                    .as_ref()
                    .expect("routing from a live editor parks it");
                assert_eq!(
                    parked.state.anns.len(),
                    1,
                    "the parked state is the live one, not the snapshot"
                );
            }
            other => panic!("expected AiConfig, got {other:?}"),
        }

        // Esc on the AiConfig page restores the parked editor.
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1024.0, 640.0),
            )),
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| app.ui_impl(ui));
        assert!(
            matches!(app.route, Route::Editor(..)),
            "Esc restores the editor"
        );
        assert_eq!(
            app.editor
                .as_ref()
                .expect("the editor app rebuilt")
                .state()
                .anns
                .len(),
            1,
            "annotations survive the configure-and-back round-trip"
        );
    }

    /// A cold start (or Settings entry) has nothing parked: Esc
    /// routes Home.
    #[test]
    fn aiconfig_without_a_parked_editor_escapes_home() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let (mut app, _bridge, _host) = window_app_start(
            WindowStart::AiConfig,
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        assert!(matches!(
            app.route,
            Route::AiConfig {
                parked_editor: None
            }
        ));
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1024.0, 640.0),
            )),
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| app.ui_impl(ui));
        assert!(matches!(app.route, Route::Home));
    }

    /// The Browse pill's full window round-trip: the click routes the
    /// fetch through the bridge (the only firing path), and the
    /// bridge's arrival lands on the page keyed to the working pair
    /// (ai-config-ux T5).
    #[test]
    fn aiconfig_browse_click_fires_the_bridge_and_the_arrival_adopts() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let (mut app, bridge, _host) = window_app_start(
            WindowStart::AiConfig,
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1024.0, 640.0));
        let frame = |ctx: &egui::Context, app: &mut WindowApp, events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                events,
                screen_rect: Some(screen),
                ..egui::RawInput::default()
            };
            let _ = ctx.run_ui(raw, |ui| app.ui_impl(ui));
        };
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        let browse = ctx
            .read_response(egui::Id::new("ai-model-browse"))
            .expect("the Browse pill registers")
            .rect;
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(browse.center()),
                egui::Event::PointerButton {
                    pos: browse.center(),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: browse.center(),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert_eq!(
            bridge.models_fetches.borrow().as_slice(),
            [(
                "openai".to_string(),
                "https://api.openai.com/v1".to_string()
            )],
            "the click is the only firing path"
        );

        // The bridge answers for the working pair: the page adopts.
        *bridge.models_answer.borrow_mut() = Some(crate::shell::ModelsArrival {
            provider: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            result: Ok(vec![flintshot_core::ModelListing {
                id: "gpt-4o-mini".into(),
                vision: None,
            }]),
        });
        frame(&ctx, &mut app, vec![]);
        assert_eq!(
            app.ai_config
                .as_ref()
                .and_then(crate::ai_config::AiConfigApp::models_arrived)
                .map(|models| models.len()),
            Some(1),
            "the arrival lands on the page"
        );
    }

    /// The Configure-link route poll: an armed intent routes from a
    /// live editor with parking (the in-window path the page bridge
    /// arms through the app's shared state).
    #[test]
    fn aiconfig_route_intent_routes_and_parks_from_the_editor() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let (mut app, _bridge, host) = window_app_start(
            WindowStart::Editor(Box::new(RoutedEditor {
                state: editor_state(),
                selection_first: false,
                session_id: None,
            })),
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        app.editor
            .as_mut()
            .expect("editor start builds the editor app")
            .state_mut()
            .anns
            .push(line_ann_fixture());
        host.inner.borrow_mut().route_ai_config = Some(());
        app.logic_impl(&ctx);
        assert!(
            matches!(app.route, Route::AiConfig { ref parked_editor } if parked_editor.is_some()),
            "the poll routes to AiConfig with the editor parked"
        );
    }

    /// Full-window typing regression: open the transform editor on
    /// the `AiConfig` route, click the prompt field, let quiet frames
    /// pass, then type — the draft must receive the text. Frames
    /// advance time so section fold animations settle like a real
    /// window; the quiet frames pin that the field keeps keyboard
    /// focus across frames (the section body's old invisible
    /// measuring probe used to surrender it every frame).
    #[test]
    fn aiconfig_typing_reaches_the_transform_prompt_through_the_window() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let (mut app, _bridge, _host) = window_app_start(
            WindowStart::AiConfig,
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1024.0, 640.0));
        let mut t = 0.0_f32;
        let frame =
            |t: &mut f32, ctx: &egui::Context, app: &mut WindowApp, events: Vec<egui::Event>| {
                *t += 1.0 / 60.0;
                let raw = egui::RawInput {
                    time: Some(f64::from(*t)),
                    predicted_dt: 1.0 / 60.0,
                    screen_rect: Some(screen),
                    events,
                    ..egui::RawInput::default()
                };
                let _ = ctx.run_ui(raw, |ui| {
                    app.logic_impl(ui.ctx());
                    app.ui_impl(ui);
                });
            };
        let click = |t: &mut f32, ctx: &egui::Context, app: &mut WindowApp, at: egui::Pos2| {
            frame(
                t,
                ctx,
                app,
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
        };

        for _ in 0..30 {
            frame(&mut t, &ctx, &mut app, vec![]);
        }
        let new_pill = ctx
            .read_response(egui::Id::new("ai-transform-new"))
            .expect("the New transform pill registers")
            .rect;
        click(&mut t, &ctx, &mut app, new_pill.center());
        assert!(
            app.ai_config
                .as_ref()
                .and_then(|page| page.editing_prompt())
                .is_some(),
            "the New transform click opened the inline editor"
        );
        for _ in 0..30 {
            frame(&mut t, &ctx, &mut app, vec![]);
        }
        let field = ctx
            .read_response(egui::Id::new("ai-transform-prompt"))
            .expect("the editor's prompt field registers")
            .rect;
        click(&mut t, &ctx, &mut app, field.center());
        for _ in 0..5 {
            frame(&mut t, &ctx, &mut app, vec![]);
        }
        frame(
            &mut t,
            &ctx,
            &mut app,
            vec![egui::Event::Text("ocr it".into())],
        );

        let prompt = app
            .ai_config
            .as_ref()
            .and_then(|page| page.editing_prompt())
            .expect("the editor stayed open");
        assert_eq!(prompt, "ocr it", "typed text lands in the prompt draft");
    }

    /// `AiConfig` shares the stable geometry (the Settings precedent).
    #[test]
    fn aiconfig_route_uses_the_stable_geometry() {
        let g = route_geometry(&Route::AiConfig {
            parked_editor: None,
        });
        assert_eq!(g.size, EDITOR_PAGE_DEFAULT_SIZE);
        assert!(g.resizable);
        assert_eq!(g.title, "flintshot — annotate");
    }

    /// The AI page's actions reach the page bridge (keyring writes,
    /// provider key-status refreshes, test connections) — one
    /// `ui_impl` frame drains the queue.
    #[test]
    fn aiconfig_actions_service_through_the_page_bridge() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let (mut app, bridge, _host) = window_app_start(
            WindowStart::AiConfig,
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        // Default provider (openai) key status seeds from the bridge.
        assert_eq!(
            bridge.key_queries.borrow().as_slice(),
            ["openai".to_string()],
            "page build seeds the key status for the config's provider"
        );

        let page = app
            .ai_config
            .as_mut()
            .expect("the AiConfig start builds the page");
        page.set_key_draft("sk-test");
        page.commit_key_draft();
        page.config_mut().ai.model = "gpt-4o-mini".into();
        page.test_connection();

        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1024.0, 640.0));
        let raw = egui::RawInput {
            screen_rect: Some(screen),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| app.ui_impl(ui));

        let saved = bridge.saved_keys.borrow().clone();
        assert_eq!(
            saved,
            vec![("openai".to_string(), "sk-test".to_string())],
            "the key commit reaches the keyring seam once"
        );
        let tests = bridge.tests.borrow().clone();
        assert_eq!(tests.len(), 1, "the test connection fires");
        assert_eq!(tests[0].1, "gpt-4o-mini");
        assert_eq!(tests[0].2, "https://api.openai.com/v1");
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
        assert_eq!(g.size, EDITOR_PAGE_DEFAULT_SIZE);
        assert!(g.resizable);
        assert_eq!(g.title, "flintshot");
    }

    #[test]
    fn editor_route_uses_the_stable_viewport_geometry() {
        let g = route_geometry(&Route::Editor(Box::new(RoutedEditor {
            state: editor_state(),
            selection_first: false,
            session_id: None,
        })));
        assert_eq!(g.size, EDITOR_PAGE_DEFAULT_SIZE);
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
                session_id: None,
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
                    session_id: None,
                })),
            );
        });
        let commands = transition_commands(&full);
        assert!(commands.contains(&egui::ViewportCommand::InnerSize(EDITOR_PAGE_DEFAULT_SIZE)));
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
                session_id: None,
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
                    session_id: None,
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
            session_id: None,
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
                session_id: None,
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
                session_id: None,
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
                session_id: None,
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
                    session_id: None,
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
            session_id: None,
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

    /// The tray's *Open app window* ping while the window sits on the
    /// settings page: the route-home intent returns it home in place
    /// — raise + focus alone would read as "the click did nothing".
    #[test]
    fn route_home_intent_returns_the_settings_route_home() {
        let host = RecordingHost::default();
        let (mut app, _bridge, host) =
            window_app_start(WindowStart::Settings, StubPageBridge::default(), host);
        host.inner.borrow_mut().route_home = Some(());
        let ctx = egui::Context::default();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        assert!(
            matches!(app.route, Route::Home),
            "the app-focus ping routes the settings page home"
        );
        assert!(app.settings.is_none(), "the settings page is dropped");
        assert!(
            !transition_commands(&full)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close)),
            "the route home never closes the window"
        );
    }

    // --- The Settings start + focus intent (settings-improvement) ---

    /// A Settings start opens the settings route with its page
    /// constructed from the bridge's config snapshot.
    #[test]
    fn settings_start_opens_the_settings_route() {
        let (app, _bridge, _host) = window_app_start(
            WindowStart::Settings,
            StubPageBridge::default(),
            RecordingHost::default(),
        );
        assert!(matches!(app.route, Route::Settings));
        assert!(app.settings.is_some(), "the settings page is live");
        assert!(app.editor.is_none());
    }

    /// The focus intent swaps the settings page in place while the
    /// window is open — no close, the window survives.
    #[test]
    fn route_settings_intent_swaps_the_page_in_place() {
        let host = RecordingHost::default();
        let (mut app, _bridge, host) =
            window_app_start(WindowStart::Home, StubPageBridge::default(), host);
        host.inner.borrow_mut().route_settings = Some(());
        let ctx = egui::Context::default();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        assert!(
            matches!(app.route, Route::Settings),
            "the intent swapped the route in-place"
        );
        assert!(app.settings.is_some(), "the settings page went live");
        assert!(
            !transition_commands(&full)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close)),
            "the swap never closes the window"
        );
    }

    /// A settings intent while already on Settings raises the
    /// already-open toast (the ping is audible) with no churn — the
    /// no-swap guard keeps repeat broadcasts from rebuilding the page.
    #[test]
    fn route_settings_intent_is_ignored_on_settings() {
        let host = RecordingHost::default();
        let (mut app, _bridge, host) = window_app_start(
            WindowStart::Settings,
            StubPageBridge::default(),
            host.clone(),
        );
        host.inner.borrow_mut().route_settings = Some(());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let full = ctx.run_ui(
            egui::RawInput {
                time: Some(100.0),
                ..egui::RawInput::default()
            },
            |ui| {
                app.logic_impl(ui.ctx());
                app.ui_impl(ui);
            },
        );
        assert!(
            matches!(app.route, Route::Settings) && transition_commands(&full).is_empty(),
            "settings stays settings with no churn"
        );
        let toast = app
            .toast
            .take()
            .expect("the already-open ping raised a toast");
        assert!(
            !toast.expired(100.0),
            "a fresh toast outlives its raise frame"
        );
    }

    /// The already-open toast is transient: past its lifetime it is
    /// gone — expired by the draw pass, never stuck on screen.
    #[test]
    fn settings_already_open_toast_expires_after_its_lifetime() {
        let host = RecordingHost::default();
        let (mut app, _bridge, host) = window_app_start(
            WindowStart::Settings,
            StubPageBridge::default(),
            host.clone(),
        );
        host.inner.borrow_mut().route_settings = Some(());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let frame_at = |ctx: &egui::Context, app: &mut WindowApp, time: f64| {
            ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    ..egui::RawInput::default()
                },
                |ui| {
                    app.logic_impl(ui.ctx());
                    app.ui_impl(ui);
                },
            )
        };
        frame_at(&ctx, &mut app, 100.0);
        assert!(app.toast.is_some(), "fixture: the toast is up");
        frame_at(&ctx, &mut app, 100.0 + 2.0);
        assert!(app.toast.is_some(), "still inside its lifetime");
        frame_at(&ctx, &mut app, 100.0 + 3.0);
        assert!(app.toast.is_none(), "expired toasts leave the screen");
    }

    /// The not-in-the-shot guarantee holds from the settings page:
    /// a Hide directive (an external capture) closes the window
    /// there too — the pre-existing gap where the Settings route
    /// consumed no directives.
    #[test]
    fn hide_directive_on_settings_closes_the_window() {
        let mut app = settings_app_with_directives(vec![HomeDirective::Hide]);
        let ctx = egui::Context::default();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        assert!(
            transition_commands(&full)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close)),
            "Hide closes the window from the settings page"
        );
    }

    /// A Show on the settings route is the focus ping — visible +
    /// focus, never a close.
    #[test]
    fn show_directive_on_settings_focuses_without_closing() {
        let mut app = settings_app_with_directives(vec![HomeDirective::Show]);
        let ctx = egui::Context::default();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        let commands = transition_commands(&full);
        assert!(
            !commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close)),
            "Show focuses; it never closes"
        );
        assert!(commands.contains(&egui::ViewportCommand::Focus));
    }

    /// The tray toggle's hide direction from the settings page
    /// (tray-show-hide-toggle): Quit closes there too.
    #[test]
    fn quit_directive_on_settings_closes_the_window() {
        let mut app = settings_app_with_directives(vec![HomeDirective::Quit]);
        let ctx = egui::Context::default();
        let full = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.logic_impl(ui.ctx());
        });
        assert!(
            transition_commands(&full)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close)),
            "Quit closes the window from the settings page"
        );
    }

    /// A settings-route window fed directives (the router owns the
    /// queue on routes without a polling app).
    fn settings_app_with_directives(directives: Vec<HomeDirective>) -> WindowApp {
        let bridge = DirectiveBridge::default();
        *bridge.pending.borrow_mut() = directives.into();
        let slot: RouteSlot = Rc::new(RefCell::new(Route::Settings));
        let outcome: OutcomeSlot = Rc::new(RefCell::new(None));
        WindowApp::new(
            &egui::Context::default(),
            WindowStart::Settings,
            Box::new(bridge),
            Rc::new(RefCell::new(Box::new(StubPageBridge::default()))),
            Box::new(RecordingHost::default()),
            "connected".into(),
            outcome,
            slot,
        )
    }

    /// The section measure-probe must not double-register widget ids:
    /// egui's duplicate-id detector paints red "🔥 First/Second use of
    /// widget ID" strokes + labels on the debug layer (topmost,
    /// unclipped) whenever one id is used at two rects in a frame —
    /// which read as page elements floating over the topbar while the
    /// settings page scrolled (the probe used to run `10_000` px
    /// offscreen, so every row tripped it).
    #[test]
    fn settings_page_never_trips_eguis_duplicate_id_paint() {
        let mut app = settings_app_with_directives(Vec::new());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 480.0));
        for frame in 0..4u16 {
            let events = if frame == 2 {
                vec![egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -240.0),
                    modifiers: egui::Modifiers::NONE,
                    phase: egui::TouchPhase::Move,
                }]
            } else {
                vec![]
            };
            let full = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..egui::RawInput::default()
                },
                |ui| app.ui_impl(ui),
            );
            for clipped in &full.shapes {
                if let egui::Shape::Text(text) = &clipped.shape {
                    let body = text.galley.text();
                    assert!(
                        !body.contains("use of widget ID"),
                        "frame {frame}: egui painted a duplicate-id debug label \
                         ({body:?}) — a widget id was registered at two rects"
                    );
                }
            }
        }
    }

    #[test]
    fn settings_body_never_paints_under_the_topbar() {
        let mut app = settings_app_with_directives(Vec::new());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 480.0));
        let mut frame = |events: Vec<egui::Event>| {
            let full = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..egui::RawInput::default()
                },
                |ui| app.ui_impl(ui),
            );
            // What actually lands on screen: each shape's bounds
            // intersected with its clip rect. (The topbar's own
            // bottom edge is a centered 2 px stroke that straddles
            // y=44 by construction but is clip-limited to the bar —
            // the intersect keeps it out of the offender set.)
            full.shapes
                .iter()
                .filter_map(|c| {
                    let eff = c.clip_rect.intersect(c.shape.visual_bounding_rect());
                    eff.is_positive().then_some(eff)
                })
                .map(|r| {
                    egui::Rect::from_min_max(
                        egui::pos2(r.min.x, r.min.y),
                        egui::pos2(
                            (r.max.x * 10.0).round() / 10.0,
                            (r.max.y * 10.0).round() / 10.0,
                        ),
                    )
                })
                .collect::<Vec<egui::Rect>>()
        };
        let settled = frame(vec![]);
        let scrolled = frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -400.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        }]);

        let bar = crate::topbar::TOPBAR_HEIGHT;
        let above = |rects: &[egui::Rect]| -> Vec<egui::Rect> {
            rects.iter().copied().filter(|r| r.min.y < bar).collect()
        };
        // (a) The scroll never slides content under the topbar: the
        // set of shapes painted above the bar's edge is exactly the
        // bar's own, unchanged by scrolling.
        assert_eq!(
            above(&settled),
            above(&scrolled),
            "scrolled content joined the topbar's region"
        );

        // (b) The scrollbar engages: content taller than the
        // viewport is reachable (the wheel moves the page — the
        // original bug's default auto-shrink never scrolled at all).
        let below_digest =
            |rects: &[egui::Rect]| -> Vec<f32> { rects.iter().map(|r| r.max.y).collect() };
        assert_ne!(
            below_digest(&settled),
            below_digest(&scrolled),
            "the wheel did not move the page — the ScrollArea never scrolls"
        );

        // (c) The body sits on the paper desk: the CentralPanel's
        // cream fill spans from the bar's edge to the screen bottom.
        let desk = scrolled
            .iter()
            .any(|r| r.min.y <= bar + 0.5 && r.max.y >= 480.0 - 0.5 && r.width() > 700.0);
        assert!(
            desk,
            "no paper-cream body panel below the topbar (missing CentralPanel)"
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
