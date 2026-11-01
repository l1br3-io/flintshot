//! Graphical, interactive portal tests — the manual pass.
//!
//! These drive the REAL XDG ScreenCast portal on a `Wayland`
//! session: each one opens a consent dialog that a human must
//! answer. They are `#[ignore]`d out of every automated run;
//! exercise them from a real graphical session with:
//!
//! ```sh
//! cargo test -p flintshot-record --test portal_session -- --ignored
//! ```
//!
//! Rows (mirrored in the manual checklist): round-trip records and
//! finalizes; denying the dialog reports `PortalDenied`; a restore
//! token skips the second prompt.

use std::thread;
use std::time::Duration;

use flintshot_core::capture::SessionKind;
use flintshot_record::{mp4, RecordConfig, RecordSource, Recorder};

/// Skip (pass) when not on a Wayland session.
fn wayland_or_skip() -> bool {
    if SessionKind::detect() == SessionKind::Wayland {
        true
    } else {
        eprintln!("SKIP: not a Wayland session (XDG_SESSION_TYPE)");
        false
    }
}

/// Record ~2s from the portal-confirmed source and assert the
/// finalized MP4. A human must approve the consent dialog.
async fn record_once(
    out: &std::path::Path,
    restore_token: Option<String>,
) -> Result<(std::path::PathBuf, Option<String>), flintshot_record::RecordError> {
    let mut cfg = RecordConfig::new(out);
    cfg.source = RecordSource::Session;
    cfg.fps = 15;
    cfg.portal_restore_token = restore_token;
    let recorder = Recorder::prepare(cfg).await?;
    let token = recorder.restore_token().map(str::to_owned);
    let handle = recorder.start()?;
    thread::sleep(Duration::from_millis(2000));
    let finalized = handle.stop()?;
    Ok((finalized, token))
}

#[tokio::test]
#[ignore = "opens the real portal consent dialog — run from a Wayland session"]
async fn portal_round_trip_records() {
    if !wayland_or_skip() {
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let (finalized, _token) = record_once(&dir.path().join("portal.mp4"), None)
        .await
        .expect("portal round trip");
    let bytes = std::fs::read(&finalized).expect("finalized file");
    assert!(
        mp4::has_box(&bytes, b"moov"),
        "portal recording must finalize"
    );
    assert!(
        bytes.len() > 10_000,
        "a 2s portal recording is real data, not an empty moov ({} bytes)",
        bytes.len()
    );
}

#[tokio::test]
#[ignore = "opens the real portal consent dialog — CANCEL it when it appears"]
async fn portal_deny_reports_portal_denied() {
    if !wayland_or_skip() {
        return;
    }
    let mut cfg = RecordConfig::new("/tmp/flintshot-portal-deny-unused.mp4");
    cfg.source = RecordSource::Session;
    let err = Recorder::prepare(cfg)
        .await
        .expect_err("cancelling the dialog must fail prepare");
    assert_eq!(err, flintshot_record::RecordError::PortalDenied);
}

#[tokio::test]
#[ignore = "opens ONE consent dialog — approve the first recording only"]
async fn portal_restore_token_skips_second_prompt() {
    if !wayland_or_skip() {
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let (_, token) = record_once(&dir.path().join("first.mp4"), None)
        .await
        .expect("first recording");
    let Some(token) = token else {
        eprintln!("SKIP: the portal granted no restore token (implementation choice)");
        return;
    };

    // Second recording with the token: no dialog should appear. If
    // one does, this test stalls until a human answers it — that IS
    // the failure signal (token not honored).
    let (second, _) = record_once(&dir.path().join("second.mp4"), Some(token))
        .await
        .expect("token-restored recording");
    let bytes = std::fs::read(&second).expect("finalized file");
    assert!(mp4::has_box(&bytes, b"moov"));
}
