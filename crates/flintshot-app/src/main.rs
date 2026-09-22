//! `flintshot` binary entry point — CLI dispatch, daemon lifecycle,
//! tray icon, hotkeys. Phase 0 wires up the CLI shape and logging; the
//! subcommand bodies are filled in across Phases 1–4.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

mod ai;
#[cfg(feature = "egui")]
mod ambient;
#[cfg(feature = "egui")]
mod app_window;
mod capture_flow;
mod cli;
mod config_edit;
mod config_watcher;
mod daemon;
mod hotkeys;
mod hotkeys_portal;
mod ipc_client;
mod ipc_server;
mod platform;
mod prepare_gate;
mod sessions;
mod single;
#[cfg(all(feature = "tray", target_os = "linux"))]
mod sni_watchdog;
#[cfg(feature = "tray")]
mod tray;
mod tray_gate;
#[cfg(feature = "tray")]
mod tray_helper;
mod upload;
mod worker;

use clap::Parser;
use color_eyre::Result;
use tracing_subscriber::EnvFilter;

use crate::cli::{Cli, Command};

fn main() -> Result<()> {
    init_reporting();
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Daemon(args)) => cli::run_daemon(&args),
        Some(Command::Capture(args)) => cli::run_capture(&args),
        Some(Command::Config) => cli::run_config(),
        Some(Command::Backends) => cli::run_backends(),
        Some(Command::Open(args)) => cli::run_open(&args),
        Some(Command::Upload(args)) => cli::run_upload(&args),
        Some(Command::Auth(args)) => cli::run_auth(&args),
        Some(Command::Hotkeys(args)) => cli::run_hotkeys(&args),
        Some(Command::App(args)) => cli::run_app(&args),
        #[cfg(feature = "tray")]
        Some(Command::Tray) => cli::run_tray_helper(),
        None => Ok(()),
    }
}

/// Initialise `color_eyre` + `tracing` with a sensible default filter.
///
/// `RUST_LOG` overrides the default; otherwise we land on `info` for
/// `flintshot` crates and `warn` for everything else.
fn init_reporting() {
    let _ = color_eyre::install();

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("flintshot=info,warn"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}
