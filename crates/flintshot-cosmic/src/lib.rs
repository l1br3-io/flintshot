//! `flintshot-cosmic` — COSMIC panel applet library facade.
//!
//! The applet is primarily a binary (`src/main.rs`), but a thin library
//! target exposes the applet model ([`app::App`], [`ipc`]) to integration
//! tests in `tests/` without requiring a libcosmic runtime. The binary
//! is a one-liner that calls `cosmic::applet::run::<App>(())`.
//!
//! See [`app`] for the model/decision logic, [`menu`] for the popup
//! view, [`ipc`] for the subscriber `Subscription` bridge, and
//! [`daemon_probe`] for the *Start daemon* spawn seam.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

pub mod app;
pub mod daemon_probe;
pub mod ipc;
pub mod menu;
