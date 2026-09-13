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
//!
//! One path-scoped exception: the native capture backends under
//! `src/capture/` may use `zbus` (XDG Screenshot Portal client). D-Bus
//! must not spread beyond that module — everywhere else in core, `use
//! zbus` fails exactly like the global bans. `ashpd` stays banned
//! everywhere (the portal-heavy app crate owns it); see
//! `docs/specs/native-capture-backends/`.

use std::path::{Path, PathBuf};

/// Prefixes banned in every file under `src/`.
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

/// Prefixes banned everywhere **except** under `src/capture/` — the
/// native capture backends speak D-Bus (zbus blocking facade); no
/// other core module may.
const CAPTURE_ONLY_PREFIXES: &[&str] = &["use zbus"];

/// Returns the prefix that bans `stripped_line` at `rel_src_path`
/// (path relative to `src/`), or `None` if the line is allowed there.
fn offending_prefix(rel_src_path: &Path, stripped_line: &str) -> Option<&'static str> {
    let in_capture = rel_src_path.starts_with(Path::new("capture"));
    if !in_capture {
        if let Some(prefix) = CAPTURE_ONLY_PREFIXES
            .iter()
            .copied()
            .find(|prefix| stripped_line.starts_with(prefix))
        {
            return Some(prefix);
        }
    }
    FORBIDDEN_PREFIXES
        .iter()
        .copied()
        .find(|prefix| stripped_line.starts_with(prefix))
}

/// The deny-list must actually flag a matching line for every prefix —
/// a regression guard for future additions that would otherwise never be
/// exercised until core really imports the crate.
#[test]
fn every_forbidden_prefix_flags_a_matching_use_line() {
    for prefix in FORBIDDEN_PREFIXES.iter().chain(CAPTURE_ONLY_PREFIXES) {
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
fn zbus_is_banned_outside_capture_but_allowed_inside() {
    assert_eq!(
        offending_prefix(Path::new("error.rs"), "use zbus::Connection;"),
        Some("use zbus"),
        "zbus must stay capture-only"
    );
    assert_eq!(
        offending_prefix(Path::new("ipc/mod.rs"), "use zbus;"),
        Some("use zbus"),
        "nested non-capture modules get no allowance"
    );
    assert_eq!(
        offending_prefix(Path::new("capture/portal.rs"), "use zbus::Connection;"),
        None,
        "capture backends may speak D-Bus"
    );
    assert_eq!(
        offending_prefix(Path::new("capture/sub/deeper.rs"), "use zbus;"),
        None,
        "the allowance is the whole capture/ subtree"
    );
    assert_eq!(
        offending_prefix(Path::new("capturex/thing.rs"), "use zbus;"),
        Some("use zbus"),
        "a component that merely starts with 'capture' is not capture/"
    );
}

#[test]
fn ashpd_stays_banned_everywhere_including_capture() {
    assert_eq!(
        offending_prefix(Path::new("capture/portal.rs"), "use ashpd::DesktopPortal;"),
        Some("use ashpd")
    );
    assert_eq!(
        offending_prefix(Path::new("upload.rs"), "use ashpd::DesktopPortal;"),
        Some("use ashpd")
    );
}

#[test]
fn no_gui_toolkit_imports_in_core_src() {
    let src_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders: Vec<(PathBuf, usize, String)> = Vec::new();

    let mut entries = Vec::new();
    collect_rust_files(&src_dir, &mut entries);

    for path in entries {
        let rel = path.strip_prefix(&src_dir).unwrap_or(&path);
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (lineno, line) in src.lines().enumerate() {
            let stripped = line.trim_start();
            if offending_prefix(rel, stripped).is_some() {
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
