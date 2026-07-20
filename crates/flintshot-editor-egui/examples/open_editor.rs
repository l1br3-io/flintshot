//! Manual smoke test for `EguiShell::run_editor`.
//!
//! Run with:
//! ```sh
//! cargo run -p flintshot-editor-egui --example open_editor
//! ```
//!
//! Captures the primary monitor and opens the editor shell. Press Esc
//! or close the window; the printed outcome should be `Cancelled`.
//!
//! Not part of the shipped surface — lives under `examples/` purely
//! as a Phase 1.B de-risking probe. Task 6 will replace this with a
//! real `flintshot region` (no `--out`) path, at which point the
//! example can be deleted.

use flintshot_core::{capture_fullscreen, shell::EditorShell, EditorState};
use flintshot_editor_egui::EguiShell;

fn main() {
    let img = capture_fullscreen(None).expect("primary monitor capture");
    println!("captured {}x{}; opening editor", img.width(), img.height());

    let outcome = EguiShell::new()
        .run_editor(EditorState::new(img))
        .expect("editor session");
    println!("EditorOutcome: {outcome:?}");
}
