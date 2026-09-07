# ADR-0042: The tray as a helper process whose exit closes the panel's popup

**Context.** Reported live on COSMIC (2026-09-07, twice): the SNI
tray **menu** still appears in screenshots launched from the tray,
after both the ADR-0036 handshake (icon hidden) and the ADR-0037
menu grace (600 ms click-anchored wait). Source reading of upstream
`cosmic-applets` master confirms why no timing can fix it:

- `cosmic-applet-status-area` **never closes an SNI popup on
  activation** — `status_menu.rs` `Msg::ClickToken` carries the
  literal `// TODO: Close menu?` to this day. The grace waits out a
  close that never happens.
- The only event that destroys the open popup is
  `Event::Unregistered` (`app.rs`), which the watcher emits when the
  item's **owning process leaves the session bus**.
- A living process cannot unregister: SNI has no unregister method,
  and `libappindicator` 0.9's `AppIndicator` wrapper is a raw
  pointer with no `Drop`/unref (verified in the vendored source), so
  the ADR-0038 drop-and-rebuild approach both leaked the
  registration (ADR-0039's duplication) *and* never fired
  `Unregistered`.

Process exit is therefore the one causal lever. The ADR-0039
duplication risk does not apply to a real exit: the watcher removes
the item when the bus name drops, and the replacement is a **new
process** with a fresh bus name — no same-path re-registration, no
leaked object.

**Decision.** The tray icon moves out of the daemon into a
**helper process**: the hidden `flintshot tray` subcommand
(`tray_helper.rs`) builds the icon + menu behind the ADR-0032
watcher barrier, pumps GLib (ADR-0031), drains `MenuEvent`s, and
forwards clicks to the daemon over IPC like any client. The daemon's
`TraySupervisor` spawns it at startup and reaps/spawns on the tray
gate's orders: `TrayAction::Hide` **stops the helper** (exit →
unregister → panel destroys popup), `Show`/`ForceShow` **respawn**
it. The ADR-0036/0037 machinery is unchanged — depths, graces,
handshake, safety bound all stand; only the action's meaning
changed from a `set_visible` flip to process lifecycle.

- **Capture/quit clicks exit the helper by design** (after
  forwarding): the click's own exit *is* the pre-hide. The
  supervisor observes the unexpected exit and feeds it to the gate
  as `pre_hide(TrayMenu)` — menu grace anchored at the observation,
  hold window bridging the spawn+handshake latency, exactly the
  ADR-0037 shape with a causal chain instead of a speculative
  animation to wait out.
- **Hotkey/IPC captures** pre-hide as before (hotkey: at dispatch;
  IPC: the handshake's step), now as a supervisor stop; the icon
  grace covers the exit → unregister → panel-remove chain.
- **Daemon death** ends the helper: it holds a Subscribe connection
  and exits when the stream EOFs, so no stale icon outlives the
  daemon. `Quit` clicks exit it directly.
- macOS/Windows ride the same mechanism (their native menus close
  on activation anyway; the respawn cost is the price of one code
  path).

**Consequences.**

- Tray-launched captures finally grab a **menu-free frame on COSMIC**
  — causally, not by timing luck. Click-to-grab stays ≈ 600–700 ms
  (observation-anchored menu grace), the same rhythm as before.
- Every daemon capture costs a helper **respawn** after the grab:
  the icon disappears at the trigger (as the Passive flip did) and
  returns ~0.5–1 s after the pixels stop mattering, via a full SNI
  registration. Monitor hot-plug refreshes the *Fullscreen on
  monitor* submenu per respawn (a small win back). A dead watcher at
  respawn re-runs the ≤ ~2 s poke barrier (rare, logged).
- Startup race: the helper may spawn before the daemon's IPC socket
  is bound; its watch exits, the supervisor classifies the exit as a
  click pre-hide, and the hold window retries at ~500 ms cadence
  until the socket is up. Bounded, logged, self-healing.
- A daemon restart may briefly overlap the old helper (stream-EOF
  exit not yet observed) with a new one — at worst a transient
  duplicate icon for well under a second, the same class of race SNI
  panels always have across restarts.
- The ADR-0041 applet fix is unaffected; the two compose (an applet
  capture also stops the helper, closing any open SNI popup).
- Upstream's `TODO: Close menu?` remains the better long-term fix
  for everyone; when panels close popups on activation, the helper
  could regress to a `set_visible` flip (ADR-0039 mechanism) to shed
  the respawn cost.
- Supersedes the *mechanism* of ADR-0039 (one in-process
  registration, `set_visible` flips); its timing analysis and
  ADR-0036/0037 semantics carry over intact. The daemon drops its
  GTK/GLib dependency path entirely — registration, watcher, and
  pumping all live in the helper now.
