//! The app window (`flintshot app`): a Win11-Snipping-Tool-style
//! capture home.
//!
//! Process model (spec `docs/specs/app-window/`): the window is an
//! IPC **client** — same stance as the tray helper (ADR-0042). The
//! UI thread runs the egui window
//! ([`EguiShell::run_home`]); a background **pump thread** owns a
//! current-thread tokio runtime, holds the daemon subscription,
//! delegates capture requests after the hide grace, and pushes
//! [`HomeDirective`]s back at the UI. No capture ever runs in this
//! process — the daemon spawns the standard worker, so tray-hide
//! handshakes and Event fan-out apply unchanged.
//!
//! ## Auto-start
//!
//! No daemon at subscribe time → spawn `flintshot daemon` detached
//! and retry (bounded). The window opens immediately either way;
//! the status line narrates ("connecting…" → "starting daemon…" →
//! "connected" / failure).
//!
//! ## Hide / restore
//!
//! A capture request first hides the window (the UI's own viewport
//! command), then — after [`HIDE_GRACE_MS`] — delegates. Restore is
//! learned from the subscription: [`HomeCore`] counts
//! `CaptureStarted` (+) against `SessionEnded` (−, saturating — the
//! daemon's reaper emits it when a capture worker exits, so a
//! crashed worker still restores the window). A daemon disconnect
//! force-shows with an error status rather than leaving the window
//! hidden forever. The pump then stays in a degraded cmd-only loop
//! (no auto-reconnect in this effort).

use std::time::Duration;

use color_eyre::eyre;
use flintshot_core::ipc::Event as IpcEvent;
use flintshot_core::ipc::Request;
use flintshot_editor_egui::{HomeBridge, HomeDirective, MonitorEntry};
use tokio::sync::mpsc as tokio_mpsc;

use crate::ipc_server::{decode_event, read_frame};

/// Pause between hiding the window and delegating the capture, so
/// the compositor has unmapped it before the worker freezes the
/// screen. Matches the tray icon-grace bound's rationale
/// (ADR-0036); best-effort on Wayland where unmap is asynchronous.
const HIDE_GRACE_MS: u64 = 250;

/// How long the pump waits for an auto-started daemon to answer
/// before giving up (status flips to the failure text; the window
/// stays open in the degraded mode).
const DAEMON_START_TIMEOUT: Duration = Duration::from_secs(10);

/// Retry cadence while waiting for the daemon's socket to answer.
const CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(250);

/// A capture intent crossing UI → pump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PumpCommand {
    /// Delegate `Request::CaptureRegion`.
    Region,
    /// Delegate `Request::CaptureFullscreen` on `monitor`.
    Fullscreen(Option<u32>),
}

impl PumpCommand {
    /// The IPC request this intent delegates.
    fn request(self) -> Request {
        match self {
            Self::Region => Request::CaptureRegion {
                monitor: None,
                delay_ms: 0,
            },
            Self::Fullscreen(monitor) => Request::CaptureFullscreen { monitor },
        }
    }
}

/// Pure hide/restore state machine driven by the subscription
/// stream. Owned exclusively by the pump thread; extracted so the
/// transitions are unit-testable without sockets.
#[derive(Debug, Default, PartialEq)]
struct HomeCore {
    /// A capture was requested and the window hid for it.
    hidden: bool,
    /// Capture workers the daemon has started minus those whose
    /// processes exited. Saturating: a `SessionEnded` for a capture
    /// that began before we subscribed clamps at 0 (documented
    /// edge in the spec).
    in_flight: u32,
}

impl HomeCore {
    /// The pump accepted a capture command: the window is (or is
    /// about to be) hidden.
    fn capture_requested(&mut self) {
        self.hidden = true;
    }

    /// Apply one subscription event; `Some(_)` when a directive
    /// should reach the UI.
    #[allow(
        clippy::wildcard_enum_match_arm,
        reason = "IpcEvent is #[non_exhaustive]; only the capture-lifecycle variants matter here"
    )]
    fn on_event(&mut self, event: &IpcEvent) -> Option<HomeDirective> {
        match event {
            IpcEvent::CaptureStarted => {
                self.in_flight += 1;
                None
            }
            IpcEvent::SessionEnded => {
                self.in_flight = self.in_flight.saturating_sub(1);
                self.maybe_show()
            }
            IpcEvent::AppFocus => self.app_focus(),
            _ => None,
        }
    }

    /// A focus ping: surface the window — visibly re-focusing an
    /// already-visible idle window is the ping's whole point — but
    /// never raise it over a live capture session.
    fn app_focus(&mut self) -> Option<HomeDirective> {
        if self.in_flight == 0 {
            self.hidden = false;
            Some(HomeDirective::Show)
        } else {
            None
        }
    }

    /// Restore when the last in-flight capture finished and we were
    /// hiding.
    fn maybe_show(&mut self) -> Option<HomeDirective> {
        if self.hidden && self.in_flight == 0 {
            self.force_show()
        } else {
            None
        }
    }

    /// Un-hide unconditionally (used for failure paths where
    /// leaving the window hidden would strand the user).
    fn force_show(&mut self) -> Option<HomeDirective> {
        if self.hidden {
            self.hidden = false;
            Some(HomeDirective::Show)
        } else {
            None
        }
    }

    /// The subscription died: nothing more can be learned, so the
    /// window must come back and say so.
    fn disconnected(&mut self) -> Vec<HomeDirective> {
        self.in_flight = 0;
        let mut out = self
            .force_show()
            .into_iter()
            .collect::<Vec<HomeDirective>>();
        out.push(HomeDirective::SetStatus(
            "daemon connection lost — restart `flintshot app`".to_string(),
        ));
        out
    }
}

/// `flintshot app` entry point: acquire the app lock (focus-ping and
/// exit if held), start the pump, run the window.
pub(crate) fn run() -> eyre::Result<()> {
    let Some(_lock) = crate::single::acquire_app_lock()? else {
        return focus_existing();
    };

    let (cmd_tx, cmd_rx) = tokio_mpsc::channel::<PumpCommand>(8);
    let (directive_tx, directive_rx) = std::sync::mpsc::channel::<HomeDirective>();

    let pump = std::thread::Builder::new()
        .name("flintshot-app-pump".into())
        .spawn({
            let directive_tx = directive_tx.clone();
            move || pump_main(cmd_rx, directive_tx)
        })
        .map_err(|e| eyre::eyre!("spawning the app pump thread: {e}"))?;

    let bridge = Bridge {
        cmd_tx,
        directive_rx,
    };

    let ran = crate::ambient::with_ambient_runtime(|| {
        flintshot_editor_egui::EguiShell::new().run_home(Box::new(bridge), "connecting…".into())
    });
    // The window closed. Join the pump (its command sender died with
    // the bridge, so its loops exit on the next recv).
    let _ = pump.join();
    drop(directive_tx);

    ran.map_err(|e| eyre::eyre!("ambient runtime failure: {e}"))
        .and_then(|ran| ran.map_err(|e| eyre::eyre!("running the app window: {e}")))
}

/// The lock is held by a live window: ask the daemon to focus it and
/// exit. Without a daemon the ping has no recipient — surface that
/// distinctly rather than pretending to have focused something.
fn focus_existing() -> eyre::Result<()> {
    match crate::ipc_client::try_delegate(&Request::OpenApp) {
        Ok(crate::ipc_client::DelegateOutcome::Delegated) => {
            tracing::info!("app window already running; asked the daemon to focus it");
            Ok(())
        }
        Ok(crate::ipc_client::DelegateOutcome::NoDaemon) => Err(eyre::eyre!(
            "the app lock is held but no daemon answers; stale `flintshot app` process?"
        )),
        Err(e) => Err(eyre::eyre!("focusing the running app window: {e}")),
    }
}

/// The pump thread's whole life: connect (auto-starting the daemon),
/// then pump commands → requests and events → directives.
fn pump_main(
    mut cmd_rx: tokio_mpsc::Receiver<PumpCommand>,
    directive_tx: std::sync::mpsc::Sender<HomeDirective>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!(error = %e, "app pump could not build a runtime");
            return;
        }
    };

    runtime.block_on(async move {
        let mut core = HomeCore::default();
        if let Some(stream) = ensure_subscribed(&directive_tx).await {
            let _ = directive_tx.send(HomeDirective::SetStatus("connected".to_string()));
            if connected_loop(stream, &mut cmd_rx, &directive_tx, &mut core).await
                == LoopExit::Disconnected
            {
                degraded_loop(cmd_rx, &directive_tx, &mut core).await;
            }
        } else {
            degraded_loop(cmd_rx, &directive_tx, &mut core).await;
        }
    });
}

/// Subscribe with daemon auto-start. `None` when the daemon never
/// answered within [`DAEMON_START_TIMEOUT`] (status already sent).
async fn ensure_subscribed(
    directive_tx: &std::sync::mpsc::Sender<HomeDirective>,
) -> Option<crate::ipc_server::Stream> {
    let deadline = tokio::time::Instant::now() + DAEMON_START_TIMEOUT;
    let mut spawned = false;
    loop {
        match crate::ipc_client::subscribe_to(None).await {
            Ok(Some(stream)) => return Some(stream),
            Ok(None) => {
                if !spawned {
                    spawned = true;
                    let _ =
                        directive_tx.send(HomeDirective::SetStatus("starting daemon…".to_string()));
                    if let Err(e) = spawn_daemon() {
                        let _ = directive_tx.send(HomeDirective::SetStatus(format!(
                            "could not start the daemon: {e}"
                        )));
                        return None;
                    }
                }
            }
            Err(e) => tracing::warn!(error = %e, "subscribing to the daemon"),
        }
        if tokio::time::Instant::now() >= deadline {
            let _ = directive_tx.send(HomeDirective::SetStatus(format!(
                "no daemon answered within {} s — is `flintshot daemon` healthy?",
                DAEMON_START_TIMEOUT.as_secs()
            )));
            return None;
        }
        tokio::time::sleep(CONNECT_RETRY_INTERVAL).await;
    }
}

/// Launch `flintshot daemon` detached. stdin is null (the daemon
/// reads nothing); stdout + stderr stay inherited — the daemon's
/// `tracing` output goes to stdout, and an auto-started daemon's
/// logs must survive for diagnostics (the harness caught this: a
/// nulled stdout silently swallowed them).
fn spawn_daemon() -> eyre::Result<()> {
    let exe = std::env::current_exe()
        .map_err(|e| eyre::eyre!("resolving current_exe to start the daemon: {e}"))?;
    std::process::Command::new(exe)
        .arg("daemon")
        .stdin(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| eyre::eyre!("spawning `flintshot daemon`: {e}"))
}

/// Why [`connected_loop`] returned. `Disconnected` hands the (moved)
/// command receiver to [`degraded_loop`] — the exit is communicated
/// as a value because `tokio::select!` arms cannot move the
/// receiver that the losing `recv()` future still borrows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopExit {
    /// The window closed (command sender dropped).
    WindowClosed,
    /// The subscription stream errored or closed.
    Disconnected,
}

/// The subscribed loop: select over commands (hide-grace + delegate)
/// and events (drive [`HomeCore`]).
async fn connected_loop(
    mut stream: crate::ipc_server::Stream,
    cmd_rx: &mut tokio_mpsc::Receiver<PumpCommand>,
    directive_tx: &std::sync::mpsc::Sender<HomeDirective>,
    core: &mut HomeCore,
) -> LoopExit {
    loop {
        tokio::select! {
            command = cmd_rx.recv() => {
                match command {
                    None => return LoopExit::WindowClosed,
                    Some(command) => {
                        core.capture_requested();
                        tokio::time::sleep(Duration::from_millis(HIDE_GRACE_MS)).await;
                        match crate::ipc_client::delegate_async(None, &command.request()).await {
                            Ok(crate::ipc_client::DelegateOutcome::Delegated) => {}
                            Ok(crate::ipc_client::DelegateOutcome::NoDaemon) => {
                                report_delegate_failure(
                                    directive_tx,
                                    core,
                                    "no daemon answered the capture request",
                                );
                            }
                            Err(e) => report_delegate_failure(
                                directive_tx,
                                core,
                                &format!("capture request failed: {e}"),
                            ),
                        }
                    }
                }
            }
            frame = read_frame(&mut stream) => {
                let decoded = frame.and_then(|payload| decode_event(&payload));
                if let Ok(event) = decoded {
                    if let Some(directive) = core.on_event(&event) {
                        let _ = directive_tx.send(directive);
                    }
                } else {
                    tracing::warn!("daemon subscription died");
                    for directive in core.disconnected() {
                        let _ = directive_tx.send(directive);
                    }
                    return LoopExit::Disconnected;
                }
            }
        }
    }
}

/// No subscription (start failure or disconnect): keep answering
/// commands with an error status + show, so a click can never strand
/// the window hidden.
async fn degraded_loop(
    mut cmd_rx: tokio_mpsc::Receiver<PumpCommand>,
    directive_tx: &std::sync::mpsc::Sender<HomeDirective>,
    core: &mut HomeCore,
) {
    while let Some(command) = cmd_rx.recv().await {
        core.capture_requested();
        report_delegate_failure(
            directive_tx,
            core,
            "daemon unreachable — restart `flintshot app` after starting the daemon",
        );
        let _ = command; // the request itself is unservable here
    }
}

/// Tell the UI a capture request went nowhere: un-hide + status.
fn report_delegate_failure(
    directive_tx: &std::sync::mpsc::Sender<HomeDirective>,
    core: &mut HomeCore,
    message: &str,
) {
    tracing::warn!("{message}");
    if let Some(directive) = core.force_show() {
        let _ = directive_tx.send(directive);
    }
    let _ = directive_tx.send(HomeDirective::SetStatus(message.to_string()));
}

/// [`HomeBridge`] impl joining the egui window to the pump: commands
/// out (blocking send — the UI thread is outside the pump's
/// reactor), directives in (polled every frame).
struct Bridge {
    cmd_tx: tokio_mpsc::Sender<PumpCommand>,
    directive_rx: std::sync::mpsc::Receiver<HomeDirective>,
}

impl HomeBridge for Bridge {
    fn request_capture_region(&mut self) {
        if let Err(e) = self.cmd_tx.blocking_send(PumpCommand::Region) {
            tracing::warn!(error = %e, "app pump is gone; capture request dropped");
        }
    }

    fn request_capture_fullscreen(&mut self, monitor: Option<u32>) {
        if let Err(e) = self.cmd_tx.blocking_send(PumpCommand::Fullscreen(monitor)) {
            tracing::warn!(error = %e, "app pump is gone; capture request dropped");
        }
    }

    fn poll_directive(&mut self) -> Option<HomeDirective> {
        self.directive_rx.try_recv().ok()
    }

    fn monitors(&mut self) -> Vec<MonitorEntry> {
        monitor_entries()
    }
}

/// Build the monitor picker entries, mirroring the tray submenu's
/// label format. Enumeration failure yields an empty list — the
/// picker falls back to "Primary monitor".
fn monitor_entries() -> Vec<MonitorEntry> {
    match flintshot_core::monitors() {
        Ok(list) => list
            .iter()
            .map(|m| MonitorEntry {
                index: m.index,
                label: if m.is_primary {
                    format!("{} · {} (primary)", m.index, m.name)
                } else {
                    format!("{} · {}", m.index, m.name)
                },
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_round_trip_shows_on_session_end() {
        let mut core = HomeCore::default();
        core.capture_requested();
        assert_eq!(core.on_event(&IpcEvent::CaptureStarted), None);
        assert_eq!(
            core.on_event(&IpcEvent::SessionEnded),
            Some(HomeDirective::Show),
            "the last worker exiting restores the window"
        );
        assert!(!core.hidden);
    }

    #[test]
    fn session_ended_without_request_does_not_show() {
        // A tray/hotkey capture completing while our window is
        // visible must not steal focus.
        let mut core = HomeCore::default();
        assert_eq!(core.on_event(&IpcEvent::CaptureStarted), None);
        assert_eq!(core.on_event(&IpcEvent::SessionEnded), None);
        assert!(!core.hidden, "nothing to restore");
    }

    #[test]
    fn overlapping_captures_show_only_after_both_end() {
        let mut core = HomeCore::default();
        core.capture_requested();
        core.on_event(&IpcEvent::CaptureStarted); // ours
        core.on_event(&IpcEvent::CaptureStarted); // a racing hotkey
        assert_eq!(core.on_event(&IpcEvent::SessionEnded), None);
        assert_eq!(
            core.on_event(&IpcEvent::SessionEnded),
            Some(HomeDirective::Show)
        );
    }

    #[test]
    fn pre_subscription_session_ended_clamps_at_zero() {
        let mut core = HomeCore::default();
        core.capture_requested();
        // A worker from before our subscription exits (in_flight was
        // 0): saturating, no underflow; the show is correct — we are
        // hidden and nothing is flying.
        assert_eq!(
            core.on_event(&IpcEvent::SessionEnded),
            Some(HomeDirective::Show)
        );
        assert_eq!(core.in_flight, 0);
        // A second stray Ended stays a no-op.
        assert_eq!(core.on_event(&IpcEvent::SessionEnded), None);
    }

    #[test]
    fn app_focus_refocuses_idle_visible_window() {
        let mut core = HomeCore::default();
        assert_eq!(
            core.on_event(&IpcEvent::AppFocus),
            Some(HomeDirective::Show),
            "re-focusing a visible idle window is the ping's purpose"
        );
    }

    #[test]
    fn app_focus_ignored_mid_capture() {
        let mut core = HomeCore::default();
        core.capture_requested();
        core.on_event(&IpcEvent::CaptureStarted);
        assert_eq!(
            core.on_event(&IpcEvent::AppFocus),
            None,
            "a focus ping must not raise the window over a live editor"
        );
    }

    #[test]
    fn app_focus_un_hides_an_idle_hidden_window() {
        // Hidden (grace started) but the delegate failed → not
        // in flight; a focus ping should still surface it.
        let mut core = HomeCore::default();
        core.capture_requested();
        assert_eq!(
            core.on_event(&IpcEvent::AppFocus),
            Some(HomeDirective::Show)
        );
    }

    #[test]
    fn disconnect_force_shows_with_status() {
        let mut core = HomeCore::default();
        core.capture_requested();
        core.on_event(&IpcEvent::CaptureStarted);
        let directives = core.disconnected();
        assert!(directives.contains(&HomeDirective::Show));
        assert!(matches!(
            directives.last(),
            Some(HomeDirective::SetStatus(_))
        ));
        assert_eq!(core.in_flight, 0);
        assert!(!core.hidden);
    }

    #[test]
    fn commands_map_to_requests() {
        assert_eq!(
            PumpCommand::Region.request(),
            Request::CaptureRegion {
                monitor: None,
                delay_ms: 0
            }
        );
        assert_eq!(
            PumpCommand::Fullscreen(Some(2)).request(),
            Request::CaptureFullscreen { monitor: Some(2) }
        );
    }

    #[test]
    fn hide_grace_matches_the_icon_grace_bound() {
        // 250 ms: the same bound ADR-0036 found sufficient for a
        // compositor-side disappearance. If either constant moves,
        // re-derive the other.
        assert_eq!(HIDE_GRACE_MS, 250);
    }
}
