//! `flintshot-cosmic` — COSMIC panel applet binary.
//!
//! Phase 0: stub binary that confirms libcosmic compiles in our CI matrix.
//! Phase 3b (`phase-3b-cosmic-applet`) fills in the panel button + popup
//! UI that talks IPC to a running `flintshot daemon`.
//!
//! Task 002 lands the pure model modules ([`app`], [`daemon_probe`]) so
//! their decision logic is headless-tested before Task 003 adds the
//! `cosmic::Application` impl. The stub `fn main` stays until then.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

// Task 002 lands the pure model + daemon-probe seam; Task 003's
// `cosmic::Application` impl will consume them. Until then the bin
// target's non-test build sees them as dead code (the `#[cfg(test)]`
// unit tests do exercise them). Suppress rather than add an artificial
// call from the stub `main`.
#[allow(
    dead_code,
    reason = "pure model consumed by Task 003's Application impl + unit tests"
)]
mod app;
#[allow(
    dead_code,
    reason = "daemon-probe seam consumed by Task 005's Start-daemon button + unit tests"
)]
mod daemon_probe;

// Intentionally pulls libcosmic into the dep graph even though we don't
// call it yet — the goal of Phase 0 was to verify the toolchain can link
// against it. Phase 3b replaces this with a real `cosmic::applet::run`.
fn main() {
    println!(
        "flintshot-cosmic: panel applet not implemented yet (Phase 3b). \
         libcosmic linked successfully."
    );
}
