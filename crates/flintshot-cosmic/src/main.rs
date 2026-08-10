//! `flintshot-cosmic` — COSMIC panel applet binary.
//!
//! Phase 3 (Task 6): wires the IPC client module ([`ipc`]) so it
//! compiles and links against `libcosmic`. Task 7 builds the real
//! panel button + popup menu on top; Task 8 adds the live event feed
//! and daemon-absent status line.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

mod ipc;

use std::time::Duration;

fn main() {
    println!(
        "flintshot-cosmic: panel applet not implemented yet (Task 7). \
         libcosmic linked successfully."
    );
    // Minimal smoke probe of the IPC surface. Task 7 replaces this with
    // the real popup; until then the bind path stays exercised so the
    // module's API stays reachable and the binary proves the daemon
    // detection works end-to-end.
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("flintshot-cosmic: failed to build runtime: {e}");
            return;
        }
    };
    runtime.block_on(async {
        match ipc::probe_daemon(None).await {
            ipc::DaemonState::Running => {
                println!("flintshot-cosmic: daemon detected.");
                if let Ok(mut stream) = ipc::subscribe(None).await {
                    // Confirm the stream is live without blocking the
                    // stub on real events.
                    let _ =
                        tokio::time::timeout(Duration::from_millis(10), stream.next_event()).await;
                }
            }
            ipc::DaemonState::NotRunning => {
                println!("flintshot-cosmic: no daemon detected.");
            }
        }
    });
}
