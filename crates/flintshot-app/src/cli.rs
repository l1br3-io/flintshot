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
use flintshot_core::{capture_fullscreen, Config};
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
            report_editor_save(&p);
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
    Ok(())
}

/// Worker → daemon: tell the running daemon that the editor saved to
/// `path` so it can broadcast [`Event::Saved`] to IPC subscribers and
/// record the path for `Request::PinLast` (Phase 3, Task 004).
///
/// Fire-and-forget and never fatal: standalone captures (no daemon) log
/// at `debug` and continue; the editor outcome stands either way. The
/// send happens *after* the editor window closes (we only report a
/// finalised outcome). Task 003 will add a sibling `Pinned(path)` report
/// by routing through the same [`report`](crate::ipc_client::report)
/// seam with a different [`Request`].
///
/// [`Event::Saved`]: flintshot_core::ipc::Event::Saved
#[cfg(feature = "egui")]
fn report_editor_save(path: &std::path::Path) {
    crate::ipc_client::report(&Request::ReportSaved {
        path: path.to_path_buf(),
    });
}

/// Headless-build fallback: matches [`run_editor_for_capture`]'s feature
/// gate so the helper is defined on every feature set (the call site is
/// uniformly gated).
#[cfg(not(feature = "egui"))]
fn report_editor_save(_path: &std::path::Path) {}

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

/// `flintshot pin <path>` — Phase 3.
///
/// Loads an image from disk (any format the `image` crate decodes) and
/// opens a borderless always-on-top floating window showing it. The pin
/// persists as its own process until the user closes it (`Esc` or the
/// WM close button). See ADR-0023 for why a pin is a separate process.
pub fn run_pin(args: &PinArgs) -> Result<()> {
    let img = image::open(&args.path)
        .map_err(|e| {
            color_eyre::eyre::eyre!("could not decode image at {}: {e}", args.path.display())
        })
        .with_context(|| format!("loading pin source {}", args.path.display()))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    #[allow(
        clippy::cast_precision_loss,
        reason = "image dims ≤ 2^24 are exact in f32"
    )]
    let rect = flintshot_core::Rect::from_xywh(0.0, 0.0, w as f32, h as f32);
    tracing::info!(
        path = %args.path.display(),
        dims = ?img.dimensions(),
        "opening pin window"
    );
    run_pin_for_image(img, rect)
}

/// Hand the decoded image to the egui pin window. Gated on the `egui`
/// feature, matching [`run_editor_for_capture`]'s pattern.
#[cfg(feature = "egui")]
fn run_pin_for_image(img: image::RgbaImage, rect: flintshot_core::Rect) -> Result<()> {
    use flintshot_core::EditorShell;
    flintshot_editor_egui::EguiShell::new()
        .run_pin(img, rect)
        .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))
        .context("running pin window")
}

/// Headless-build fallback: the pin window requires eframe, which is
/// gated on the `egui` feature. Without it the subcommand can't open a
/// window and surfaces a clear error rather than silently exiting.
#[cfg(not(feature = "egui"))]
fn run_pin_for_image(_img: image::RgbaImage, _rect: flintshot_core::Rect) -> Result<()> {
    Err(color_eyre::eyre::eyre!(
        "pin requires the `egui` feature; rebuild with default features"
    ))
}

/// `flintshot upload` — Phase 4.
pub fn run_upload(args: &UploadArgs) -> Result<()> {
    let backend = args.backend.as_deref().unwrap_or("(config default)");
    tracing::info!(
        path = %args.path.display(),
        backend,
        "upload subcommand: not implemented yet (Phase 4)"
    );
    if !args.path.exists() {
        return Err(color_eyre::eyre::eyre!(
            "no such file: {}",
            args.path.display()
        ))
        .with_context(|| format!("validating upload source: {}", args.path.display()));
    }
    Ok(())
}
