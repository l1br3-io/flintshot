//! Headless end-to-end pipeline tests: `videotestsrc` stands in for
//! the real capture sources (the CI spine — no display, no portal,
//! no PipeWire), the in-house MP4 walker asserts the finalize
//! contract, and the whole lifecycle rides the public API only.
//!
//! Both tests skip loudly (with a named reason printed) when the
//! machine carries no H.264 encoder — the capability report decides,
//! mirroring the engine's own degrade-don't-panic rule.

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use flintshot_record::mp4;
use flintshot_record::{RecordConfig, RecordSource, Recorder};

/// Skip guard: recording needs one chain encoder installed.
fn encoder_or_skip() {
    let capability = Recorder::probe();
    if capability.selected_encoder().is_none() {
        let names = std::env::current_exe()
            .map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .ok()
            .flatten()
            .unwrap_or_else(|| "pipeline tests".into());
        eprintln!(
            "SKIP {names}: no H.264 encoder on this machine — {}",
            capability.install_hint()
        );
    }
}

fn test_config(out: &Path) -> RecordConfig {
    let mut cfg = RecordConfig::new(out);
    cfg.source = RecordSource::TestSrc;
    cfg.fps = 10;
    cfg
}

#[tokio::test]
async fn testsrc_records_and_finalizes() {
    encoder_or_skip();
    let dir = tempfile::tempdir().expect("tempdir");
    let out = dir.path().join("recorded.mp4");
    let recorder = Recorder::prepare(test_config(&out)).await.expect("prepare");
    let handle = recorder.start().expect("start reaches PLAYING");

    // ~1s of frames at 10 fps, with margin for preroll.
    thread::sleep(Duration::from_millis(1200));

    // Pre-stop: a real container header, but no moov — the finalize
    // contract is what stop() buys.
    let pre = std::fs::read(&out).expect("output exists while recording");
    assert!(mp4::has_box(&pre, b"ftyp"), "ftyp must exist before stop");
    assert!(
        !mp4::has_box(&pre, b"moov"),
        "moov must not exist before stop"
    );

    let finalized = handle.stop().expect("stop finalizes");
    assert_eq!(finalized, out);

    let post = std::fs::read(&out).expect("finalized file readable");
    assert!(mp4::has_box(&post, b"ftyp"));
    assert!(mp4::has_box(&post, b"moov"), "moov must exist after stop");
    assert!(
        post.len() > pre.len(),
        "finalize must add the moov payload ({} -> {})",
        pre.len(),
        post.len()
    );
}

#[tokio::test]
async fn duration_cap_autostops() {
    encoder_or_skip();
    let dir = tempfile::tempdir().expect("tempdir");
    let out = dir.path().join("capped.mp4");
    let mut cfg = test_config(&out);
    cfg.max_duration = Some(Duration::from_millis(500));

    let recorder = Recorder::prepare(cfg).await.expect("prepare");
    let handle = recorder.start().expect("start reaches PLAYING");
    // No explicit stop: the cap thread must finalize on its own.
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(bytes) = std::fs::read(&out) {
            if mp4::has_box(&bytes, b"moov") {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "duration cap did not finalize within 15s"
        );
        thread::sleep(Duration::from_millis(100));
    }

    // stop() after the cap returns the same outcome, immediately.
    let before = Instant::now();
    let finalized = handle.stop().expect("stop observes the cap's outcome");
    assert_eq!(finalized, out);
    assert!(
        before.elapsed() < Duration::from_secs(2),
        "post-cap stop must not wait for another finalize"
    );
}
