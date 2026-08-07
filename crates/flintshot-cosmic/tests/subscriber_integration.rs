//! Mock-daemon integration test for the IPC subscriber stream.
//!
//! `#[ignore]`'d because it binds a real local socket (no display, no
//! libcosmic runtime needed). Run with:
//!
//! ```sh
//! cargo test -p flintshot-cosmic -- --ignored subscriber_integration
//! ```

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures::StreamExt;
use interprocess::local_socket::traits::tokio::Listener as _;
use interprocess::local_socket::ListenerOptions;
use tokio::time::timeout;

use flintshot_core::ipc::{socket_path, Event};
use flintshot_cosmic::app::{App, ConnectionState, Message};
use flintshot_cosmic::ipc;
use flintshot_ipc::{encode_msg, read_frame, socket_name, write_frame, Stream};

/// Unique counter so parallel `--ignored` runs don't collide on the
/// socket filename inside the shared temp dir.
static UNIQUE: AtomicU64 = AtomicU64::new(0);

/// Timeout for a single `stream.next()` — generous for localhost I/O
/// but catches a hung stream quickly.
const NEXT_TIMEOUT: Duration = Duration::from_secs(5);

/// The full subscriber lifecycle against a mock daemon:
///
/// 1. Mock binds a temp socket → subscriber connects + sends Subscribe.
/// 2. Mock emits `CaptureStarted` → `EditorOpened` → `Saved { path }`.
/// 3. The subscriber forwards each as `IpcEvent`, driving `App::apply`
///    through the expected status-text sequence.
/// 4. Mock closes → subscriber yields `DaemonGone` → `App::apply` flips
///    `connection` to `NoDaemon`.
/// 5. Stream ends (`next()` → `None`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs a real local socket (no display); run explicitly"]
async fn subscriber_streams_events_then_signals_daemon_gone() {
    // ── temp XDG_RUNTIME_DIR so socket_path(None) → <temp>/flintshot.sock ──
    //
    // The subscriber resolves `socket_path(None)` which reads
    // `XDG_RUNTIME_DIR`. Pointing it at a fresh temp dir isolates the
    // test from any real daemon socket on the system.
    let dir = tempfile::tempdir().expect("tempdir");
    let prev_xdg = std::env::var_os("XDG_RUNTIME_DIR");
    std::env::set_var("XDG_RUNTIME_DIR", dir.path());

    // Give this invocation a unique sub-dir so two `--ignored` runs in
    // parallel never share the same `flintshot.sock`.
    let n = UNIQUE.fetch_add(1, Ordering::Relaxed);
    let run_dir = dir.path().join(format!("run-{n}"));
    std::fs::create_dir_all(&run_dir).expect("mkdir run dir");
    std::env::set_var("XDG_RUNTIME_DIR", &run_dir);

    // ── mock daemon ────────────────────────────────────────────────
    //
    // Binds on socket_path(None) = <run_dir>/flintshot.sock, accepts
    // one connection, reads + discards Subscribe, writes the capture
    // lifecycle event sequence, then closes.
    let target_path = socket_path(None);
    let mock_path = target_path.clone();
    let mock_task = tokio::spawn(async move {
        let name = socket_name(&mock_path).expect("socket_name");
        let listener = ListenerOptions::new()
            .name(name)
            .try_overwrite(true)
            .create_tokio()
            .expect("bind mock listener");

        let mut conn: Stream = listener.accept().await.expect("accept");

        // Consume the Subscribe request the subscriber sends.
        let _subscribe = read_frame(&mut conn).await.expect("read Subscribe");

        // Emit the lifecycle events.
        let events = [
            Event::CaptureStarted,
            Event::EditorOpened,
            Event::Saved {
                path: PathBuf::from("/tmp/flintshot-mock.png"),
            },
        ];
        for event in &events {
            let bytes = encode_msg(event).expect("encode event");
            write_frame(&mut conn, &bytes).await.expect("write event");
        }

        // Close → subscriber's read loop hits EOF → DaemonGone.
        drop(conn);
    });

    // ── drive the subscriber stream ────────────────────────────────
    let mut stream = ipc::subscriber_stream();
    let mut app = App::new_for_integration_test();

    // 1. DaemonUp — subscriber emits this after connect + Subscribe.
    let msg = timeout(NEXT_TIMEOUT, stream.next())
        .await
        .expect("timed out waiting for DaemonUp")
        .expect("stream ended before DaemonUp");
    assert!(
        matches!(msg, Message::DaemonUp),
        "expected DaemonUp first, got {msg:?}"
    );
    app.apply(&msg);
    assert_eq!(app.connection(), ConnectionState::Connected);

    // 2–4. Three lifecycle events → status-text advances.
    let expected_texts = [
        "capturing…",
        "editor open",
        "saved: /tmp/flintshot-mock.png",
    ];
    for expected in &expected_texts {
        let msg = timeout(NEXT_TIMEOUT, stream.next())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for event ({expected})"))
            .unwrap_or_else(|| panic!("stream ended before event ({expected})"));
        match &msg {
            Message::IpcEvent(event) => {
                assert_eq!(
                    App::status_text(event),
                    *expected,
                    "status text mismatch for {event:?}"
                );
            }
            other => panic!("expected IpcEvent, got {other:?}"),
        }
        app.apply(&msg);
        assert_eq!(app.last_event_text(), *expected);
    }

    // 5. Mock closed → DaemonGone → connection flips to NoDaemon.
    let msg = timeout(NEXT_TIMEOUT, stream.next())
        .await
        .expect("timed out waiting for DaemonGone")
        .expect("stream ended before DaemonGone");
    assert!(
        matches!(msg, Message::DaemonGone),
        "expected DaemonGone after mock close, got {msg:?}"
    );
    app.apply(&msg);
    assert_eq!(app.connection(), ConnectionState::NoDaemon);

    // 6. Stream has ended.
    let ended = timeout(NEXT_TIMEOUT, stream.next())
        .await
        .expect("timed out waiting for stream end");
    assert!(ended.is_none(), "stream should end after DaemonGone");

    // ── restore env ────────────────────────────────────────────────
    match prev_xdg {
        Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
        None => std::env::remove_var("XDG_RUNTIME_DIR"),
    }

    mock_task.await.expect("mock task panicked");
}
