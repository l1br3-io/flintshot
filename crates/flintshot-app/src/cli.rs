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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};
use color_eyre::{eyre::Context, Result};
use flintshot_core::ipc::Request;
use flintshot_core::{
    select_backend, BackendReport, CaptureOptions, Config, ImageMime, SessionKind, UploadContext,
    UploadOutcome,
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
    /// The single Capture action: the OS's own selection UI where the
    /// selected backend supports it (portal interactive on Wayland),
    /// else today's monitor-grab → editor/overlay flow.
    Capture(CaptureArgs),
    /// Print the resolved configuration as TOML to stdout.
    Config,
    /// List registered capture backends, their availability, and what
    /// `auto` would select right now (probes only — never grabs
    /// pixels). Override via `--backend <ID>` on `region`/`fullscreen`
    /// or the `[capture] backend` config key.
    Backends,
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
/// shot"); `open` never requests the phase.
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
    /// Capture backend override (`auto`, `xcap`, `portal` on Linux).
    /// Defaults to config's `[capture] backend`.
    #[arg(long, value_name = "BACKEND")]
    pub backend: Option<String>,
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
    /// Capture backend override (`auto`, `xcap`, `portal` on Linux).
    /// Defaults to config's `[capture] backend`.
    #[arg(long, value_name = "BACKEND")]
    pub backend: Option<String>,
    #[command(flatten)]
    pub interaction: InteractionArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

/// Arguments for `flintshot capture` — the single Capture action.
///
/// Deliberately no `--monitor`: on the native path the OS's own
/// selection UI picks the region (monitor selection is its job), and
/// the fallback path grabs the primary monitor with selection in the
/// editor.
#[derive(Debug, Args)]
pub struct CaptureArgs {
    /// Delay before capturing, in milliseconds.
    #[arg(long, default_value_t = 0, value_name = "MILLIS")]
    pub delay_ms: u32,
    /// Capture backend override (`auto`, `xcap`, `portal` on Linux).
    /// Defaults to config's `[capture] backend`.
    #[arg(long, value_name = "BACKEND")]
    pub backend: Option<String>,
    /// Post-grab action on the native path: `editor` (default),
    /// `clipboard` (copy the grab, no windows), or `upload` (default
    /// upload backend; URL → clipboard). Overrides
    /// `[capture] after_grab`. Implies `--standalone` delegation
    /// semantics — the IPC request cannot carry it.
    #[arg(long, value_name = "ACTION", value_parser = validate_after_action)]
    pub after: Option<String>,
    #[command(flatten)]
    pub interaction: InteractionArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

/// Post-grab action on the native capture path (see `[capture]
/// after_grab`). The fallback (`xcap`) path never dispatches on this —
/// its grabs always land in the editor/overlay flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PostGrabAction {
    /// Open the grab in the editor (today's path; the default).
    Editor,
    /// Copy the grab to the system clipboard, no windows.
    Clipboard,
    /// Upload via the default upload backend; URL → clipboard.
    Upload,
}

/// Pure post-grab dispatch: CLI flag > config value, unknown values
/// resolve `PostGrabAction::Editor` with a warning (string-tolerant
/// policy — the config keeps the raw string, so a typo neither bricks
/// loading nor gets rewritten by a save).
fn resolve_post_grab(config_value: &str, flag: Option<&str>) -> PostGrabAction {
    let raw = flag.unwrap_or(config_value);
    match raw {
        "clipboard" => PostGrabAction::Clipboard,
        "upload" => PostGrabAction::Upload,
        "editor" => PostGrabAction::Editor,
        other => {
            if flag.is_none() {
                tracing::warn!(
                    value = other,
                    "unknown after-grab action; falling back to \"editor\""
                );
            }
            PostGrabAction::Editor
        }
    }
}

/// Clap `value_parser` for `--after`: only the three known actions
/// parse; anything else is a hard CLI error naming the valid set (the
/// config field tolerates unknowns, a just-typed flag does not —
/// `validate_capture_backend_arg` precedent).
fn validate_after_action(value: &str) -> std::result::Result<String, String> {
    const VALID: [&str; 3] = ["editor", "clipboard", "upload"];
    if VALID.contains(&value) {
        Ok(value.to_string())
    } else {
        Err(format!(
            "unknown after action `{value}` (valid: {})",
            VALID.join(", ")
        ))
    }
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
    validate_capture_backend_arg(args.backend.as_deref())?;
    let interaction = args.interaction.mode_override();
    if delegate_to_daemon(
        &Request::CaptureRegion {
            monitor: args.monitor,
            delay_ms: args.delay_ms,
        },
        &args.output,
        "region",
        interaction,
        false,
    ) {
        return Ok(());
    }
    if args.delay_ms > 0 {
        tracing::debug!(delay_ms = args.delay_ms, "sleeping before capture");
        std::thread::sleep(Duration::from_millis(u64::from(args.delay_ms)));
    }
    capture_and_dispatch(
        args.monitor,
        "region",
        &args.output,
        interaction,
        args.backend.as_deref(),
    )
}

/// `flintshot fullscreen` — Phase 1 capture+editor, with Phase 2
/// detect-and-delegate. See [`run_region`].
pub fn run_fullscreen(args: &FullscreenArgs) -> Result<()> {
    validate_capture_backend_arg(args.backend.as_deref())?;
    let interaction = args.interaction.mode_override();
    if delegate_to_daemon(
        &Request::CaptureFullscreen {
            monitor: args.monitor,
        },
        &args.output,
        "fullscreen",
        interaction,
        false,
    ) {
        return Ok(());
    }
    capture_and_dispatch(
        args.monitor,
        "fullscreen",
        &args.output,
        interaction,
        args.backend.as_deref(),
    )
}

/// `flintshot capture` — the single Capture action
/// (native-capture-ux). One command on every surface: the selected
/// backend (CLI `--backend` > `[capture] backend` > auto) decides the
/// flow —
///
/// - **native**: backend supports selection → the OS's own UI picks
///   the region (`capture_selection`), then the grab routes to the
///   editor (`--out` writes a PNG instead). Dismissal and interactive
///   Response timeouts exit quietly (exit 0, one info log).
/// - **fallback**: today's `region` flow unchanged — primary-monitor
///   grab under the prepare handshake, then editor/overlay with
///   selection-first.
///
/// Post-grab routing beyond editor/`--out` (`[capture] after_grab`)
/// lands with the `after_grab` task.
pub fn run_capture(args: &CaptureArgs) -> Result<()> {
    validate_capture_backend_arg(args.backend.as_deref())?;
    let interaction = args.interaction.mode_override();
    if delegate_to_daemon(
        &Request::Capture {
            delay_ms: args.delay_ms,
        },
        &args.output,
        "capture",
        interaction,
        args.after.is_some(),
    ) {
        return Ok(());
    }
    if args.delay_ms > 0 {
        tracing::debug!(delay_ms = args.delay_ms, "sleeping before capture");
        std::thread::sleep(Duration::from_millis(u64::from(args.delay_ms)));
    }

    let watch = SupersedeWatch::subscribe_at(None);
    if watch.superseded() {
        tracing::info!("capture worker superseded before its grab; exiting");
        return Ok(());
    }

    let backend = resolve_capture_backend(args.backend.as_deref())?;
    if backend.supports_selection() {
        let post_grab = resolve_post_grab(
            &cfg_for_post_grab().capture.after_grab,
            args.after.as_deref(),
        );
        native_capture_and_dispatch(
            &args.output,
            interaction,
            &watch,
            backend.as_ref(),
            post_grab,
        )
    } else {
        // GOTCHA: the fallback flow ignores post-grab routing —
        // selection happens in the editor/overlay on this path, and a
        // raw primary-monitor dump to the clipboard (or an unannotated
        // upload of one) is never what the user meant. Every fallback
        // grab lands in today's editor flow; only the native path,
        // where the OS UI already performed the selection, dispatches
        // on `after_grab`. The mode label stays "region" so the
        // editor's selection-first landing phase and logs stay
        // byte-identical.
        capture_and_dispatch(
            None,
            "region",
            &args.output,
            interaction,
            args.backend.as_deref(),
        )
    }
}

/// Config load for the post-grab dispatch decision (lossy: a broken
/// config warns and falls back to built-in defaults, whose
/// `after_grab` is `editor`).
fn cfg_for_post_grab() -> Config {
    match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "could not load config; using built-in defaults");
            Config::default()
        }
    }
}

/// Resolve the capture backend for the flow decision (CLI > config >
/// auto). Shared by [`run_capture`]'s two flows so both branches log
/// the same resolution.
fn resolve_capture_backend(
    backend_pin: Option<&str>,
) -> Result<Box<dyn flintshot_core::capture::Capturer>> {
    let cfg = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "could not load config; using built-in defaults");
            Config::default()
        }
    };
    let preference = backend_pin.or(Some(cfg.capture.backend.as_str()));
    let backend = select_backend(preference).map_err(|e| color_eyre::eyre::eyre!(e.to_string()))?;
    tracing::info!(backend = backend.id(), "capture backend resolved");
    Ok(backend)
}

/// Native flow: run the OS's selection UI under the prepare handshake
/// (flintshot's own chrome must not sit in the portal's frozen frame),
/// then dispatch the grab on `post_grab` — `--out` PNG (wins over
/// everything), editor shell, clipboard copy, or upload. `None`-style
/// quiet exits (superseded, dismissed, timed out) return `Ok(())`
/// with one info log; real failures bubble.
fn native_capture_and_dispatch(
    output: &OutputArgs,
    interaction: Option<InteractionMode>,
    watch: &SupersedeWatch,
    backend: &dyn flintshot_core::capture::Capturer,
    post_grab: PostGrabAction,
) -> Result<()> {
    // The guard mirrors `capture_prepared`: the handshake, the grab,
    // and the restore all sit inside its scope so the tray returns the
    // instant the portal takes its pixels.
    let _prepared = CaptureGuard::request_at(None);
    if watch.superseded() {
        tracing::info!("native capture superseded before the grab; exiting");
        return Ok(());
    }
    let cfg = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "could not load config; using built-in defaults");
            Config::default()
        }
    };
    // `CaptureOptions` is `#[non_exhaustive]`: mutate-from-default is
    // the cross-crate construction path.
    let mut opts = CaptureOptions::default();
    opts.include_cursor = cfg.capture.include_cursor;

    tracing::info!(
        backend = backend.id(),
        "opening the OS selection UI (native capture)"
    );
    let img = match backend.capture_selection(&opts) {
        Ok(img) => img,
        Err(e) if quiet_capture_exit(&e) => {
            // Dismissal / Response timeout: not an error UX (spec
            // Flow). One log line, exit 0, no notification.
            tracing::info!(error = %e, "native capture dismissed; exiting quietly");
            return Ok(());
        }
        Err(e) => {
            return Err(color_eyre::eyre::eyre!(e.to_string()))
                .with_context(|| "native capture via the OS selection UI");
        }
    };

    if let Some(path) = &output.out {
        write_png(&img, path)?;
        tracing::info!(path = %path.display(), "native capture saved");
        return Ok(());
    }

    match post_grab {
        PostGrabAction::Editor => {
            if watch.superseded() {
                tracing::info!("native capture superseded before the shell; exiting cancelled");
                return Ok(());
            }
            // Mode label "capture" (not "region"): selection already
            // happened in the OS UI — the editor opens plain.
            run_capture_shell(img, "capture", interaction, watch)?;
        }
        PostGrabAction::Clipboard => {
            copy_image_to_clipboard(&img)?;
            fire_capture_notification("Capture copied to clipboard");
            tracing::info!("native capture copied to clipboard");
        }
        PostGrabAction::Upload => {
            upload_capture_and_report(&img)?;
        }
    }
    Ok(())
}

/// Copy the grab to the system clipboard as an RGBA image. Unlike the
/// best-effort URL copies, a failure here is a real capture failure —
/// the user asked for the clipboard and nothing else happened.
fn copy_image_to_clipboard(img: &image::RgbaImage) -> Result<()> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| color_eyre::eyre::eyre!("clipboard: {e}"))?;
    clipboard
        .set_image(arboard::ImageData {
            width: img.width() as usize,
            height: img.height() as usize,
            bytes: std::borrow::Cow::Borrowed(img.as_raw()),
        })
        .map_err(|e| color_eyre::eyre::eyre!("copying image to clipboard: {e}"))
}

/// Upload the grab through the default upload backend and apply the
/// same side effects as `flintshot upload`: URL → clipboard,
/// notification, and the daemon report (which broadcasts
/// `Event::Uploaded` — the window-less flow's equivalent of the
/// editor's save report).
fn upload_capture_and_report(img: &image::RgbaImage) -> Result<()> {
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), ImageFormat::Png)
        .with_context(|| "encoding the capture for upload")?;

    let cfg = cfg_for_post_grab();
    let backend_id = cfg.upload.default_backend.clone();
    let backend = crate::upload::resolve_backend(&backend_id)
        .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))
        .with_context(|| format!("resolving upload backend `{backend_id}`"))?;
    let entry_name = credential_entry_name(&backend_id);
    let entry = keyring::Entry::new("flintshot", &entry_name).ok();
    let ctx = UploadContext {
        credentials: entry.as_ref(),
        filename: Some("flintshot-capture.png"),
    };

    tracing::info!(%backend_id, "uploading native capture");
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| color_eyre::eyre::eyre!("creating tokio runtime: {e}"))?;
    let outcome = runtime
        .block_on(backend.upload(&png, ImageMime::Png, &ctx))
        .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))?;

    println!("{}", outcome.url);
    copy_url_to_clipboard(&outcome.url);
    fire_upload_notification(true, &outcome.url);
    report_uploaded_to_daemon(&outcome.url, outcome.delete_handle.as_deref());
    tracing::info!(%outcome.url, "native capture uploaded");
    Ok(())
}

/// Best-effort desktop notification for the window-less capture
/// paths (`after_grab = clipboard`): with no editor window to show
/// the result, the notification is the only feedback.
fn fire_capture_notification(message: &str) {
    if let Err(e) = notify_rust::Notification::new()
        .appname("flintshot")
        .summary("flintshot capture")
        .body(message)
        .show()
    {
        tracing::warn!(error = %e, "could not show desktop notification");
    }
}

/// Classify a capture error as a **quiet exit**: the user dismissed
/// the OS selection UI, or the interactive Response never arrived
/// within its budget (spike C: the wedged-session hang). Both are
/// "changed my mind" outcomes — exit 0 with one info log, never an
/// error toast.
///
/// GOTCHA: substring matching over the backend's error detail is the
/// established contract — `capture_selection` documents
/// `"cancelled"` (portal code 1, macOS `"permission"` precedent) and
/// the portal adds `"timed out"` for the interactive budget. Core's
/// error enum is stringly for backend detail by design (0.x; see
/// `FlintshotError::Capture`).
fn quiet_capture_exit(err: &flintshot_core::FlintshotError) -> bool {
    match err {
        flintshot_core::FlintshotError::Capture { detail } => {
            detail.contains("cancelled") || detail.contains("timed out")
        }
        _ => false,
    }
}

/// Validate a `--backend` value for `region`/`fullscreen`: `auto`,
/// any id registered in this build, or nothing. Unlike the config
/// field (which tolerates unknown strings), a bad CLI value is a hard
/// error — the user just typed it, mirroring `validate_auth_backend`
/// and upload's `--backend`.
fn validate_capture_backend_arg(backend: Option<&str>) -> Result<()> {
    let Some(pin) = backend else {
        return Ok(());
    };
    if pin == "auto" {
        return Ok(());
    }
    let valid: Vec<&str> = flintshot_core::registered_backends()
        .iter()
        .map(|b| b.id())
        .collect();
    if valid.contains(&pin) {
        Ok(())
    } else {
        Err(color_eyre::eyre::eyre!(
            "unknown capture backend `{pin}` (valid: auto, {})",
            valid.join(", ")
        ))
    }
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
/// if delegated (the caller should exit). Skipped when `--standalone`
/// is set, when `--out` is given (the PNG-write path is headless and
/// must run in-process; the daemon's worker would open an editor
/// instead), when an explicit `--overlay`/`--editor` flag is set (the
/// IPC request cannot carry it, so honoring the flag means running
/// here), or when `ipc_incompatible_flag` is set (`--after`: same
/// cannot-carry-it rationale). A delegation failure falls back to
/// standalone with a warning rather than failing the whole capture.
fn delegate_to_daemon(
    request: &Request,
    output: &OutputArgs,
    mode: &str,
    interaction: Option<InteractionMode>,
    ipc_incompatible_flag: bool,
) -> bool {
    if output.standalone || output.out.is_some() {
        return false;
    }
    if ipc_incompatible_flag {
        tracing::debug!(
            "{mode}: explicit flag that cannot ride the IPC request; running standalone \
             (the daemon worker would re-resolve it from config)"
        );
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
/// `backend_pin` is the CLI `--backend` override (config joins in
/// [`capture_prepared`], CLI > config per the upload `--backend`
/// precedent).
///
/// The worker subscribes to daemon events for its whole lifetime: a
/// supersede before the shell opens cancels the worker instead
/// (never open a window the replacing capture would grab through).
fn capture_and_dispatch(
    monitor: Option<u32>,
    mode: &str,
    output: &OutputArgs,
    interaction: Option<InteractionMode>,
    backend_pin: Option<&str>,
) -> Result<()> {
    let watch = SupersedeWatch::subscribe_at(None);
    if watch.superseded() {
        tracing::info!("{mode} worker superseded before its grab; exiting");
        return Ok(());
    }
    let Some(img) = capture_prepared(monitor, mode, &watch, None, backend_pin)? else {
        tracing::info!("{mode} worker superseded during the prepare handshake; exiting");
        return Ok(());
    };

    if let Some(path) = &output.out {
        write_png(&img, path)?;
        tracing::info!(path = %path.display(), "{mode} capture saved");
    } else {
        if watch.superseded() {
            tracing::info!("{mode} worker superseded before the shell; exiting cancelled");
            return Ok(());
        }
        run_capture_shell(img, mode, interaction, &watch)?;
    }
    Ok(())
}

/// Grab the monitor with every flintshot surface cleared from the
/// screen (smart-window-hiding; the ADR-0036 tray-hide generalized):
/// the prepare handshake, the capture, and the restore report all sit
/// inside the guard's scope, so the tray returns the
/// instant the pixels are taken — success or failure — while the
/// editor / overlay session beyond this function never holds anything
/// hostage. No daemon (standalone capture) costs one socket probe.
///
/// `None` means a supersede arrived mid-handshake (Task 004's
/// subscription flag): the capture is abandoned without grabbing —
/// the newer capture owns the screen.
///
/// `backend_pin` is the CLI `--backend` value; the config's
/// `[capture] backend` joins here (CLI > config > auto — upload's
/// `--backend` precedence). The grab goes through the backend
/// registry; the chosen id is logged so a mis-pinned backend is
/// diagnosable from the worker log.
fn capture_prepared(
    monitor: Option<u32>,
    mode: &str,
    watch: &SupersedeWatch,
    override_path: Option<&std::path::Path>,
    backend_pin: Option<&str>,
) -> Result<Option<image::RgbaImage>> {
    let _prepared = CaptureGuard::request_at(override_path);
    if watch.superseded() {
        return Ok(None);
    }
    let cfg = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "could not load config; using built-in defaults");
            Config::default()
        }
    };
    let preference = backend_pin.or(Some(cfg.capture.backend.as_str()));
    let backend = select_backend(preference).map_err(|e| color_eyre::eyre::eyre!(e.to_string()))?;
    tracing::info!(backend = backend.id(), "{mode} capture backend resolved");
    // `CaptureOptions` is `#[non_exhaustive]`: mutate-from-default is
    // the cross-crate construction path.
    let mut opts = CaptureOptions::default();
    opts.include_cursor = cfg.capture.include_cursor;
    backend
        .capture_monitor(monitor, &opts)
        .map(Some)
        .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))
        .with_context(|| format!("capturing {mode} on monitor {monitor:?}"))
}

/// RAII screen-clear for one capture: on construction, asks a running
/// daemon to clear every flintshot surface (app window, tray)
/// for the grab; on drop, reports the grab finished so the tray
/// returns. Best-effort end to end — a
/// capture must never fail because the daemon was slow, absent, or
/// hostile. See ADR-0036 (extended by smart-window-hiding).
struct CaptureGuard {
    /// The daemon registered our prepare, so a
    /// [`Request::ReportCaptured`] is owed to release it.
    report_owed: bool,
}

impl CaptureGuard {
    /// Testable seam: request against an explicit socket path.
    fn request_at(override_path: Option<&std::path::Path>) -> Self {
        match crate::ipc_client::prepare_capture_to(override_path) {
            Ok(crate::ipc_client::PrepareCaptureOutcome::NoDaemon) => Self { report_owed: false },
            // The unacked case already warned inside the helper; the
            // report is owed either way (the daemon registered us).
            Ok(crate::ipc_client::PrepareCaptureOutcome::Registered { .. }) => {
                Self { report_owed: true }
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "prepare-capture handshake failed; capturing with flintshot chrome as-is"
                );
                Self { report_owed: false }
            }
        }
    }
}

impl Drop for CaptureGuard {
    fn drop(&mut self) {
        if self.report_owed {
            report_captured_to_daemon();
        }
    }
}

/// The close-hook cell shared with the supersede listener thread.
type CloseHookSlot = Arc<std::sync::Mutex<Option<Box<dyn Fn() + Send>>>>;

/// A capture worker's session-long subscription to daemon events,
/// narrowed to the one thing the worker cares about outside its own
/// window: [`Event::CaptureSuperseded`] — a newer capture replaced
/// this session, so close the window gracefully and exit cancelled
/// (see the Replace policy, smart-window-hiding).
///
/// A listener thread owns the subscription stream; [`Self::superseded`]
/// is the flag it sets, [`Self::set_close_hook`] installs the
/// window-closing callback fired at the same moment (the egui
/// `CloseHandle` closes the window through its context — the
/// documented thread-safe path). No daemon (standalone capture):
/// inert — no listener, the flag never fires, today's behavior.
/// One socket probe is the whole cost.
///
/// The thread is detached by design: it ends when the stream closes
/// (daemon shutdown) or the worker process exits; nothing in the
/// worker blocks on it.
struct SupersedeWatch {
    /// Set the moment `CaptureSuperseded` arrives.
    flag: Arc<AtomicBool>,
    /// Window-close callback, fired once at supersede time (or
    /// immediately on registration if the supersede already arrived).
    close_hook: CloseHookSlot,
}

impl SupersedeWatch {
    /// An inert watch (standalone / no daemon): never superseded.
    fn inert() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            close_hook: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Subscribe to the daemon at `override_path` (or the default
    /// socket). Failure to connect degrades to [`Self::inert`] —
    /// graceful close is an optimization the hard kill backstops.
    /// One socket probe is the whole standalone cost.
    fn subscribe_at(override_path: Option<&std::path::Path>) -> Self {
        let watch = Self::inert();
        let flag = Arc::clone(&watch.flag);
        let close_hook = Arc::clone(&watch.close_hook);
        // Owned for the thread: connect and pump must share one
        // reactor (a stream registered on a dropped runtime is dead).
        let path = override_path.map(std::path::PathBuf::from);
        if std::thread::Builder::new()
            .name("flintshot-supersede-watch".into())
            .spawn(move || watch_supersede(path.as_deref(), flag, close_hook))
            .is_err()
        {
            tracing::warn!("spawning the supersede watch; continuing without graceful close");
        }
        watch
    }

    /// Whether a newer capture superseded this session.
    fn superseded(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    /// The flag cell itself (test seam).
    #[cfg(test)]
    fn flag(&self) -> &Arc<AtomicBool> {
        &self.flag
    }

    /// Install the window-close callback. If the supersede already
    /// arrived, the hook fires immediately — the window may have
    /// opened after the broadcast (recapture chains).
    fn set_close_hook(&self, hook: Box<dyn Fn() + Send>) {
        let mut slot = self
            .close_hook
            .lock()
            .expect("supersede hook mutex poisoned");
        if self.flag.load(Ordering::Acquire) {
            drop(slot);
            hook();
        } else {
            *slot = Some(hook);
        }
    }
}

/// The listener thread body: subscribe, then read the stream until
/// `CaptureSuperseded` (fire the flag + hook, one shot) or the
/// stream ends. Connect and read share one reactor — a stream moved
/// off the runtime it connected on is unusable.
fn watch_supersede(
    override_path: Option<&std::path::Path>,
    flag: Arc<AtomicBool>,
    close_hook: CloseHookSlot,
) {
    use flintshot_core::ipc::Event;

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::warn!(error = %e, "supersede watch runtime; graceful close disabled");
            return;
        }
    };
    runtime.block_on(async move {
        let stream = match crate::ipc_client::subscribe_to(override_path).await {
            Ok(Some(stream)) => stream,
            Ok(None) => return, // no daemon: inert
            Err(e) => {
                tracing::warn!(error = %e, "subscribing for supersede; continuing without");
                return;
            }
        };
        let mut stream = stream;
        loop {
            // Daemon gone (or frame corrupt): nothing more to hear.
            let Ok(frame) = crate::ipc_server::read_frame(&mut stream).await else {
                break;
            };
            let event = match crate::ipc_server::decode_event(&frame) {
                Ok(event) => event,
                Err(e) => {
                    tracing::debug!(error = %e, "skipping undecodable event");
                    continue;
                }
            };
            if matches!(event, Event::CaptureSuperseded) {
                tracing::info!("capture superseded by a newer one; closing gracefully");
                flag.store(true, Ordering::Release);
                let hook = close_hook
                    .lock()
                    .expect("supersede hook mutex poisoned")
                    .take();
                if let Some(hook) = hook {
                    hook();
                }
                break;
            }
        }
    });
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
/// editor window. `watch` is the worker's session subscription —
/// passed down so the shell's windows register their close handles
/// and the recapture loops honor a supersede between sessions.
fn run_capture_shell(
    img: image::RgbaImage,
    mode: &str,
    interaction: Option<InteractionMode>,
    watch: &SupersedeWatch,
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
        InteractionMode::Overlay => run_overlay_with_config(img, mode, &cfg, watch),
        InteractionMode::Editor => run_editor_with_config(img, mode, selection_first, &cfg, watch),
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
    // The `open` path is not a capture session: the daemon never
    // supersedes it, so an inert watch keeps the signature uniform.
    run_editor_with_config(img, mode, false, &cfg, &SupersedeWatch::inert())
}

/// [`run_editor_for_capture`] over an already-loaded config (the
/// interaction-mode resolution path loads config once and shares it).
/// `selection_first` opens the region landing phase (Select
/// pre-armed, dim mask, hint line) — see [`selection_first_for`]; the
/// `open` path always passes `false`.
///
/// A `NewCapture` outcome (the editor's `+` chip / Ctrl+N) loops:
/// the window has closed — it cannot be in the next shot — so we
/// wait out the unmap grace, grab a fresh monitor, and re-run the
/// session. A region recapture re-enters selection-first;
/// fullscreen stays plain (the whole monitor is the shot). Any other
/// outcome ends the cycle. A supersede arriving anywhere in the
/// chain — mid-session (the close handle closes the window, the
/// outcome resolves `Cancelled`) or between sessions
/// ([`session_next`] stops the chain) — ends the cycle as cancelled.
/// The worker process persists across recaptures, so the daemon's
/// `SessionEnded` (and the app window's restore) waits for the whole
/// chain, Win11-style.
#[cfg(feature = "egui")]
fn run_editor_with_config(
    img: image::RgbaImage,
    mode: &str,
    selection_first: bool,
    cfg: &Config,
    watch: &SupersedeWatch,
) -> Result<()> {
    // One close handle for the whole chain: each session's window
    // installs itself; a supersede closes whichever is open.
    let close = flintshot_editor_egui::CloseHandle::new();
    let close_for_hook = close.clone();
    watch.set_close_hook(Box::new(move || close_for_hook.close()));

    let mut session = Some((img, selection_first));
    while let Some((img, selection_first)) = session.take() {
        let outcome = run_editor_session(img, mode, selection_first, cfg, &close)?;
        match session_next(&outcome, watch.superseded()) {
            SessionNext::Stop => {
                tracing::info!("editor chain stopped: superseded");
                report_editor_outcome(outcome);
                return Ok(());
            }
            SessionNext::Recapture(mode) => {
                let Some(fresh) = recapture_image(watch)? else {
                    tracing::info!("recapture superseded; ending the chain");
                    report_editor_outcome(outcome);
                    return Ok(());
                };
                // A region recapture re-enters selection-first;
                // fullscreen stays plain (the whole monitor is the shot).
                let region = selection_first_for_recapture(mode);
                session = Some((fresh, region));
                tracing::info!(
                    selection_first = region,
                    "new capture from the editor; reopening"
                );
            }
            SessionNext::Report => {
                report_editor_outcome(outcome);
                return Ok(());
            }
        }
    }
    Ok(())
}

/// What the editor / overlay session loop does with one session's
/// outcome, given whether a supersede was seen. Pure so the loop's
/// decision (supersede beats even a recapture request) is
/// unit-testable.
#[cfg(feature = "egui")]
fn session_next(outcome: &flintshot_core::EditorOutcome, superseded: bool) -> SessionNext {
    if superseded {
        return SessionNext::Stop;
    }
    match recapture_request(outcome) {
        Some(mode) => SessionNext::Recapture(mode),
        None => SessionNext::Report,
    }
}

/// The session-loop decision (see [`session_next`]).
#[cfg(feature = "egui")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionNext {
    /// End the chain (superseded): report the outcome and exit.
    Stop,
    /// The outcome asked for a fresh capture of `mode`.
    Recapture(flintshot_core::NewCaptureMode),
    /// A terminal outcome; report it and exit.
    Report,
}

/// The shared recapture step: the closed window needs the unmap
/// grace before the fresh grab (same bound the app window's hide
/// uses), then a monitor is captured with the screen prepared.
/// `None` = superseded mid-handshake: abandon the chain.
///
/// No CLI backend pin here: like the monitor choice, a recapture is a
/// fresh grab — the config's `[capture] backend` re-resolves inside
/// [`capture_prepared`].
#[cfg(feature = "egui")]
fn recapture_image(watch: &SupersedeWatch) -> Result<Option<image::RgbaImage>> {
    std::thread::sleep(Duration::from_millis(crate::app_window::HIDE_GRACE_MS));
    capture_prepared(None, "recapture", watch, None, None)
        .with_context(|| "recapturing for a new editor session")
}

/// Whether a recapture of `mode` reopens selection-first (region
/// does — the landing experience; fullscreen never does).
#[cfg(feature = "egui")]
fn selection_first_for_recapture(mode: flintshot_core::NewCaptureMode) -> bool {
    matches!(mode, flintshot_core::NewCaptureMode::Region)
}

/// One editor window's blocking session (the pre-recapture body of
/// [`run_editor_with_config`]). `close` installs into the window so
/// the supersede listener can close it from its own thread.
#[cfg(feature = "egui")]
fn run_editor_session(
    img: image::RgbaImage,
    mode: &str,
    selection_first: bool,
    cfg: &Config,
    close: &flintshot_editor_egui::CloseHandle,
) -> Result<flintshot_core::EditorOutcome> {
    use flintshot_core::EditorShell;

    let state = state_from_config(img, cfg);

    // One report per actual window: this session (fresh capture,
    // recapture reopen, or `open`) is about to open one.
    report_editor_window_opened_to_daemon();

    // Under the ambient-runtime guard: accesskit → zbus 5 needs a
    // reactor on this thread for the window to come up (see
    // `crate::ambient`). Dropped before `report_editor_outcome`.
    let outcome = crate::ambient::with_ambient_runtime(|| {
        let shell = if selection_first {
            flintshot_editor_egui::EguiShell::new()
                .with_selection_first()
                .with_close_handle(close.clone())
        } else {
            flintshot_editor_egui::EguiShell::new().with_close_handle(close.clone())
        };
        shell.run_editor(state)
    })
    .and_then(|ran| ran.map_err(|e| color_eyre::eyre::eyre!(e.to_string())))
    .with_context(|| format!("running editor for {mode} capture"))?;

    Ok(outcome)
}

/// Which recapture an outcome asks for, if any. Pure so the loop's
/// decision is unit-testable.
#[cfg(feature = "egui")]
#[allow(
    clippy::wildcard_enum_match_arm,
    reason = "EditorOutcome is #[non_exhaustive]; `_` is mandatory"
)]
fn recapture_request(
    outcome: &flintshot_core::EditorOutcome,
) -> Option<flintshot_core::NewCaptureMode> {
    match outcome {
        flintshot_core::EditorOutcome::NewCapture { mode } => Some(*mode),
        _ => None,
    }
}

/// Run the Phase 5 fullscreen frozen-capture overlay over an
/// already-loaded config (mirror of [`run_editor_with_config`]).
///
/// The overlay opens with `Tool::Select` (forced inside
/// `OverlayApp`), so the first drag draws a region selection;
/// `Esc` cancels, `Enter` accepts (Enter = Copy: the annotated
/// region lands on the clipboard and the overlay closes; the shell
/// lingers briefly to keep the X11 selection alive). A `NewCapture`
/// outcome (the rail's `+` chip) loops like the editor's: fresh
/// grab, fresh overlay. A supersede (mid-session via the close
/// handle, or between sessions via [`session_next`]) ends the chain
/// as cancelled, exactly like the editor loop.
#[cfg(feature = "egui")]
fn run_overlay_with_config(
    img: image::RgbaImage,
    mode: &str,
    cfg: &Config,
    watch: &SupersedeWatch,
) -> Result<()> {
    let close = flintshot_editor_egui::CloseHandle::new();
    let close_for_hook = close.clone();
    watch.set_close_hook(Box::new(move || close_for_hook.close()));

    let mut next_image = Some(img);
    while let Some(img) = next_image.take() {
        let state = state_from_config(img, cfg);

        // One report per actual window (the overlay session about to
        // run — fresh or recapture reopen).
        report_editor_window_opened_to_daemon();

        // Guard mirrors `run_editor_with_config` (see `crate::ambient`).
        let outcome = crate::ambient::with_ambient_runtime(|| {
            flintshot_editor_egui::EguiShell::new()
                .with_close_handle(close.clone())
                .run_overlay(state)
        })
        .and_then(|ran| ran.map_err(|e| color_eyre::eyre::eyre!(e.to_string())))
        .with_context(|| format!("running overlay for {mode} capture"))?;

        match session_next(&outcome, watch.superseded()) {
            SessionNext::Stop => {
                tracing::info!("overlay chain stopped: superseded");
                report_editor_outcome(outcome);
                return Ok(());
            }
            SessionNext::Recapture(_) => {
                let Some(fresh) = recapture_image(watch)? else {
                    tracing::info!("overlay recapture superseded; ending the chain");
                    report_editor_outcome(outcome);
                    return Ok(());
                };
                next_image = Some(fresh);
                tracing::info!("new capture from the overlay; reopening");
            }
            SessionNext::Report => {
                report_editor_outcome(outcome);
                return Ok(());
            }
        }
    }
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
        EditorOutcome::Cancelled => {
            tracing::info!("editor cancelled; no file written");
        }
        _ => tracing::warn!(?outcome, "editor returned unhandled outcome"),
    }
}

/// Headless-build fallback: editor is unavailable without the `egui`
/// feature. The image argument is accepted (and dropped) purely so the
/// call site in [`run_capture_shell`] is uniform across feature sets.
/// `selection_first` and `watch` are likewise accepted and dropped
/// (mirrors the egui-build signature).
#[cfg(not(feature = "egui"))]
fn run_editor_with_config(
    _img: image::RgbaImage,
    _mode: &str,
    _selection_first: bool,
    _cfg: &Config,
    _watch: &SupersedeWatch,
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
/// "last saved" (for `OpenLast` and upload-from-last).
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

/// Tell a running daemon an annotation window is about to run, so it
/// broadcasts `Event::EditorOpened` — the truthful window signal, one
/// report per actual window (fresh capture, recapture reopen, `open`;
/// never for `--out`). Best-effort: a missing daemon is a silent
/// no-op (the worker may be standalone). Mirrors
/// [`report_saved_to_daemon`].
fn report_editor_window_opened_to_daemon() {
    report_editor_window_opened_to_daemon_at(None);
}

/// Testable seam for [`report_editor_window_opened_to_daemon`]:
/// report against an explicit socket path.
fn report_editor_window_opened_to_daemon_at(override_path: Option<&std::path::Path>) {
    match crate::ipc_client::delegate_to(override_path, &Request::ReportEditorWindowOpened) {
        Ok(crate::ipc_client::DelegateOutcome::Delegated) => {
            tracing::debug!("reported an editor/overlay window opening");
        }
        Ok(crate::ipc_client::DelegateOutcome::NoDaemon) => {
            tracing::debug!("no daemon running; skipping the window-open report");
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "could not report the window opening; ignoring"
            );
        }
    }
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

/// `flintshot backends` — capture-backend diagnostic. Prints every
/// registered backend's probe result and the current `auto` pick;
/// always exits 0 (a diagnostic, not a health gate).
#[allow(
    clippy::unnecessary_wraps,
    reason = "every run_* keeps the Result shape the main dispatch arms share"
)]
pub fn run_backends() -> Result<()> {
    print!(
        "{}",
        format_backend_report(&flintshot_core::backend_report())
    );
    Ok(())
}

/// Rendered `flintshot backends` output (pure; unit-tested).
fn format_backend_report(report: &BackendReport) -> String {
    use std::fmt::Write as _;
    let mut out = format!("session: {}\n", session_str(report.session));
    for entry in &report.entries {
        if entry.available {
            let _ = writeln!(out, "{}: available ({})", entry.id, entry.name);
        } else if let Some(detail) = &entry.detail {
            let _ = writeln!(out, "{}: unavailable ({detail})", entry.id);
        } else {
            let _ = writeln!(out, "{}: unavailable", entry.id);
        }
    }
    let _ = writeln!(out, "auto selects: {}", report.auto_selects);
    out
}

/// Lower-case session-kind label for the report header. Future
/// variants degrade to `"other"` (`SessionKind` is non-exhaustive).
fn session_str(session: SessionKind) -> &'static str {
    match session {
        SessionKind::X11 => "x11",
        SessionKind::Wayland => "wayland",
        _ => "other",
    }
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

    // ── Editor recapture loop decisions ─────────────────────────

    #[cfg(feature = "egui")]
    #[test]
    fn recapture_request_maps_only_new_capture_outcomes() {
        use flintshot_core::{EditorOutcome, NewCaptureMode};
        assert_eq!(
            recapture_request(&EditorOutcome::NewCapture {
                mode: NewCaptureMode::Region
            }),
            Some(NewCaptureMode::Region)
        );
        assert_eq!(
            recapture_request(&EditorOutcome::NewCapture {
                mode: NewCaptureMode::Fullscreen
            }),
            Some(NewCaptureMode::Fullscreen)
        );
        for other in [
            EditorOutcome::Cancelled,
            EditorOutcome::CopiedToClipboard,
            EditorOutcome::Uploaded(String::new()),
            EditorOutcome::Saved(std::path::PathBuf::from("/tmp/x.png")),
        ] {
            assert!(
                recapture_request(&other).is_none(),
                "{other:?} must end the session, not loop"
            );
        }
    }

    #[cfg(feature = "egui")]
    #[test]
    fn region_recapture_reenters_selection_first_fullscreen_does_not() {
        use flintshot_core::NewCaptureMode;
        assert!(selection_first_for_recapture(NewCaptureMode::Region));
        assert!(!selection_first_for_recapture(NewCaptureMode::Fullscreen));
    }

    // ── Supersede watch (smart-window-hiding Task 004) ───────────

    #[cfg(feature = "egui")]
    #[test]
    fn session_next_stops_the_chain_when_superseded() {
        use flintshot_core::{EditorOutcome, NewCaptureMode};
        // A supersede between sessions beats even a NewCapture
        // outcome: the new capture owns the screen now.
        let next = session_next(
            &EditorOutcome::NewCapture {
                mode: NewCaptureMode::Region,
            },
            true,
        );
        assert!(matches!(next, SessionNext::Stop), "got {next:?}");
    }

    #[cfg(feature = "egui")]
    #[test]
    fn session_next_recaptures_when_not_superseded() {
        use flintshot_core::{EditorOutcome, NewCaptureMode};
        let next = session_next(
            &EditorOutcome::NewCapture {
                mode: NewCaptureMode::Fullscreen,
            },
            false,
        );
        assert!(
            matches!(next, SessionNext::Recapture(NewCaptureMode::Fullscreen)),
            "got {next:?}"
        );
    }

    #[cfg(feature = "egui")]
    #[test]
    fn session_next_reports_terminal_outcomes() {
        use flintshot_core::EditorOutcome;
        let next = session_next(&EditorOutcome::Cancelled, false);
        assert!(matches!(next, SessionNext::Report));
    }

    #[test]
    fn inert_watch_is_never_superseded() {
        // Standalone / no-daemon workers keep today's behavior: the
        // watch never fires, so no code path short-circuits.
        let watch = SupersedeWatch::inert();
        assert!(!watch.superseded());
    }

    #[test]
    fn watch_fires_late_close_hooks_immediately() {
        // A hook registered after the supersede already arrived must
        // still fire — the window may not have existed at broadcast
        // time but is open by the time the shell registers it.
        use std::sync::atomic::{AtomicBool, Ordering};
        let watch = SupersedeWatch::inert();
        watch.flag().store(true, Ordering::Release);
        let fired = Arc::new(AtomicBool::new(false));
        let probe = Arc::clone(&fired);
        watch.set_close_hook(Box::new(move || {
            probe.store(true, Ordering::Release);
        }));
        assert!(
            fired.load(Ordering::Acquire),
            "a late-registered hook fires on registration"
        );
    }

    /// The watch's listener against a real daemon: a broadcast
    /// `CaptureSuperseded` sets the flag and fires the close hook.
    /// `#[ignore]` (needs a real local socket). Run with:
    /// `cargo test -p flintshot-app -- --ignored supersede`
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "needs a real local socket (no display); run explicitly"]
    async fn supersede_watch_hears_a_real_broadcast() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let dir = tempfile::tempdir().expect("tempdir");
        let sock = dir.path().join("flintshot-supersede-watch.sock");
        let server = crate::ipc_server::IpcServer::bind(Some(&sock)).expect("binding");
        let bus = server.bus();
        let server_task = tokio::spawn(async move {
            let _ = server.serve().await;
        });

        // `subscribe_at` blocks on its own little runtime — keep it
        // off the test's reactor (mirrors the sync worker call site).
        let sock_clone = sock.clone();
        let watch =
            tokio::task::spawn_blocking(move || SupersedeWatch::subscribe_at(Some(&sock_clone)))
                .await
                .expect("subscribe task");
        assert!(!watch.superseded(), "fresh watch is not superseded");
        let fired = Arc::new(AtomicBool::new(false));
        let probe = Arc::clone(&fired);
        watch.set_close_hook(Box::new(move || {
            probe.store(true, Ordering::Release);
        }));

        // Retry the broadcast (bounded) to bridge the inherent
        // subscribe/register race, mirroring the ipc_server tests.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !(watch.superseded() && fired.load(Ordering::Acquire))
            && std::time::Instant::now() < deadline
        {
            bus.broadcast(flintshot_core::ipc::Event::CaptureSuperseded);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(watch.superseded(), "the flag fired on the broadcast");
        assert!(
            fired.load(Ordering::Acquire),
            "the close hook fired with the flag"
        );

        server_task.abort();
    }

    // ── Pin removal (remove-pin-to-desktop Task 002) ─────────────

    #[test]
    fn pin_subcommand_is_rejected() {
        let err = Cli::try_parse_from(["flintshot", "pin", "/tmp/x.png"])
            .expect_err("pin must not be a subcommand");
        assert!(
            err.to_string().to_lowercase().contains("unrecognized"),
            "expected an unrecognized-subcommand error, got: {err}"
        );
    }

    #[test]
    fn help_text_mentions_no_pin() {
        use clap::CommandFactory;
        let help = Cli::command().render_help().to_string();
        // Token-level match: "Snipping" (the app subcommand's copy)
        // contains "pin" as a substring and is not a pin reference.
        let mentions_pin = help
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|tok| tok.eq_ignore_ascii_case("pin"));
        assert!(
            !mentions_pin,
            "`--help` must not advertise a pin subcommand: {help}"
        );
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
    fn report_editor_window_opened_is_silent_when_no_daemon() {
        // Same contract: a standalone worker reports nothing, errors
        // nothing. The `--out` headless path never calls this at all
        // (it never opens a shell).
        report_editor_window_opened_to_daemon_at(Some(std::path::Path::new(
            "/this/does/not/exist/flintshot-test-window-opened.sock",
        )));
    }

    #[test]
    fn tray_hidden_guard_owes_no_report_without_a_daemon() {
        // Standalone capture: nothing was hidden, so Drop has nothing
        // to report — and must not attempt a connection. The helper
        // behind the skew fallback keeps the same contract.
        let outcome = crate::ipc_client::hide_tray_for_capture_to(Some(std::path::Path::new(
            "/this/does/not/exist/flintshot-test-guard.sock",
        )))
        .expect("absent socket is not an error");
        assert_eq!(
            outcome,
            crate::ipc_client::HideTrayOutcome::NoDaemon,
            "no daemon → no hide registered → no report owed"
        );
    }

    // ── CaptureGuard (smart-window-hiding Task 007) ──────────────

    #[test]
    fn capture_guard_owes_no_report_without_a_daemon() {
        let guard = CaptureGuard::request_at(Some(std::path::Path::new(
            "/this/does/not/exist/flintshot-test-capture-guard.sock",
        )));
        assert!(
            !guard.report_owed,
            "no daemon → vacuous prepare → no report owed"
        );
        drop(guard); // silent by contract
    }

    #[test]
    fn capture_prepared_abandons_when_superseded_before_the_grab() {
        // The flag may flip during the handshake wait (Task 004's
        // subscription): the guard returns without grabbing — no
        // display is ever touched.
        let watch = SupersedeWatch::inert();
        watch
            .flag()
            .store(true, std::sync::atomic::Ordering::Release);
        let outcome = capture_prepared(
            None,
            "region",
            &watch,
            Some(std::path::Path::new(
                "/this/does/not/exist/flintshot-test-superseded.sock",
            )),
            None,
        )
        .expect("an abandoned grab is Ok(None), not an error");
        assert!(outcome.is_none(), "superseded → no grab at all");
    }

    #[test]
    fn capture_prepared_grabs_when_not_superseded() {
        // Inert watch, no daemon: the vacuous prepare proceeds to the
        // grab — which needs a display, so this runs only under a
        // session. Headless CI asserts the abandonment path above.
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return; // headless: covered by capture_prepared_abandons
        }
        let watch = SupersedeWatch::inert();
        let outcome = capture_prepared(
            None,
            "fullscreen",
            &watch,
            Some(std::path::Path::new(
                "/this/does/not/exist/flintshot-test-grab.sock",
            )),
            None,
        );
        assert!(
            matches!(&outcome, Ok(Some(_)) | Err(_)),
            "not superseded: the grab runs ({outcome:?})"
        );
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

    // ── Capture backend helpers ──────────────────────────────────

    #[test]
    fn validate_capture_backend_accepts_auto_and_registered_ids() {
        assert!(validate_capture_backend_arg(None).is_ok());
        assert!(validate_capture_backend_arg(Some("auto")).is_ok());
        assert!(validate_capture_backend_arg(Some("xcap")).is_ok());
        for backend in flintshot_core::registered_backends() {
            assert!(
                validate_capture_backend_arg(Some(backend.id())).is_ok(),
                "registered id {} must validate",
                backend.id()
            );
        }
    }

    #[test]
    fn validate_capture_backend_rejects_unknown_naming_valid_ids() {
        let err = validate_capture_backend_arg(Some("bogus")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("bogus"), "error must name the bad id: {msg}");
        assert!(msg.contains("auto"), "error must list auto: {msg}");
        assert!(msg.contains("xcap"), "error must list xcap: {msg}");
    }

    #[test]
    fn region_and_fullscreen_parse_backend_flag() {
        let cli = Cli::parse_from(["flintshot", "region", "--backend", "xcap"]);
        match cli.command {
            Some(Command::Region(args)) => {
                assert_eq!(args.backend.as_deref(), Some("xcap"));
            }
            other => panic!("expected region, got {other:?}"),
        }
        let cli = Cli::parse_from(["flintshot", "fullscreen", "--backend", "auto"]);
        match cli.command {
            Some(Command::Fullscreen(args)) => {
                assert_eq!(args.backend.as_deref(), Some("auto"));
            }
            other => panic!("expected fullscreen, got {other:?}"),
        }
    }

    #[test]
    fn capture_parses_flag_set() {
        let cli = Cli::parse_from([
            "flintshot",
            "capture",
            "--delay-ms",
            "500",
            "--backend",
            "portal",
            "--out",
            "/tmp/shot.png",
            "--standalone",
        ]);
        match cli.command {
            Some(Command::Capture(args)) => {
                assert_eq!(args.delay_ms, 500);
                assert_eq!(args.backend.as_deref(), Some("portal"));
                assert_eq!(
                    args.output.out.as_deref(),
                    Some(std::path::Path::new("/tmp/shot.png"))
                );
                assert!(args.output.standalone);
                assert_eq!(args.interaction.mode_override(), None);
            }
            other => panic!("expected capture, got {other:?}"),
        }
    }

    #[test]
    fn capture_parses_bare_and_interaction_flags() {
        let cli = Cli::parse_from(["flintshot", "capture"]);
        match cli.command {
            Some(Command::Capture(args)) => {
                assert_eq!(args.delay_ms, 0);
                assert_eq!(args.backend, None);
                assert_eq!(args.output.out, None);
                assert!(!args.output.standalone);
            }
            other => panic!("expected capture, got {other:?}"),
        }
        let cli = Cli::parse_from(["flintshot", "capture", "--editor"]);
        match cli.command {
            Some(Command::Capture(args)) => {
                assert_eq!(
                    args.interaction.mode_override(),
                    Some(InteractionMode::Editor)
                );
            }
            other => panic!("expected capture, got {other:?}"),
        }
    }

    /// Pinned: `--monitor` is deliberately absent from `capture` —
    /// selection is the OS UI's job on the native path, and the
    /// fallback grabs the primary monitor.
    #[test]
    fn capture_rejects_monitor_flag() {
        assert!(
            Cli::try_parse_from(["flintshot", "capture", "--monitor", "0"]).is_err(),
            "capture must not carry --monitor"
        );
    }

    #[test]
    fn capture_parses_after_flag() {
        let cli = Cli::parse_from(["flintshot", "capture", "--after", "clipboard"]);
        match cli.command {
            Some(Command::Capture(args)) => {
                assert_eq!(args.after.as_deref(), Some("clipboard"));
            }
            other => panic!("expected capture, got {other:?}"),
        }
    }

    #[test]
    fn capture_rejects_unknown_after_action() {
        let err = Cli::try_parse_from(["flintshot", "capture", "--after", "print"]);
        assert!(err.is_err(), "unknown --after must be a hard CLI error");
        let rendered = err.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(
            rendered.contains("editor"),
            "error must name the valid actions: {rendered}"
        );
        assert!(
            rendered.contains("clipboard") && rendered.contains("upload"),
            "error must name every valid action: {rendered}"
        );
    }

    #[test]
    fn run_capture_rejects_unknown_backend_before_any_delegation() {
        let args = CaptureArgs {
            delay_ms: 0,
            backend: Some("nope".into()),
            after: None,
            interaction: InteractionArgs {
                overlay: false,
                editor: false,
            },
            output: OutputArgs {
                out: None,
                standalone: false,
            },
        };
        assert!(run_capture(&args).is_err());
    }

    /// Pure post-grab dispatch: the CLI flag beats config, unknown
    /// config values resolve `editor` (string-tolerant), and `editor`
    /// is the default throughout.
    #[test]
    fn post_grab_flag_beats_config() {
        use PostGrabAction::*;
        // Flag wins over every config value.
        assert_eq!(
            resolve_post_grab("clipboard", Some("upload")),
            Upload,
            "explicit flag must beat the config value"
        );
        assert_eq!(resolve_post_grab("upload", Some("editor")), Editor);
        assert_eq!(resolve_post_grab("editor", Some("clipboard")), Clipboard);
        // Config decides when no flag.
        assert_eq!(resolve_post_grab("clipboard", None), Clipboard);
        assert_eq!(resolve_post_grab("upload", None), Upload);
        assert_eq!(resolve_post_grab("editor", None), Editor);
        // Unknown config → editor (warn + default).
        assert_eq!(resolve_post_grab("print-it", None), Editor);
        // Unknown flag never reaches here (CLI validation rejects it),
        // but the fn stays total: unknown resolves editor.
        assert_eq!(resolve_post_grab("clipboard", Some("nope")), Editor);
    }

    /// The `--after` flag cannot ride the IPC request, so it must
    /// force the standalone path exactly like the interaction flags.
    #[test]
    fn after_flag_skips_daemon_delegation() {
        let output = OutputArgs {
            out: None,
            standalone: false,
        };
        assert!(
            !delegate_to_daemon(
                &Request::Capture { delay_ms: 0 },
                &output,
                "capture",
                None,
                true,
            ),
            "an explicit --after flag must not delegate to the daemon"
        );
    }

    /// Quiet-cancel classification: dismissal in the OS selection UI
    /// and the interactive Response timeout both exit quietly (exit 0,
    /// one info log, no notification). Everything else is a real
    /// failure the user must see.
    #[test]
    fn quiet_capture_exit_classifies_dismissal_and_timeout_only() {
        use flintshot_core::FlintshotError;
        let quiet = [
            "portal screenshot cancelled (response code 1)",
            "portal screenshot timed out after 300 s (no Response signal)",
        ];
        for detail in quiet {
            assert!(
                quiet_capture_exit(&FlintshotError::Capture {
                    detail: detail.into()
                }),
                "must classify quietly: {detail}"
            );
        }
        let loud = [
            "portal screenshot decode failed: bad bytes",
            "shm screencopy failed",
            "permission denied",
        ];
        for detail in loud {
            assert!(
                !quiet_capture_exit(&FlintshotError::Capture {
                    detail: detail.into()
                }),
                "must NOT be quiet: {detail}"
            );
        }
        assert!(
            !quiet_capture_exit(&FlintshotError::NotImplemented("no selection")),
            "non-capture errors are never quiet cancels"
        );
    }

    #[test]
    fn backends_subcommand_parses_bare() {
        let cli = Cli::parse_from(["flintshot", "backends"]);
        assert!(
            matches!(cli.command, Some(Command::Backends)),
            "backends takes no arguments"
        );
    }

    #[test]
    fn backend_report_prints_status_lines_and_final_auto_pick() {
        use flintshot_core::{BackendReport, BackendReportEntry, SessionKind};

        let report = BackendReport::new(
            SessionKind::Wayland,
            vec![
                BackendReportEntry::new("portal", "XDG Screenshot Portal", true, None),
                BackendReportEntry::new(
                    "xcap",
                    "xcap",
                    false,
                    Some("no display connection".to_string()),
                ),
            ],
            "portal",
        );
        let out = format_backend_report(&report);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "session: wayland");
        assert!(lines.contains(&"portal: available (XDG Screenshot Portal)"));
        assert!(lines.contains(&"xcap: unavailable (no display connection)"));
        assert_eq!(
            *lines.last().expect("non-empty report"),
            "auto selects: portal",
            "the auto pick is the final line"
        );
    }

    #[test]
    fn backend_report_renders_unavailable_without_detail() {
        use flintshot_core::{BackendReport, BackendReportEntry, SessionKind};

        let report = BackendReport::new(
            SessionKind::Other,
            vec![BackendReportEntry::new("xcap", "xcap", false, None)],
            "xcap",
        );
        let out = format_backend_report(&report);
        assert!(out.contains("session: other"));
        assert!(out.contains("xcap: unavailable\n"));
        assert!(out.ends_with("auto selects: xcap\n"));
    }

    #[test]
    fn run_region_rejects_unknown_backend_before_any_delegation() {
        let args = RegionArgs {
            monitor: None,
            delay_ms: 0,
            backend: Some("nope".into()),
            interaction: InteractionArgs {
                overlay: false,
                editor: false,
            },
            output: OutputArgs {
                out: None,
                standalone: false,
            },
        };
        assert!(run_region(&args).is_err());
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
                false,
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
