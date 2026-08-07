//! `flintshot-cosmic` — COSMIC panel applet binary.
//!
//! A thin IPC client that mirrors the Phase 2 tray's actions (Capture
//! region, Capture fullscreen, per-monitor submenu, Open config, Quit),
//! exposes the Phase 3 *Pin last*, and additionally subscribes to the
//! daemon's `Event` stream to render a live status line. The applet does
//! no capture and no editing — every action is an IPC request to a
//! running `flintshot daemon`.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

mod app;
#[allow(
    dead_code,
    reason = "daemon-probe seam consumed by Task 005's Start-daemon button + unit tests"
)]
mod daemon_probe;
mod menu;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::App>(())
}
