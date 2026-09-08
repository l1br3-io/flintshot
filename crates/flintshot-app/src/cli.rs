//! CLI definition and subcommand dispatch.
//!
//! Phase 1 Task 3 wired `region` / `fullscreen` to real capture calls
//! and added the `config` subcommand. Task 6 rerouted both through
//! [`EguiShell::run_editor`] when no `--out` is given; with `--out`,
//! Task 3's PNG-write path is preserved (headless / scripting escape
//! hatch). The editor path is gated on the `egui` Cargo feature.
//!
//! [`EguiShell::run_editor`]: flintshot_editor_egui::EguiShell::run_editor

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};
use color_eyre::{eyre::Context, Result};
use flintshot_core::ipc::Request;
use flintshot_core::{
    capture_fullscreen, Config, ImageMime, Point, Rect, UploadContext, UploadOutcome,
};
use image::ImageFormat;

/// Cross-platform screenshot capture and annotation.
#[derive(Debug, Parser)]
#[command(
    name = "flintshot",
    version,
    propagate_version = true,
    subcommand_required = false,
    about = "Cross-platform screenshot capture and annotation",
    long_about = "Flameshot-inspired screenshot tool, written in pure Rust.\nSee docs/ROADMAP.md for current phase and available features."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// All `flintshot` subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the long-running daemon (tray, hotkeys, IPC server). [Phase 2]
    Daemon(DaemonArgs),
    /// Capture a region and open the editor. [Phase 1]
    Region(RegionArgs),
    /// Capture a full monitor and open the editor. [Phase 1]
    Fullscreen(FullscreenArgs),
    /// Print the resolved configuration as TOML to stdout.
    Config,
    /// Open a pin-to-desktop window for an existing image. [Phase 3]
    Pin(PinArgs),
    /// Open an existing image in the editor. [Phase 3]
    Open(OpenArgs),
    /// Upload an image to a backend and print the resulting URL. [Phase 4]
    Upload(UploadArgs),
    /// Set or clear credentials for an upload backend. [Phase 4]
    Auth(AuthArgs),
    /// Manage global hotkeys (bind via the Wayland portal). [Phase 6]
    Hotkeys(HotkeysArgs),
    /// Open the app window (the capture-controls home, in the spirit
    /// of the Windows 11 Snipping Tool). Auto-starts the daemon when
    /// none is running.
    App,
    /// Internal: the tray-helper process, spawned and supervised by
    /// the daemon (ADR-0042). Hidden from `--help`; not for direct
    /// use.
    #[cfg(feature = "tray")]
    #[command(hide = true)]
    Tray,
}

/// Arguments for `flintshot daemon`.
#[derive(Debug, Args)]
pub struct DaemonArgs {
    /// Replace any running daemon instance instead of joining it.
    #[arg(long)]
    pub replace: bool,
}

/// Shared capture-output flags, flattened into each capture subcommand.
///
/// Flattening (rather than inheriting) keeps `clap`'s `--help` output
/// per-subcommand: `flintshot region --help` shows `--out` alongside
/// the region-specific flags, which reads better than a shared
/// "COMMON OPTIONS" section.
#[derive(Debug, Args)]
pub struct OutputArgs {
    /// Write the PNG to `PATH` and skip the editor entirely
    /// (headless / scripting path).
    ///
    /// When omitted, the captured screenshot is opened in the editor
    /// (requires the `egui` Cargo feature, which is on by default;
    /// `--no-default-features` builds reject `region` / `fullscreen`
    /// without `--out` with a clear error).
    ///
    /// Format is always PNG regardless of the extension; a non-`.png`
    /// path still yields valid PNG bytes.
    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,
    /// Skip daemon detect-and-delegate: always run the capture + editor
    /// in this process, even if a daemon is running. Escape hatch for
    /// debugging (delegated captures centralise their `Event` stream;
    /// `--standalone` does not).
    #[arg(long)]
    pub standalone: bool,
}

/// Per-invocation interaction-mode override flags (Phase 5). Mutually
/// exclusive; when neither is passed, `[ui] interaction-mode` decides.
/// An explicit flag also skips daemon delegation: the IPC request
/// cannot carry it, and the daemon's worker re-resolves the mode from
/// its own config — delegating would silently drop the user's flag.
#[derive(Debug, Args)]
pub struct InteractionArgs {
    /// Open the fullscreen frozen-capture overlay instead of the
    /// editor window. Overrides `[ui] interaction-mode`. On Wayland
    /// this is best-effort: the overlay may not raise above other
    /// windows or receive input on all compositors (a config-set
    /// `overlay` falls back to the editor window there; the explicit
    /// flag is honored with a warning).
    #[arg(long, conflicts_with = "editor")]
    pub overlay: bool,
    /// Open the editor window even when `[ui] interaction-mode` is
    /// `"overlay"`.
    #[arg(long, conflicts_with = "overlay")]
    pub editor: bool,
}

impl InteractionArgs {
    /// Map the parsed flags to a mode override, if any. With neither
    /// flag set (the common case) the config value decides.
    fn mode_override(&self) -> Option<InteractionMode> {
        match (self.overlay, self.editor) {
            (false, false) => None,
            // clap rejects both flags together; the arm order is just
            // defensive totality.
            (true, _) => Some(InteractionMode::Overlay),
            (false, true) => Some(InteractionMode::Editor),
        }
    }
}

/// How a fresh capture is presented for annotation (Phase 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractionMode {
    /// Separate editor window (Phase 1 behaviour; the default).
    Editor,
    /// Fullscreen frozen-capture overlay.
    Overlay,
}

impl InteractionMode {
    /// Kebab-case name matching the `[ui] interaction-mode` config
    /// value and the CLI flag names.
    fn as_str(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Overlay => "overlay",
        }
    }

    /// Parse a `[ui] interaction-mode` config value. Unknown values
    /// fall back to `Editor` with a warning — same policy as
    /// `magnifier-modifier` (config keeps the raw string, so a typo
    /// neither bricks loading nor gets rewritten by a save).
    fn from_config(value: &str) -> Self {
        match value {
            "editor" => Self::Editor,
            "overlay" => Self::Overlay,
            other => {
                tracing::warn!(
                    value = other,
                    "unknown interaction-mode; falling back to \"editor\""
                );
                Self::Editor
            }
        }
    }
}

/// Where a resolved [`InteractionMode`] came from, for the decision
/// log emitted per capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractionSource {
    /// Explicit `--overlay` / `--editor` CLI flag.
    Flag,
    /// `[ui] interaction-mode` config value.
    Config,
    /// The platform overrode the config value (Wayland demotes
    /// `overlay` to the editor window).
    PlatformFallback,
}

impl InteractionSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Flag => "cli-flag",
            Self::Config => "config",
            Self::PlatformFallback => "platform-fallback",
        }
    }
}

/// Resolve the interaction mode for a capture: CLI flag >
/// `[ui] interaction-mode` config value > platform fallback
/// (Wayland → editor, Phase 5 Task 007).
///
/// The force-flag semantics: an explicit `--overlay` on Wayland is
/// honored best-effort (warned), because the flag is an informed
/// override — the fallback exists to protect the *default* config
/// path, not to veto explicit requests. A config-resolved `overlay`
/// on Wayland is demoted to the editor window with an info log
/// naming the fallback (spec exit criterion 4).
fn resolve_interaction_mode(
    cli_flag: Option<InteractionMode>,
    config_value: &str,
    wayland: bool,
) -> (InteractionMode, InteractionSource) {
    let Some(mode) = cli_flag else {
        let mode = InteractionMode::from_config(config_value);
        if wayland && mode == InteractionMode::Overlay {
            tracing::info!(
                "Wayland session detected; falling back to the editor window \
                 (a fullscreen overlay cannot reliably raise above other \
                 windows or grab input on all Wayland compositors)"
            );
            return (InteractionMode::Editor, InteractionSource::PlatformFallback);
        }
        return (mode, InteractionSource::Config);
    };
    if wayland && mode == InteractionMode::Overlay {
        tracing::warn!(
            "--overlay on a Wayland session is best-effort: the fullscreen \
             overlay may not raise above other windows or receive input \
             on all compositors"
        );
    }
    (mode, InteractionSource::Flag)
}

/// Decide whether the editor window opens selection-first (the
/// `region` capture landing experience): yes iff the capture is a
/// `region` *and* the resolved shell is the editor window. This is
/// the single app-crate decision point (plan decision 4) — it covers
/// the default `editor` mode on all platforms, the `--editor` flag,
/// and the Wayland demotion of config-set `overlay` (the demoted
/// editor is still region's editor path). Fullscreen stays plain even
/// when demoted (spec Open Question 1: "the whole monitor is the
/// shot"); `open` / `pin` never request the phase.
fn selection_first_for(mode: &str, resolved: InteractionMode) -> bool {
    mode == "region" && resolved == InteractionMode::Editor
}

/// Arguments for `flintshot region`.
#[derive(Debug, Args)]
pub struct RegionArgs {
    /// Index of the monitor to capture (0-based). Defaults to the active monitor.
    #[arg(long, value_name = "N")]
    pub monitor: Option<u32>,
    /// Delay before capturing, in milliseconds.
    #[arg(long, default_value_t = 0, value_name = "MILLIS")]
    pub delay_ms: u32,
    #[command(flatten)]
    pub interaction: InteractionArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

/// Arguments for `flintshot fullscreen`.
#[derive(Debug, Args)]
pub struct FullscreenArgs {
    /// Index of the monitor to capture (0-based). Defaults to the active monitor.
    #[arg(long, value_name = "N")]
    pub monitor: Option<u32>,
    #[command(flatten)]
    pub interaction: InteractionArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

/// Arguments for `flintshot pin`.
#[derive(Debug, Args)]
pub struct PinArgs {
    /// Path to the image file to pin.
    pub path: PathBuf,
    /// Window top-left position as `X,Y` (physical px). Defaults to the
    /// active monitor's centre.
    #[arg(long, value_name = "X,Y", value_parser = parse_point)]
    pub at: Option<Point>,
    /// Window size as `WxH` (physical px). Defaults to the image's
    /// natural dimensions.
    #[arg(long, value_name = "WxH", value_parser = parse_size)]
    pub size: Option<(u32, u32)>,
    /// Resting opacity in `0.0..=1.0`. Overrides `[pin] default-opacity`.
    #[arg(long, value_name = "N")]
    pub opacity: Option<f32>,
    /// Do not copy the image to the clipboard when the pin closes
    /// (overrides `[pin] copy-on-close = true`).
    #[arg(long)]
    pub no_clipboard: bool,
}

/// Arguments for `flintshot upload`.
#[derive(Debug, Args)]
pub struct UploadArgs {
    /// Path to the image file to upload.
    pub path: PathBuf,
    /// Upload backend to use (`imgur`, ...). Defaults to config's `default_backend`.
    #[arg(long, value_name = "NAME")]
    pub backend: Option<String>,
}

/// Arguments for `flintshot auth`.
#[derive(Debug, Args)]
pub struct AuthArgs {
    /// Backend to configure (`imgur`). Only `imgur` is supported in Phase 4.
    pub backend: String,
    /// Remove the stored credential instead of setting one.
    #[arg(long)]
    pub clear: bool,
}

/// Arguments for `flintshot open`.
#[derive(Debug, Args)]
pub struct OpenArgs {
    /// Path to the image file to open in the editor.
    pub path: PathBuf,
}

/// Arguments for `flintshot hotkeys`.
#[derive(Debug, Args)]
pub struct HotkeysArgs {
    #[command(subcommand)]
    pub command: HotkeysCommand,
}

/// `flintshot hotkeys` subcommands.
#[derive(Debug, Subcommand)]
pub enum HotkeysCommand {
    /// Open the global-shortcuts bind dialog via the running daemon's
    /// portal session (Wayland). X11/macOS/Windows hotkeys come from
    /// `[hotkeys]` config and need no bind step.
    Bind,
}

/// `flintshot daemon` — Phase 2.
///
/// Starts the long-running daemon: single-instance lock, tokio side
/// thread, system tray (if built with `--features tray`), a spike
/// global hotkey, and the event loop. Blocks until Ctrl-C. The IPC
/// server, real hotkeys, and `--replace` takeover land in Tasks
/// 004–011; see [`crate::daemon`].
pub fn run_daemon(args: &DaemonArgs) -> Result<()> {
    crate::daemon::run(args.replace)
}

/// `flintshot region` — Phase 1 capture+editor, with Phase 2
/// detect-and-delegate.
///
/// If a daemon is running (and `--standalone`/`--out` aren't set), the
/// request is handed to it so the capture's `Event`s reach subscribers
/// and the daemon's worker opens the editor. Otherwise the in-process
/// Phase 1 capture+editor path runs (or a PNG is written with `--out`).
pub fn run_region(args: &RegionArgs) -> Result<()> {
    let interaction = args.interaction.mode_override();
    if delegate_to_daemon(
        &Request::CaptureRegion {
            monitor: args.monitor,
            delay_ms: args.delay_ms,
        },
        &args.output,
        "region",
        interaction,
    ) {
        return Ok(());
    }
    if args.delay_ms > 0 {
        tracing::debug!(delay_ms = args.delay_ms, "sleeping before capture");
        std::thread::sleep(Duration::from_millis(u64::from(args.delay_ms)));
    }
    capture_and_dispatch(args.monitor, "region", &args.output, interaction)
}

/// `flintshot fullscreen` — Phase 1 capture+editor, with Phase 2
/// detect-and-delegate. See [`run_region`].
pub fn run_fullscreen(args: &FullscreenArgs) -> Result<()> {
    let interaction = args.interaction.mode_override();
    if delegate_to_daemon(
        &Request::CaptureFullscreen {
            monitor: args.monitor,
        },
        &args.output,
        "fullscreen",
        interaction,
    ) {
        return Ok(());
    }
    capture_and_dispatch(args.monitor, "fullscreen", &args.output, interaction)
}

/// `flintshot open <path>` — Phase 3. Load an existing image from disk
/// into the editor (no capture). Spawned by the daemon's `OpenLast`
/// dispatch so the user can re-edit the most recently saved capture.
pub fn run_open(args: &OpenArgs) -> Result<()> {
    if !args.path.exists() {
        return Err(color_eyre::eyre::eyre!(
            "no such file: {}",
            args.path.display()
        ));
    }
    let img = image::open(&args.path)
        .with_context(|| format!("loading image {}", args.path.display()))?
        .to_rgba8();
    run_editor_for_capture(img, "open")
}

/// Try to hand a capture `request` to a running daemon. Returns `true`
/// if delegated (the caller should exit). Skipped when `--standalone` is
/// set, when `--out` is given (the PNG-write path is headless and must
/// run in-process; the daemon's worker would open an editor instead),
/// or when an explicit `--overlay`/`--editor` flag is set (the IPC
/// request cannot carry it, so honoring the flag means running here).
/// A delegation failure falls back to standalone with a warning rather
/// than failing the whole capture.
fn delegate_to_daemon(
    request: &Request,
    output: &OutputArgs,
    mode: &str,
    interaction: Option<InteractionMode>,
) -> bool {
    if output.standalone || output.out.is_some() {
        return false;
    }
    if interaction.is_some() {
        tracing::debug!(
            "{mode}: explicit interaction-mode flag; running standalone \
             (the daemon worker would re-resolve the mode from config)"
        );
        return false;
    }
    match crate::ipc_client::try_delegate(request) {
        Ok(crate::ipc_client::DelegateOutcome::Delegated) => {
            tracing::info!("delegated `{mode}` to the running daemon");
            true
        }
        Ok(crate::ipc_client::DelegateOutcome::NoDaemon) => false,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "could not delegate `{mode}` to the daemon; falling back to standalone"
            );
            false
        }
    }
}

/// Capture a monitor and either write a PNG (`--out`) or open the
/// interaction shell (no `--out`). Shared by [`run_region`] /
/// [`run_fullscreen`]; `mode` is just the log label ("region" /
/// "fullscreen"). `interaction` is the resolved CLI-flag override, if
/// any — the config value joins in [`run_capture_shell`].
fn capture_and_dispatch(
    monitor: Option<u32>,
    mode: &str,
    output: &OutputArgs,
    interaction: Option<InteractionMode>,
) -> Result<()> {
    let img = capture_with_tray_hidden(monitor, mode)?;

    match &output.out {
        Some(path) => {
            write_png(&img, path)?;
            tracing::info!(path = %path.display(), "{mode} capture saved");
        }
        None => run_capture_shell(img, mode, interaction)?,
    }
    Ok(())
}

/// Grab the monitor with any daemon's tray icon hidden (ADR-0036):
/// the hide handshake, the capture, and the restore report all sit
/// inside the guard's scope, so the icon returns the instant the
/// pixels are taken — success or failure — while the editor /
/// overlay session beyond this function never holds it hostage.
/// No daemon (standalone capture) costs one socket probe.
fn capture_with_tray_hidden(monitor: Option<u32>, mode: &str) -> Result<image::RgbaImage> {
    let _tray_hidden = TrayHiddenGuard::request_at(None);
    capture_fullscreen(monitor)
        .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))
        .with_context(|| format!("capturing {mode} on monitor {monitor:?}"))
}

/// RAII tray-hide for one capture: on construction, asks a running
/// daemon to hide its tray icon (so it never lands in the frozen
/// frame); on drop, reports the grab finished so the icon returns.
/// Best-effort end to end — a capture must never fail because the
/// daemon was slow, absent, or hostile. See ADR-0036.
struct TrayHiddenGuard {
    /// The daemon registered our hide request, so a
    /// [`Request::ReportCaptured`] is owed to release it.
    report_owed: bool,
}

impl TrayHiddenGuard {
    /// Testable seam: request against an explicit socket path.
    fn request_at(override_path: Option<&std::path::Path>) -> Self {
        match crate::ipc_client::hide_tray_for_capture_to(override_path) {
            Ok(crate::ipc_client::HideTrayOutcome::NoDaemon) => Self { report_owed: false },
            // The unacked case already warned inside the helper; the
            // report is owed either way (the daemon registered us).
            Ok(crate::ipc_client::HideTrayOutcome::Registered { .. }) => Self { report_owed: true },
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "tray-hide handshake failed; capturing with the tray as-is"
                );
                Self { report_owed: false }
            }
        }
    }
}

impl Drop for TrayHiddenGuard {
    fn drop(&mut self) {
        if self.report_owed {
            report_captured_to_daemon();
        }
    }
}

/// Tell a running daemon this capture's grab finished so it can
/// restore its tray icon (pairs the hide handshake, ADR-0036).
/// Best-effort fire-and-forget, mirroring [`report_saved_to_daemon`].
fn report_captured_to_daemon() {
    report_captured_to_daemon_at(None);
}

/// Testable seam for [`report_captured_to_daemon`]: report against an
/// explicit socket path. `NoDaemon` is a silent no-op; transport
/// errors are logged at `warn` and swallowed so a report failure
/// never breaks a finished capture.
fn report_captured_to_daemon_at(override_path: Option<&std::path::Path>) {
    match crate::ipc_client::delegate_to(override_path, &Request::ReportCaptured) {
        Ok(crate::ipc_client::DelegateOutcome::Delegated) => {
            tracing::debug!("reported capture done; the daemon may restore its tray");
        }
        Ok(crate::ipc_client::DelegateOutcome::NoDaemon) => {
            tracing::debug!("no daemon running; skipping capture-done report");
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "could not report capture done to the daemon; ignoring"
            );
        }
    }
}

/// Resolve the interaction mode for a fresh capture (CLI flag >
/// `[ui] interaction-mode` > platform fallback — see
/// [`resolve_interaction_mode`]), log the decision, and open the
/// matching shell: the Phase 5 fullscreen overlay or the Phase 1
/// editor window.
fn run_capture_shell(
    img: image::RgbaImage,
    mode: &str,
    interaction: Option<InteractionMode>,
) -> Result<()> {
    let cfg = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "could not load config; using built-in defaults"
            );
            Config::default()
        }
    };
    let (resolved, source) = resolve_interaction_mode(
        interaction,
        &cfg.ui.interaction_mode,
        crate::platform::session_is_wayland(),
    );
    let selection_first = selection_first_for(mode, resolved);
    tracing::info!(
        interaction = resolved.as_str(),
        source = source.as_str(),
        selection_first,
        "{mode} capture interaction mode resolved"
    );
    match resolved {
        InteractionMode::Overlay => run_overlay_with_config(img, mode, &cfg),
        InteractionMode::Editor => run_editor_with_config(img, mode, selection_first, &cfg),
    }
}

/// Hand the captured image to the egui editor and log the outcome.
/// Returns `Ok(())` regardless of which `EditorOutcome` the user picked
/// (saved / copied / cancelled); only shell or render failures bubble
/// up as `Err`. Loads config (lossily) for the UI defaults — callers
/// that already hold a config use [`run_editor_with_config`]. Never
/// selection-first: its only caller is `open` (an existing image, not
/// a region capture).
#[cfg(feature = "egui")]
fn run_editor_for_capture(img: image::RgbaImage, mode: &str) -> Result<()> {
    let cfg = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "could not load config; opening editor with built-in defaults"
            );
            Config::default()
        }
    };
    run_editor_with_config(img, mode, false, &cfg)
}

/// [`run_editor_for_capture`] over an already-loaded config (the
/// interaction-mode resolution path loads config once and shares it).
/// `selection_first` opens the region landing phase (Select
/// pre-armed, dim mask, hint line) — see [`selection_first_for`]; the
/// `open` path always passes `false`.
#[cfg(feature = "egui")]
fn run_editor_with_config(
    img: image::RgbaImage,
    mode: &str,
    selection_first: bool,
    cfg: &Config,
) -> Result<()> {
    use flintshot_core::EditorShell;

    let state = state_from_config(img, cfg);

    // Under the ambient-runtime guard: accesskit → zbus 5 needs a
    // reactor on this thread for the window to come up (see
    // `crate::ambient`). Dropped before `report_editor_outcome`.
    let outcome = crate::ambient::with_ambient_runtime(|| {
        let shell = if selection_first {
            flintshot_editor_egui::EguiShell::new().with_selection_first()
        } else {
            flintshot_editor_egui::EguiShell::new()
        };
        shell.run_editor(state)
    })
    .and_then(|ran| ran.map_err(|e| color_eyre::eyre::eyre!(e.to_string())))
    .with_context(|| format!("running editor for {mode} capture"))?;

    report_editor_outcome(outcome);
    Ok(())
}

/// Run the Phase 5 fullscreen frozen-capture overlay over an
/// already-loaded config (mirror of [`run_editor_with_config`]).
///
/// The overlay opens with `Tool::Select` (forced inside
/// `OverlayApp`), so the first drag draws a region selection;
/// `Esc` cancels, `Enter` accepts (Enter = Copy: the annotated
/// region lands on the clipboard and the overlay closes; the shell
/// lingers briefly to keep the X11 selection alive).
#[cfg(feature = "egui")]
fn run_overlay_with_config(img: image::RgbaImage, mode: &str, cfg: &Config) -> Result<()> {
    let state = state_from_config(img, cfg);

    // Guard mirrors `run_editor_with_config` (see `crate::ambient`).
    let outcome = crate::ambient::with_ambient_runtime(|| {
        flintshot_editor_egui::EguiShell::new().run_overlay(state)
    })
    .and_then(|ran| ran.map_err(|e| color_eyre::eyre::eyre!(e.to_string())))
    .with_context(|| format!("running overlay for {mode} capture"))?;

    report_editor_outcome(outcome);
    Ok(())
}

/// Build an [`EditorState`] for a fresh capture with the persisted
/// `[ui]` defaults (color / stroke / font / flip-to-move) applied
/// (Task 15; prominent-move-tool Task 002). Shared by the
/// editor-window and overlay shell paths so both sessions start
/// from the same persisted tool settings.
#[cfg(feature = "egui")]
fn state_from_config(img: image::RgbaImage, cfg: &Config) -> flintshot_core::EditorState {
    let mut state = flintshot_core::EditorState::new(img);
    state.color = cfg.ui.default_color;
    state.text_color = cfg.ui.default_text_color;
    state.stroke_width = cfg.ui.default_stroke_width;
    state.font_size = cfg.ui.default_font_size;
    state.flip_to_move = cfg.ui.flip_to_move;
    state
}

/// Log a finished editor / overlay session's outcome and report a
/// save back to the daemon (see [`report_saved_to_daemon`]).
/// Shared by the editor-window and overlay paths so their outcome
/// handling can't drift apart.
#[cfg(feature = "egui")]
fn report_editor_outcome(outcome: flintshot_core::EditorOutcome) {
    use flintshot_core::EditorOutcome;

    // `EditorOutcome` is `#[non_exhaustive]`; the wildcard arm is
    // mandatory from outside `flintshot-core` and covers future
    // variants we don't yet name.
    #[allow(
        clippy::wildcard_enum_match_arm,
        reason = "EditorOutcome is #[non_exhaustive]; _ is mandatory"
    )]
    match outcome {
        EditorOutcome::Saved(p) => {
            tracing::info!(path = %p.display(), "editor saved");
            report_saved_to_daemon(&p);
        }
        EditorOutcome::CopiedToClipboard => {
            tracing::info!("editor copied to clipboard");
        }
        EditorOutcome::Uploaded(url) => {
            tracing::info!(%url, "editor uploaded");
        }
        EditorOutcome::Pinned => {
            tracing::info!("editor pinned image to desktop");
        }
        EditorOutcome::Cancelled => {
            tracing::info!("editor cancelled; no file written");
        }
        _ => tracing::warn!(?outcome, "editor returned unhandled outcome"),
    }
}

/// Headless-build fallback: editor is unavailable without the `egui`
/// feature. The image argument is accepted (and dropped) purely so the
/// call site in [`run_capture_shell`] is uniform across feature sets.
/// `selection_first` is likewise accepted and dropped (mirrors the
/// egui-build signature).
#[cfg(not(feature = "egui"))]
fn run_editor_with_config(
    _img: image::RgbaImage,
    _mode: &str,
    _selection_first: bool,
    _cfg: &Config,
) -> Result<()> {
    Err(color_eyre::eyre::eyre!(
        "editor not built; pass --out PATH to capture headlessly \
         (this binary was compiled without the `egui` feature)"
    ))
}

/// Headless-build fallback for [`run_open`]'s entry point (see
/// [`run_editor_with_config`]'s note; pre-existing gap, surfaced by
/// this effort's `--no-default-features` gate).
#[cfg(not(feature = "egui"))]
fn run_editor_for_capture(_img: image::RgbaImage, _mode: &str) -> Result<()> {
    Err(color_eyre::eyre::eyre!(
        "editor not built; this binary was compiled without the `egui` feature"
    ))
}

/// Headless-build fallback for the overlay (see
/// [`run_editor_with_config`]'s note).
#[cfg(not(feature = "egui"))]
fn run_overlay_with_config(_img: image::RgbaImage, _mode: &str, _cfg: &Config) -> Result<()> {
    Err(color_eyre::eyre::eyre!(
        "overlay not built; pass --out PATH to capture headlessly \
         (this binary was compiled without the `egui` feature)"
    ))
}

/// Report a save back to a running daemon so it can track the path as
/// "last saved" (for `PinLast`/`OpenLast` and upload-from-last).
/// Best-effort: a missing daemon is a silent no-op (the worker may be
/// standalone). See ADR-0023.
#[cfg(feature = "egui")]
fn report_saved_to_daemon(path: &std::path::Path) {
    report_saved_to_daemon_at(None, path);
}

/// Testable seam for [`report_saved_to_daemon`]: report against an
/// explicit socket path. `NoDaemon` (absent socket / refused connect) is
/// a silent no-op; transport errors are logged at `warn` and swallowed
/// so a report failure never breaks a successful save. Reuses
/// [`ipc_client::delegate_to`]'s connect+frame path.
#[cfg(feature = "egui")]
fn report_saved_to_daemon_at(
    override_path: Option<&std::path::Path>,
    saved_path: &std::path::Path,
) {
    match crate::ipc_client::delegate_to(
        override_path,
        &Request::ReportSaved {
            path: saved_path.to_path_buf(),
        },
    ) {
        Ok(crate::ipc_client::DelegateOutcome::Delegated) => {
            tracing::debug!(path = %saved_path.display(), "reported save to daemon");
        }
        Ok(crate::ipc_client::DelegateOutcome::NoDaemon) => {
            tracing::debug!("no daemon running; skipping save report");
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "could not report save to the daemon; ignoring"
            );
        }
    }
}

/// Report an upload result back to a running daemon so it can broadcast
/// `Event::Uploaded` to subscribers (tray, COSMIC applet, IPC scripts).
/// Best-effort: a missing daemon is a silent no-op (the worker may be
/// standalone). Extends ADR-0023's worker-reports-back pattern.
fn report_uploaded_to_daemon(url: &str, delete_handle: Option<&str>) {
    report_uploaded_to_daemon_at(None, url, delete_handle);
}

/// Testable seam for [`report_uploaded_to_daemon`]: report against an
/// explicit socket path. `NoDaemon` (absent socket / refused connect) is
/// a silent no-op; transport errors are logged at `warn` and swallowed
/// so a report failure never breaks a successful upload. Mirrors
/// [`report_saved_to_daemon_at`].
fn report_uploaded_to_daemon_at(
    override_path: Option<&std::path::Path>,
    url: &str,
    delete_handle: Option<&str>,
) {
    match crate::ipc_client::delegate_to(
        override_path,
        &Request::ReportUploaded {
            url: url.to_string(),
            delete_handle: delete_handle.map(str::to_string),
        },
    ) {
        Ok(crate::ipc_client::DelegateOutcome::Delegated) => {
            tracing::debug!(%url, "reported upload to daemon");
        }
        Ok(crate::ipc_client::DelegateOutcome::NoDaemon) => {
            tracing::debug!("no daemon running; skipping upload report");
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "could not report upload to the daemon; ignoring"
            );
        }
    }
}

/// `flintshot config` — Phase 1 Task 3.
///
/// Loads the user's config (falling back to defaults if absent) and
/// prints the resolved TOML to stdout. Output is always valid TOML.
pub fn run_config() -> Result<()> {
    let cfg = Config::load()
        .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))
        .with_context(|| "loading flintshot config")?;
    let toml = toml::to_string_pretty(&cfg)
        .map_err(|e| color_eyre::eyre::eyre!("serialization failure: {e}"))?;
    print!("{toml}");
    Ok(())
}

/// Encode `img` as PNG to `path`, creating parent directories as
/// needed. Format is forced to PNG regardless of the path extension
/// so `--out foo.bar` still produces a valid PNG.
fn write_png(img: &image::RgbaImage, path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating output directory {}", parent.display()))?;
        }
    }
    img.save_with_format(path, ImageFormat::Png)
        .with_context(|| format!("writing PNG to {}", path.display()))
}

/// `flintshot pin` — Phase 3.
///
/// Loads an image from disk, resolves the window geometry (image natural
/// size by default; `--size` overrides; active-monitor centre by
/// default, `--at` overrides), merges CLI overrides over `[pin]` config
/// (`--opacity`, `--no-clipboard`), and hands the image + rect + resolved
/// config to the egui pin viewport (`EguiShell::run_pin`), which opens
/// a borderless always-on-top window.
pub fn run_pin(args: &PinArgs) -> Result<()> {
    if !args.path.exists() {
        return Err(color_eyre::eyre::eyre!(
            "no such file: {}",
            args.path.display()
        ));
    }

    let img = image::open(&args.path)
        .with_context(|| format!("loading pin image {}", args.path.display()))?
        .to_rgba8();

    // A config load failure is non-fatal: fall back to built-in pin
    // defaults so the pin still opens.
    let cfg = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "could not load config; using built-in pin defaults"
            );
            Config::default()
        }
    };

    // CLI overrides win over config defaults. The resolved PinConfig
    // is passed to the egui shell via its builder (the `run_pin` trait
    // signature stays `(image, rect)` — config rides the shell).
    let mut pin_cfg = cfg.pin.clone();
    if let Some(op) = args.opacity {
        pin_cfg.default_opacity = op;
    }
    if args.no_clipboard {
        pin_cfg.copy_on_close = false;
    }

    let rect = pin_window_rect(img.dimensions(), args.at, args.size, active_monitor_geo());

    tracing::debug!(
        opacity = pin_cfg.default_opacity,
        copy_on_close = pin_cfg.copy_on_close,
        rect = ?rect,
        "pin resolved; opening viewport"
    );

    run_pin_window(img, rect, pin_cfg)
}

/// Open the pin viewport. Task 2 hands the image + rect + resolved
/// config to the egui shell's real borderless always-on-top window.
#[cfg(feature = "egui")]
fn run_pin_window(
    img: image::RgbaImage,
    rect: Rect,
    config: flintshot_core::PinConfig,
) -> Result<()> {
    use flintshot_core::EditorShell;

    crate::ambient::with_ambient_runtime(|| {
        flintshot_editor_egui::EguiShell::new()
            .with_pin_config(config)
            .run_pin(img, rect)
    })
    .and_then(|ran| ran.map_err(|e| color_eyre::eyre::eyre!("running pin window: {e}")))?;
    Ok(())
}

/// Headless-build fallback: the pin viewport needs the `egui` feature.
#[cfg(not(feature = "egui"))]
fn run_pin_window(
    _img: image::RgbaImage,
    _rect: Rect,
    _config: flintshot_core::PinConfig,
) -> Result<()> {
    Err(color_eyre::eyre::eyre!(
        "pin window not built; this binary was compiled without the `egui` feature"
    ))
}

/// A monitor's geometry, used to centre the pin window. Stripped from
/// [`flintshot_core::MonitorInfo`] so the centre math is pure and
/// headless-testable — `MonitorInfo` is `#[non_exhaustive]` and thus not
/// constructable in this crate's unit tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MonitorGeo {
    /// Virtual-desktop origin X (physical px).
    x: i32,
    /// Virtual-desktop origin Y (physical px).
    y: i32,
    /// Width (physical px).
    width: u32,
    /// Height (physical px).
    height: u32,
}

/// Pick the active monitor's geometry for pin centring: the primary
/// monitor, falling back to the first. Returns `None` (with a warning)
/// when no monitors are available or enumeration fails — callers then
/// place the pin at the origin.
fn active_monitor_geo() -> Option<MonitorGeo> {
    match flintshot_core::monitors() {
        Ok(list) if !list.is_empty() => {
            let primary = list.iter().find(|m| m.is_primary).unwrap_or(&list[0]);
            Some(MonitorGeo {
                x: primary.x,
                y: primary.y,
                width: primary.width,
                height: primary.height,
            })
        }
        Ok(_) => {
            tracing::warn!("no monitors available; placing pin at (0, 0)");
            None
        }
        Err(e) => {
            tracing::warn!(error = %e, "could not enumerate monitors; placing pin at (0, 0)");
            None
        }
    }
}

/// Resolve the pin window rect from the image dimensions and any CLI
/// overrides. Size defaults to the image's natural dimensions; position
/// defaults to the active monitor's centre. A window larger than its
/// monitor is clamped to the monitor's top-left rather than placed at a
/// negative coordinate.
#[allow(
    clippy::cast_precision_loss,
    reason = "monitor coords/dims fit f32 exactly"
)]
fn pin_window_rect(
    img_dims: (u32, u32),
    at: Option<Point>,
    size: Option<(u32, u32)>,
    monitor: Option<MonitorGeo>,
) -> Rect {
    let (w, h) = size.unwrap_or(img_dims);
    let pos = at.unwrap_or_else(|| match monitor {
        Some(m) => {
            let off_x = (m.width as f32 - w as f32).max(0.0) / 2.0;
            let off_y = (m.height as f32 - h as f32).max(0.0) / 2.0;
            Point::new(m.x as f32 + off_x, m.y as f32 + off_y)
        }
        None => Point::new(0.0, 0.0),
    });
    Rect::from_xywh(pos.x, pos.y, w as f32, h as f32)
}

/// Parse an `--at X,Y` value into a [`Point`]. Both halves accept any
/// `f32`-parseable text (so `100,200` and `3.5,4` both work).
fn parse_point(s: &str) -> Result<Point, String> {
    let (xs, ys) = s
        .split_once(',')
        .ok_or_else(|| format!("`{s}` is not `X,Y` (expected a comma)"))?;
    let x = xs
        .trim()
        .parse::<f32>()
        .map_err(|e| format!("invalid X coordinate `{xs}`: {e}"))?;
    let y = ys
        .trim()
        .parse::<f32>()
        .map_err(|e| format!("invalid Y coordinate `{ys}`: {e}"))?;
    Ok(Point::new(x, y))
}

/// Parse a `--size WxH` value into a `(width, height)` pair.
fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (ws, hs) = s
        .split_once('x')
        .ok_or_else(|| format!("`{s}` is not `WxH` (expected an `x`)"))?;
    let w = ws
        .trim()
        .parse::<u32>()
        .map_err(|e| format!("invalid width `{ws}`: {e}"))?;
    let h = hs
        .trim()
        .parse::<u32>()
        .map_err(|e| format!("invalid height `{hs}`: {e}"))?;
    Ok((w, h))
}

/// `flintshot upload` — Phase 4.
///
/// Loads the image, sniffs its format, resolves the backend, runs the
/// upload on a tokio runtime, then prints the URL to stdout (last line,
/// machine-parseable), copies it to the clipboard, and fires a desktop
/// notification. Diagnostics go to stderr (via `color_eyre`); exit is
/// non-zero on any failure.
pub fn run_upload(args: &UploadArgs) -> Result<()> {
    match run_upload_core(args) {
        Ok(outcome) => {
            println!("{}", outcome.url);
            copy_url_to_clipboard(&outcome.url);
            fire_upload_notification(true, &outcome.url);
            report_uploaded_to_daemon(&outcome.url, outcome.delete_handle.as_deref());
            Ok(())
        }
        Err(e) => {
            let msg = format!("{e:#}");
            fire_upload_notification(false, &msg);
            Err(e)
        }
    }
}

/// Core upload logic — returns the full [`UploadOutcome`] on success.
/// Extracted so [`run_upload`] can layer on the side effects (stdout /
/// clipboard / notification / daemon report) uniformly.
fn run_upload_core(args: &UploadArgs) -> Result<UploadOutcome> {
    if !args.path.exists() {
        return Err(color_eyre::eyre::eyre!(
            "no such file: {}",
            args.path.display()
        ));
    }

    let bytes =
        std::fs::read(&args.path).with_context(|| format!("reading {}", args.path.display()))?;

    let mime = sniff_mime(&bytes)?;

    let cfg = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "could not load config; using built-in defaults");
            Config::default()
        }
    };
    let backend_id =
        resolve_backend_id(args.backend.as_deref(), &cfg.upload.default_backend).to_string();

    let backend = crate::upload::resolve_backend(&backend_id)
        .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))
        .with_context(|| format!("resolving upload backend `{backend_id}`"))?;

    let entry_name = credential_entry_name(&backend_id);
    let entry = keyring::Entry::new("flintshot", &entry_name).ok();

    let filename = args.path.file_name().and_then(|n| n.to_str());
    let ctx = UploadContext {
        credentials: entry.as_ref(),
        filename,
    };

    tracing::info!(path = %args.path.display(), %backend_id, "starting upload");

    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| color_eyre::eyre::eyre!("creating tokio runtime: {e}"))?;

    let outcome = runtime
        .block_on(backend.upload(&bytes, mime, &ctx))
        .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))?;

    Ok(outcome)
}

/// Resolve the backend id: CLI `--backend` wins, then config
/// `default-backend`, then the built-in `"imgur"`.
fn resolve_backend_id<'a>(cli_override: Option<&'a str>, config_default: &'a str) -> &'a str {
    cli_override.unwrap_or(config_default)
}

/// Sniff the image format from raw bytes and map to [`ImageMime`].
fn sniff_mime(bytes: &[u8]) -> Result<ImageMime> {
    let format = image::guess_format(bytes)
        .map_err(|e| color_eyre::eyre::eyre!("detecting image format: {e}"))?;
    match format {
        ImageFormat::Png => Ok(ImageMime::Png),
        ImageFormat::Jpeg => Ok(ImageMime::Jpeg),
        ImageFormat::WebP => Ok(ImageMime::Webp),
        other => Err(color_eyre::eyre::eyre!(
            "unsupported image format: {other:?}"
        )),
    }
}

/// Construct the keyring entry name for a backend's Client-ID.
fn credential_entry_name(backend_id: &str) -> String {
    format!("{backend_id}:client-id")
}

/// Best-effort copy of the URL to the system clipboard.
fn copy_url_to_clipboard(url: &str) {
    match arboard::Clipboard::new() {
        Ok(mut cb) => {
            if let Err(e) = cb.set_text(url.to_string()) {
                tracing::warn!(error = %e, "could not copy URL to clipboard");
            }
        }
        Err(e) => tracing::warn!(error = %e, "could not access clipboard"),
    }
}

/// Best-effort desktop notification. `success` controls the message
/// template: `"Uploaded to <msg>"` on success, `"Upload failed: <msg>"`
/// on error.
fn fire_upload_notification(success: bool, message: &str) {
    let body = if success {
        format!("Uploaded to {message}")
    } else {
        format!("Upload failed: {message}")
    };
    if let Err(e) = notify_rust::Notification::new()
        .appname("flintshot")
        .summary("flintshot upload")
        .body(&body)
        .show()
    {
        tracing::warn!(error = %e, "could not show desktop notification");
    }
}

/// `flintshot auth` — Phase 4.
///
/// Sets or clears the Imgur Client-ID in the OS keychain. This is the
/// only credential-entry path: no config-file field exists (secrets-
/// stay-out-of-config boundary). The credential is never written to
/// config, logs, or stdout — only to the keychain. All user-facing
/// messages go to stderr.
pub fn run_auth(args: &AuthArgs) -> Result<()> {
    validate_auth_backend(&args.backend)?;

    let entry_name = credential_entry_name(&args.backend);
    let entry = keyring::Entry::new("flintshot", &entry_name)
        .map_err(|e| color_eyre::eyre::eyre!("accessing keyring: {e}"))?;

    if args.clear {
        clear_credential(&entry)
    } else {
        set_credential(&entry)
    }
}

/// Validate that the backend supports credential storage. Phase 4
/// supports only `imgur`.
fn validate_auth_backend(backend: &str) -> Result<()> {
    if backend == "imgur" {
        Ok(())
    } else {
        Err(color_eyre::eyre::eyre!(
            "unsupported auth backend `{backend}` (Phase 4 supports only `imgur`)"
        ))
    }
}

/// Prompt for and store the Client-ID in the keychain.
fn set_credential(entry: &keyring::Entry) -> Result<()> {
    let client_id = read_client_id()?;
    entry
        .set_password(&client_id)
        .map_err(|e| color_eyre::eyre::eyre!("storing Client-ID in keyring: {e}"))?;
    eprintln!("Imgur Client-ID stored in keychain.");
    Ok(())
}

/// Remove the Client-ID from the keychain. Clearing an absent entry is
/// a no-op (exit 0).
fn clear_credential(entry: &keyring::Entry) -> Result<()> {
    match entry.delete_credential() {
        Ok(()) => {
            eprintln!("Imgur Client-ID removed from keychain.");
            Ok(())
        }
        Err(keyring::Error::NoEntry) => {
            eprintln!("No Imgur Client-ID stored.");
            Ok(())
        }
        Err(e) => Err(color_eyre::eyre::eyre!("clearing keyring entry: {e}")),
    }
}

/// Read the Client-ID: interactive hidden-input prompt when stdin is a
/// TTY, or `IMGUR_CLIENT_ID` env var when it is not (non-interactive
/// fallback for piped / CI environments).
fn read_client_id() -> Result<String> {
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() {
        rpassword::prompt_password("Imgur Client-ID: ")
            .map_err(|e| color_eyre::eyre::eyre!("reading Client-ID: {e}"))
    } else {
        std::env::var("IMGUR_CLIENT_ID").map_err(|_| {
            color_eyre::eyre::eyre!(
                "stdin is not a TTY and IMGUR_CLIENT_ID is not set; \
                 run interactively or set the env var"
            )
        })
    }
}

/// `flintshot hotkeys` — Phase 6. Dispatches to the hotkeys
/// subcommand.
pub fn run_hotkeys(args: &HotkeysArgs) -> Result<()> {
    match args.command {
        HotkeysCommand::Bind => run_hotkeys_bind(),
    }
}

/// `flintshot app` — open the capture home window. See
/// [`crate::app_window`] for the process model (IPC client,
/// daemon auto-start, hide/restore).
#[cfg(feature = "egui")]
pub fn run_app() -> Result<()> {
    crate::app_window::run()
}

/// Headless-build fallback: the app window needs the `egui` feature.
#[cfg(not(feature = "egui"))]
pub fn run_app() -> Result<()> {
    Err(color_eyre::eyre::eyre!(
        "app window not built; this binary was compiled without the `egui` feature"
    ))
}

/// `flintshot tray` — the daemon-supervised tray helper (ADR-0042).
/// Runs until a capture/quit click exits it or the daemon goes away;
/// the daemon respawns it. Not a user-facing subcommand (hidden).
#[cfg(feature = "tray")]
pub(crate) fn run_tray_helper() -> Result<()> {
    crate::tray_helper::run()
}

/// `flintshot hotkeys bind` — ask the running daemon to open the
/// xdg-desktop-portal `GlobalShortcuts` bind dialog (with the daemon's
/// `[hotkeys]` chords as suggestions). Binding must happen in the
/// daemon's portal session — portal bindings belong to the session
/// that bound them — so with no daemon running this errors clearly
/// instead of trying to bind standalone.
fn run_hotkeys_bind() -> Result<()> {
    run_hotkeys_bind_at(None)
}

/// Testable seam for [`run_hotkeys_bind`]: send the bind request
/// against an explicit socket path, await the daemon's one-frame reply,
/// and render it. Exit 0 with the bound chords on success; a non-zero
/// error (`eyre::Report`) with the failure reason otherwise.
fn run_hotkeys_bind_at(override_path: Option<&std::path::Path>) -> Result<()> {
    match crate::ipc_client::bind_shortcuts_to(override_path)? {
        Some(event) => render_bind_outcome(&event),
        None => Err(color_eyre::eyre::eyre!(
            "no running daemon; start `flintshot daemon` first"
        )),
    }
}

/// Turn the daemon's bind reply into the CLI's output: bindings on
/// success, a cancellation notice on `bind-cancelled`, the carried
/// message on other errors, and a protocol complaint for anything
/// unexpected.
fn render_bind_outcome(event: &flintshot_core::ipc::Event) -> Result<()> {
    match event {
        flintshot_core::ipc::Event::ShortcutsBound { bindings } => {
            for b in bindings {
                println!("{} = {}", b.id, b.trigger);
            }
            Ok(())
        }
        flintshot_core::ipc::Event::Error { code, message } if code == "bind-cancelled" => Err(
            color_eyre::eyre::eyre!("bind dismissed — no shortcuts were changed"),
        ),
        flintshot_core::ipc::Event::Error { code: _, message } => {
            Err(color_eyre::eyre::eyre!("hotkeys bind failed: {message}"))
        }
        other => Err(color_eyre::eyre::eyre!(
            "unexpected reply from daemon: {other:?}"
        )),
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, reason = "exact pixel coords, no arithmetic error")]
mod tests {
    use super::*;

    const fn geo(x: i32, y: i32, w: u32, h: u32) -> MonitorGeo {
        MonitorGeo {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn pin_rect_defaults_to_image_size_at_origin_without_monitor() {
        let r = pin_window_rect((100, 200), None, None, None);
        assert_eq!(r, Rect::from_xywh(0.0, 0.0, 100.0, 200.0));
    }

    #[test]
    fn pin_rect_size_override_changes_size_not_position() {
        let r = pin_window_rect((100, 200), None, Some((50, 80)), None);
        assert_eq!(r, Rect::from_xywh(0.0, 0.0, 50.0, 80.0));
    }

    #[test]
    fn pin_rect_at_override_sets_top_left() {
        let r = pin_window_rect((100, 200), Some(Point::new(10.0, 20.0)), None, None);
        assert_eq!(r.min, Point::new(10.0, 20.0));
        assert_eq!(r.width(), 100.0);
        assert_eq!(r.height(), 200.0);
    }

    #[test]
    fn pin_rect_centers_on_monitor() {
        // 1000x800 monitor at origin, 200x100 window → centre (400, 350).
        let r = pin_window_rect((200, 100), None, None, Some(geo(0, 0, 1000, 800)));
        assert_eq!(r.min, Point::new(400.0, 350.0));
        assert_eq!(r.width(), 200.0);
        assert_eq!(r.height(), 100.0);
    }

    #[test]
    fn pin_rect_centers_on_monitor_with_offset_origin() {
        // Secondary monitor at (1920, 0), 1000x800, 200x100 window.
        let r = pin_window_rect((200, 100), None, None, Some(geo(1920, 0, 1000, 800)));
        assert_eq!(r.min, Point::new(1920.0 + 400.0, 350.0));
    }

    #[test]
    fn pin_rect_clamps_to_origin_when_bigger_than_monitor() {
        let r = pin_window_rect((2000, 2000), None, None, Some(geo(0, 0, 1000, 800)));
        assert_eq!(r.min, Point::new(0.0, 0.0));
    }

    #[test]
    fn pin_rect_at_overrides_monitor_centering() {
        let r = pin_window_rect(
            (200, 100),
            Some(Point::new(5.0, 6.0)),
            None,
            Some(geo(0, 0, 1000, 800)),
        );
        assert_eq!(r.min, Point::new(5.0, 6.0));
    }

    #[test]
    fn parse_point_accepts_comma_pair() {
        assert_eq!(parse_point("100,200").unwrap(), Point::new(100.0, 200.0));
        assert_eq!(parse_point(" 3 , 4 ").unwrap(), Point::new(3.0, 4.0));
    }

    #[test]
    fn parse_point_rejects_missing_comma() {
        assert!(parse_point("100 200").is_err());
    }

    #[test]
    fn parse_point_rejects_non_numeric() {
        assert!(parse_point("a,b").is_err());
    }

    #[test]
    fn parse_size_accepts_x_pair() {
        assert_eq!(parse_size("800x600").unwrap(), (800, 600));
        assert_eq!(parse_size(" 1280 x 720 ").unwrap(), (1280, 720));
    }

    #[test]
    fn parse_size_rejects_garbage() {
        assert!(parse_size("800").is_err());
        assert!(parse_size("800,600").is_err());
    }

    /// A standalone worker (no daemon) must not error when it tries to
    /// report a save — the absent socket yields `NoDaemon`, which the
    /// helper swallows. Reaching the end of the call = success.
    #[cfg(feature = "egui")]
    #[test]
    fn report_saved_to_daemon_is_silent_when_no_daemon() {
        report_saved_to_daemon_at(
            Some(std::path::Path::new(
                "/this/does/not/exist/flintshot-test-report.sock",
            )),
            std::path::Path::new("/tmp/flintshot-saved.png"),
        );
    }

    #[test]
    fn report_captured_to_daemon_is_silent_when_no_daemon() {
        // Mirror of the save-report contract: a standalone capture
        // must not error (or even log above debug) reporting done.
        report_captured_to_daemon_at(Some(std::path::Path::new(
            "/this/does/not/exist/flintshot-test-captured.sock",
        )));
    }

    #[test]
    fn tray_hidden_guard_owes_no_report_without_a_daemon() {
        // Standalone capture: nothing was hidden, so Drop has nothing
        // to report — and must not attempt a connection.
        let guard = TrayHiddenGuard::request_at(Some(std::path::Path::new(
            "/this/does/not/exist/flintshot-test-guard.sock",
        )));
        assert!(
            !guard.report_owed,
            "no daemon → no hide registered → no report owed"
        );
        drop(guard); // silent by contract
    }

    // ── Upload helpers ───────────────────────────────────────────

    /// A standalone worker (no daemon) must not error when it tries to
    /// report an upload — the absent socket yields `NoDaemon`, which the
    /// helper swallows. Reaching the end of the call = success.
    #[test]
    fn report_uploaded_to_daemon_is_silent_when_no_daemon() {
        report_uploaded_to_daemon_at(
            Some(std::path::Path::new(
                "/this/does/not/exist/flintshot-test-report.sock",
            )),
            "https://imgur.com/abc123",
            Some("delete-hash-xyz"),
        );
    }

    #[test]
    fn resolve_backend_id_prefers_cli_override() {
        assert_eq!(resolve_backend_id(Some("custom"), "imgur"), "custom");
    }

    #[test]
    fn resolve_backend_id_uses_config_when_no_override() {
        assert_eq!(resolve_backend_id(None, "0x0"), "0x0");
    }

    #[test]
    fn resolve_backend_id_defaults_to_imgur_unchanged() {
        assert_eq!(resolve_backend_id(None, "imgur"), "imgur");
    }

    #[test]
    fn sniff_mime_detects_png() {
        let header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(sniff_mime(&header).unwrap(), ImageMime::Png);
    }

    #[test]
    fn sniff_mime_detects_jpeg() {
        let header = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(sniff_mime(&header).unwrap(), ImageMime::Jpeg);
    }

    #[test]
    fn sniff_mime_rejects_unknown_format() {
        assert!(sniff_mime(b"hello world not an image").is_err());
    }

    #[test]
    fn credential_entry_name_includes_backend_id() {
        assert_eq!(credential_entry_name("imgur"), "imgur:client-id");
        assert_eq!(credential_entry_name("0x0"), "0x0:client-id");
    }

    #[test]
    fn run_upload_nonexistent_file_errors() {
        let args = UploadArgs {
            path: PathBuf::from("/nonexistent/path/does-not-exist.png"),
            backend: None,
        };
        assert!(run_upload(&args).is_err());
    }

    // ── Auth helpers ──────────────────────────────────────────────

    #[test]
    fn validate_auth_backend_accepts_imgur() {
        assert!(validate_auth_backend("imgur").is_ok());
    }

    #[test]
    fn validate_auth_backend_rejects_unknown() {
        assert!(validate_auth_backend("0x0").is_err());
        assert!(validate_auth_backend("").is_err());
    }

    #[test]
    fn run_auth_rejects_unknown_backend() {
        let args = AuthArgs {
            backend: "nope".to_string(),
            clear: false,
        };
        assert!(run_auth(&args).is_err());
    }

    #[test]
    fn auth_args_parses_imgur_without_clear() {
        let cli = Cli::parse_from(["flintshot", "auth", "imgur"]);
        match cli.command {
            Some(Command::Auth(args)) => {
                assert_eq!(args.backend, "imgur");
                assert!(!args.clear);
            }
            other => panic!("expected Auth, got {other:?}"),
        }
    }

    #[test]
    fn auth_args_parses_clear_flag() {
        let cli = Cli::parse_from(["flintshot", "auth", "imgur", "--clear"]);
        match cli.command {
            Some(Command::Auth(args)) => {
                assert_eq!(args.backend, "imgur");
                assert!(args.clear);
            }
            other => panic!("expected Auth, got {other:?}"),
        }
    }

    // ── Hotkeys bind (Phase 6 Task 001) ──────────────────────────

    #[test]
    fn hotkeys_bind_parses() {
        let cli = Cli::parse_from(["flintshot", "hotkeys", "bind"]);
        match cli.command {
            Some(Command::Hotkeys(args)) => match args.command {
                HotkeysCommand::Bind => {}
            },
            other => panic!("expected Hotkeys, got {other:?}"),
        }
    }

    #[test]
    fn hotkeys_without_subcommand_is_rejected() {
        // `flintshot hotkeys` alone must not silently do nothing.
        let err = Cli::try_parse_from(["flintshot", "hotkeys"]).expect_err("needs a subcommand");
        assert!(
            err.to_string().to_lowercase().contains("subcommand"),
            "expected a subcommand-required error, got: {err}"
        );
    }

    /// No daemon → the exact actionable error from the spec, not a raw
    /// connect error. Uses an explicit socket path so the test never
    /// touches a real daemon's socket.
    #[test]
    fn hotkeys_bind_without_daemon_errors_with_start_daemon_hint() {
        let err = run_hotkeys_bind_at(Some(std::path::Path::new(
            "/this/does/not/exist/flintshot-test-bind.sock",
        )))
        .expect_err("absent socket must error");
        assert_eq!(
            err.to_string(),
            "no running daemon; start `flintshot daemon` first"
        );
    }

    // ── Bind outcome rendering (Phase 6, Task 003) ────────────────

    use flintshot_core::ipc::{Event as IpcEvent, ShortcutBinding};

    #[test]
    fn bind_outcome_success_renders_each_binding() {
        let event = IpcEvent::ShortcutsBound {
            bindings: vec![ShortcutBinding {
                id: "region".into(),
                trigger: "<Control><Shift>Print".into(),
            }],
        };
        assert!(render_bind_outcome(&event).is_ok());
    }

    #[test]
    fn bind_outcome_cancelled_is_a_dismissal_error() {
        let event = IpcEvent::Error {
            code: "bind-cancelled".into(),
            message: "Portal request was cancelled".into(),
        };
        let err = render_bind_outcome(&event).expect_err("cancelled must error");
        assert!(err.to_string().contains("dismissed"), "got: {err}");
    }

    #[test]
    fn bind_outcome_failure_carries_the_message() {
        let event = IpcEvent::Error {
            code: "bind-failed".into(),
            message: "the portal exploded".into(),
        };
        let err = render_bind_outcome(&event).expect_err("failure must error");
        assert!(
            err.to_string().contains("the portal exploded"),
            "got: {err}"
        );
    }

    #[test]
    fn bind_outcome_unexpected_event_is_an_error() {
        assert!(render_bind_outcome(&IpcEvent::CaptureStarted).is_err());
    }

    // ── Interaction mode (Phase 5 Task 001) ──────────────────────

    #[test]
    fn resolve_interaction_mode_flag_wins_over_config() {
        assert_eq!(
            resolve_interaction_mode(Some(InteractionMode::Editor), "overlay", false),
            (InteractionMode::Editor, InteractionSource::Flag)
        );
        assert_eq!(
            resolve_interaction_mode(Some(InteractionMode::Overlay), "editor", false),
            (InteractionMode::Overlay, InteractionSource::Flag)
        );
    }

    #[test]
    fn resolve_interaction_mode_uses_config_when_no_flag() {
        assert_eq!(
            resolve_interaction_mode(None, "editor", false),
            (InteractionMode::Editor, InteractionSource::Config)
        );
        assert_eq!(
            resolve_interaction_mode(None, "overlay", false),
            (InteractionMode::Overlay, InteractionSource::Config)
        );
    }

    #[test]
    fn resolve_interaction_mode_unknown_config_falls_back_to_editor() {
        assert_eq!(
            resolve_interaction_mode(None, "hologram", false),
            (InteractionMode::Editor, InteractionSource::Config)
        );
    }

    // ── Platform fallback (Phase 5 Task 007) ─────────────────────

    #[test]
    fn wayland_config_overlay_falls_back_to_editor() {
        // The spec's honest-UX stance: a fullscreen overlay cannot
        // reliably raise/grab on all Wayland compositors, so the
        // config value is demoted with a named-source decision.
        assert_eq!(
            resolve_interaction_mode(None, "overlay", true),
            (InteractionMode::Editor, InteractionSource::PlatformFallback)
        );
    }

    #[test]
    fn wayland_config_editor_stays_config_sourced() {
        // Wayland would have chosen the editor anyway; no fallback
        // noise for the mode the platform prefers.
        assert_eq!(
            resolve_interaction_mode(None, "editor", true),
            (InteractionMode::Editor, InteractionSource::Config)
        );
    }

    #[test]
    fn wayland_unknown_config_value_is_from_config_not_platform() {
        // The unknown-value fallback lives in `from_config`; the
        // platform leg never sees an overlay to demote.
        assert_eq!(
            resolve_interaction_mode(None, "hologram", true),
            (InteractionMode::Editor, InteractionSource::Config)
        );
    }

    #[test]
    fn wayland_overlay_flag_is_honored_best_effort() {
        // Force-flag semantics: an explicit --overlay is an informed
        // override — honored on Wayland (best-effort), not refused.
        assert_eq!(
            resolve_interaction_mode(Some(InteractionMode::Overlay), "editor", true),
            (InteractionMode::Overlay, InteractionSource::Flag)
        );
    }

    #[test]
    fn wayland_editor_flag_stays_flag_sourced() {
        assert_eq!(
            resolve_interaction_mode(Some(InteractionMode::Editor), "overlay", true),
            (InteractionMode::Editor, InteractionSource::Flag)
        );
    }

    // ── Selection-first wiring (region-selection-first Task 002) ─

    #[test]
    fn region_editor_shell_is_selection_first() {
        // Any source that lands region on the editor window — config
        // default, `--editor` flag, or the Wayland overlay demotion —
        // opens selection-first.
        assert!(selection_first_for("region", InteractionMode::Editor));
    }

    #[test]
    fn region_true_overlay_is_not_selection_first() {
        // The overlay has its own selection chrome; the editor flag is
        // meaningless there.
        assert!(!selection_first_for("region", InteractionMode::Overlay));
    }

    #[test]
    fn fullscreen_editor_shell_is_not_selection_first() {
        assert!(!selection_first_for("fullscreen", InteractionMode::Editor));
    }

    #[test]
    fn open_editor_shell_is_not_selection_first() {
        assert!(!selection_first_for("open", InteractionMode::Editor));
    }

    #[test]
    fn wayland_region_overlay_demotion_lands_selection_first() {
        // The composed path from `run_capture_shell`: config-set
        // overlay on Wayland demotes to the editor, which is still
        // region's editor path → selection-first.
        let (resolved, _source) = resolve_interaction_mode(None, "overlay", true);
        assert_eq!(resolved, InteractionMode::Editor);
        assert!(selection_first_for("region", resolved));
    }

    #[test]
    fn wayland_fullscreen_overlay_demotion_stays_plain_editor() {
        // Spec Open Question 1 (lean no): fullscreen's contract is
        // "the whole monitor is the shot" — its demotion must not
        // grow a spotlight.
        let (resolved, _source) = resolve_interaction_mode(None, "overlay", true);
        assert_eq!(resolved, InteractionMode::Editor);
        assert!(!selection_first_for("fullscreen", resolved));
    }

    #[test]
    fn region_parses_overlay_flag() {
        let cli = Cli::parse_from(["flintshot", "region", "--overlay"]);
        match cli.command {
            Some(Command::Region(args)) => {
                assert!(args.interaction.overlay);
                assert!(!args.interaction.editor);
                assert_eq!(
                    args.interaction.mode_override(),
                    Some(InteractionMode::Overlay)
                );
            }
            other => panic!("expected Region, got {other:?}"),
        }
    }

    #[test]
    fn fullscreen_parses_editor_flag() {
        let cli = Cli::parse_from(["flintshot", "fullscreen", "--editor"]);
        match cli.command {
            Some(Command::Fullscreen(args)) => {
                assert_eq!(
                    args.interaction.mode_override(),
                    Some(InteractionMode::Editor)
                );
            }
            other => panic!("expected Fullscreen, got {other:?}"),
        }
    }

    #[test]
    fn interaction_flags_default_to_no_override() {
        let cli = Cli::parse_from(["flintshot", "region"]);
        match cli.command {
            Some(Command::Region(args)) => {
                assert_eq!(args.interaction.mode_override(), None);
            }
            other => panic!("expected Region, got {other:?}"),
        }
    }

    #[test]
    fn overlay_and_editor_flags_are_mutually_exclusive() {
        let err = Cli::try_parse_from(["flintshot", "region", "--overlay", "--editor"]);
        assert!(
            err.is_err(),
            "--overlay and --editor together must be rejected"
        );
    }

    #[test]
    fn explicit_mode_flag_skips_daemon_delegation() {
        // An explicit mode flag cannot ride the IPC request (the
        // daemon's worker re-resolves from its own config), so the
        // flag must force the standalone path before any socket IO.
        let output = OutputArgs {
            out: None,
            standalone: false,
        };
        assert!(
            !delegate_to_daemon(
                &Request::CaptureRegion {
                    monitor: None,
                    delay_ms: 0
                },
                &output,
                "region",
                Some(InteractionMode::Overlay),
            ),
            "an explicit mode flag must not delegate to the daemon"
        );
    }

    // ── Phase 5 Task 002: shell state construction ────────────────

    /// Both shells (editor window + overlay) start from the same
    /// persisted `[ui]` defaults — the factored builder must map
    /// every field.
    #[cfg(feature = "egui")]
    #[test]
    fn state_from_config_applies_ui_defaults() {
        use flintshot_core::{Color, EditorState, Tool};
        let mut cfg = Config::default();
        cfg.ui.default_color = Color::rgb(1, 2, 3);
        cfg.ui.default_text_color = Color::rgb(4, 5, 6);
        cfg.ui.default_stroke_width = 7.0;
        cfg.ui.default_font_size = 21.0;
        let img = image::RgbaImage::from_raw(2, 2, vec![255; 16]).expect("2×2×4 bytes");
        let state: EditorState = state_from_config(img, &cfg);
        assert_eq!(state.color, Color::rgb(1, 2, 3));
        assert_eq!(state.text_color, Color::rgb(4, 5, 6));
        assert_eq!(state.stroke_width, 7.0);
        assert_eq!(state.font_size, 21.0);
        // Untouched baseline: the tool default stays core's choice
        // here — the overlay forces Select inside `OverlayApp::new`,
        // not in the shared builder.
        assert_eq!(state.tool, Tool::Move);
    }

    /// The persisted `[ui] flip-to-move` knob must reach the state
    /// both ways — it gates the commit-flip policy for the session.
    #[cfg(feature = "egui")]
    #[test]
    fn state_from_config_maps_flip_to_move_both_ways() {
        let img = || image::RgbaImage::from_raw(2, 2, vec![255; 16]).expect("2×2×4 bytes");
        let mut cfg = Config::default();
        cfg.ui.flip_to_move = false;
        let state = state_from_config(img(), &cfg);
        assert!(!state.flip_to_move, "false config must reach the state");
        let state = state_from_config(img(), &Config::default());
        assert!(state.flip_to_move, "true config must reach the state");
    }
}
