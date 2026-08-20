# ADR-0029: Global hotkeys on Wayland via the xdg-desktop-portal (`ashpd`)

**Context.** `global-hotkey` has no Wayland backend: Wayland has no
portable global-grab API, and the pre-Phase 6 daemon simply logged a
"bind a compositor keybinding to `flintshot region`" workaround and
left `[hotkeys] unused. The trap that makes *trying* X11 grabs wrong:
under XWayland, `GlobalHotKeyManager` registration can **succeed**
while the compositor never delivers events — silent dead hotkeys, the
worst failure mode a hotkey system can have.

Alternatives considered:

- **evdev / uinput key grabbing** — reads all keyboard input from
  below the compositor. Needs udev/input-group permissions (or a
  privileged helper), bypasses compositor keymaps and grabs, races
  with the compositor's own shortcuts, and is a security-sensitive
  attack surface. Rejected.
- **Compositor-specific shortcut protocols** (wlr-global-shortcuts,
  COSMIC's shortcut protocol, …) — none is standardized across
  compositors; supporting two desktops means two integrations and a
  third for the third desktop. Rejected as the *primary* path.
- **Compositor keybindings (status quo)** — works everywhere, zero
  code; but per-compositor setup, chords live outside flintshot's
  config, and there is no in-app re-bind flow. Kept as the *documented
  fallback*, not the feature.
- **xdg-desktop-portal `GlobalShortcuts`** (portal 1.16, 2023) — the
  cross-desktop standard. KDE Plasma and COSMIC implement the
  frontend; GNOME and most wlroots compositors do not (as of this
  writing). Chosen.

**Decision.** Portal-first on Wayland, via `ashpd`.

- `ashpd = 0.13` with `default-features = false, features =
  ["tokio", "global_shortcuts"]` — ashpd's defaults pull async-io;
  pinning keeps the single tokio runtime (ADR-0018's side thread)
  authoritative. `zbus` arrives transitively.
- **Selection is deterministic, not try-X11-then-fallback:** session
  detection (`WAYLAND_DISPLAY` / `XDG_SESSION_TYPE`) picks the portal
  path on Wayland sessions; `global-hotkey` stays byte-identical on
  X11/macOS/Windows. The XWayland silent-grab trap rules out probing.
- **Silent attach + user-initiated bind.** Startup calls
  `ListShortcuts` (never dialogs) and subscribes to `Activated`; the
  bind dialog opens only when the user asks — `flintshot hotkeys
  bind` over IPC, or the tray's *Bind shortcuts…* item. Binding is
  IPC-mediated because bindings belong to the session that made them:
  a standalone CLI bind would create bindings the daemon never
  receives, so without a daemon the CLI refuses with "start
  `flintshot daemon` first".
- **Suggestions, not authority.** `[hotkeys]` chords are converted
  (pure fn) into portal `preferred_trigger` hints; runtime dispatch
  keys off whatever the portal reports bound. The user may bind
  different keys in the dialog — that is correct by design.
- Activations dispatch through the same `worker::dispatch` table as
  tray clicks and IPC requests (one dispatch path);
  `ShortcutsChanged` is consumed so re-binds made in the compositor's
  own settings UI keep the logged binding set honest.
- **Fallback is classified and never fatal:** no session bus / portal
  frontend missing / interface too old each produce a distinct
  notice naming the compositor-keybind workaround; the daemon (tray +
  IPC) must start without a portal, full stop.

**Consequences.**

- Runtime requirement: xdg-desktop-portal ≥ 1.16 **and** a frontend
  implementing GlobalShortcuts. "COSMIC ships it" is upstream-true
  but package-dependent — dogfooding (Phase 6, L5) found
  `xdg-desktop-portal-cosmic 0.1.0pop1` (Ubuntu 24.04-based) does not
  list the interface, and the daemon correctly fell back. Check
  `/usr/share/xdg-desktop-portal/portals/*.portal` before debugging
  flintshot on a "should work" desktop.
- GNOME and wlroots users get the honest fallback — no worse than
  pre-Phase 6, and bridging that gap (layer of compositor protocols)
  is explicitly out of scope.
- **App-id nuance for a windowless daemon:** the portal resolves the
  caller's identity from bus credentials and matches installed
  `.desktop` files, so the bind dialog may show a generic or binary
  name. Worst case cosmetic; installing a `flintshot.desktop` whose
  basename matches the binary mitigates.
- Bind-dialog UX (look, dismissal → `bind-cancelled`, persistence) is
  the portal's job; flintshot re-attaches at start instead of
  persisting bindings itself.
- Config hot-reload is deliberately asymmetric: on Wayland a
  `[hotkeys]` edit logs a suggestions-changed notice (re-binding
  needs the user dialog) while X11 keeps `HotkeyRegistry::refresh`
  re-registration. Documented at the branch in `daemon.rs` so it
  doesn't read like a bug.
- Chord conversion carries a key-name table gotcha
  (`global-hotkey`/keyboard-types `PrintScreen` vs GDK `Print`,
  `Ctrl` vs `<Control>`); unknown tokens degrade to "no suggestion"
  so a config typo can never break binding.
- **No timeout on the CLI's bind-reply wait.** Every daemon-side
  terminal path writes exactly one reply (success, cancel, failure,
  or `gone` when the session task died — a mid-session portal crash
  is logged and recorded on the handle, not left vague), and a dead
  daemon closes the socket, which errors the read. The only
  indefinite wait is the user's own open dialog, where a timeout
  would be *wrong* (minutes is a legitimate deliberation time);
  Ctrl-C is safe and leaves the daemon healthy.
