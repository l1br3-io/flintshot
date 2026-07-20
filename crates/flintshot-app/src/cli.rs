//! CLI definition and subcommand dispatch.
//!
//! Phase 1 Task 3 wires `region` / `fullscreen` to real capture calls
//! and adds the `config` subcommand. The editor wiring (Task 6) will
//! reroute `region` / `fullscreen` through `EguiShell` when no `--out`
//! is given; for now both subcommands write a PNG.

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};
use color_eyre::{eyre::Context, Result};
use flintshot_core::{capture_fullscreen, Config};
use image::ImageFormat;

use crate::time_util::iso8601_now;

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
    /// Where to write the PNG. Defaults to `/tmp/flintshot-<timestamp>.png`.
    ///
    /// Format is always PNG regardless of the extension; a non-`.png`
    /// path still yields valid PNG bytes.
    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,
}

impl OutputArgs {
    /// Resolve the output path, falling back to the timestamped default
    /// when `--out` wasn't given. Pure — does not touch the filesystem.
    fn resolve_path(&self) -> PathBuf {
        match &self.out {
            Some(p) => p.clone(),
            None => PathBuf::from(format!("/tmp/flintshot-{}.png", iso8601_now())),
        }
    }
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
// Signature returns `Result<()>` so future phases can add real failures
// without rewriting call sites in `main`.
#[allow(clippy::unnecessary_wraps)]
pub fn run_daemon(_args: &DaemonArgs) -> Result<()> {
    tracing::info!("daemon subcommand: not implemented yet (Phase 2)");
    Ok(())
}

/// `flintshot region` — Phase 1 Task 3.
///
/// Captures the primary monitor and writes a PNG. The interactive
/// region-select tool lands in Task 14; for now `region` and
/// `fullscreen` produce equivalent output (full primary monitor).
pub fn run_region(args: &RegionArgs) -> Result<()> {
    if args.delay_ms > 0 {
        tracing::debug!(delay_ms = args.delay_ms, "sleeping before capture");
        std::thread::sleep(Duration::from_millis(u64::from(args.delay_ms)));
    }

    let img = capture_fullscreen(args.monitor)
        .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))
        .with_context(|| format!("capturing region on monitor {:?}", args.monitor))?;

    let path = args.output.resolve_path();
    write_png(&img, &path)?;
    tracing::info!(path = %path.display(), "region capture saved");
    Ok(())
}

/// `flintshot fullscreen` — Phase 1 Task 3.
pub fn run_fullscreen(args: &FullscreenArgs) -> Result<()> {
    let img = capture_fullscreen(args.monitor)
        .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))
        .with_context(|| format!("capturing fullscreen on monitor {:?}", args.monitor))?;

    let path = args.output.resolve_path();
    write_png(&img, &path)?;
    tracing::info!(path = %path.display(), "fullscreen capture saved");
    Ok(())
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
pub fn run_pin(args: &PinArgs) -> Result<()> {
    tracing::info!(
        path = %args.path.display(),
        "pin subcommand: not implemented yet (Phase 3)"
    );
    // Touch the path so we surface common mistakes (missing file, permission)
    // even in the stub — gives the user early feedback.
    if !args.path.exists() {
        return Err(color_eyre::eyre::eyre!(
            "no such file: {}",
            args.path.display()
        ));
    }
    Ok(())
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
