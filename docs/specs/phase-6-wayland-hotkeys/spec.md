---
status: draft
started: 2026-08-20
---

# Spec — Phase 6: Wayland global hotkeys via `ashpd`

> Draft effort spec. For product context, see
> [`../../VISION.md`](../../VISION.md); for sequencing, see
> [`../../ROADMAP.md`](../../ROADMAP.md); for the technical substrate
> this builds on, see [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md)
> (§Tech stack — "Wayland hotkeys (P6)", §Cross-platform concerns,
> §Known risks & mitigations). The shipped phases this effort stands
> on: [Phase 2](../phase-2-daemon-tray-hotkeys/spec.md) (daemon,
> `HotkeyRegistry`, worker dispatch, config hot-reload),
> [Phase 3b](../phase-3-pin-cosmic/spec.md) (COSMIC applet context).

## Objective

Give Wayland users **real global hotkeys** — no compositor keybinding
setup — by wiring the daemon to the xdg-desktop-portal
**GlobalShortcuts** interface via `ashpd`. Today, `global-hotkey` has
no Wayland backend; the daemon logs a "bind a compositor keybinding to
`flintshot region`" workaround and the `[hotkeys]` section is unused
(`crates/flintshot-app/src/hotkeys.rs`). After this effort, a COSMIC
or KDE Plasma user starts the daemon, binds once through the portal
dialog, and their capture keys just work — same dispatch path, same
worker, same IPC broadcast as on X11.

**Key stances (decided at spec time):**

1. **Wayland-only addition.** The `global-hotkey` path on
   X11/macOS/Windows is untouched — byte-identical behavior, same
   tests. The portal path activates only where `global-hotkey`
   cannot (Wayland session detected).
2. **Auto-listen + manual re-bind.** At daemon start the portal
   session attaches silently to any *already-bound* shortcuts (no
   dialog). Opening the bind dialog is user-initiated: a tray
   "Bind shortcuts…" item and a `flintshot hotkeys bind` subcommand.
   The daemon never spams the dialog on every start.
3. **Config chords are suggestions, not authority.** `[hotkeys]`
   chord strings are passed to the portal as *suggested* triggers
   for `region` / `fullscreen`. Runtime dispatch keys off whatever
   the portal reports as bound — the user may bind different keys in
   the dialog, and that is correct.
4. **Graceful fallback, never fatal.** No session bus, portal
   missing, xdg-desktop-portal < 1.16, or a compositor that does not
   implement GlobalShortcuts (GNOME, wlroots) → the daemon keeps the
   current compositor-keybind notice (promoted from `warn`-ish
   wording to a clear, actionable `tracing::info!`) and continues.
   Tray + IPC are unaffected. The daemon must start without a
   portal, full stop.

**Success looks like:**

- On COSMIC or KDE Plasma with xdg-desktop-portal ≥ 1.16: `flintshot
  daemon` starts clean; `flintshot hotkeys bind` (or the tray item)
  opens the portal dialog offering the configured chords; after
  binding, pressing the key fires the region capture through the
  same worker dispatch as an X11 hotkey — latency indistinguishable
  from the compositor-keybind workaround.
- On sway/GNOME (no GlobalShortcuts): identical-to-today behavior —
  notice in the log, tray and IPC fully functional, daemon exit code
  unaffected.
- On X11: nothing changed at all.

## Scope (in / out)

**In:**

- `ashpd` dependency (workspace-sanctioned in ARCHITECTURE.md §Tech)
  with its async/tokio plumbing; GlobalShortcuts session lifecycle
  owned by a dedicated module in `flintshot-app` (successor to
  today's Wayland no-op branch of `HotkeyRegistry`).
- Platform selection: Wayland detection picks portal vs
  `global-hotkey`; explicit override not required (detection, not
  config).
- Startup attach: create the portal session, list bound shortcuts,
  subscribe to activation signals — silently, dialog-free.
- Bind flow: `flintshot hotkeys bind` subcommand (CLI, `Command`
  enum grows one variant) + tray "Bind shortcuts…" item; both call
  `BindShortcuts` with the config chords as suggested triggers and
  the user's actual choices logged at `info`.
- `ShortcutsChanged` signal handling: when the portal reports
  bindings changed (user re-bound from system settings), refresh the
  id → request map and log the new set at `info`.
- Event bridge: portal activation signals → the existing daemon main
  loop → worker dispatch table (same path as tray clicks and
  `global-hotkey` events). A channel bridge, mirroring the
  tray/hotkey receiver-drain pattern in `daemon.rs`.
- Config hot-reload semantics on Wayland: a `[hotkeys]` change
  cannot silently re-register (re-binding needs the user dialog);
  log an actionable notice pointing at `flintshot hotkeys bind`.
  `HotkeyRegistry::refresh` semantics stay X11-only.
- Fallback detection + notice (portal absent, version too old,
  interface unimplemented) — distinct log messages per cause so
  users can self-diagnose.
- Docs: ADR-0029 (portal-over-evdev/other alternatives), CHANGELOG,
  TEST_MATRIX Phase 6 rows promoted from "Not yet implemented".

**Out:**

- Any change to X11/macOS/Windows hotkey behavior, `[hotkeys]` serde
  format, or default chords.
- Per-portal persistence management (binding persistence is the
  portal's job; we re-attach at start).
- Media-key / non-capture shortcuts — only `region` and `fullscreen`.
- COSMIC applet changes (it talks IPC; nothing portal-related).
- GNOME/wlroots support beyond the honest fallback notice.

## Deliverables

1. `flintshot-app::hotkeys` (or a sibling `hotkeys_portal` module):
   the portal session manager — platform selection, silent attach,
   bind invocation, signal subscription, fallback classification.
2. `flintshot-app::cli`: `HotkeysArgs` + `flintshot hotkeys bind`
   — talks to the running daemon over IPC so binding happens in the
   daemon's portal session. With no daemon running it errors clearly
   ("start `flintshot daemon` first") rather than binding standalone:
   portal bindings belong to the session that bound them, so a
   standalone bind is compositor-dependent and out of contract.
3. `flintshot-app::daemon` + tray: the event bridge into the main
   loop, the "Bind shortcuts…" menu item, reload-notice semantics.
4. ADR-0029, CHANGELOG `[Unreleased]`, TEST_MATRIX Phase 6 rows
   filled (COSMIC + KDE verified; fallback verified on sway or
   mocked-old-portal).

## Exit criteria

1. COSMIC (dogfood): fresh daemon start is silent; `hotkeys bind`
   dialog shows suggested chords; bound key fires `region` capture;
   restart re-attaches without a dialog; second `bind` run replaces
   bindings cleanly.
2. KDE Plasma (dogfood): same flows as COSMIC.
3. sway or portal-disabled: daemon logs the fallback notice naming
   the compositor-keybind workaround; tray + IPC still work; daemon
   exit code 0 on Ctrl-C as today.
4. X11: `HotkeyRegistry` unit tests and manual flows unchanged
   (regression gate: existing tests pass without modification).
5. `[hotkeys]` edit on Wayland mid-daemon: notice logged, no crash,
   no dialog, previous bindings keep firing.
6. All standing gates green: `fmt`, `clippy --workspace --all-targets
   -D warnings`, workspace tests, doc tests,
   `no_gui_types_in_core.rs`.

## Technologies

Workspace stack plus **one new dependency**: `ashpd` (latest stable,
`tokio`/async features as required by its GlobalShortcuts API; `zbus`
arrives transitively). Tokio is already in the tree (upload, IPC).
No other additions. xdg-desktop-portal ≥ 1.16 required at runtime
for the portal path — detected, not assumed.

## Commands

Standing gate (AGENTS.md) plus the phase-specific entry points:

```sh
cargo run -- daemon                 # portal attach on Wayland
cargo run -- hotkeys bind           # open the bind dialog
just run-shortcut region            # existing workaround path (fallback)
```

## Boundaries

- `flintshot-core` gains **nothing portal-shaped**: no `ashpd`, no
  `zbus`, no session-bus types (the `no_gui_types_in_core.rs` spirit;
  extend that test's deny-list with `ashpd` if practical). Pure
  helpers (e.g., chord → portal accelerator strings) may live in
  core *only* if they carry no system deps — otherwise they stay in
  `flintshot-app`.
- Always: portal failures degrade to log-and-continue; every log
  line names the cause and the workaround; dispatch goes through the
  existing worker table (no second dispatch path).
- Ask first: any dependency beyond `ashpd`; touching `[hotkeys]`
  serde/format; changing daemon startup ordering.
- Never: make the portal a hard daemon dependency; spawn the bind
  dialog without user action; regress the X11 path; silence lints
  with `#[allow]` without a justifying comment.

## Success criteria

- Bind → press → capture latency on Wayland/portal is within
  hand-measurement noise of the compositor-keybind workaround
  (< ~50 ms added).
- Zero X11 regressions (full TEST_MATRIX re-run on the X11 rows).
- One code path from "shortcut activated" to "capture started" for
  tray, X11 hotkeys, and portal hotkeys alike.

## Decisions

- **Portal via `ashpd`** over evdev/keygrab hacks — ADR-0029 will
  record context/decision/consequences (incl. the GNOME/wlroots
  gap and the ≥ 1.16 requirement).
- **Suggestions-not-authority config** — `[hotkeys]` advises the
  dialog; the portal's bound state is the runtime truth.
- **Auto-listen + manual bind** — no startup dialog spam.
- **`ShortcutsChanged` handled** — refreshes the id map when the
  portal reports the user re-bound from system settings.
