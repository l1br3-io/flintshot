//! `flintshot-cosmic` — COSMIC panel applet binary.
//!
//! A panel button that opens a popup mirroring the tray menu (capture
//! actions, per-monitor submenu, open last, open config, quit). The
//! applet is a pure IPC client: it talks to a running `flintshot
//! daemon` over the local socket and never links `flintshot-app` or
//! `egui` (ADR-0005). Task 8 will add the live event stream and daemon-
//! absent status line.
//!
//! The dispatch logic (menu item → daemon `Request`) lives in [`menu`],
//! unit-tested headless. The libcosmic [`app`] module is the UI shell.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

mod app;
mod ipc;
mod menu;

/// Entry point. Hands control to libcosmic's applet runtime, which
/// drives the [`app::App`] `cosmic::Application` impl.
fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::App>(())
}
