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
use flintshot_core::{capture_fullscreen, Config, ImageMime, Point, Rect, UploadContext};
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
    pub output: OutputArgs,
}

/// Arguments for `flintshot fullscreen`.
#[derive(Debug, Args)]
pub struct FullscreenArgs {
    /// Index of the monitor to capture (0-based). Defaults to the active monitor.
    #[arg(long, value_name = "N")]
    pub monitor: Option<u32>,
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

/// Arguments for `flintshot open`.
#[derive(Debug, Args)]
pub struct OpenArgs {
    /// Path to the image file to open in the editor.
    pub path: PathBuf,
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
    if delegate_to_daemon(
        &Request::CaptureRegion {
            monitor: args.monitor,
            delay_ms: args.delay_ms,
        },
        &args.output,
        "region",
    ) {
        return Ok(());
    }
    if args.delay_ms > 0 {
        tracing::debug!(delay_ms = args.delay_ms, "sleeping before capture");
        std::thread::sleep(Duration::from_millis(u64::from(args.delay_ms)));
    }
    capture_and_dispatch(args.monitor, "region", &args.output)
}

/// `flintshot fullscreen` — Phase 1 capture+editor, with Phase 2
/// detect-and-delegate. See [`run_region`].
pub fn run_fullscreen(args: &FullscreenArgs) -> Result<()> {
    if delegate_to_daemon(
        &Request::CaptureFullscreen {
            monitor: args.monitor,
        },
        &args.output,
        "fullscreen",
    ) {
        return Ok(());
    }
    capture_and_dispatch(args.monitor, "fullscreen", &args.output)
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
/// set or when `--out` is given (the PNG-write path is headless and must
/// run in-process; the daemon's worker would open an editor instead).
/// A delegation failure falls back to standalone with a warning rather
/// than failing the whole capture.
fn delegate_to_daemon(request: &Request, output: &OutputArgs, mode: &str) -> bool {
    if output.standalone || output.out.is_some() {
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
/// editor (no `--out`). Shared by [`run_region`] / [`run_fullscreen`];
/// `mode` is just the log label ("region" / "fullscreen").
fn capture_and_dispatch(monitor: Option<u32>, mode: &str, output: &OutputArgs) -> Result<()> {
    let img = capture_fullscreen(monitor)
        .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))
        .with_context(|| format!("capturing {mode} on monitor {monitor:?}"))?;

    match &output.out {
        Some(path) => {
            write_png(&img, path)?;
            tracing::info!(path = %path.display(), "{mode} capture saved");
        }
        None => run_editor_for_capture(img, mode)?,
    }
    Ok(())
}

/// Hand the captured image to the egui editor and log the outcome.
/// Returns `Ok(())` regardless of which `EditorOutcome` the user picked
/// (saved / copied / cancelled); only shell or render failures bubble
/// up as `Err`.
#[cfg(feature = "egui")]
fn run_editor_for_capture(img: image::RgbaImage, mode: &str) -> Result<()> {
    use flintshot_core::{Config, EditorOutcome, EditorShell, EditorState};

    // Apply persisted UI defaults (color / stroke / font) before opening
    // the editor (Task 15). A load failure is non-fatal — fall back to
    // EditorState::new's hardcoded defaults and let the editor proceed.
    let state = match Config::load() {
        Ok(cfg) => {
            let mut s = EditorState::new(img);
            s.color = cfg.ui.default_color;
            s.text_color = cfg.ui.default_text_color;
            s.stroke_width = cfg.ui.default_stroke_width;
            s.font_size = cfg.ui.default_font_size;
            s
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "could not load config; opening editor with built-in defaults"
            );
            EditorState::new(img)
        }
    };

    let outcome = flintshot_editor_egui::EguiShell::new()
        .run_editor(state)
        .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))
        .with_context(|| format!("running editor for {mode} capture"))?;

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
    Ok(())
}

/// Headless-build fallback: editor is unavailable without the `egui`
/// feature. The image argument is accepted (and dropped) purely so the
/// call site in [`capture_and_dispatch`] is uniform across feature sets.
#[cfg(not(feature = "egui"))]
fn run_editor_for_capture(_img: image::RgbaImage, _mode: &str) -> Result<()> {
    Err(color_eyre::eyre::eyre!(
        "editor not built; pass --out PATH to capture headlessly \
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

    flintshot_editor_egui::EguiShell::new()
        .with_pin_config(config)
        .run_pin(img, rect)
        .map_err(|e| color_eyre::eyre::eyre!("running pin window: {e}"))?;
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
        Ok(url) => {
            println!("{url}");
            copy_url_to_clipboard(&url);
            fire_upload_notification(true, &url);
            Ok(())
        }
        Err(e) => {
            let msg = format!("{e:#}");
            fire_upload_notification(false, &msg);
            Err(e)
        }
    }
}

/// Core upload logic — returns the URL on success. Extracted so
/// [`run_upload`] can layer on the side effects (stdout / clipboard /
/// notification) uniformly.
fn run_upload_core(args: &UploadArgs) -> Result<String> {
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

    Ok(outcome.url)
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

    // ── Upload helpers ───────────────────────────────────────────

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
}
