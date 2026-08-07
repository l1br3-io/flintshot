//! Guard test: `flintshot-ipc` is transport-only — it owns the wire
//! format (framing + socket helpers + the fire-and-forget client
//! helpers) and must NOT pull daemon-side types. The daemon's
//! `SubscriberBus`, `DaemonState`, `IpcServer`, `handle_connection`,
//! and `tokio::sync::broadcast` fan-out stay in `flintshot-app`.
//!
//! Mirrors `flintshot-core/tests/no_gui_types_in_core.rs`: a pure
//! source scan of the crate's `src/`, no compilation of the daemon
//! crate required. See ADR-0025.

use std::path::PathBuf;

const FORBIDDEN_PATTERNS: &[&str] = &[
    "use tokio::sync::broadcast",
    "SubscriberBus",
    "DaemonState",
    "IpcServer",
    "handle_connection",
];

#[test]
fn no_daemon_side_types_in_ipc_src() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders: Vec<(PathBuf, usize, String)> = Vec::new();

    let mut entries = Vec::new();
    collect_rust_files(&manifest_dir, &mut entries);

    for path in entries {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (lineno, line) in src.lines().enumerate() {
            if FORBIDDEN_PATTERNS.iter().any(|pat| line.contains(pat)) {
                offenders.push((path.clone(), lineno + 1, line.to_string()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "flintshot-ipc must not pull daemon-side types (see ADR-0025):\n{}",
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
