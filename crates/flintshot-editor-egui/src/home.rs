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
use std::path::{Path, PathBuf};
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
    /// The window must close now with **user-quit semantics**: the
    /// process exits, the daemon is unaffected, and a mid-editor
    /// session discards exactly as an X / Alt+F4 would — the tray
    /// toggle's hide direction (tray-show-hide-toggle). The outer
    /// loop classifies this close as a plain user quit (no reopen
    /// wait).
    Quit,
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

    /// Resume a session capture (a "This session" card click):
    /// reload it from the session store — image plus committed
    /// annotations — and route to the editor page. Same contract as
    /// [`Self::open_image`]: `true` = routed; `false` = store miss
    /// (the entry was saved or discarded meanwhile), the window
    /// stays as it was. Default: no routing — minimal bridges and
    /// test doubles.
    fn open_session_capture(&mut self, _id: flintshot_core::SessionId) -> bool {
        false
    }

    /// Session cards for the "This session" section, newest first.
    /// Default: none — test doubles and minimal bridges.
    fn session_captures(&mut self) -> Vec<SessionCard> {
        Vec::new()
    }

    /// Discard one session capture (the card's Discard chip, after
    /// its two-step confirm): the entry and its files go. Default:
    /// no-op.
    fn discard_session_capture(&mut self, _id: flintshot_core::SessionId) {}

    /// Take one pending directive, oldest first. Called every
    /// frame (see [`REPOLL_INTERVAL`]).
    fn poll_directive(&mut self) -> Option<HomeDirective>;

    /// Recent image paths for the welcome grid, most recent first
    /// (the app's bridge answers from `flintshot_core::recents`).
    /// Default: none — test doubles and minimal bridges.
    fn recent_images(&mut self) -> Vec<PathBuf> {
        Vec::new()
    }

    // -- Recents card actions (recent-captures-v2; defaults keep
    //    test doubles compiling; the app crate's real bridge
    //    overrides these) --

    /// Copy a recent capture's file to the clipboard (the card's
    /// **Copy** hover chip). The implementation honors the Wayland
    /// fresh-handle rule and keeps the X11 selection alive for the
    /// window's lifetime. Default: no-op.
    fn copy_recent(&mut self, _path: &Path) {}

    /// Reveal a recent capture's parent folder in the platform file
    /// manager (the **Reveal** chip). Default: no-op.
    fn reveal_recent(&mut self, _path: &Path) {}

    /// Delete a recent capture permanently: remove the file from
    /// disk and drop it from the recents index (the **Delete**
    /// chip, after its on-chip "Sure?" confirm). Default: no-op.
    fn delete_recent(&mut self, _path: &Path) {}

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

/// The adaptive grid's column cap — past it, wider windows keep
/// 8 columns instead of growing postage stamps.
const MAX_GRID_COLUMNS: usize = 8;

/// The adaptive grid's narrowest card, logical px. A narrower
/// window gets fewer columns, never a squeezed card.
const MIN_CARD_WIDTH: f32 = 150.0;

/// Adaptive grid columns for `available` logical px: floor-fill at
/// `min_card` (the most whole cards that fit, gap-separated),
/// clamped so a narrow window gets fewer columns and a huge one
/// stops at [`MAX_GRID_COLUMNS`]. Pure; unit-tested.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "floor of a positive ratio is the definition here; usize is the column count"
)]
fn columns_for_width(available: f32, min_card: f32, gap: f32) -> usize {
    let fits = ((available + gap) / (min_card + gap)).floor() as usize;
    fits.clamp(1, MAX_GRID_COLUMNS)
}

/// The card width when `cols` columns share `available` px at `gap`
/// spacing (cards stretch to fill; no dead margin on the right).
/// Pure; unit-tested.
#[expect(
    clippy::cast_precision_loss,
    reason = "column counts are ≤ MAX_GRID_COLUMNS (8) — exact in f32"
)]
fn column_width(available: f32, cols: usize, gap: f32) -> f32 {
    (available - (cols - 1) as f32 * gap) / cols as f32
}

/// A recent card's size on a grid column of `card_w`: the thumbnail
/// box is the card width minus an 8 px inset per side; the caption
/// band keeps its 36 px.
fn recent_card_size(card_w: f32) -> egui::Vec2 {
    egui::vec2(card_w, (card_w - 16.0) + 36.0)
}

/// A decoded recent: the downscaled thumbnail texture plus the
/// metadata the card caption needs (original dimensions, file size,
/// last-modified time) — gathered once at decode, never re-stat'd
/// per frame. The mtime is a decode-time snapshot: a file rewritten
/// in place keeps the cached age until its thumb re-decodes.
pub(crate) struct RecentThumb {
    texture: egui::TextureHandle,
    dims: (u32, u32),
    bytes: u64,
    mtime: u64,
}

/// One resumable session capture for the "This session" grid —
/// the home bridge's face of a `SessionStore` entry.
#[derive(Debug, Clone)]
pub struct SessionCard {
    /// The store entry's id (resume and discard address it).
    pub id: flintshot_core::SessionId,
    /// The on-disk PNG backing the card (thumbnail source; the
    /// store's advisory write may briefly lag the entry).
    pub png_path: PathBuf,
    /// Original image dimensions (the caption's first term).
    pub dims: (u32, u32),
    /// Unix-seconds arrival time (the caption's age term).
    pub created_at: u64,
}

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
    thumbs: HashMap<PathBuf, RecentThumb>,
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
    /// The card whose Delete chip is in the "Sure?" confirm step.
    /// `None` = no armed delete; any click elsewhere disarms.
    delete_armed: Option<PathBuf>,
    /// Session cards for the "This session" grid (refreshed per
    /// frame — entries come and go with editor round-trips, and
    /// the bridge answers from an in-memory index).
    session: Vec<SessionCard>,
    /// The session card whose Discard chip is in the "Sure?"
    /// confirm step. `None` = none armed.
    discard_armed: Option<flintshot_core::SessionId>,
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
        let session = bridge.session_captures();
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
            delete_armed: None,
            session,
            discard_armed: None,
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

/// A recent's display stem: the file name without extension
/// ("capture" when the path has none). Pure.
pub(crate) fn card_stem(path: &Path) -> String {
    path.file_stem().map_or_else(
        || "capture".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    )
}

/// Unix seconds now — the age terms' common `now` (session cards'
/// arrival stamps and the recents thumbs' decode-time mtimes).
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A session card's age, from the store's arrival timestamp (not
/// file mtime — the PNG write lands off-thread).
fn session_age_secs(created_at: u64) -> u64 {
    unix_now().saturating_sub(created_at)
}

/// The card's age line with counts: `now`, `5 mins ago`, `3 hours
/// ago`, `yesterday`, `4 days ago` (singulars collapse to `1 min
/// ago` & co.). The spec's thresholds, counted. Pure for tests.
pub(crate) fn age_label(secs: u64) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    let plural = |n: u64, unit: &str| {
        if n == 1 {
            format!("1 {unit} ago")
        } else {
            format!("{n} {unit}s ago")
        }
    };
    match secs {
        s if s < MINUTE => "now".to_owned(),
        s if s < HOUR => plural(s / MINUTE, "min"),
        s if s < DAY => plural(s / HOUR, "hour"),
        s if s < 2 * DAY => "yesterday".to_owned(),
        s => plural(s / DAY, "day"),
    }
}

/// A human file size for the card's metadata line: `512 B`,
/// `240 KB`, `1.4 MB`. Pure for tests.
#[expect(
    clippy::cast_precision_loss,
    reason = "u64 file sizes stay far below 2^53; the one-decimal rounding is the point"
)]
pub(crate) fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{bytes} B")
    }
}

/// The card's metadata line: `1920×1080 · 240 KB · 5 mins ago`.
/// Pure for tests.
pub(crate) fn metadata_caption(dims: (u32, u32), bytes: u64, age_secs: u64) -> String {
    format!(
        "{}×{} · {} · {}",
        dims.0,
        dims.1,
        format_size(bytes),
        age_label(age_secs)
    )
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
            let quit = matches!(directive, HomeDirective::Quit);
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
            if quit {
                // The tray toggle's hide direction: the same viewport
                // close, but the outer loop classifies it as a user
                // quit (the process exits; no reopen wait).
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

    /// The welcome body: hero, secondaries, the session grid, the
    /// recents grid / empty card, and the bottom status line
    /// (ADR-0047's watchdog contract lives above this in
    /// `logic_impl`).
    fn welcome_body(&mut self, ui: &mut egui::Ui) {
        ui.add_space(crate::theme::SPACE_SCALE[2]);
        self.hero_card(ui);
        ui.add_space(crate::theme::SPACE_SCALE[2]);
        self.secondary_cards(ui);
        ui.add_space(crate::theme::SPACE_SCALE[3]);
        self.session_section(ui);
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

    /// The "This session" grid: resumable unsaved captures, above
    /// the recents. Hidden entirely when the store is empty; the
    /// list refreshes every frame (editor round-trips file and
    /// graduate entries between renders, and the bridge answers
    /// from an in-memory index — a small Vec clone). Scrolls like
    /// the recents grid once rows overflow the window.
    fn session_section(&mut self, ui: &mut egui::Ui) {
        self.session = self.bridge.session_captures();
        if self.session.is_empty() {
            return;
        }
        ui.label(
            egui::RichText::new("THIS SESSION")
                .size(crate::theme::TYPE_SM)
                .color(crate::theme::palette::GOLD_DEEP),
        );
        ui.add_space(crate::theme::SPACE_SCALE[1]);
        let mut pending_png = false;
        egui::ScrollArea::vertical()
            .id_salt("session-scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let session = self.session.clone();
                for row in session.chunks(RECENTS_PER_ROW) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing =
                            egui::vec2(crate::theme::SPACE_SCALE[2], crate::theme::SPACE_SCALE[2]);
                        for card in row {
                            if self.session_card(ui, card) {
                                pending_png = true;
                            }
                        }
                    });
                }
            });
        ui.add_space(crate::theme::SPACE_SCALE[3]);
        if pending_png {
            // A card's arrival PNG is still landing (the store
            // writes off-thread) — poll until it decodes.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// One session card: the recent-card language with an "unsaved"
    /// caption and a single hover Discard chip (two-step, the
    /// Delete-chip pattern). A body click resumes the capture
    /// through the bridge. Returns whether the thumbnail's PNG had
    /// not landed yet (the section's repaint hint).
    fn session_card(&mut self, ui: &mut egui::Ui, card: &SessionCard) -> bool {
        use crate::theme::palette as p;
        let Some(thumb) = self.thumbnail(ui.ctx(), &card.png_path) else {
            return true;
        };
        let (texture_id, tsize, bytes) =
            (thumb.texture.id(), thumb.texture.size_vec2(), thumb.bytes);
        let id_key = card.id.raw();
        let armed = self.discard_armed == Some(card.id);

        let (rect, _) = ui.allocate_exact_size(egui::vec2(112.0, 148.0), egui::Sense::hover());
        let resp = ui
            .interact(
                rect,
                egui::Id::new(("session", id_key)),
                egui::Sense::click(),
            )
            .on_hover_text("Resume this capture — unsaved");
        ui.painter().rect(
            rect,
            egui::CornerRadius::same(crate::theme::CUTOUT_RADIUS),
            p::PAPER_CREAM,
            if resp.hovered() {
                egui::Stroke::new(1.25, p::GOLD_DEEP)
            } else {
                crate::theme::HAIRLINE_STROKE
            },
            egui::StrokeKind::Middle,
        );

        let (tw, th) = (tsize.x, tsize.y);
        let scale = 74.0 / tw.max(th);
        let size = egui::vec2(tw * scale, th * scale);
        let thumb_center = egui::pos2(rect.center().x, rect.top() + 50.0);
        let img_rect = egui::Rect::from_center_size(thumb_center, size);
        ui.painter().image(
            texture_id,
            img_rect,
            egui::Rect {
                min: egui::pos2(0.0, 0.0),
                max: egui::pos2(1.0, 1.0),
            },
            p::PAPER_CREAM,
        );

        // Two caption lines: the unsaved affordance over metadata.
        let name_galley = ui.painter().layout_no_wrap(
            "Unsaved".to_owned(),
            egui::FontId::proportional(crate::theme::TYPE_XS),
            p::GOLD_DEEP,
        );
        let meta_galley = ui.painter().layout_no_wrap(
            metadata_caption(card.dims, bytes, session_age_secs(card.created_at)),
            egui::FontId::proportional(crate::theme::TYPE_XS),
            p::INK_SOFT,
        );
        ui.painter().galley(
            egui::pos2(
                rect.center().x - name_galley.size().x / 2.0,
                rect.bottom() - 34.0,
            ),
            name_galley,
            p::GOLD_DEEP,
        );
        ui.painter().galley(
            egui::pos2(
                rect.center().x - meta_galley.size().x / 2.0,
                rect.bottom() - 17.0,
            ),
            meta_galley,
            p::INK_SOFT,
        );

        let chip_clicked = self.session_discard_chip(
            ui,
            rect,
            id_key,
            resp.hovered() || resp.contains_pointer(),
            armed,
        );

        if resp.clicked() && !chip_clicked {
            self.discard_armed = None;
            if self.bridge.open_session_capture(card.id) {
                self.session = self.bridge.session_captures();
            }
        }
        if armed && !resp.contains_pointer() {
            self.discard_armed = None;
        }
        false
    }

    /// The session card's single hover chip: Discard, two-step —
    /// first click arms ("Sure?", gold fill), second commits via
    /// the bridge. Returns whether this chip consumed the click.
    fn session_discard_chip(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        id_key: u64,
        show: bool,
        armed: bool,
    ) -> bool {
        use crate::theme::palette as p;
        if !show {
            return false;
        }
        let label = if armed { "Sure?" } else { "Discard" };
        let galley = ui.painter().layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(crate::theme::TYPE_XS),
            p::INK,
        );
        let w = galley.size().x + 14.0;
        let chip_rect = egui::Rect::from_min_size(
            egui::pos2(rect.center().x - w / 2.0, rect.top() + 8.0),
            egui::vec2(w, 22.0),
        );
        let chip = ui
            .interact(
                chip_rect,
                egui::Id::new(("session-discard", id_key)),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let radius = egui::CornerRadius::same(crate::theme::POD_RADIUS);
        ui.painter().add(if chip.hovered() || armed {
            crate::theme::SHADOW_LIFT.as_shape(chip_rect, radius)
        } else {
            crate::theme::SHADOW_RESTING.as_shape(chip_rect, radius)
        });
        ui.painter().rect(
            chip_rect,
            radius,
            if armed {
                p::with_alpha(p::GOLD, 60)
            } else {
                p::PAPER_CREAM
            },
            egui::Stroke::NONE,
            egui::StrokeKind::Middle,
        );
        let inner = chip_rect.shrink(2.0);
        ui.painter().rect(
            inner,
            radius,
            p::PAPER_CREAM,
            egui::Stroke::new(1.5, p::PAPER),
            egui::StrokeKind::Inside,
        );
        ui.painter().galley(
            egui::pos2(
                chip_rect.center().x - galley.size().x / 2.0,
                chip_rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            p::INK,
        );
        if chip.clicked() {
            if armed {
                let id = flintshot_core::SessionId::from_raw(id_key);
                self.discard_armed = None;
                let path = self
                    .session
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.png_path.clone());
                self.bridge.discard_session_capture(id);
                self.session = self.bridge.session_captures();
                if let Some(path) = path {
                    self.thumbs.remove(&path);
                }
            } else {
                self.discard_armed = Some(flintshot_core::SessionId::from_raw(id_key));
            }
            true
        } else {
            false
        }
    }

    /// The recents grid (with thumbnails decoded + downscaled on
    /// demand) or, when empty, the friendly empty card — never
    /// blank space. Dead paths render nothing (pruned at load;
    /// undecodable files skip their card silently). The grid is
    /// adaptive: columns floor-fill the available width at
    /// [`MIN_CARD_WIDTH`], cards stretch to fill their column, and
    /// the section scrolls once the rows overflow the window (the
    /// hero and secondaries stay pinned).
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
        let gap = crate::theme::SPACE_SCALE[2];
        let cols = columns_for_width(ui.available_width(), MIN_CARD_WIDTH, gap);
        let size = recent_card_size(column_width(ui.available_width(), cols, gap));
        egui::ScrollArea::vertical()
            .id_salt("recents-scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let recent = self.recent.clone();
                for row in recent.chunks(cols) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(gap, gap);
                        for path in row {
                            self.recent_card(ui, path, size);
                        }
                    });
                }
            });
    }

    /// One recent's card: letterboxed thumbnail on cream, the
    /// filename and metadata caption lines below, and — on hover —
    /// a row of cutout chips over the thumbnail (Copy / Reveal /
    /// Delete). A click anywhere else on the card routes through
    /// the existing `open_image` path. Delete is a two-step confirm
    /// on the chip itself: the first click arms ("Sure?"), the
    /// second commits; leaving the card or clicking elsewhere
    /// disarms. Sized by the adaptive grid (`size` from
    /// [`recent_card_size`]); the thumbnail letterboxes into the
    /// square-ish box the width affords (source decodes at ≤ 2×
    /// [`THUMB_EDGE`], so very wide columns upscale softly —
    /// accepted for a thumbnail).
    fn recent_card(&mut self, ui: &mut egui::Ui, path: &PathBuf, size: egui::Vec2) {
        use crate::theme::palette as p;
        let Some(thumb) = self.thumbnail(ui.ctx(), path) else {
            return;
        };
        let (texture_id, tsize, dims, bytes, mtime) = (
            thumb.texture.id(),
            thumb.texture.size_vec2(),
            thumb.dims,
            thumb.bytes,
            thumb.mtime,
        );
        let path_key = path.to_string_lossy().to_string();
        let armed = self.delete_armed.as_deref() == Some(path.as_path());

        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let resp = ui
            .interact(
                rect,
                egui::Id::new(("recent", path_key.clone())),
                egui::Sense::click(),
            )
            .on_hover_text(path_key.clone());
        ui.painter().rect(
            rect,
            egui::CornerRadius::same(crate::theme::CUTOUT_RADIUS),
            p::PAPER_CREAM,
            if resp.hovered() {
                egui::Stroke::new(1.25, p::FOREST)
            } else {
                crate::theme::HAIRLINE_STROKE
            },
            egui::StrokeKind::Middle,
        );

        // Letterbox: the fitted image centers in the thumb box (the
        // card's width minus an 8 px inset per side, below the chip
        // row; the captions own the bottom 36 px).
        let box_side = rect.width() - 16.0;
        let (tw, th) = (tsize.x, tsize.y);
        let scale = box_side / tw.max(th);
        let size = egui::vec2(tw * scale, th * scale);
        let thumb_center = egui::pos2(rect.center().x, rect.top() + 8.0 + box_side / 2.0);
        let img_rect = egui::Rect::from_center_size(thumb_center, size);
        ui.painter().image(
            texture_id,
            img_rect,
            egui::Rect {
                min: egui::pos2(0.0, 0.0),
                max: egui::pos2(1.0, 1.0),
            },
            p::PAPER_CREAM,
        );

        // Two caption lines: filename over metadata.
        let name_galley = ui.painter().layout_no_wrap(
            card_stem(path),
            egui::FontId::proportional(crate::theme::TYPE_XS),
            p::INK,
        );
        let meta_galley = ui.painter().layout_no_wrap(
            metadata_caption(dims, bytes, unix_now().saturating_sub(mtime)),
            egui::FontId::proportional(crate::theme::TYPE_XS),
            p::INK_SOFT,
        );
        ui.painter().galley(
            egui::pos2(
                rect.center().x - name_galley.size().x / 2.0,
                rect.bottom() - 34.0,
            ),
            name_galley,
            p::INK,
        );
        ui.painter().galley(
            egui::pos2(
                rect.center().x - meta_galley.size().x / 2.0,
                rect.bottom() - 17.0,
            ),
            meta_galley,
            p::INK_SOFT,
        );

        // Hover chips over the thumbnail's top edge (gated on
        // `contains_pointer`, not `hovered`: a chip registered in a
        // prior frame is hit-test-topmost under the pointer, which
        // would steal `hovered` from the card and un-render the row
        // one frame before the click lands).
        let chip_clicked = self.card_chips(
            ui,
            rect,
            path,
            &path_key,
            resp.hovered() || resp.contains_pointer(),
            armed,
        );

        // Card body click (not a chip): the existing open path.
        if resp.clicked() && !chip_clicked {
            self.delete_armed = None;
            let _ = self.bridge.open_image(path.clone());
            self.recent = self.bridge.recent_images();
        }

        // Leaving the card disarms a pending delete ("clicking
        // anywhere else resets" — the pointer must leave to click
        // elsewhere). `contains_pointer`: the pointer on this
        // card's own chips is still "on the card".
        if armed && !resp.contains_pointer() {
            self.delete_armed = None;
        }
    }

    /// The card's hover chip row (the `floating.rs` cutout-chip
    /// pattern, card-sized): Copy / Reveal / Delete over the
    /// thumbnail's top edge. Delete is a two-step confirm on the
    /// chip itself — the first click arms ("Sure?", gold fill), the
    /// second commits. Returns whether a chip consumed the click
    /// (the card body must not also open).
    fn card_chips(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        path: &Path,
        path_key: &str,
        show: bool,
        armed: bool,
    ) -> bool {
        use crate::theme::palette as p;
        if !show {
            return false;
        }
        let label = if armed { "Sure?" } else { "\u{1f5d1}" };
        let chip_specs: [(&str, &str, f32); 3] = [
            ("recent-copy", "\u{1f4cb}", 28.0),
            ("recent-reveal", "\u{1f5c0}", 28.0),
            ("recent-delete", label, if armed { 44.0 } else { 28.0 }),
        ];
        let total: f32 = chip_specs.iter().map(|(_, _, w)| *w).sum::<f32>() + 12.0;
        let mut x = rect.center().x - total / 2.0;
        let y = rect.top() + 8.0;
        let mut chip_clicked = false;
        for (id, glyph, w) in chip_specs {
            let chip_rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, 22.0));
            x += w + 6.0;
            let chip_id = egui::Id::new((id, path_key));
            let chip = ui
                .interact(chip_rect, chip_id, egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let radius = egui::CornerRadius::same(crate::theme::POD_RADIUS);
            ui.painter()
                .add(if chip.hovered() || (armed && id == "recent-delete") {
                    crate::theme::SHADOW_LIFT.as_shape(chip_rect, radius)
                } else {
                    crate::theme::SHADOW_RESTING.as_shape(chip_rect, radius)
                });
            ui.painter().rect(
                chip_rect,
                radius,
                if armed && id == "recent-delete" {
                    p::with_alpha(p::GOLD, 60)
                } else {
                    p::PAPER_CREAM
                },
                egui::Stroke::NONE,
                egui::StrokeKind::Middle,
            );
            // The white inner outline, 2 px inset.
            let inner = chip_rect.shrink(2.0);
            ui.painter().rect(
                inner,
                radius,
                p::PAPER_CREAM,
                egui::Stroke::new(1.5, p::PAPER),
                egui::StrokeKind::Inside,
            );
            let galley = ui.painter().layout_no_wrap(
                glyph.to_owned(),
                egui::FontId::proportional(crate::theme::TYPE_XS),
                p::INK,
            );
            ui.painter().galley(
                egui::pos2(
                    chip_rect.center().x - galley.size().x / 2.0,
                    chip_rect.center().y - galley.size().y / 2.0,
                ),
                galley,
                p::INK,
            );
            if chip.clicked() {
                chip_clicked = true;
                match id {
                    "recent-copy" => self.bridge.copy_recent(path),
                    "recent-reveal" => self.bridge.reveal_recent(path),
                    _ => {
                        if armed {
                            self.delete_armed = None;
                            self.bridge.delete_recent(path);
                            self.recent = self.bridge.recent_images();
                            self.thumbs.remove(path);
                        } else {
                            self.delete_armed = Some(path.to_path_buf());
                        }
                    }
                }
            }
        }
        chip_clicked
    }

    /// Decode + downscale a recent's thumbnail on demand, cached
    /// in memory for the session together with the original
    /// dimensions and file size the caption shows (one stat per
    /// file, at decode). `None` when the file is gone or
    /// undecodable — the card silently skips.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "thumbnail dims are ≤ 2× the card edge by construction; u32 dims ≤ 2^24 are exact in f32"
    )]
    fn thumbnail(&mut self, ctx: &egui::Context, path: &PathBuf) -> Option<&RecentThumb> {
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
            let bytes_mtime = std::fs::metadata(path).ok().map_or((0, 0), |m| {
                let secs = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs());
                (m.len(), secs)
            });
            self.thumbs.insert(
                path.clone(),
                RecentThumb {
                    texture: handle,
                    dims: (w, h),
                    bytes: bytes_mtime.0,
                    mtime: bytes_mtime.1,
                },
            );
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
    fn quit_directive_leaves_display_state_alone() {
        let mut ui = ui_state();
        ui.apply(&HomeDirective::Quit);
        assert_eq!(ui.status, "connected");
    }

    #[test]
    fn quit_directive_closes_the_window_headlessly() {
        // The tray toggle's hide direction
        // (tray-show-hide-toggle): the same close the Hide path
        // performs — the outer loop classifies it with user-quit
        // semantics (process exits).
        let bridge = RecordingBridge::default();
        bridge
            .pending
            .lock()
            .expect("pending mutex")
            .push_back(HomeDirective::Quit);
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        app.logic_impl(&ctx);
        // The directive was consumed (not skipped as unknown).
        assert!(
            app.bridge.poll_directive().is_none(),
            "the Quit directive was consumed by the frame"
        );
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
        /// Card action targets (the hover chips' bridge calls).
        copied: Vec<PathBuf>,
        revealed: Vec<PathBuf>,
        deleted: Vec<PathBuf>,
        /// Session-card targets: resume clicks and discards.
        resumed: Vec<flintshot_core::SessionId>,
        discarded: Vec<flintshot_core::SessionId>,
    }

    /// Bridge double that records requests and replays directives.
    #[derive(Default, Clone)]
    struct RecordingBridge {
        rec: Recording,
        pending: Arc<Mutex<VecDeque<HomeDirective>>>,
        recents: Arc<Mutex<Vec<PathBuf>>>,
        sessions: Arc<Mutex<Vec<SessionCard>>>,
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
            self.recents.lock().expect("recents mutex").clone()
        }

        fn copy_recent(&mut self, path: &Path) {
            self.rec
                .0
                .lock()
                .expect("recorder mutex")
                .copied
                .push(path.to_path_buf());
        }

        fn reveal_recent(&mut self, path: &Path) {
            self.rec
                .0
                .lock()
                .expect("recorder mutex")
                .revealed
                .push(path.to_path_buf());
        }

        fn delete_recent(&mut self, path: &Path) {
            let mut rec = self.rec.0.lock().expect("recorder mutex");
            rec.deleted.push(path.to_path_buf());
            // The real bridge's index drop, mirrored: the deleted
            // path leaves the recents list.
            self.recents
                .lock()
                .expect("recents mutex")
                .retain(|p| p != path);
        }

        fn session_captures(&mut self) -> Vec<SessionCard> {
            self.sessions.lock().expect("sessions mutex").clone()
        }

        fn open_session_capture(&mut self, id: flintshot_core::SessionId) -> bool {
            self.rec.0.lock().expect("recorder mutex").resumed.push(id);
            true
        }

        fn discard_session_capture(&mut self, id: flintshot_core::SessionId) {
            let mut rec = self.rec.0.lock().expect("recorder mutex");
            rec.discarded.push(id);
            // The real bridge's store removal, mirrored.
            self.sessions
                .lock()
                .expect("sessions mutex")
                .retain(|c| c.id != id);
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
    fn age_label_counts_every_threshold() {
        assert_eq!(age_label(0), "now");
        assert_eq!(age_label(59), "now");
        assert_eq!(age_label(60), "1 min ago");
        assert_eq!(age_label(300), "5 mins ago");
        assert_eq!(age_label(3_600), "1 hour ago");
        assert_eq!(age_label(7_200), "2 hours ago");
        assert_eq!(age_label(86_400), "yesterday");
        assert_eq!(age_label(172_800), "2 days ago");
    }

    #[test]
    fn format_size_spans_bytes_kb_mb() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2_048), "2 KB");
        assert_eq!(format_size(245_760), "240 KB");
        assert_eq!(format_size(1_500_000), "1.4 MB");
    }

    /// The spec's card caption line, verbatim shape.
    #[test]
    fn metadata_caption_composes_the_spec_line() {
        assert_eq!(
            metadata_caption((1920, 1080), 245_760, 300),
            "1920×1080 · 240 KB · 5 mins ago"
        );
    }

    /// The card stem is the file stem only.
    #[test]
    fn card_stem_drops_extension_and_directory() {
        let path = PathBuf::from("/tmp/shots/flintshot-2026-09-20-10-00-00.png");
        assert_eq!(card_stem(&path), "flintshot-2026-09-20-10-00-00");
        assert_eq!(card_stem(Path::new("/")), "capture");
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
        welcome_frame_sized(app, ctx, events, egui::vec2(1024.0, 768.0))
    }

    fn welcome_frame_sized(
        app: &mut HomeApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        screen: egui::Vec2,
    ) -> egui::FullOutput {
        // The hero speaks the display family — installed by
        // `theme::apply` in every real window (the router owns it).
        let raw = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), screen)),
            ..egui::RawInput::default()
        };
        ctx.run_ui(raw, |ui| app.ui_impl(ui))
    }

    /// A click at `at` (the welcome tests' event shape).
    fn click_at(at: egui::Pos2) -> Vec<egui::Event> {
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
        ]
    }

    /// The hover chips' deterministic centers, mirrored from
    /// `recent_card`'s layout: `which` 0=Copy 1=Reveal 2=Delete.
    fn chip_center(card: egui::Rect, which: usize) -> egui::Pos2 {
        let widths = [28.0_f32, 28.0, 28.0];
        let total: f32 = widths.iter().sum::<f32>() + 12.0;
        let mut x = card.center().x - total / 2.0;
        for (i, w) in widths.iter().enumerate() {
            if i == which {
                return egui::pos2(x + w / 2.0, card.top() + 8.0 + 11.0);
            }
            x += w + 6.0;
        }
        unreachable!("three chips");
    }

    /// One recent fixture: a real decodable png + the bridge wired
    /// for it. Returns (bridge, recorder, path).
    fn one_recent() -> (RecordingBridge, Recording, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("shot.png");
        let img = image::RgbaImage::from_pixel(40, 30, image::Rgba([200, 40, 40, 255]));
        img.save(&png).unwrap();
        // keep the tempdir alive for the test's lifetime
        std::mem::forget(dir);
        let bridge = RecordingBridge {
            recents: Arc::new(Mutex::new(vec![png.clone()])),
            ..RecordingBridge::default()
        };
        let rec = bridge.rec.clone();
        (bridge, rec, png)
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
            recents: Arc::new(Mutex::new(vec![png.clone()])),
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

    /// Hovering a card reveals the chip row; the Copy and Reveal
    /// chips route to the bridge (and do NOT open the editor).
    #[test]
    fn hover_chips_route_card_actions() {
        let (bridge, rec, png) = one_recent();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let _ = welcome_frame(&mut app, &ctx, vec![]);
        let card = ctx
            .read_response(egui::Id::new(("recent", png.to_string_lossy().to_string())))
            .expect("the card registers")
            .rect;

        // Settle: hover the card first so the chip row lays out and
        // registers (egui assigns clicks from the prior frame's
        // widgets), then click each chip.
        let _ = welcome_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(card.center())],
        );
        let _ = welcome_frame(&mut app, &ctx, click_at(chip_center(card, 0)));
        let _ = welcome_frame(&mut app, &ctx, click_at(chip_center(card, 1)));

        let guard = rec.0.lock().expect("recorder mutex");
        assert_eq!(guard.copied, vec![png.clone()], "the Copy chip routed");
        assert_eq!(guard.revealed, vec![png.clone()], "the Reveal chip routed");
        assert!(
            guard.opened.is_empty(),
            "chip clicks must not open the editor"
        );
        assert!(guard.deleted.is_empty(), "no delete on these chips");
    }

    /// Delete is two-step on the chip: the first click arms
    /// ("Sure?"), only the second commits — and the grid drops the
    /// card on the refresh that follows.
    #[test]
    fn delete_chip_requires_the_sure_step() {
        let (bridge, rec, png) = one_recent();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let _ = welcome_frame(&mut app, &ctx, vec![]);
        let card = ctx
            .read_response(egui::Id::new(("recent", png.to_string_lossy().to_string())))
            .expect("the card registers")
            .rect;

        // Settle the hover, then arm — no delete yet.
        let _ = welcome_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(card.center())],
        );
        let _ = welcome_frame(&mut app, &ctx, click_at(chip_center(card, 2)));
        assert_eq!(
            app.delete_armed,
            Some(png.clone()),
            "the first click arms the chip"
        );
        assert!(
            rec.0.lock().expect("recorder mutex").deleted.is_empty(),
            "one click must not delete"
        );

        // Step two: commit at the (now wider, same-center) chip.
        let _ = welcome_frame(&mut app, &ctx, click_at(chip_center(card, 2)));
        let guard = rec.0.lock().expect("recorder mutex");
        assert_eq!(guard.deleted, vec![png.clone()], "the second click commits");
        assert_eq!(app.delete_armed, None, "committed: disarmed");

        // The refresh dropped the card from the grid (the response
        // memory outlives the widget, so pin the app's own state).
        let _ = welcome_frame(&mut app, &ctx, vec![]);
        assert!(
            app.recent.is_empty(),
            "the deleted recent left the grid on the refresh"
        );
        assert!(app.thumbs.is_empty(), "its thumbnail cache entry went too");
    }

    /// A card click away from the chips still routes the open path
    /// (and disarms any pending delete).
    #[test]
    fn card_click_off_the_chips_still_opens() {
        let (bridge, rec, png) = one_recent();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let _ = welcome_frame(&mut app, &ctx, vec![]);
        let card = ctx
            .read_response(egui::Id::new(("recent", png.to_string_lossy().to_string())))
            .expect("the card registers")
            .rect;

        // The thumb's center — well below the chip row.
        let at = egui::pos2(card.center().x, card.top() + 60.0);
        let _ = welcome_frame(&mut app, &ctx, click_at(at));
        let guard = rec.0.lock().expect("recorder mutex");
        assert_eq!(guard.opened, vec![png], "the body click opened the image");
        assert!(guard.copied.is_empty() && guard.deleted.is_empty());
    }

    /// Overflow scrolls the section, not the page: on a short
    /// viewport the scroll area engages, the hero stays pinned, and
    /// below-the-fold cards lay out beyond the viewport. (18 cards
    /// at 1024 px = 3 adaptive rows of 6; the fixed-4 rows of the
    /// mockup era are gone.)
    #[test]
    fn recents_overflow_scrolls_the_section_not_the_hero() {
        let dir = tempfile::tempdir().unwrap();
        let mut recents = Vec::new();
        for i in 0..18 {
            let png = dir.path().join(format!("shot-{i}.png"));
            image::RgbaImage::from_pixel(40, 30, image::Rgba([200, 40, 40, 255]))
                .save(&png)
                .unwrap();
            recents.push(png);
        }
        let first = recents[0].clone();
        let last = recents[17].clone();
        let bridge = RecordingBridge {
            recents: Arc::new(Mutex::new(recents)),
            ..RecordingBridge::default()
        };
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let _ = welcome_frame_sized(&mut app, &ctx, vec![], egui::vec2(1024.0, 420.0));

        let hero = ctx
            .read_response(egui::Id::new("hero-capture"))
            .expect("the hero renders")
            .rect;
        assert!(hero.bottom() < 420.0, "the hero stays inside the viewport");
        let first = ctx
            .read_response(egui::Id::new((
                "recent",
                first.to_string_lossy().to_string(),
            )))
            .expect("the first card lays out")
            .rect;
        let last_card = ctx
            .read_response(egui::Id::new((
                "recent",
                last.to_string_lossy().to_string(),
            )))
            .expect("clipped cards still lay out")
            .rect;
        assert!(
            first.top() < 420.0,
            "the grid starts inside the viewport (the section is bounded, not pushing the page)"
        );
        assert!(
            last_card.top() > 420.0,
            "the below-the-fold card sits beyond the viewport (content taller than the pane)"
        );
    }

    // --- Adaptive recents grid (home-capture-grids Task 003) ---

    /// Twelve recents under one bridge — the adaptive-grid fixture.
    fn twelve_recents() -> (RecordingBridge, Vec<PathBuf>) {
        let dir = tempfile::tempdir().unwrap();
        let mut recents = Vec::new();
        for i in 0..12 {
            let png = dir.path().join(format!("shot-{i}.png"));
            image::RgbaImage::from_pixel(40, 30, image::Rgba([200, 40, 40, 255]))
                .save(&png)
                .unwrap();
            recents.push(png);
        }
        std::mem::forget(dir);
        let bridge = RecordingBridge {
            recents: Arc::new(Mutex::new(recents.clone())),
            ..RecordingBridge::default()
        };
        (bridge, recents)
    }

    /// Cards sharing the first card's row (top within 1 px).
    ///
    /// GOTCHA (egui): `Context::read_response` serves the *previous*
    /// pass — geometry read right after a single `run_ui` is the
    /// last frame's layout, not this one's (first-ever pass excepted,
    /// which is why the single-frame tests get away with it). Any
    /// test that asserts geometry at a given window size must run
    /// that size **twice** before reading.
    fn row_mates(ctx: &egui::Context, ids: &[egui::Id], first: egui::Id) -> Vec<egui::Rect> {
        let top = ctx
            .read_response(first)
            .expect("the first card laid out")
            .rect
            .top();
        ids.iter()
            .filter_map(|id| ctx.read_response(*id))
            .filter(|r| (r.rect.top() - top).abs() < 1.0)
            .map(|r| r.rect)
            .collect()
    }

    /// The grid reflows: a wide window lays more cards per row than
    /// a narrow one.
    #[test]
    fn recents_grid_adapts_columns_to_width() {
        let (bridge, paths) = twelve_recents();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let ids: Vec<egui::Id> = paths
            .iter()
            .map(|p| egui::Id::new(("recent", p.to_string_lossy().to_string())))
            .collect();
        for _ in 0..2 {
            let _ = welcome_frame_sized(&mut app, &ctx, vec![], egui::vec2(800.0, 768.0));
        }
        let narrow = row_mates(&ctx, &ids, ids[0]).len();
        for _ in 0..2 {
            let _ = welcome_frame_sized(&mut app, &ctx, vec![], egui::vec2(1600.0, 768.0));
        }
        let wide = row_mates(&ctx, &ids, ids[0]).len();
        assert!(
            wide > narrow,
            "1600 px lays {wide} per row, 800 px lays {narrow} — the grid must reflow"
        );
    }

    /// Row-mates share equal widths spanning the full available
    /// width (the grid has no dead margin on the right).
    #[test]
    fn recents_grid_cards_fill_their_row() {
        let (bridge, paths) = twelve_recents();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let ids: Vec<egui::Id> = paths
            .iter()
            .map(|p| egui::Id::new(("recent", p.to_string_lossy().to_string())))
            .collect();
        let width = 1024.0;
        for _ in 0..2 {
            let _ = welcome_frame_sized(&mut app, &ctx, vec![], egui::vec2(width, 768.0));
        }
        let row = row_mates(&ctx, &ids, ids[0]);
        assert!(row.len() > 1, "the fixture must lay more than one column");
        let w0 = row[0].width();
        assert!(
            row.iter().all(|r| (r.width() - w0).abs() < 1.0),
            "row-mates share equal widths"
        );
        let margin = 16.0;
        assert!(
            (row[0].left() - margin).abs() < 1.0,
            "the row starts at the panel margin"
        );
        assert!(
            (row.last().unwrap().right() - (width - margin)).abs() < 1.0,
            "the row ends at the panel margin — no dead space"
        );
    }

    /// The grid floor-fill: narrow stays at one column, huge clamps
    /// at the cap, and the 700/1024/1600 locals land where the math
    /// says (min card 150, gap 12).
    #[test]
    fn columns_for_width_floor_fills_and_clamps() {
        assert_eq!(columns_for_width(1.0, 150.0, 12.0), 1, "tiny: floor");
        assert_eq!(columns_for_width(150.0, 150.0, 12.0), 1, "exactly one");
        assert_eq!(
            columns_for_width(312.0, 150.0, 12.0),
            2,
            "two cards + one gap exactly"
        );
        assert_eq!(
            columns_for_width(311.9, 150.0, 12.0),
            1,
            "just under the second card"
        );
        assert_eq!(columns_for_width(668.0, 150.0, 12.0), 4, "700 px window");
        assert_eq!(columns_for_width(992.0, 150.0, 12.0), 6, "1024 px window");
        assert_eq!(
            columns_for_width(1_000_000.0, 150.0, 12.0),
            MAX_GRID_COLUMNS,
            "huge: cap"
        );
    }

    /// Cards stretch: `cols` columns split the available width with
    /// no dead margin on the right.
    #[test]
    fn column_width_spans_the_available_space() {
        assert_eq!(column_width(312.0, 2, 12.0), 150.0, "exact fit");
        assert!((column_width(1568.0, 8, 12.0) - 185.5).abs() < 0.01);
        assert!((column_width(1568.0, 1, 12.0) - 1568.0).abs() < 0.01);
    }

    /// A dead recent path renders nothing and breaks nothing.
    #[test]
    fn dead_recent_path_is_silently_absent() {
        let bridge = RecordingBridge {
            recents: Arc::new(Mutex::new(vec![PathBuf::from("/nonexistent/shot.png")])),
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

    /// The decode gathers mtime once — the caption's age term and
    /// the day grouping read it from the thumb, never re-stat the
    /// file on the render path (home-capture-grids Task 002).
    #[test]
    fn recent_thumb_gathers_mtime_at_decode() {
        let (bridge, _rec, png) = one_recent();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let _ = welcome_frame(&mut app, &ctx, vec![]);
        let thumb = app.thumbs.get(&png).expect("the recent decoded");
        let expected = std::fs::metadata(&png)
            .expect("fixture file exists")
            .modified()
            .expect("mtime readable")
            .duration_since(std::time::UNIX_EPOCH)
            .expect("post-epoch mtime")
            .as_secs();
        assert_eq!(thumb.mtime, expected, "decode-time mtime, one stat");
    }

    /// One session fixture: a real decodable png + the bridge wired
    /// with a card over it. Returns (bridge, recorder, id).
    fn one_session() -> (RecordingBridge, Recording, flintshot_core::SessionId) {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("1.png");
        image::RgbaImage::from_pixel(40, 30, image::Rgba([40, 90, 200, 255]))
            .save(&png)
            .unwrap();
        // keep the tempdir alive for the test's lifetime
        std::mem::forget(dir);
        let id = flintshot_core::SessionId::from_raw(1);
        let bridge = RecordingBridge {
            sessions: Arc::new(Mutex::new(vec![SessionCard {
                id,
                png_path: png.clone(),
                dims: (40, 30),
                created_at: 0,
            }])),
            ..RecordingBridge::default()
        };
        let rec = bridge.rec.clone();
        (bridge, rec, id)
    }

    /// An empty session store renders no section at all.
    #[test]
    fn empty_session_store_renders_no_section() {
        let mut app = HomeApp::new(Box::new(RecordingBridge::default()), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let _ = welcome_frame(&mut app, &ctx, vec![]);
        assert!(
            ctx.read_response(egui::Id::new(("session", 0u64)))
                .is_none(),
            "no store entries, no session cards"
        );
        assert!(app.session.is_empty());
    }

    /// Session cards render with live thumbnails; a body click
    /// resumes through the bridge.
    #[test]
    fn session_cards_render_and_click_resumes() {
        let (bridge, rec, id) = one_session();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let _ = welcome_frame(&mut app, &ctx, vec![]);
        let card = ctx
            .read_response(egui::Id::new(("session", id.raw())))
            .expect("a decodable session entry renders its card");

        // The thumb's center — below the chip row.
        let at = egui::pos2(card.rect.center().x, card.rect.top() + 60.0);
        let _ = welcome_frame(&mut app, &ctx, click_at(at));
        assert_eq!(
            rec.0.lock().expect("recorder mutex").resumed,
            vec![id],
            "clicking a session card resumes it"
        );
    }

    /// The session Discard chip is two-step: the first click arms
    /// ("Sure?"), the second commits and the card leaves the grid.
    #[test]
    fn session_discard_chip_requires_the_sure_step() {
        let (bridge, rec, id) = one_session();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let _ = welcome_frame(&mut app, &ctx, vec![]);
        let card = ctx
            .read_response(egui::Id::new(("session", id.raw())))
            .expect("the card registers")
            .rect;

        // Settle the hover, then arm — no discard yet.
        let chip_at = |card: egui::Rect| egui::pos2(card.center().x, card.top() + 8.0 + 11.0);
        let _ = welcome_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(card.center())],
        );
        let _ = welcome_frame(&mut app, &ctx, click_at(chip_at(card)));
        assert_eq!(app.discard_armed, Some(id), "the first click arms the chip");
        assert!(
            rec.0.lock().expect("recorder mutex").discarded.is_empty(),
            "one click must not discard"
        );

        // Step two: commit.
        let _ = welcome_frame(&mut app, &ctx, click_at(chip_at(card)));
        let guard = rec.0.lock().expect("recorder mutex");
        assert_eq!(guard.discarded, vec![id], "the second click commits");
        drop(guard);
        assert_eq!(app.discard_armed, None, "committed: disarmed");
        let _ = welcome_frame(&mut app, &ctx, vec![]);
        assert!(
            app.session.is_empty(),
            "the discarded capture left the grid on the refresh"
        );
    }

    /// The session section scrolls like the recents grid: on a
    /// short viewport the first row stays visible and the
    /// below-the-fold cards sit beyond it.
    #[test]
    fn session_overflow_scrolls_the_section() {
        let dir = tempfile::tempdir().unwrap();
        let mut cards = Vec::new();
        for i in 0..12u64 {
            let png = dir.path().join(format!("{i}.png"));
            image::RgbaImage::from_pixel(40, 30, image::Rgba([40, 90, 200, 255]))
                .save(&png)
                .unwrap();
            cards.push(SessionCard {
                id: flintshot_core::SessionId::from_raw(i),
                png_path: png,
                dims: (40, 30),
                created_at: i,
            });
        }
        let bridge = RecordingBridge {
            sessions: Arc::new(Mutex::new(cards)),
            ..RecordingBridge::default()
        };
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let _ = welcome_frame_sized(&mut app, &ctx, vec![], egui::vec2(1024.0, 420.0));

        let first = ctx
            .read_response(egui::Id::new(("session", 0u64)))
            .expect("the first session card lays out")
            .rect;
        let last = ctx
            .read_response(egui::Id::new(("session", 11u64)))
            .expect("clipped session cards still lay out")
            .rect;
        assert!(
            first.bottom() <= 420.0,
            "the first row stays inside the viewport"
        );
        assert!(
            last.top() > 420.0,
            "the below-the-fold card sits beyond the viewport"
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
