//! The eframe app that drives the flintshot home (app) window.
//!
//! [`HomeApp`] is the Win11-Snipping-Tool-style capture home: one
//! prominent **Capture** control (the single Capture action,
//! native-capture-ux — the OS's own selection UI where the backend
//! supports it, else the editor flow), an **Open** control (native
//! file dialog → the picked image routes the same window to its
//! editor page, single-window), and a **Paste** control (clipboard
//! image → the editor page). This crate stays IPC-free — the window
//! talks to the world exclusively through [`HomeBridge`], which the
//! app crate implements (subscription pump + routing live there).
//!
//! Close/reopen contract (ADR-0047): a Capture click asks the
//! bridge to start the capture and then sends
//! [`egui::ViewportCommand::Close`] — a destroyed window cannot be
//! captured, on every compositor (winit's Wayland `set_visible` is
//! a hard no-op, so hiding is not a portable guarantee). The caller
//! (which owns the process) waits out the unmap grace, delegates,
//! and reopens a fresh window on [`HomeDirective::Show`]. **Open**
//! does *not* close the window: the editor runs elsewhere (its own
//! process — see [`HomeBridge::open_image`]), so there is nothing
//! to keep out of a shot. While the window is open, a standing
//! `request_repaint_after` keeps directives (status, focus pings)
//! flowing even when idle.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use eframe::App;
use egui::{Context, Key, KeyboardShortcut, Modifiers, ViewportCommand};

/// How often an idle home window re-polls directives (status
/// changes, focus pings arrive independent of input). Shared by the
/// editor page's directive pump (single-window).
pub(crate) const REPOLL_INTERVAL: Duration = Duration::from_millis(250);

/// How long after a Capture click the window re-issues its close
/// command if it is somehow still alive. A dropped
/// `ViewportCommand::Close` strands the armed capture intent — the
/// outer loop only delegates once the window dies (ADR-0047), so a
/// stuck close wedges the whole flow until the user closes the
/// window by hand. The repaint heartbeat guarantees the check runs
/// within one [`REPOLL_INTERVAL`] of the delay elapsing.
const CLOSE_WATCHDOG_DELAY: Duration = Duration::from_millis(500);

/// Whether the **Paste** control renders (and its `P` accelerator
/// arms). Live since the single-window effort: the clipboard image
/// routes to the editor page in-process (see
/// [`HomeBridge::request_paste`]).
const SHOW_PASTE: bool = true;

/// The hero's pop-in ramp, seconds.
const HERO_POP_IN_SECONDS: f32 = 0.25;

/// Thumbnail card edge, logical px (decoded at 2× for crispness).
const THUMB_EDGE: f32 = 96.0;

/// Welcome grain seed — any fixed value.
const WELCOME_GRAIN_SEED: u64 = 0xFEED;

/// A directive from the app crate into the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeDirective {
    /// The window should be (re)opened / brought forward: the
    /// capture session ended, the daemon went away, or someone
    /// asked for the window (focus ping). While a window is open
    /// this is just a focus; the caller's outer loop treats it as
    /// the reopen signal when the window is closed (ADR-0047).
    Show,
    /// The window must close now — a capture is imminent and it may
    /// not be in the shot. Covers **external** triggers too (the
    /// pre-grab `HideForCapture` handshake and any `CaptureStarted`),
    /// closing the ADR-0047 gap where only self-triggered captures
    /// hid the window. The outer loop distinguishes this close from
    /// a user quit and reopens on the session's end.
    Hide,
    /// Replace the status line text (daemon/connection state).
    SetStatus(String),
}

/// The home window's only seam to the outside world. Implemented by
/// the app crate; every method runs on the UI thread.
pub trait HomeBridge {
    /// Start the single Capture action (the window closes itself
    /// right after; the implementation owns the grace + delegation +
    /// the reopen decision).
    fn request_capture(&mut self);

    /// Open `path` in the editor — the `flintshot open` path. Returns
    /// whether the image routed (`true`: the window closes right
    /// after and the editor takes over as the same window;
    /// `false`: the file could not be loaded — a status line
    /// explains, and the window stays exactly as it was). The decode
    /// runs on the UI thread right here (after the blocking `rfd`
    /// dialog returns).
    fn open_image(&mut self, path: PathBuf) -> bool;

    /// Paste: read the system clipboard's image and route to the
    /// editor page. Returns whether an image routed (`true`: the
    /// window closes right after, exactly like [`Self::open_image`]);
    /// `false` — no image on the clipboard — is a status-line
    /// message, the window stays.
    fn request_paste(&mut self) -> bool;

    /// Take one pending directive, oldest first. Called every
    /// frame (see [`REPOLL_INTERVAL`]).
    fn poll_directive(&mut self) -> Option<HomeDirective>;

    /// Recent image paths for the welcome grid, most recent first
    /// (the app's bridge answers from `flintshot_core::recents`).
    /// Default: none — test doubles and minimal bridges.
    fn recent_images(&mut self) -> Vec<PathBuf> {
        Vec::new()
    }

    // -- Settings-route plumbing (defaults keep test doubles
    //    compiling; the app crate's real bridge overrides these) --

    /// Load the current config. Default: built-in defaults.
    fn load_config(&mut self) -> flintshot_core::Config {
        flintshot_core::Config::default()
    }

    /// Persist the config through the existing save + watcher
    /// round-trip. Default: no-op.
    fn save_config(&mut self, _config: &flintshot_core::Config) {}

    /// The detected display server, for the System section's
    /// readonly row. Default: `"unknown"`.
    fn display_server(&mut self) -> String {
        "unknown".to_owned()
    }

    /// The running version, for the System section's readonly row.
    /// Default: `"dev"`.
    fn version(&mut self) -> String {
        "dev".to_owned()
    }

    /// Open the user's config file in their editor
    /// (`$EDITOR`/`$VISUAL`, template-materialized first when the
    /// file is missing). The *Edit config file* link's action;
    /// errors are the implementation's to log — the page never
    /// blocks on it. Default: no-op.
    fn open_config_file(&mut self) {}

    /// The fully-commented default config TOML, rendered read-only
    /// by the *View default config* link. Default: the embedded
    /// asset.
    fn default_config_toml(&mut self) -> String {
        flintshot_core::default_config_toml().to_owned()
    }

    /// Whether the session is Wayland — gates the settings page's
    /// *Rebind shortcuts…* row (the portal bind dialog is
    /// Wayland-only; X11/macOS/Windows re-register hotkeys from
    /// `[hotkeys]` directly and need no bind step). Default:
    /// `false`.
    fn session_is_wayland(&mut self) -> bool {
        false
    }

    /// Open the xdg-desktop-portal `GlobalShortcuts` bind dialog
    /// through the running daemon (the *Rebind shortcuts…* row's
    /// action). The implementation owns the IPC + outcome logging.
    /// Default: no-op.
    fn request_rebind(&mut self) {}
}

/// Recents per grid row — the mockup's 4-column layout.
const RECENTS_PER_ROW: usize = 4;

/// The eframe app for the home window. Owns no IPC state — see
/// [`HomeBridge`].
pub(crate) struct HomeApp {
    /// Implementation-supplied world (pump, delegation).
    bridge: Box<dyn HomeBridge>,
    /// Recent image paths for the welcome grid (refreshed at
    /// construction, on `Show`, and after a routed image).
    recent: Vec<PathBuf>,
    /// Decoded thumbnails keyed by path — in-memory only, never
    /// persisted (the recents index stores paths alone).
    thumbs: HashMap<PathBuf, egui::TextureHandle>,
    /// The welcome's age, seconds — drives the hero's spring
    /// pop-in and stops once past the ramp.
    age: f32,
    /// The grain tile for the welcome background.
    grain: Option<egui::TextureHandle>,
    /// The last drop already routed (egui holds dropped files
    /// across frames; each file must route exactly once).
    last_drop: Option<PathBuf>,
    /// Pure UI state, kept separate so directive application is
    /// headless-testable.
    ui: HomeUiState,
    /// Keyboard shortcut: C → capture.
    capture_shortcut: KeyboardShortcut,
    /// Keyboard shortcut: O → open.
    open_shortcut: KeyboardShortcut,
    /// Keyboard shortcut: P → paste (only consumed while
    /// [`SHOW_PASTE`] is `true`).
    paste_shortcut: KeyboardShortcut,
    /// When the Capture control was last clicked — the close that
    /// must follow it is armed. Feeds the stuck-close watchdog; the
    /// latch never clears (the window dies instead).
    capture_clicked_at: Option<std::time::Instant>,
    /// The watchdog has warned once (subsequent re-issues are silent
    /// repeats of an idempotent command).
    close_watchdog_warned: bool,
}

/// Pure UI state of the home window — the part directives mutate.
/// Extracted from [`HomeApp`] so the transition logic is
/// unit-testable without an egui context.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HomeUiState {
    /// Status line contents.
    status: String,
}

impl HomeUiState {
    /// Apply one directive (the status line is the only mutable
    /// display state — visibility is the caller's close/reopen
    /// business, per ADR-0047).
    fn apply(&mut self, directive: &HomeDirective) {
        if let HomeDirective::SetStatus(text) = directive {
            self.status.clone_from(text);
        }
    }
}

impl HomeApp {
    /// Construct the app over a bridge. Pure: no egui context is
    /// touched until the first `update`.
    pub(crate) fn new(mut bridge: Box<dyn HomeBridge>, initial_status: String) -> Self {
        let recent = bridge.recent_images();
        Self {
            ui: HomeUiState {
                status: initial_status,
            },
            recent,
            thumbs: HashMap::new(),
            age: 0.0,
            grain: None,
            last_drop: None,
            bridge,
            capture_shortcut: KeyboardShortcut::new(Modifiers::NONE, Key::C),
            open_shortcut: KeyboardShortcut::new(Modifiers::NONE, Key::O),
            paste_shortcut: KeyboardShortcut::new(Modifiers::NONE, Key::P),
            capture_clicked_at: None,
            close_watchdog_warned: false,
        }
    }

    /// Start the Capture action through the bridge and close the
    /// window — the not-in-the-shot guarantee (ADR-0047). Shared by
    /// the Capture button and its keyboard accelerator. Order
    /// matters: the intent must be registered before the close tears
    /// this app down.
    pub(crate) fn start_capture(&mut self, ctx: &Context) {
        self.bridge.request_capture();
        self.capture_clicked_at = Some(std::time::Instant::now());
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    // -- Settings-route passthroughs (the router drives the page) --

    pub(crate) fn load_config(&mut self) -> flintshot_core::Config {
        self.bridge.load_config()
    }

    pub(crate) fn save_config(&mut self, config: &flintshot_core::Config) {
        self.bridge.save_config(config);
    }

    pub(crate) fn display_server(&mut self) -> String {
        self.bridge.display_server()
    }

    pub(crate) fn version(&mut self) -> String {
        self.bridge.version()
    }

    pub(crate) fn open_config_file(&mut self) {
        self.bridge.open_config_file();
    }

    pub(crate) fn default_config_toml(&mut self) -> String {
        self.bridge.default_config_toml()
    }

    pub(crate) fn session_is_wayland(&mut self) -> bool {
        self.bridge.session_is_wayland()
    }

    pub(crate) fn request_rebind(&mut self) {
        self.bridge.request_rebind();
    }

    /// The status line's current text (the header's overflow menu
    /// surfaces it on the Home route).
    pub(crate) fn status_line(&self) -> String {
        self.ui.status.clone()
    }

    /// Poll one directive from the home bridge — the window router
    /// owns directive handling on the Loading route, where no route
    /// app is live (see `crate::window`).
    pub(crate) fn poll_directive(&mut self) -> Option<HomeDirective> {
        self.bridge.poll_directive()
    }

    /// The Open control: native file dialog, then hand the picked
    /// path to the bridge. Cancelling the dialog is a plain no-op —
    /// the window stays exactly as it was. A routed image also keeps
    /// the window: the bridge arms the editor route and the window
    /// router swaps it in-place (editor-capture-ux Task 006 — the
    /// surface identity survives every non-capture transition).
    pub(crate) fn handle_open(&mut self, _ctx: &Context) {
        if let Some(path) = pick_image_path() {
            let _ = self.bridge.open_image(path);
        }
    }
}

/// Prompt for an image to open via `rfd`. Reuses the editor
/// save path's blocking-modal pattern (`crates/flintshot-editor-egui`
/// already depends on `rfd`; the eframe event loop is already
/// blocking, and the dialog is what the user is driving anyway).
/// `None` when the user cancels. Shared by the home's Open control
/// and the editor page's chrome action (single-window).
pub(crate) fn pick_image_path() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Image files", &["png", "jpg", "jpeg", "webp", "bmp"])
        .pick_file()
}

/// Whether the stuck-close watchdog fires at `now` for a Capture
/// click at `clicked_at`. Pure so the bound is unit-testable without
/// sleeping.
fn close_watchdog_overdue(clicked_at: std::time::Instant, now: std::time::Instant) -> bool {
    now.duration_since(clicked_at) >= CLOSE_WATCHDOG_DELAY
}

impl App for HomeApp {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.logic_impl(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui_impl(ui);
    }
}

/// The recents caption: file stem + relative age ("shot · 5 mins
/// ago"). Pure for tests.
fn recent_caption(path: &std::path::Path) -> String {
    let stem = path.file_stem().map_or_else(
        || "capture".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    let age = age_seconds(path).unwrap_or(u64::MAX);
    format!("{stem} · {}", relative_time(age))
}

/// Seconds since `path` was last modified. `None` when the file is
/// gone or its mtime predates the epoch's representable range.
fn age_seconds(path: &std::path::Path) -> Option<u64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let age = std::time::SystemTime::now().duration_since(modified).ok()?;
    Some(age.as_secs())
}

/// Relative-time caption with the spec's thresholds: now (under a
/// minute) · mins · hours · yesterday · days. Pure for tests.
pub(crate) fn relative_time(secs: u64) -> &'static str {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    const YESTERDAY: u64 = 2 * DAY;
    match secs {
        s if s < MINUTE => "now",
        s if s < HOUR => "mins ago",
        s if s < DAY => "hours ago",
        s if s < YESTERDAY => "yesterday",
        _ => "days ago",
    }
}

impl HomeApp {
    /// Headless-callable core of [`App::logic`]: directive polling and
    /// keyboard accelerators. Lives in `logic` (not `ui`) because
    /// eframe calls it even without recent input. `pub(crate)`: the
    /// window router (`crate::window`) delegates here on the Home
    /// route.
    pub(crate) fn logic_impl(&mut self, ctx: &Context) {
        // Directives (status changes, focus pings) arrive independent
        // of input; keep a slow repaint heartbeat so they land.
        ctx.request_repaint_after(REPOLL_INTERVAL);

        while let Some(directive) = self.bridge.poll_directive() {
            let focus = matches!(directive, HomeDirective::Show);
            let hide = matches!(directive, HomeDirective::Hide);
            if focus {
                // Re-focus is the cheap refresh point: captures the
                // saves that happened while the editor page was up.
                self.recent = self.bridge.recent_images();
            }
            self.ui.apply(&directive);
            if hide {
                // Not-in-the-shot for externally triggered captures:
                // the same close the self-trigger path performs, on
                // the daemon's pre-grab say-so (ADR-0047 gap).
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            if focus {
                // The window is open by construction here (a closed
                // one has no frames to run); Show while open is the
                // focus ping — raise and focus.
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            }
        }

        self.run_close_watchdog(ctx);

        if ctx.input_mut(|i| i.consume_shortcut(&self.capture_shortcut)) {
            self.start_capture(ctx);
        } else if ctx.input_mut(|i| i.consume_shortcut(&self.open_shortcut)) {
            self.handle_open(ctx);
        } else if SHOW_PASTE && ctx.input_mut(|i| i.consume_shortcut(&self.paste_shortcut)) {
            // A routed paste keeps the window: the router swaps the
            // editor route in-place (same no-close contract as the
            // Open control).
            let _ = self.bridge.request_paste();
        }
    }

    /// Re-issue the close when the window outlives its Capture click
    /// by [`CLOSE_WATCHDOG_DELAY`] — a compositor dropping the first
    /// `ViewportCommand::Close` would otherwise strand the armed
    /// capture intent (the outer loop delegates only once the window
    /// dies). Warns once; the re-issue itself repeats every frame —
    /// idempotent, and the window's death is the only real exit.
    fn run_close_watchdog(&mut self, ctx: &Context) {
        let Some(clicked_at) = self.capture_clicked_at else {
            return;
        };
        if !close_watchdog_overdue(clicked_at, std::time::Instant::now()) {
            return;
        }
        if !self.close_watchdog_warned {
            self.close_watchdog_warned = true;
            tracing::warn!("home window still alive after a Capture click; re-issuing the close");
        }
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    /// Headless-callable core of [`App::ui`] (mirrors `EditorApp`'s
    /// `logic_impl` / `ui_impl` split). `pub(crate)`: the window
    /// router delegates here on the Home route.
    ///
    /// The welcome (Gradia pattern #1 on the pop-up-book skin):
    /// grain-tiled cream, a forest hero card with the spring
    /// pop-in, Open / Paste secondary cards, and the recents grid
    /// — every state renders content, never blank.
    pub(crate) fn ui_impl(&mut self, ui: &mut egui::Ui) {
        // Whole-page drop target: any dropped image file routes
        // through the existing open path (best-effort — winit's
        // Wayland DND varies; open/paste/recents keep the page
        // complete regardless).
        self.handle_dropped_files(ui.ctx());
        let dt = ui.ctx().input(|i| i.unstable_dt);
        self.age = (self.age + dt).min(HERO_POP_IN_SECONDS);

        let panel = egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(crate::theme::palette::PAPER_CREAM)
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ui, |ui| {
                self.paint_grain(ui);
                self.welcome_body(ui);
            });
        let _ = panel;
    }

    /// The welcome body: hero, secondaries, recents grid / empty
    /// card, and the bottom status line (ADR-0047's watchdog
    /// contract lives above this in `logic_impl`).
    fn welcome_body(&mut self, ui: &mut egui::Ui) {
        ui.add_space(crate::theme::SPACE_SCALE[2]);
        self.hero_card(ui);
        ui.add_space(crate::theme::SPACE_SCALE[2]);
        self.secondary_cards(ui);
        ui.add_space(crate::theme::SPACE_SCALE[3]);
        self.recents_section(ui);
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.label(
                egui::RichText::new(&self.ui.status)
                    .color(crate::theme::palette::INK_SOFT)
                    .size(crate::theme::TYPE_SM),
            );
        });
    }

    /// The hero: a forest-filled cutout card with the 8 px white
    /// border, spring pop-in on first open, "Take a screenshot" in
    /// the display weight. Click (or the C accelerator, handled in
    /// `logic_impl`) fires the existing capture flow.
    fn hero_card(&mut self, ui: &mut egui::Ui) {
        use crate::theme::palette as p;
        let eased = crate::theme::ease::spring(self.age / HERO_POP_IN_SECONDS);
        let eased = if self.age >= HERO_POP_IN_SECONDS {
            1.0
        } else {
            eased.clamp(0.0, 1.08)
        };
        // Pop-in: the card grows from an inset; overshoot lets it
        // breathe slightly past the resting size.
        let inset = ((1.0 - eased) * 40.0).max(0.0);
        let width = ui.available_width().max(320.0);
        let full = egui::vec2(width, 120.0);
        let (rect, _) = ui.allocate_exact_size(full, egui::Sense::hover());
        let card = rect.shrink(inset);
        let resp = ui
            .interact(rect, egui::Id::new("hero-capture"), egui::Sense::click())
            .on_hover_text("Capture · C");
        let radius = egui::CornerRadius::same(crate::theme::CUTOUT_RADIUS);
        // The paper shadow: the hero is a card lying on the desk.
        ui.painter()
            .add(crate::theme::SHADOW_PAPER.as_shape(card, radius));
        ui.painter().rect(
            card,
            radius,
            p::FOREST_DEEP,
            egui::Stroke::NONE,
            egui::StrokeKind::Inside,
        );
        // The mockup's white inner outline (inset, 2.5 px).
        let outline = card.shrink(6.0);
        ui.painter().rect(
            outline,
            radius,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(2.5, p::CREAM_TEXT),
            egui::StrokeKind::Inside,
        );
        // The rotated flame badge.
        let flame_center = egui::pos2(card.left() + 48.0, card.center().y);
        let (sin, cos) = (-8.0_f32.to_radians()).sin_cos();
        let rot = |dx: f32, dy: f32| {
            egui::pos2(
                flame_center.x + dx * cos - dy * sin,
                flame_center.y + dx * sin + dy * cos,
            )
        };
        let badge = [
            rot(-12.0, -12.0),
            rot(12.0, -12.0),
            rot(12.0, 12.0),
            rot(-12.0, 12.0),
        ];
        ui.painter().add(egui::Shape::convex_polygon(
            badge.to_vec(),
            p::GOLD,
            egui::Stroke::NONE,
        ));
        ui.painter().text(
            flame_center,
            egui::Align2::CENTER_CENTER,
            "🔥",
            egui::FontId::proportional(crate::theme::TYPE_SM),
            p::FOREST_INK,
        );
        // The headline with the gold emphasis.
        let lead = ui.painter().layout_no_wrap(
            "Capture. Annotate. Hand it off — ".to_owned(),
            egui::FontId::new(
                crate::theme::TYPE_XL,
                egui::FontFamily::Name(crate::theme::FAMILY_DISPLAY.into()),
            ),
            p::CREAM_TEXT,
        );
        let gold = ui.painter().layout_no_wrap(
            "in seconds.".to_owned(),
            egui::FontId::new(
                crate::theme::TYPE_XL,
                egui::FontFamily::Name(crate::theme::FAMILY_DISPLAY.into()),
            ),
            p::GOLD,
        );
        let text_w = lead.size().x + gold.size().x;
        let lead_h = lead.size().y;
        let base = egui::pos2(
            (card.center().x - text_w / 2.0).max(outline.left() + 60.0),
            card.center().y - lead_h / 2.0,
        );
        ui.painter().galley(base, lead.clone(), p::CREAM_TEXT);
        ui.painter()
            .galley(base + egui::vec2(lead.size().x, 0.0), gold, p::GOLD);
        if resp.clicked() {
            let ctx = ui.ctx().clone();
            self.start_capture(&ctx);
        }
    }

    /// The three mockup hcards: New capture / Open image / Paste —
    /// cream cutouts with a hover lift.
    fn secondary_cards(&mut self, ui: &mut egui::Ui) {
        use crate::theme::palette as p;
        ui.horizontal(|ui| {
            for (id, icon, label, hint) in [
                ("home-capture", "✚", "New capture", " · C"),
                ("home-open", "🗀", "Open image", " · O"),
                ("home-paste", "📋", "Paste", " · P"),
            ] {
                let galley = ui.painter().layout_no_wrap(
                    format!("{label}{hint}"),
                    egui::FontId::proportional(crate::theme::TYPE_MD),
                    p::INK,
                );
                let w = 16.0 + 20.0 + 8.0 + galley.size().x + 16.0;
                let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 40.0), egui::Sense::hover());
                let resp = ui
                    .interact(rect, egui::Id::new(id), egui::Sense::click())
                    .on_hover_text(label);
                // Hover lift: 2 px up, a touch wider.
                let lift = if resp.hovered() { 2.0 } else { 0.0 };
                let card = rect.translate(egui::vec2(0.0, -lift));
                ui.painter().add(if resp.hovered() {
                    crate::theme::SHADOW_LIFT
                        .as_shape(card, egui::CornerRadius::same(crate::theme::POD_RADIUS))
                } else {
                    crate::theme::SHADOW_RESTING
                        .as_shape(card, egui::CornerRadius::same(crate::theme::POD_RADIUS))
                });
                ui.painter().rect(
                    card,
                    egui::CornerRadius::same(crate::theme::POD_RADIUS),
                    if resp.hovered() {
                        p::with_alpha(p::GOLD, 30)
                    } else {
                        p::PAPER_CREAM
                    },
                    if resp.hovered() {
                        egui::Stroke::new(1.25, p::FOREST)
                    } else {
                        crate::theme::HAIRLINE_STROKE
                    },
                    egui::StrokeKind::Middle,
                );
                ui.painter().text(
                    egui::pos2(card.left() + 16.0, card.center().y),
                    egui::Align2::LEFT_CENTER,
                    icon,
                    egui::FontId::proportional(crate::theme::TYPE_MD),
                    p::INK,
                );
                let pos = egui::pos2(
                    card.left() + 16.0 + 20.0 + 8.0,
                    card.center().y - galley.size().y / 2.0,
                );
                ui.painter().galley(pos, galley, p::INK);
                if resp.clicked() {
                    let ctx = ui.ctx().clone();
                    match id {
                        "home-capture" => self.start_capture(&ctx),
                        "home-open" => self.handle_open(&ctx),
                        _ => {
                            let _ = self.bridge.request_paste();
                        }
                    }
                }
            }
        });
    }

    /// The recents grid (with thumbnails decoded + downscaled on
    /// demand) or, when empty, the friendly empty card — never
    /// blank space. Dead paths render nothing (pruned at load;
    /// undecodable files skip their card silently).
    fn recents_section(&mut self, ui: &mut egui::Ui) {
        use crate::theme::palette as p;
        ui.label(
            egui::RichText::new("RECENT CAPTURES")
                .size(crate::theme::TYPE_SM)
                .color(p::INK_SOFT),
        );
        if self.recent.is_empty() {
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width().min(420.0), 64.0),
                egui::Sense::hover(),
            );
            let _ = ui.interact(rect, egui::Id::new("recents-empty"), egui::Sense::hover());
            ui.painter().rect(
                rect,
                egui::CornerRadius::same(crate::theme::CUTOUT_RADIUS),
                crate::theme::palette::PAPER_CREAM,
                crate::theme::HAIRLINE_STROKE,
                egui::StrokeKind::Middle,
            );
            let galley = ui.painter().layout_no_wrap(
                "No captures yet — your recent shots will land here".to_owned(),
                egui::FontId::proportional(crate::theme::TYPE_SM),
                crate::theme::palette::INK_SOFT,
            );
            let pos = egui::pos2(
                rect.center().x - galley.size().x / 2.0,
                rect.center().y - galley.size().y / 2.0,
            );
            ui.painter()
                .galley(pos, galley, crate::theme::palette::INK_SOFT);
            return;
        }
        // The mockup's 4-column grid: fixed-width cards chunked
        // into rows of four.
        let recent = self.recent.clone();
        for row in recent.chunks(RECENTS_PER_ROW) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing =
                    egui::vec2(crate::theme::SPACE_SCALE[2], crate::theme::SPACE_SCALE[2]);
                for path in row {
                    self.recent_card(ui, path);
                }
            });
        }
    }

    /// One recent's thumb card: letterboxed thumbnail on white,
    /// click routes through the existing `open_image` path.
    fn recent_card(&mut self, ui: &mut egui::Ui, path: &PathBuf) {
        let Some(texture) = self.thumbnail(ui.ctx(), path) else {
            return;
        };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(112.0, 112.0), egui::Sense::hover());
        let resp = ui
            .interact(
                rect,
                egui::Id::new(("recent", path.to_string_lossy().to_string())),
                egui::Sense::click(),
            )
            .on_hover_text(path.to_string_lossy().to_string());
        ui.painter().rect(
            rect,
            egui::CornerRadius::same(crate::theme::CUTOUT_RADIUS),
            crate::theme::palette::PAPER_CREAM,
            if resp.hovered() {
                egui::Stroke::new(1.25, crate::theme::palette::FOREST)
            } else {
                crate::theme::HAIRLINE_STROKE
            },
            egui::StrokeKind::Middle,
        );
        // Caption: file stem + relative time, ink-soft, under the
        // thumb. Reserves 16 px inside the card.
        let caption = recent_caption(path);
        let caption_galley = ui.painter().layout_no_wrap(
            caption,
            egui::FontId::proportional(crate::theme::TYPE_XS),
            crate::theme::palette::INK_SOFT,
        );
        // Letterbox: the fitted image centers in the card.
        let thumb = texture.size_vec2();
        let (tw, th) = (thumb.x, thumb.y);
        let scale = 80.0 / tw.max(th);
        let size = egui::vec2(tw * scale, th * scale);
        let thumb_center = egui::pos2(rect.center().x, rect.center().y - 6.0);
        let img_rect = egui::Rect::from_center_size(thumb_center, size);
        ui.painter().image(
            texture.id(),
            img_rect,
            egui::Rect {
                min: egui::pos2(0.0, 0.0),
                max: egui::pos2(1.0, 1.0),
            },
            crate::theme::palette::PAPER_CREAM,
        );
        ui.painter().galley(
            egui::pos2(
                rect.center().x - caption_galley.size().x / 2.0,
                rect.bottom() - 18.0,
            ),
            caption_galley,
            crate::theme::palette::INK_SOFT,
        );
        if resp.clicked() {
            let _ = self.bridge.open_image(path.clone());
            self.recent = self.bridge.recent_images();
        }
    }

    /// Decode + downscale a recent's thumbnail on demand, cached
    /// in memory for the session. `None` when the file is gone or
    /// undecodable — the card silently skips.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "thumbnail dims are ≤ 2× the card edge by construction; u32 dims ≤ 2^24 are exact in f32"
    )]
    fn thumbnail(&mut self, ctx: &egui::Context, path: &PathBuf) -> Option<&egui::TextureHandle> {
        if !self.thumbs.contains_key(path) {
            let img = image::open(path).ok()?;
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            // Downscale to 2× the card edge (crispness), preserving
            // aspect; smaller images stay 1:1.
            let fit = THUMB_EDGE * 2.0;
            let s = fit / (w.max(h) as f32);
            let (tw, th) = if s < 1.0 {
                (
                    (((w as f32) * s).round() as u32).max(1),
                    (((h as f32) * s).round() as u32).max(1),
                )
            } else {
                (w, h)
            };
            let same_size = tw == w && th == h;
            let small = if same_size {
                rgba
            } else {
                image::imageops::resize(&rgba, tw, th, image::imageops::FilterType::Triangle)
            };
            let pixels = small
                .pixels()
                .map(|p| crate::theme::palette::premultiplied(p.0[0], p.0[1], p.0[2], p.0[3]))
                .collect::<Vec<_>>();
            let image = egui::ColorImage::new([tw as usize, th as usize], pixels);
            let handle = ctx.load_texture(
                format!("recent-{}", path.to_string_lossy()),
                image,
                egui::TextureOptions::default(),
            );
            self.thumbs.insert(path.clone(), handle);
        }
        self.thumbs.get(path)
    }

    /// Tile the grain over the welcome background — the first
    /// paint call inside the panel so every card sits above it.
    fn paint_grain(&mut self, ui: &mut egui::Ui) {
        if self.grain.is_none() {
            let image =
                egui::ColorImage::new([64, 64], crate::theme::grain::tile(WELCOME_GRAIN_SEED, 64));
            let options = egui::TextureOptions {
                wrap_mode: egui::TextureWrapMode::Repeat,
                ..Default::default()
            };
            self.grain = Some(ui.ctx().load_texture("welcome-grain", image, options));
        }
        if let Some(texture) = self.grain.as_ref() {
            crate::theme::grain::paint_grain(ui.painter(), ui.max_rect(), texture);
        }
    }

    /// Route a dropped image file through the existing open path —
    /// once per file (egui holds drops across frames).
    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone()));
        if dropped == self.last_drop {
            return;
        }
        if let Some(path) = dropped {
            self.last_drop = Some(path.clone());
            let _ = self.bridge.open_image(path);
            self.recent = self.bridge.recent_images();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    fn ui_state() -> HomeUiState {
        HomeUiState {
            status: "connected".into(),
        }
    }

    #[test]
    fn show_directive_leaves_display_state_alone() {
        // Visibility is the caller's close/reopen business (ADR-0047);
        // the UI only owns the status line.
        let mut ui = ui_state();
        ui.apply(&HomeDirective::Show);
        assert_eq!(ui.status, "connected");
    }

    #[test]
    fn status_directive_replaces_text() {
        let mut ui = ui_state();
        ui.apply(&HomeDirective::SetStatus("daemon went away".into()));
        assert_eq!(ui.status, "daemon went away");
    }

    #[test]
    fn hide_directive_leaves_display_state_alone() {
        let mut ui = ui_state();
        ui.apply(&HomeDirective::Hide);
        assert_eq!(ui.status, "connected");
    }

    #[test]
    fn hide_directive_closes_the_window_headlessly() {
        // A default context stands in for the real one: the close
        // command queues without a display (the CloseHandle test
        // pattern); what is under test is that the Hide branch is
        // reached at all — a panic or a missed branch fails this.
        let bridge = RecordingBridge::default();
        bridge
            .pending
            .lock()
            .expect("pending mutex")
            .push_back(HomeDirective::Hide);
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        app.logic_impl(&ctx);
        // The directive was consumed (not skipped as unknown).
        assert!(
            app.bridge.poll_directive().is_none(),
            "the Hide directive was consumed by the frame"
        );
    }

    #[test]
    fn close_watchdog_bound_is_half_a_second() {
        // Long enough that a busy frame never trips it on a healthy
        // close, short enough that a wedged window recovers in
        // ~an instant of user time.
        assert_eq!(CLOSE_WATCHDOG_DELAY, Duration::from_millis(500));
    }

    #[test]
    fn close_watchdog_fires_only_past_the_bound() {
        let t0 = std::time::Instant::now();
        assert!(!close_watchdog_overdue(t0, t0 + Duration::from_millis(499)));
        assert!(close_watchdog_overdue(t0, t0 + Duration::from_millis(500)));
    }

    #[test]
    fn an_aged_capture_click_runs_the_watchdog_without_panicking() {
        // Headless shape of the stuck close: the latch aged past the
        // bound, a default context stands in for the real one — the
        // re-issued Close queues, the frame completes. A panic (e.g.
        // a None mishandled) fails this.
        let bridge = RecordingBridge::default();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        app.capture_clicked_at = Some(
            std::time::Instant::now()
                .checked_sub(CLOSE_WATCHDOG_DELAY)
                .expect("an Instant older than 500 ms always exists on this platform"),
        );
        let ctx = egui::Context::default();
        app.logic_impl(&ctx);
        assert!(app.close_watchdog_warned, "the overdue latch warned");
    }

    #[test]
    fn a_fresh_capture_click_does_not_fire_the_watchdog() {
        let bridge = RecordingBridge::default();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        app.capture_clicked_at = Some(std::time::Instant::now());
        let ctx = egui::Context::default();
        app.logic_impl(&ctx);
        assert!(!app.close_watchdog_warned, "within the bound: quiet");
    }

    /// Pinned: the Paste control is live on this branch (single-window
    /// effort) — its route is [`HomeBridge::request_paste`]. Flipping
    /// [`SHOW_PASTE`] off breaks this test on purpose.
    #[test]
    #[expect(
        clippy::assertions_on_constants,
        reason = "the constant IS the pinned contract; the test exists to fail on flip"
    )]
    fn show_paste_const_is_true_on_this_branch() {
        assert!(SHOW_PASTE);
    }

    /// Shared recorder so tests can inspect a bridge handed to
    /// `HomeApp` as a `Box<dyn HomeBridge>` (no downcast needed).
    #[derive(Default, Clone)]
    struct Recording(Arc<Mutex<Recorder>>);

    #[derive(Default)]
    struct Recorder {
        capture_requests: usize,
        opened: Vec<PathBuf>,
        paste_requests: usize,
        /// Whether the paste double "has an image on the clipboard".
        clipboard_has_image: bool,
    }

    /// Bridge double that records requests and replays directives.
    #[derive(Default, Clone)]
    struct RecordingBridge {
        rec: Recording,
        pending: Arc<Mutex<VecDeque<HomeDirective>>>,
        recents: Vec<PathBuf>,
    }

    impl HomeBridge for RecordingBridge {
        fn request_capture(&mut self) {
            self.rec.0.lock().expect("recorder mutex").capture_requests += 1;
        }

        fn open_image(&mut self, path: PathBuf) -> bool {
            let mut rec = self.rec.0.lock().expect("recorder mutex");
            rec.opened.push(path);
            // Fail (false) for paths containing "bad", route otherwise
            // — lets one double cover both contract arms.
            !rec.opened
                .last()
                .is_some_and(|p| p.to_string_lossy().contains("bad"))
        }

        fn request_paste(&mut self) -> bool {
            let mut rec = self.rec.0.lock().expect("recorder mutex");
            rec.paste_requests += 1;
            rec.clipboard_has_image
        }

        fn poll_directive(&mut self) -> Option<HomeDirective> {
            self.pending.lock().expect("pending mutex").pop_front()
        }

        fn recent_images(&mut self) -> Vec<PathBuf> {
            self.recents.clone()
        }
    }

    #[test]
    fn bridge_requests_flow_through() {
        let bridge = RecordingBridge::default();
        let rec = bridge.rec.clone();
        let mut boxed: Box<dyn HomeBridge> = Box::new(bridge);
        boxed.request_capture();
        assert!(boxed.open_image(PathBuf::from("/tmp/shot.png")));
        boxed.request_paste();
        let guard = rec.0.lock().expect("recorder mutex");
        assert_eq!(guard.capture_requests, 1);
        assert_eq!(guard.opened, [PathBuf::from("/tmp/shot.png")]);
        assert_eq!(guard.paste_requests, 1);
    }

    #[test]
    fn open_image_failure_reports_false() {
        // The contract the window acts on: false = the file did not
        // load, the window must NOT close for it.
        let bridge = RecordingBridge::default();
        let mut boxed: Box<dyn HomeBridge> = Box::new(bridge);
        assert!(!boxed.open_image(PathBuf::from("/tmp/bad.png")));
    }

    #[test]
    fn relative_time_hits_every_threshold() {
        assert_eq!(relative_time(0), "now");
        assert_eq!(relative_time(59), "now");
        assert_eq!(relative_time(60), "mins ago");
        assert_eq!(relative_time(3_599), "mins ago");
        assert_eq!(relative_time(3_600), "hours ago");
        assert_eq!(relative_time(86_399), "hours ago");
        assert_eq!(relative_time(86_400), "yesterday");
        assert_eq!(relative_time(172_799), "yesterday");
        assert_eq!(relative_time(172_800), "days ago");
    }

    #[test]
    fn caption_is_stem_plus_relative_time() {
        // A just-modified file reads "now"; the stem is file-stem
        // only (no extension, no directory).
        let dir = std::env::temp_dir();
        let path = dir.join("flintshot_caption_probe.png");
        std::fs::write(&path, b"x").expect("write probe");
        let caption = recent_caption(&path);
        std::fs::remove_file(&path).ok();
        assert!(
            caption.starts_with("flintshot_caption_probe · "),
            "caption starts with the stem, got {caption}"
        );
        assert!(caption.ends_with("now"), "a fresh file reads now");
    }

    #[test]
    fn capture_accelerator_fires_the_bridge_request() {
        // A synthetic C keystroke through a default context must
        // reach the bridge — the accelerator is part of the Capture
        // control's contract.
        let bridge = RecordingBridge::default();
        let rec = bridge.rec.clone();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        ctx.begin_pass(egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::C,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        });
        app.logic_impl(&ctx);
        let _ = ctx.end_pass();
        assert_eq!(
            rec.0.lock().expect("recorder mutex").capture_requests,
            1,
            "the C accelerator must fire request_capture exactly once"
        );
    }

    #[test]
    fn paste_accelerator_fires_the_bridge_request() {
        // Paste is live (single-window): the P accelerator must reach
        // the bridge. A clipboard without an image routes false — no
        // close, the window stays (no window churn on a text
        // clipboard).
        let bridge = RecordingBridge::default();
        let rec = bridge.rec.clone();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let full = ctx.run_ui(
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
            rec.0.lock().expect("recorder mutex").paste_requests,
            1,
            "the P accelerator must fire request_paste exactly once"
        );
        assert!(
            full.viewport_output
                .values()
                .flat_map(|v| v.commands.iter())
                .all(|c| !matches!(c, egui::ViewportCommand::Close)),
            "an empty clipboard must not close the window"
        );
    }

    #[test]
    fn paste_accelerator_keeps_the_window_on_a_routed_image() {
        // A routed paste no longer closes (editor-capture-ux Task
        // 006): the bridge arms the editor route and the window
        // router swaps it in-place.
        let bridge = RecordingBridge::default();
        bridge
            .rec
            .0
            .lock()
            .expect("recorder mutex")
            .clipboard_has_image = true;
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let full = ctx.run_ui(
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
        assert!(
            full.viewport_output
                .values()
                .flat_map(|v| v.commands.iter())
                .all(|c| !matches!(c, egui::ViewportCommand::Close)),
            "a routed paste keeps the window — the router swaps the route"
        );
    }

    // --- Welcome screen (pop-up book Task 010) ---

    fn welcome_frame(
        app: &mut HomeApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        // The hero speaks the display family — installed by
        // `theme::apply` in every real window (the router owns it).
        let raw = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1024.0, 768.0),
            )),
            ..egui::RawInput::default()
        };
        ctx.run_ui(raw, |ui| app.ui_impl(ui))
    }

    /// Every welcome state renders the hero — never blank.
    #[test]
    fn welcome_renders_hero_and_secondaries_in_every_state() {
        let bridge = RecordingBridge::default();
        let rec = bridge.rec.clone();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::apply(&ctx);
        let _ = welcome_frame(&mut app, &ctx, vec![]);
        assert!(
            ctx.read_response(egui::Id::new("hero-capture")).is_some(),
            "the hero card renders"
        );
        assert!(ctx.read_response(egui::Id::new("home-open")).is_some());
        assert!(ctx.read_response(egui::Id::new("home-paste")).is_some());
        assert_eq!(rec.0.lock().expect("recorder mutex").capture_requests, 0);
    }

    /// An empty recents list shows the friendly empty card.
    #[test]
    fn welcome_without_recents_shows_the_empty_card() {
        let mut app = HomeApp::new(Box::new(RecordingBridge::default()), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let _ = welcome_frame(&mut app, &ctx, vec![]);
        assert!(
            ctx.read_response(egui::Id::new("recents-empty")).is_some(),
            "the empty state renders a card, not blank space"
        );
    }

    /// A real recent renders a thumb card; clicking it routes
    /// through the existing open path.
    #[test]
    fn recents_grid_renders_a_real_thumbnail_and_reopens_it() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("shot.png");
        let img = image::RgbaImage::from_pixel(40, 30, image::Rgba([200, 40, 40, 255]));
        img.save(&png).unwrap();

        let bridge = RecordingBridge {
            recents: vec![png.clone()],
            ..RecordingBridge::default()
        };
        let rec = bridge.rec.clone();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let _ = welcome_frame(&mut app, &ctx, vec![]);
        let card = ctx
            .read_response(egui::Id::new(("recent", png.to_string_lossy().to_string())))
            .expect("the thumb card registers for a decodable recent");
        assert!(
            ctx.read_response(egui::Id::new("recents-empty")).is_none(),
            "a non-empty grid renders no empty card"
        );
        let at = card.rect.center();
        let _ = welcome_frame(
            &mut app,
            &ctx,
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
        assert_eq!(
            rec.0.lock().expect("recorder mutex").opened,
            vec![png],
            "clicking a recent routes the existing open_image path"
        );
    }

    /// A dead recent path renders nothing and breaks nothing.
    #[test]
    fn dead_recent_path_is_silently_absent() {
        let bridge = RecordingBridge {
            recents: vec![PathBuf::from("/nonexistent/shot.png")],
            ..RecordingBridge::default()
        };
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let _ = welcome_frame(&mut app, &ctx, vec![]);
        assert!(
            ctx.read_response(egui::Id::new(("recent", "/nonexistent/shot.png")))
                .is_none(),
            "an undecodable recent renders no card"
        );
    }

    /// A dropped image file routes through the existing open path —
    /// exactly once per file.
    #[test]
    fn dropped_image_file_routes_through_open() {
        let bridge = RecordingBridge::default();
        let rec = bridge.rec.clone();
        let drop = PathBuf::from("/tmp/dropped.png");
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let raw = |dropped: bool| egui::RawInput {
            dropped_files: if dropped {
                vec![egui::DroppedFile {
                    path: Some(drop.clone()),
                    ..Default::default()
                }]
            } else {
                Vec::new()
            },
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1024.0, 768.0),
            )),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw(true), |ui| app.ui_impl(ui));
        let _ = ctx.run_ui(raw(true), |ui| app.ui_impl(ui));
        assert_eq!(
            rec.0.lock().expect("recorder mutex").opened,
            vec![drop],
            "the drop routes exactly once while egui holds the file"
        );
    }
}
