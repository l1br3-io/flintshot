//! CLI definition and subcommand dispatch.
//!
//! Phase 0 ships the full subcommand surface as stubs; later phases
//! replace each `run_*` body with real implementations.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use color_eyre::{eyre::Context, Result};

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
    /// Throwaway debug flag: load the config and print the resolved
    /// TOML to stdout. Removed in Phase 1 Task 3 when the real
    /// `config` subcommand lands.
    #[arg(long, hide = true)]
    pub dump_config: bool,

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

/// Arguments for `flintshot region`.
#[derive(Debug, Args)]
pub struct RegionArgs {
    /// Index of the monitor to capture (0-based). Defaults to the active monitor.
    #[arg(long, value_name = "N")]
    pub monitor: Option<u32>,
    /// Delay before capturing, in milliseconds.
    #[arg(long, default_value_t = 0, value_name = "MILLIS")]
    pub delay_ms: u32,
}

/// Arguments for `flintshot fullscreen`.
#[derive(Debug, Args)]
pub struct FullscreenArgs {
    /// Index of the monitor to capture (0-based). Defaults to the active monitor.
    #[arg(long, value_name = "N")]
    pub monitor: Option<u32>,
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

/// `flintshot region` — Phase 1.
// See `run_daemon` for the `allow`.
#[allow(clippy::unnecessary_wraps)]
pub fn run_region(args: &RegionArgs) -> Result<()> {
    tracing::info!(
        monitor = ?args.monitor,
        delay_ms = args.delay_ms,
        "region subcommand: not implemented yet (Phase 1)"
    );
    Ok(())
}

/// `flintshot fullscreen` — Phase 1.
// See `run_daemon` for the `allow`.
#[allow(clippy::unnecessary_wraps)]
pub fn run_fullscreen(args: &FullscreenArgs) -> Result<()> {
    tracing::info!(
        monitor = ?args.monitor,
        "fullscreen subcommand: not implemented yet (Phase 1)"
    );
    Ok(())
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
