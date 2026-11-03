//! Integration tests for `flinshot capture --stdout`
//! (cli-scripting CS4): the real binary, a real grab, stdout as pure
//! image payload.
//!
//! The grab needs an X display (Xvfb in CI-style runs, X-Wayland on
//! a session) — without one the tests soft-skip; the headless units
//! in `cli.rs` pin the conflict matrix, format resolution, and the
//! encode seam.

#![forbid(unsafe_code)]

use std::process::Command;

fn spawn_capture(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_flintshot"))
        .args([
            "capture",
            "--standalone",
            "--backend",
            "xcap",
            "--geometry",
            "10,20,300,400",
        ])
        .args(args)
        .output()
        .expect("spawning the flintshot binary")
}

/// `--stdout --format webp` writes WebP bytes — the export-webp
/// interop row — measuring exactly the grabbed rectangle.
#[test]
fn stdout_carries_image_bytes_in_the_requested_format() {
    if std::env::var_os("DISPLAY").is_none() {
        return; // headless: the cli.rs units pin the seams
    }
    let output = spawn_capture(&["--stdout", "--format", "webp"]);
    assert!(
        output.status.success(),
        "the capture must succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.stdout.is_empty(),
        "stdout carries the image payload"
    );
    assert_eq!(
        image::guess_format(&output.stdout).ok(),
        Some(image::ImageFormat::WebP),
        "the stream is webp, not a png fallback"
    );
    let decoded = image::load_from_memory(&output.stdout).expect("stdout bytes decode as an image");
    assert_eq!(
        (decoded.width(), decoded.height()),
        (300, 400),
        "the streamed webp measures the grabbed rect"
    );
}

/// `--stdout --json`: the image owns stdout; the JSON result line
/// rides stderr (the only channel left under payload-only
/// discipline) and carries `path: "-"`.
#[test]
fn stdout_keeps_the_payload_pure_and_json_rides_stderr() {
    if std::env::var_os("DISPLAY").is_none() {
        return; // headless: covered by the cli.rs units
    }
    let output = spawn_capture(&["--stdout", "--json", "--format", "png"]);
    assert!(
        output.status.success(),
        "the capture must succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Stdout is pure payload: decodes as a PNG of the grabbed rect.
    let decoded = image::load_from_memory(&output.stdout)
        .expect("stdout must be pure image bytes (no log/json noise)");
    assert_eq!((decoded.width(), decoded.height()), (300, 400));

    // Stderr carries exactly the one stable JSON line.
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let line = stderr
        .lines()
        .find(|l| l.starts_with('{'))
        .expect("the --json result line rides stderr with --stdout");
    assert_eq!(
        line,
        r#"{"path":"-","width":300,"height":400,"format":"png","backend":"xcap","geometry":"10,20,300,400"}"#,
        "the stable key set with the stdout destination convention"
    );
}
