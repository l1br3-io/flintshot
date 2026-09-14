//! Wayland global hotkeys via the xdg-desktop-portal `GlobalShortcuts`
//! interface (Phase 6).
//!
//! `global-hotkey` has no Wayland backend, and under `XWayland` its X11
//! grabs can register-but-never-fire — so on a Wayland session the
//! daemon is *portal-first*: it skips [`crate::hotkeys::HotkeyRegistry`]
//! entirely and runs the portal session owned here on the tokio side
//! (see ADR-0018's bridge; activations dispatch through the same
//! `worker::dispatch` table the IPC server uses on that thread).
//!
//! Binding is user-initiated (`flintshot hotkeys bind` over IPC, or
//! the tray's *Bind shortcuts…* item — Task 004). Startup is a
//! *silent attach*: `list_shortcuts` reports whatever the portal
//! already has bound under our ids — no dialog at daemon start.

use std::sync::{Arc, Mutex};

use ashpd::desktop::global_shortcuts::{
    BindShortcutsOptions, GlobalShortcuts, ListShortcutsOptions, NewShortcut, Shortcut,
};
use ashpd::desktop::CreateSessionOptions;
use futures_util::StreamExt;
use tokio::sync::Notify;

use crate::ipc_server::SubscriberBus;
use crate::worker::DispatchState;
use flintshot_core::ipc::{Request, ShortcutBinding};

/// The daemon's live `[hotkeys]` section: written by the main loop on
/// config hot-reload, read here at bind time so the dialog always
/// suggests the chords the config file has *right now* (see
/// `daemon::apply_wayland_hotkey_reload`).
pub(crate) type SharedHotkeys = Arc<Mutex<flintshot_core::HotkeyConfig>>;

/// Stable portal shortcut id for region capture. The same string is
/// used as the `BindShortcuts` id (Task 003), so re-binds keep
/// activating the same mapping.
pub(crate) const REGION_ID: &str = "region";

/// Stable portal shortcut id for fullscreen capture.
pub(crate) const FULLSCREEN_ID: &str = "fullscreen";

/// Convert a `[hotkeys]` chord from `global-hotkey`'s accelerator
/// format to the XDG desktop-entry accelerator format the portal
/// expects for `preferred_trigger` — e.g. `Ctrl+Shift+PrintScreen` →
/// `<Control><Shift>Print`.
///
/// Key-name table gotchas (why this is not a trivial rename):
/// - `global-hotkey` parses keys as keyboard-types `Code` names
///   (`PrintScreen`, `KeyA`, `Digit1`, `ArrowUp`); the portal side uses
///   GDK key names (`Print`, `a`, `1`, `Up`).
/// - Modifiers map to the desktop-entry long forms; `Ctrl` is **not**
///   valid there (`<Control>` is), and Meta/Super/Cmd aliases all
///   collapse to `<Super>`.
/// - Any unknown token (key **or** modifier) → `None`: the chord
///   degrades to "no suggestion" and the bind dialog still works —
///   the suggestion is a convenience, never authority.
pub(crate) fn convert_chord(chord: &str) -> Option<String> {
    let mut modifiers = Vec::new();
    let mut key = None;
    for token in chord.split('+') {
        if token.is_empty() {
            return None;
        }
        match modifier_for(token) {
            Some(m) => modifiers.push(m),
            // The final token is the key; a second key-looking token
            // (e.g. `KeyA+KeyB`) is invalid.
            None if key.is_none() => key = Some(gdk_key_name(token)?),
            None => return None,
        }
    }
    let key = key?;
    let mut out = String::with_capacity(modifiers.len() * 10 + key.len());
    for m in &modifiers {
        out.push_str(m);
    }
    out.push_str(&key);
    Some(out)
}

/// Modifier token → desktop-entry long form. Case-insensitive over
/// `global-hotkey`'s accepted aliases; `None` means "not a modifier"
/// (the token may still be the key).
fn modifier_for(token: &str) -> Option<&'static str> {
    match token.to_ascii_lowercase().as_str() {
        "ctrl" | "control" => Some("<Control>"),
        "alt" | "option" => Some("<Alt>"),
        "shift" => Some("<Shift>"),
        "meta" | "super" | "cmd" | "command" => Some("<Super>"),
        _ => None,
    }
}

/// keyboard-types `Code` name → GDK key name. `None` for anything not
/// in the table — the chord degrades to "no suggestion" rather than
/// risking a bogus accelerator the portal would reject.
fn gdk_key_name(code: &str) -> Option<String> {
    // KeyA..KeyZ / Digit0..Digit9 are generated below.
    let named = match code {
        "PrintScreen" => "Print",
        "ArrowUp" => "Up",
        "ArrowDown" => "Down",
        "ArrowLeft" => "Left",
        "ArrowRight" => "Right",
        "Space" => "space",
        "Enter" => "Return",
        "Backspace" => "BackSpace",
        "PageUp" => "Page_Up",
        "PageDown" => "Page_Down",
        "Escape" | "Tab" | "Delete" | "Home" | "End" | "Insert" => code,
        other => {
            // F1..F24 pass through verbatim.
            if let Some(n) = other.strip_prefix('F') {
                let n = n.parse::<u8>().ok()?;
                return (1..=24).contains(&n).then(|| other.to_string());
            }
            let letters = "Key";
            let digits = "Digit";
            if let Some(letter) = other.strip_prefix(letters) {
                let mut chars = letter.chars();
                let (head, tail) = (chars.next()?, chars.as_str());
                if letter.len() == 1 && head.is_ascii_uppercase() && tail.is_empty() {
                    return Some(head.to_ascii_lowercase().to_string());
                }
                return None;
            }
            if let Some(digit) = other.strip_prefix(digits) {
                return (digit.len() == 1 && digit.chars().all(|c| c.is_ascii_digit()))
                    .then(|| digit.to_string());
            }
            return None;
        }
    };
    Some(named.to_string())
}

/// Map a portal `Activated::shortcut_id` to the [`Request`] it should
/// dispatch. Both legacy shortcut ids (`region`, `fullscreen`) map to
/// the single [`Request::Capture`] — the OS UI or the editor owns
/// selection (native-capture-ux Task 005). Mirrors the chord →
/// request mapping `HotkeyRegistry` uses on X11. `None` for unknown
/// ids (logged + dropped by the caller).
pub(crate) fn request_for_shortcut_id(id: &str) -> Option<Request> {
    match id {
        REGION_ID | FULLSCREEN_ID => Some(Request::Capture { delay_ms: 0 }),
        _ => None,
    }
}

/// Which portal startup step failed. Each step fails for distinct
/// causes (see [`classify_failure`]), so classification needs to know
/// where the error came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PortalStage {
    /// Connecting to the session bus / building the portal proxy.
    Connect,
    /// `CreateSession` — a session bus exists, the portal answered.
    Session,
    /// `ListShortcuts` (silent attach) or listening for `Activated`.
    Attach,
}

/// Why the portal path is unavailable, in order of how far startup got.
/// Every variant is non-fatal: the daemon keeps serving tray + IPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FallbackKind {
    /// No D-Bus session bus reachable at all (headless box, or the
    /// compositor session is gone).
    NoSessionBus,
    /// The portal frontend is missing, or it does not implement the
    /// `GlobalShortcuts` interface (GNOME, most wlroots compositors).
    PortalMissing,
    /// The interface exists but is older than what we use.
    InterfaceUnsupported,
    /// Anything else — reported with the raw error.
    Other,
}

impl FallbackKind {
    /// The user-facing notice for this fallback. Every variant names
    /// the compositor-keybind workaround so the log line is actionable
    /// on its own.
    pub(crate) fn notice(self) -> &'static str {
        match self {
            Self::NoSessionBus => {
                "no D-Bus session bus reachable; Wayland global hotkeys unavailable. \
                 Workaround: bind a compositor keybinding to `flintshot region` / \
                 `flintshot fullscreen` (tray + IPC still work)"
            }
            Self::PortalMissing => {
                "no xdg-desktop-portal frontend with the GlobalShortcuts interface \
                 (GNOME and most wlroots compositors lack it; COSMIC and KDE ship it). \
                 Workaround: bind a compositor keybinding to `flintshot region` / \
                 `flintshot fullscreen` (tray + IPC still work)"
            }
            Self::InterfaceUnsupported => {
                "xdg-desktop-portal GlobalShortcuts interface is too old for flintshot; \
                 update xdg-desktop-portal (>= 1.16). Workaround meanwhile: bind a \
                 compositor keybinding to `flintshot region` / `flintshot fullscreen`"
            }
            Self::Other => {
                "portal GlobalShortcuts startup failed for an unknown reason. \
                 Workaround: bind a compositor keybinding to `flintshot region` / \
                 `flintshot fullscreen` (tray + IPC still work)"
            }
        }
    }
}

/// Classify a portal startup failure from the failing [`PortalStage`]
/// and the error's `Display` string. String-based on purpose: the
/// structured causes live in zbus/ashpd error types we do not depend
/// on, and the matched fragments are stable spec'd D-Bus / ashpd
/// messages, not incidental formatting.
pub(crate) fn classify_failure(stage: PortalStage, error: &str) -> FallbackKind {
    // ashpd::Error::PortalNotFound: frontend missing or interface not
    // implemented by the installed frontend.
    if error.contains("A portal frontend implementing") {
        return FallbackKind::PortalMissing;
    }
    // Spec'd D-Bus ServiceUnknown message: the portal service is not
    // installed / not owning org.freedesktop.portal.Desktop.
    if error.contains("was not provided by any .service files") {
        return FallbackKind::PortalMissing;
    }
    // ashpd::Error::RequiresVersion.
    if error.contains("This interface requires version") {
        return FallbackKind::InterfaceUnsupported;
    }
    match stage {
        // Connection/address/handshake errors surface at proxy
        // construction — no session bus.
        PortalStage::Connect => FallbackKind::NoSessionBus,
        PortalStage::Session | PortalStage::Attach => FallbackKind::Other,
    }
}

/// Run the portal `GlobalShortcuts` session for the daemon's lifetime:
/// connect, create a session, silently attach to whatever shortcuts
/// are already bound under our ids, then dispatch every `Activated`
/// through the shared worker table (same path as IPC requests on this
/// thread). `Deactivated` is deliberately not consumed — dispatching
/// on `Activated` only mirrors the Pressed-only rule in
/// `daemon::drain_hotkeys` and avoids double-firing.
///
/// Any startup failure is classified + logged and the task returns:
/// non-fatal by design, the daemon keeps serving tray + IPC. When the
/// runtime drops (Ctrl-C / IPC `Quit`), this future is dropped, the
/// zbus connection goes away, and the portal closes the session with
/// it — no explicit close needed.
///
/// Returns a [`PortalHandle`] immediately; the session task runs in
/// the background. Bind requests sent before startup finishes simply
/// wait in the channel until the task reaches its command loop (or,
/// if startup failed, receive the classified failure).
///
/// `bind_requested` is the tray's trigger: `notify_waiters()` on it
/// (sync-callable from the daemon's main thread) runs the same bind
/// path as `Request::BindShortcuts`, minus the reply channel — the
/// outcome only lands in the log.
pub(crate) fn start(
    bus: SubscriberBus,
    state: DispatchState,
    hotkeys: SharedHotkeys,
    bind_requested: Arc<Notify>,
    capture: crate::worker::CaptureDispatch,
) -> PortalHandle {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let unavailable = Arc::new(std::sync::Mutex::new(None));
    let handle = PortalHandle {
        tx,
        unavailable: Arc::clone(&unavailable),
    };
    tokio::spawn(session_task(
        bus,
        state,
        hotkeys,
        bind_requested,
        capture,
        rx,
        unavailable,
    ));
    handle
}

/// The portal session task behind [`start`]: startup (silent attach)
/// then a select loop over activations, `ShortcutsChanged`, bind
/// commands, and the tray's bind trigger.
async fn session_task(
    bus: SubscriberBus,
    state: DispatchState,
    hotkeys: SharedHotkeys,
    bind_requested: Arc<Notify>,
    capture: crate::worker::CaptureDispatch,
    mut cmds: tokio::sync::mpsc::UnboundedReceiver<PortalCmd>,
    unavailable: Arc<std::sync::Mutex<Option<BindFailure>>>,
) {
    // GOTCHA (app-id; flagged for ADR-0029 in Task 005): the portal
    // resolves the caller's app-id from the bus connection's sender
    // credentials and matches it against installed .desktop files. Our
    // daemon is windowless and runs as the `flintshot` binary — if the
    // bind dialog (Task 003) shows no/wrong app identity, install a
    // desktop file whose basename matches the binary
    // (`flintshot.desktop`). Worst case is cosmetic.
    let shortcuts = match GlobalShortcuts::new().await {
        Ok(s) => s,
        Err(e) => return abort(&unavailable, &mut cmds, PortalStage::Connect, &e),
    };
    let session = match shortcuts
        .create_session(CreateSessionOptions::default())
        .await
    {
        Ok(s) => s,
        Err(e) => return abort(&unavailable, &mut cmds, PortalStage::Session, &e),
    };

    // Silent attach: ListShortcuts never shows a dialog (only
    // BindShortcuts does), so this reports the current bindings without
    // touching the user. (The portal Response signal is consumed inside
    // the call; `response()` just takes the cached result.)
    let listed = match shortcuts
        .list_shortcuts(&session, ListShortcutsOptions::default())
        .await
    {
        Ok(request) => match request.response() {
            Ok(listed) => listed,
            Err(e) => return abort(&unavailable, &mut cmds, PortalStage::Attach, &e),
        },
        Err(e) => return abort(&unavailable, &mut cmds, PortalStage::Attach, &e),
    };
    log_bound_shortcuts(listed.shortcuts());

    let mut activated = match shortcuts.receive_activated().await {
        Ok(stream) => stream,
        Err(e) => return abort(&unavailable, &mut cmds, PortalStage::Attach, &e),
    };
    // Re-binds done outside flintshot (the compositor's own settings
    // UI) arrive here; consumed below to keep the logged binding set
    // honest. Dispatch needs no refresh: ids are stable, and the
    // portal only emits `Activated` for shortcuts that are bound.
    let mut changed = match shortcuts.receive_shortcuts_changed().await {
        Ok(stream) => stream,
        Err(e) => return abort(&unavailable, &mut cmds, PortalStage::Attach, &e),
    };
    loop {
        tokio::select! {
            event = activated.next() => {
                let Some(event) = event else {
                    session_ended(&unavailable, "the Activated signal stream closed");
                    break;
                };
                dispatch_activation(&capture, &bus, &state, event.shortcut_id());
            }
            event = changed.next() => {
                let Some(event) = event else {
                    session_ended(&unavailable, "the ShortcutsChanged signal stream closed");
                    break;
                };
                log_shortcuts_changed(event.shortcuts());
            }
            cmd = cmds.recv() => {
                let Some(PortalCmd::Bind { reply }) = cmd else {
                    session_ended(&unavailable, "every portal handle was dropped");
                    break;
                };
                let result = do_bind(&shortcuts, &session, &hotkeys).await;
                let _ = reply.send(result);
            }
            // Tray "Bind shortcuts…": same dialog as the IPC path,
            // outcome logged instead of replied.
            () = bind_requested.notified() => {
                tracing::info!("bind requested from the tray; opening portal bind dialog");
                log_tray_bind(do_bind(&shortcuts, &session, &hotkeys).await);
            }
        }
    }
}

/// A live session ended without a startup-classified cause: a signal
/// stream closed (xdg-desktop-portal crashed or restarted) or every
/// handle went away. Hotkeys are dead from here on, so say so loudly
/// — the spec's stance is that every portal log line names the cause
/// and the workaround — and leave a note on the handle so a later
/// `bind()` fails fast with the fix instead of a vague `gone`.
fn session_ended(unavailable: &Arc<std::sync::Mutex<Option<BindFailure>>>, source: &str) {
    tracing::warn!(
        source = %source,
        "portal session ended; global hotkeys stopped - restart the daemon (`flintshot daemon --replace`) to re-attach"
    );
    let failure = BindFailure {
        code: "bind-failed".into(),
        message: format!(
            "portal session ended ({source}); hotkeys stopped - restart the daemon to re-attach"
        ),
    };
    *unavailable.lock().expect("portal note mutex poisoned") = Some(failure);
}

/// Handle one `Activated` signal: map the shortcut id and dispatch the
/// `Request` through the shared worker table (same path as IPC on this
/// thread). Capture requests take the supersede-then-spawn path
/// (Replace policy) — spawned as their own task so the portal's select
/// loop stays responsive through its bounded waits (the capture
/// context serializes concurrent triggers; latest wins). Unknown ids
/// drop at debug.
fn dispatch_activation(
    capture: &crate::worker::CaptureDispatch,
    bus: &SubscriberBus,
    state: &DispatchState,
    id: &str,
) {
    match request_for_shortcut_id(id) {
        Some(request) => {
            if crate::worker::spawns_capture_worker(&request) {
                let capture = capture.clone();
                let bus = bus.clone();
                let state = state.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        crate::worker::dispatch_capture(capture, request, state, bus).await
                    {
                        tracing::warn!(error = %e, "portal hotkey capture dispatch failed");
                    }
                });
            } else if let Err(e) = crate::worker::dispatch(
                &request,
                &crate::worker::CommandSpawner::with_bus_and_sessions(
                    bus.clone(),
                    capture.sessions(),
                ),
                state,
                &mut |ev| bus.broadcast(ev),
            ) {
                tracing::warn!(error = %e, shortcut_id = %id, "portal hotkey dispatch failed");
            }
        }
        None => {
            tracing::debug!(shortcut_id = %id, "activation for unknown shortcut id; dropping");
        }
    }
}

/// Startup failed: record the classified reason on the handle (later
/// `bind()` calls short-circuit with it) and answer anything already
/// waiting in the channel, then end the task.
fn abort(
    unavailable: &Arc<std::sync::Mutex<Option<BindFailure>>>,
    cmds: &mut tokio::sync::mpsc::UnboundedReceiver<PortalCmd>,
    stage: PortalStage,
    error: &ashpd::Error,
) {
    log_fallback(stage, error);
    let kind = classify_failure(stage, &error.to_string());
    let failure = BindFailure {
        code: "bind-failed".into(),
        message: format!("{} ({error})", kind.notice()),
    };
    *unavailable.lock().expect("portal note mutex poisoned") = Some(failure.clone());
    while let Ok(PortalCmd::Bind { reply }) = cmds.try_recv() {
        let _ = reply.send(Err(failure.clone()));
    }
}

/// Run the user-initiated bind: open the portal dialog with one
/// `NewShortcut` per capture id, the *current* `[hotkeys]` chords (the
/// daemon keeps the shared copy fresh on config edits) as *suggested*
/// bindings (converted; a chord with no portal form degrades to "no
/// suggestion" at warn). Awaiting the response blocks until the user
/// closes the dialog; the caller then receives what the user actually
/// chose (suggestions are not authority). A repeat bind on the same
/// session replaces the bindings — the portal treats it as a re-bind.
async fn do_bind(
    shortcuts: &GlobalShortcuts,
    session: &ashpd::desktop::Session<GlobalShortcuts>,
    hotkeys: &SharedHotkeys,
) -> Result<Vec<ShortcutBinding>, BindFailure> {
    // Clone out under a short lock — never held across the dialog
    // await (clippy::await_holding_lock).
    let hotkeys = hotkeys
        .lock()
        .expect("hotkeys config mutex poisoned")
        .clone();
    let to_bind = vec![
        suggested_shortcut(REGION_ID, "Capture region", &hotkeys.region),
        suggested_shortcut(FULLSCREEN_ID, "Capture fullscreen", &hotkeys.fullscreen),
    ];
    let bound = shortcuts
        .bind_shortcuts(session, &to_bind, None, BindShortcutsOptions::default())
        .await
        .and_then(|request| request.response());
    match bound {
        Ok(bound) => {
            // Only our ids matter; the dialog cannot add foreign ones,
            // but filtering keeps the reply shape honest.
            let bindings: Vec<ShortcutBinding> = bound
                .shortcuts()
                .iter()
                .filter(|s| request_for_shortcut_id(s.id()).is_some())
                .map(|s| ShortcutBinding {
                    id: s.id().to_string(),
                    trigger: s.trigger_description().to_string(),
                })
                .collect();
            for b in &bindings {
                tracing::info!(id = %b.id, trigger = %b.trigger, "portal shortcut bound");
            }
            Ok(bindings)
        }
        Err(e) => Err(BindFailure {
            code: bind_failure_code_for_display(&e.to_string()).into(),
            message: e.to_string(),
        }),
    }
}

/// Tray-initiated bind outcome: success and dismissal are both normal
/// user endings (`info`); anything else is a real failure (`warn`).
fn log_tray_bind(result: Result<Vec<ShortcutBinding>, BindFailure>) {
    match result {
        Ok(bindings) => {
            tracing::info!(count = bindings.len(), "portal bind finished (tray)");
        }
        Err(failure) if failure.code == "bind-cancelled" => {
            tracing::info!("portal bind dialog dismissed (tray)");
        }
        Err(failure) => {
            tracing::warn!(code = %failure.code, error = %failure.message, "portal bind failed (tray)");
        }
    }
}

/// One dialog row: id + description, with the config chord as the
/// preferred trigger when it converts (warn + bare otherwise).
fn suggested_shortcut(id: &str, description: &str, chord: &str) -> NewShortcut {
    if let Some(accelerator) = convert_chord(chord) {
        NewShortcut::new(id, description).preferred_trigger(accelerator.as_str())
    } else {
        tracing::warn!(
            chord = %chord,
            "chord has no portal accelerator form; binding without a suggestion"
        );
        NewShortcut::new(id, description)
    }
}

/// Stable `Event::Error` code for a bind failure display string:
/// user-dismissed dialogs are distinct from real failures.
fn bind_failure_code_for_display(display: &str) -> &'static str {
    if display.contains("cancelled") {
        "bind-cancelled"
    } else {
        "bind-failed"
    }
}

// ── bind handle ──────────────────────────────────────────────────────

/// A command to the portal session task.
pub(crate) enum PortalCmd {
    /// Open the portal bind dialog; reply with the outcome.
    Bind {
        /// Where the bind result (or failure) is sent.
        reply: tokio::sync::oneshot::Sender<Result<Vec<ShortcutBinding>, BindFailure>>,
    },
}

/// Why a bind failed: a stable machine-readable code (mirrored into
/// `Event::Error.code`) plus a human-readable message.
#[derive(Debug, Clone)]
pub(crate) struct BindFailure {
    /// `"bind-failed"` or `"bind-cancelled"`.
    pub(crate) code: String,
    /// What went wrong, for the CLI to print.
    pub(crate) message: String,
}

/// Cloneable handle to the daemon's portal session: the IPC layer's
/// entry point for `Request::BindShortcuts`. Cheap to keep even when
/// the portal is unavailable — [`Self::bind`] then fails fast with the
/// classified reason.
#[derive(Clone)]
pub(crate) struct PortalHandle {
    tx: tokio::sync::mpsc::UnboundedSender<PortalCmd>,
    unavailable: Arc<std::sync::Mutex<Option<BindFailure>>>,
}

impl PortalHandle {
    /// A handle whose session can never start (non-Wayland daemon, or
    /// test harness): every bind fails immediately with `reason`.
    pub(crate) fn unavailable(reason: &str) -> Self {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            // Dropping `_rx` makes late sends fail; the note below is
            // what bind() actually reads.
            tx,
            unavailable: Arc::new(std::sync::Mutex::new(Some(BindFailure {
                code: "bind-failed".into(),
                message: reason.to_string(),
            }))),
        }
    }

    /// Open the portal bind dialog and wait for the user to finish.
    /// Returns the actual bindings chosen, or a classified failure.
    ///
    /// # Errors
    /// - Portal session unavailable/failed at startup (reason carried
    ///   from the classified fallback).
    /// - The dialog was dismissed (`code = "bind-cancelled"`).
    /// - The bind itself failed (`code = "bind-failed"`).
    pub(crate) async fn bind(&self) -> Result<Vec<ShortcutBinding>, BindFailure> {
        if let Some(failure) = self.failure_note() {
            return Err(failure);
        }
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        if self.tx.send(PortalCmd::Bind { reply: reply_tx }).is_err() {
            return Err(self.gone());
        }
        reply_rx.await.unwrap_or_else(|_| Err(self.gone()))
    }

    /// The recorded startup failure, if any. Read via a helper so the
    /// lock is never held across an await.
    fn failure_note(&self) -> Option<BindFailure> {
        self.unavailable
            .lock()
            .expect("portal note mutex poisoned")
            .clone()
    }

    fn gone(&self) -> BindFailure {
        self.failure_note().unwrap_or_else(|| BindFailure {
            code: "bind-failed".into(),
            message: "the portal session is gone".into(),
        })
    }

    /// Test seam: a handle wired to a receiver the test drives, so the
    /// bind round-trip is exercisable without a portal.
    #[cfg(test)]
    pub(crate) fn stub() -> (Self, tokio::sync::mpsc::UnboundedReceiver<PortalCmd>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (
            Self {
                tx,
                unavailable: Arc::new(std::sync::Mutex::new(None)),
            },
            rx,
        )
    }
}

/// Our ids from a portal shortcut list, as `"id=trigger"` pairs.
/// Only our ids matter; anything else bound in the session is left
/// alone.
fn our_bound_set(shortcuts: &[Shortcut]) -> Vec<String> {
    shortcuts
        .iter()
        .filter(|s| request_for_shortcut_id(s.id()).is_some())
        .map(|s| format!("{}={}", s.id(), s.trigger_description()))
        .collect()
}

/// Log the bound-shortcut set from the silent attach.
fn log_bound_shortcuts(shortcuts: &[Shortcut]) {
    let bound = our_bound_set(shortcuts);
    if bound.is_empty() {
        tracing::info!("portal session attached; nothing bound yet — run `flintshot hotkeys bind`");
    } else {
        tracing::info!(
            bound = %bound.join(", "),
            "portal session attached; bound shortcuts"
        );
    }
}

/// Log a `ShortcutsChanged` notification: the bindings were re-made
/// outside flintshot (typically the compositor's own settings UI).
/// The id → request mapping stays valid by construction — ids are
/// stable strings, so this is bookkeeping, not a re-subscribe.
fn log_shortcuts_changed(shortcuts: &[Shortcut]) {
    let bound = our_bound_set(shortcuts);
    if bound.is_empty() {
        tracing::info!(
            "portal shortcuts changed outside flintshot; nothing of ours is bound — \
             re-run `flintshot hotkeys bind` (or the tray item) to re-bind"
        );
    } else {
        tracing::info!(
            bound = %bound.join(", "),
            "portal shortcuts changed outside flintshot; new binding set"
        );
    }
}

/// Log a classified startup failure. `NoSessionBus` is `info` (it is the
/// expected state on a headless box); everything else is `warn`.
fn log_fallback(stage: PortalStage, error: &ashpd::Error) {
    match classify_failure(stage, &error.to_string()) {
        FallbackKind::NoSessionBus => {
            tracing::info!(error = %error, "{}", FallbackKind::NoSessionBus.notice());
        }
        kind @ (FallbackKind::PortalMissing
        | FallbackKind::InterfaceUnsupported
        | FallbackKind::Other) => {
            tracing::warn!(error = %error, "{}", kind.notice());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── request_for_shortcut_id ──────────────────────────────────

    #[test]
    fn region_id_maps_to_capture() {
        assert!(matches!(
            request_for_shortcut_id(REGION_ID),
            Some(Request::Capture { delay_ms: 0 })
        ));
    }

    #[test]
    fn fullscreen_id_maps_to_capture() {
        assert!(matches!(
            request_for_shortcut_id(FULLSCREEN_ID),
            Some(Request::Capture { delay_ms: 0 })
        ));
    }

    #[test]
    fn unknown_ids_map_to_none() {
        assert!(request_for_shortcut_id("nope").is_none());
        assert!(request_for_shortcut_id("").is_none());
        assert!(request_for_shortcut_id("Region").is_none());
    }

    // ── convert_chord ────────────────────────────────────────────

    #[test]
    fn default_chords_convert_to_valid_portal_accelerators() {
        assert_eq!(
            convert_chord("Ctrl+Shift+PrintScreen").as_deref(),
            Some("<Control><Shift>Print")
        );
        assert_eq!(
            convert_chord("Ctrl+Alt+PrintScreen").as_deref(),
            Some("<Control><Alt>Print")
        );
    }

    #[test]
    fn letters_and_digits_map_to_gdk_names() {
        assert_eq!(convert_chord("Ctrl+KeyQ").as_deref(), Some("<Control>q"));
        assert_eq!(convert_chord("Ctrl+Digit3").as_deref(), Some("<Control>3"));
    }

    #[test]
    fn modifier_aliases_collapse_to_long_forms() {
        assert_eq!(
            convert_chord("Control+Shift+KeyP").as_deref(),
            Some("<Control><Shift>p")
        );
        assert_eq!(convert_chord("Super+KeyS").as_deref(), Some("<Super>s"));
        assert_eq!(
            convert_chord("Meta+Alt+Shift+KeyS").as_deref(),
            Some("<Super><Alt><Shift>s")
        );
        assert_eq!(convert_chord("Command+KeyS").as_deref(), Some("<Super>s"));
    }

    #[test]
    fn function_and_navigation_keys_map() {
        assert_eq!(convert_chord("Shift+F2").as_deref(), Some("<Shift>F2"));
        assert_eq!(convert_chord("Alt+ArrowUp").as_deref(), Some("<Alt>Up"));
        assert_eq!(
            convert_chord("Ctrl+Space").as_deref(),
            Some("<Control>space")
        );
        assert_eq!(
            convert_chord("Ctrl+Enter").as_deref(),
            Some("<Control>Return")
        );
        assert_eq!(convert_chord("Alt+Tab").as_deref(), Some("<Alt>Tab"));
    }

    #[test]
    fn unknown_tokens_degrade_to_none() {
        assert_eq!(convert_chord("Ctrl+NotARealKey"), None);
        assert_eq!(convert_chord("Hyper+Shift+KeyA"), None);
        assert_eq!(convert_chord(""), None);
        // A bare key with no modifier is still a valid accelerator.
        assert_eq!(convert_chord("PrintScreen").as_deref(), Some("Print"));
    }

    // ── PortalHandle ─────────────────────────────────────────────

    use flintshot_core::ipc::ShortcutBinding;

    fn sample_binding() -> ShortcutBinding {
        ShortcutBinding {
            id: REGION_ID.into(),
            trigger: "Print".into(),
        }
    }

    #[tokio::test]
    async fn unavailable_handle_fails_bind_with_its_reason() {
        let handle = PortalHandle::unavailable("no portal for you");
        let err = handle.bind().await.expect_err("must fail");
        assert_eq!(err.code, "bind-failed");
        assert!(err.message.contains("no portal for you"), "got: {err:?}");
    }

    #[test]
    fn session_ended_leaves_an_actionable_note() {
        let unavailable = Arc::new(std::sync::Mutex::new(None));
        session_ended(&unavailable, "the Activated signal stream closed");
        let note = unavailable
            .lock()
            .expect("portal note mutex poisoned")
            .clone()
            .expect("note recorded");
        assert_eq!(note.code, "bind-failed");
        assert!(
            note.message.contains("the Activated signal stream closed"),
            "names the cause, got: {note:?}"
        );
        assert!(
            note.message.contains("restart the daemon"),
            "names the fix, got: {note:?}"
        );
    }

    #[tokio::test]
    async fn bind_round_trips_through_the_channel() {
        let (handle, mut rx) = PortalHandle::stub();
        let binder = tokio::spawn(async move { handle.bind().await });
        let cmd = rx.recv().await.expect("bind cmd arrives");
        let PortalCmd::Bind { reply } = cmd;
        reply.send(Ok(vec![sample_binding()])).expect("reply sends");
        let result = binder.await.expect("join");
        assert_eq!(result.expect("bind ok"), vec![sample_binding()]);
    }

    #[tokio::test]
    async fn bind_failure_round_trips_through_the_channel() {
        let (handle, mut rx) = PortalHandle::stub();
        let binder = tokio::spawn(async move { handle.bind().await });
        let PortalCmd::Bind { reply } = rx.recv().await.expect("bind cmd arrives");
        reply
            .send(Err(BindFailure {
                code: "bind-cancelled".into(),
                message: "dismissed".into(),
            }))
            .expect("reply sends");
        let err = binder.await.expect("join").expect_err("must fail");
        assert_eq!(err.code, "bind-cancelled");
    }

    #[tokio::test]
    async fn dropped_task_fails_bind_with_gone_message() {
        let (handle, rx) = PortalHandle::stub();
        drop(rx);
        let err = handle.bind().await.expect_err("must fail");
        assert_eq!(err.code, "bind-failed");
        assert!(err.message.contains("gone"), "got: {err:?}");
    }

    #[test]
    fn bind_failure_codes_are_stable_strings() {
        assert_eq!(
            bind_failure_code_for_display("Portal request was cancelled"),
            "bind-cancelled"
        );
        assert_eq!(
            bind_failure_code_for_display("anything else"),
            "bind-failed"
        );
    }

    // ── classify_failure ─────────────────────────────────────────

    #[test]
    fn portal_not_found_string_means_portal_missing() {
        // ashpd::Error::PortalNotFound Display.
        let err = "A portal frontend implementing \
                   `org.freedesktop.portal.GlobalShortcuts` was not found";
        assert_eq!(
            classify_failure(PortalStage::Connect, err),
            FallbackKind::PortalMissing
        );
    }

    #[test]
    fn service_unknown_string_means_portal_missing() {
        // zbus fdo ServiceUnknown Display (spec'd message) — portal
        // frontend not installed / not owning its bus name.
        let err = "Portal request failed: ZBus Error: The name \
                   org.freedesktop.portal.Desktop was not provided by any .service files";
        assert_eq!(
            classify_failure(PortalStage::Session, err),
            FallbackKind::PortalMissing
        );
    }

    #[test]
    fn version_gated_string_means_interface_unsupported() {
        // ashpd::Error::RequiresVersion Display.
        let err = "This interface requires version 3, but 1 is available";
        assert_eq!(
            classify_failure(PortalStage::Attach, err),
            FallbackKind::InterfaceUnsupported
        );
    }

    #[test]
    fn connect_stage_failure_means_no_session_bus() {
        // Any zbus connection failure surfacing at proxy construction
        // is a session-bus problem (address, handshake, io).
        for err in [
            "ZBus Error: Could not determine the address of the session bus",
            "ZBus Error: Handshake failed",
        ] {
            assert_eq!(
                classify_failure(PortalStage::Connect, err),
                FallbackKind::NoSessionBus,
                "expected NoSessionBus for: {err}"
            );
        }
    }

    #[test]
    fn later_stage_unknown_failure_is_other() {
        assert_eq!(
            classify_failure(PortalStage::Session, "something odd"),
            FallbackKind::Other
        );
    }

    // ── notices ──────────────────────────────────────────────────

    #[test]
    fn every_notice_names_the_compositor_keybind_workaround() {
        for kind in [
            FallbackKind::NoSessionBus,
            FallbackKind::PortalMissing,
            FallbackKind::InterfaceUnsupported,
            FallbackKind::Other,
        ] {
            let notice = kind.notice();
            assert!(!notice.is_empty(), "{kind:?} has no notice");
            assert!(
                notice.contains("compositor"),
                "{kind:?} notice must name the compositor-keybind workaround: {notice}"
            );
            assert!(
                notice.contains("flintshot region"),
                "{kind:?} notice must point at `flintshot region`: {notice}"
            );
        }
    }
}
