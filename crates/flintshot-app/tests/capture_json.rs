//! Integration tests for `flinshot capture --json` (cli-scripting
//! CS3): the real binary, a real grab, stdout pinned to the exact
//! stable key set.
//!
//! The grab needs an X display (Xvfb in CI-style runs, X-Wayland on
//! a session) — without one the test soft-skips; the headless units
//! in `cli.rs` pin the serialization shape.

#![forbid(unsafe_code)]

use std::process::Command;

/// `capture --geometry ... --out ... --json` writes the file AND
/// prints exactly one JSON line: the stable kebab-case key set, the
/// grabbed dimensions, and the geometry that drove the grab. The
/// line is compared as an exact string — this is the snapshot.
#[test]
fn capture_json_prints_the_stable_line_alongside_the_write() {
    if std::env::var_os("DISPLAY").is_none() {
        return; // headless: the cli.rs units pin the shape
    }
    let dir = tempfile::tempdir().unwrap();
    let out_path = dir.path().join("geom.png");
    let output = Command::new(env!("CARGO_BIN_EXE_flintshot"))
        .args([
            "capture",
            "--standalone",
            "--backend",
            "xcap",
            "--geometry",
            "10,20,300,400",
            "--json",
            "--out",
        ])
        .arg(&out_path)
        .output()
        .expect("spawning the flintshot binary");

    assert!(
        output.status.success(),
        "the capture must succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_path.exists(), "--json never suppresses the write");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let expected = format!(
        r#"{{"path":{path},"width":300,"height":400,"format":"png","backend":"xcap","geometry":"10,20,300,400"}}"#,
        path = serde_json::to_string(&out_path.display().to_string()).unwrap(),
    );
    assert_eq!(
        stdout.trim_end(),
        expected,
        "stdout is exactly the one stable result line"
    );
}
