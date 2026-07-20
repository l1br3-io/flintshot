//! `flintshot` binary entry point — CLI dispatch, daemon lifecycle,
//! tray icon, hotkeys. Phase 0 wires up the CLI shape and logging; the
//! subcommand bodies are stubs that get filled in across Phases 1–4.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

mod cli;

use clap::Parser;
use color_eyre::Result;
use flintshot_core::Config;
use tracing_subscriber::EnvFilter;

use crate::cli::{Cli, Command};

fn main() -> Result<()> {
    init_reporting();
    let cli = Cli::parse();

    if cli.dump_config {
        return dump_config();
    }

    match cli.command {
        Some(Command::Daemon(args)) => cli::run_daemon(&args),
        Some(Command::Region(args)) => cli::run_region(&args),
        Some(Command::Fullscreen(args)) => cli::run_fullscreen(&args),
        Some(Command::Pin(args)) => cli::run_pin(&args),
        Some(Command::Upload(args)) => cli::run_upload(&args),
        None => Ok(()),
    }
}

/// Throwaway: print the resolved config TOML. Removed in Phase 1 Task 3
/// when the real `config` subcommand lands.
fn dump_config() -> Result<()> {
    let cfg = Config::load().map_err(|e| color_eyre::eyre::eyre!(e.to_string()))?;
    let toml = toml::to_string_pretty(&cfg)
        .map_err(|e| color_eyre::eyre::eyre!("serialization failure: {e}"))?;
    print!("{toml}");
    Ok(())
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
