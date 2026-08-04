//! `flintshot-cosmic` — COSMIC panel applet binary.
//!
//! Phase 0: stub binary that confirms libcosmic compiles in our CI matrix.
//! Phase 3b (`phase-3b-cosmic-applet`) fills in the panel button + popup
//! UI that talks IPC to a running `flintshot daemon`.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

// Intentionally pulls libcosmic into the dep graph even though we don't
// call it yet — the goal of Phase 0 is to verify the toolchain can link
// against it. Phase 3b replaces this with a real `cosmic::app::Daemon`.
fn main() {
    println!(
        "flintshot-cosmic: panel applet not implemented yet (Phase 3b). \
         libcosmic linked successfully."
    );
}
