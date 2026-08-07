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

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<flintshot_cosmic::app::App>(())
}
