//! Boundary test: `flintshot-cosmic` must not import capture/editor/GUI
//! toolkit crates. Mirrors `flintshot-core/tests/no_gui_types_in_core.rs`.
//!
//! The applet is an IPC client — it does no capture (`xcap`), no editing
//! (`eframe`/`egui`), and has no tray/hotkey surface (`tray-icon`,
//! `global-hotkey`). Cargo's dep graph is the primary guard (the crate
//! simply doesn't declare those deps); this test is cheap
//! defence-in-depth — it scans the crate's `src/` for forbidden imports
//! so a `path = "../flintshot-app"` typo in `Cargo.toml` can't slip a
//! forbidden dep past the dep graph. Matches the core boundary test's
//! "scan src/ for imports" approach.

use std::path::PathBuf;

const FORBIDDEN_PREFIXES: &[&str] = &[
    "use xcap",
    "use eframe",
    "use egui",
    "use global_hotkey",
    "use tray_icon",
    // Defence-in-depth: the crate should not reach into the editor or the
    // app (both are GUI/capture-bearing). Cargo prevents the dep; this
    // catches a path typo.
    "use flintshot_editor_egui",
    "use flintshot_app",
];

#[test]
fn no_capture_or_editor_imports_in_cosmic_src() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders: Vec<(PathBuf, usize, String)> = Vec::new();

    let mut entries = Vec::new();
    collect_rust_files(&manifest_dir, &mut entries);

    for path in entries {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (lineno, line) in src.lines().enumerate() {
            let stripped = line.trim_start();
            if FORBIDDEN_PREFIXES
                .iter()
                .any(|prefix| stripped.starts_with(prefix))
            {
                offenders.push((path.clone(), lineno + 1, line.to_string()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "flintshot-cosmic must not import capture/editor/GUI types \
         (see ADR-0005 / the Phase 3b spec §Boundaries):\n{}",
        offenders
            .iter()
            .map(|(p, n, l)| format!("  {}:{n}: {l}", p.display()))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn collect_rust_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_rust_files(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}
