//! Guard test: no GUI toolkit types may leak into `flintshot-core`'s
//! public API or its source. See `docs/ARCHITECTURE.md` §Coding conventions.
//!
//! This test greps its own crate's `src/` for forbidden imports — GUI
//! toolkits (egui/eframe/iced/cosmic/libcosmic/slint/gdk/gtk) **and** the
//! Phase 2 transport/system crates (`interprocess`, `global_hotkey`,
//! `tray_icon`, `notify`) — and fails if any are found. The `libcosmic`
//! crate is imported as `cosmic` in Rust source, so both prefixes are
//! listed. The IPC schema (`flintshot-core::ipc`) may use only
//! `postcard`/`serde`/std types; the transport/server implementations
//! live in `flintshot-app`. It runs without compiling the toolkit
//! crates — pure source scan.

use std::path::PathBuf;

const FORBIDDEN_PREFIXES: &[&str] = &[
    "use egui",
    "use eframe",
    "use iced",
    "use cosmic",
    "use libcosmic",
    "use slint",
    "use gdk",
    "use gtk",
    // Phase 2 transport / system crates — keep core schema-only.
    "use interprocess",
    "use global_hotkey",
    "use tray_icon",
    "use notify",
    // Phase 6: the xdg-desktop-portal client lives in flintshot-app;
    // core stays portal-free (plan: Wayland hotkeys via ashpd).
    "use ashpd",
];

/// The deny-list must actually flag a matching line for every prefix —
/// a regression guard for future additions that would otherwise never be
/// exercised until core really imports the crate.
#[test]
fn every_forbidden_prefix_flags_a_matching_use_line() {
    for prefix in FORBIDDEN_PREFIXES {
        let line = format!("{prefix}::example::Thing;");
        assert!(
            line.trim_start().starts_with(prefix),
            "deny-list prefix {prefix:?} does not flag its own use line"
        );
    }
    assert!(
        FORBIDDEN_PREFIXES.contains(&"use ashpd"),
        "Phase 6 added ashpd to flintshot-app; core must deny it too"
    );
}

#[test]
fn no_gui_toolkit_imports_in_core_src() {
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
        "flintshot-core must not import GUI toolkit types (see ADR-0012):\n{}",
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
