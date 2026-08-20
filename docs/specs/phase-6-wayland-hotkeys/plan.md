# Plan: Phase 6 — Wayland global hotkeys via `ashpd`

## Overview

Wire the daemon to the xdg-desktop-portal **GlobalShortcuts**
interface (`ashpd` 0.13, `global_shortcuts` feature, tokio runtime)
so Wayland users get real global hotkeys. Portal-first on Wayland:
`session_is_wayland()` (already in `platform.rs`) selects the portal
path; `global-hotkey` stays byte-identical for X11/macOS/Windows.
The portal manager lives on the daemon's existing tokio side thread
(ADR-0018 bridge); activations dispatch through the same
`worker::dispatch` table the IPC server already uses from that
thread. Binding is user-initiated: `flintshot hotkeys bind` (IPC →
daemon session) or a tray item, with `[hotkeys]` chords passed as
*suggested* bindings. No portal → today's fallback notice, daemon
unaffected.

## Architecture Decisions

- **Portal-first on Wayland, not try-X11-then-fallback.** Under
  XWayland, `GlobalHotKeyManager` registration can *succeed* while
  the compositor never delivers events (silent dead hotkeys).
  `session_is_wayland()` → portal path, deterministically.
- **Portal manager on the tokio side** (`run_tokio_side`), not the
  main loop. zbus is async; the tokio side already dispatches
  `Request`s via `worker::dispatch` (same path as IPC), so
  activations need **no main-loop bridge** — only the bind trigger
  crosses threads (`tokio::sync::Notify`, sync-callable from the
  main loop's tray handler and from `handle_connection`'s
  `Subscribe`-style interception in ipc_server.rs:341).
- **Shortcut ids are stable strings**: `"region"` / `"fullscreen"`
  match `Activated::shortcut_id` → `Request::CaptureRegion{..}` /
  `CaptureFullscreen{..}`. Same mapping used by
  `HotkeyRegistry` on X11.
- **Chord conversion is a pure fn** in `flintshot-app`
  (`global-hotkey` format `"Ctrl+Shift+PrintScreen"` → portal
  accelerator format), unit-tested; `flintshot-core` gains nothing
  portal-shaped (`no_gui_types_in_core.rs` deny-list grows
  `use ashpd`).
- **IPC contract first**: `Request::BindShortcuts` (+ defensive
  no-op arm in `worker::dispatch`, alongside `Subscribe`) lands
  before any portal code, so the CLI ↔ daemon contract is stable.

## Task Index

### Phase 1: Contract + session (high risk, early)
- [x] Task 1: IPC contract `Request::BindShortcuts` + `flintshot hotkeys bind` CLI → `tasks/001-ipc-bind-contract.md`
- [ ] Task 2: Portal session — silent attach, activation dispatch, Wayland selection, fallback classification → `tasks/002-portal-session.md`

### Checkpoint: First end-to-end fire (after Tasks 1–2)
- [ ] Standing gates green (`fmt`, `clippy -D warnings`, workspace tests, doc tests)
- [ ] On a Wayland compositor with pre-bound shortcuts: key press → capture (manual)
- [ ] On headless CI / no-portal: daemon starts clean with classified notice (log inspection)

### Phase 2: Bind flow + integration
- [ ] Task 3: `BindShortcuts` flow — portal dialog with config-suggested chords, CLI end-to-end → `tasks/003-bind-flow.md`
- [ ] Task 4: Tray "Bind shortcuts…" item + `ShortcutsChanged` refresh + Wayland config hot-reload notice → `tasks/004-tray-reload-signals.md`

### Checkpoint: Feature complete (after Tasks 3–4)
- [ ] Full bind → fire → re-bind cycle works on COSMIC or KDE (manual)
- [ ] `[hotkeys]` edit mid-daemon on Wayland: notice, no crash, old bindings still fire (manual)
- [ ] X11 regression pass: hotkeys + tray unchanged (manual/CI)

### Phase 3: Ship prep
- [ ] Task 5: Dogfood both compositors, TEST_MATRIX rows, ADR-0029, CHANGELOG → `tasks/005-dogfood-docs.md`

### Checkpoint: Complete
- [ ] Spec exit criteria 1–6 verified (spec.md §Exit criteria)
- [ ] Ready for `/review` → `/ship`

## Risks and Mitigations

| Risk                                                             | Impact | Mitigation                                                                                                   |
| ---------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------ |
| Portal app-id resolution for a windowless daemon (dialog shows wrong/no app name; bindings keyed oddly) | High   | Task 2 spike verifies early on COSMIC; desktop-file naming documented in ADR-0029; worst case is cosmetic      |
| Session bus absent / portal < 1.16 / interface unimplemented (GNOME, wlroots) | Med    | Fallback classification with distinct log messages per cause (Task 2); daemon never treats portal as required |
| `ashpd` default features pull async-std (runtime clash with tokio) | Med    | Pin `default-features = false, features = ["tokio", "global_shortcuts"]` in Task 2                            |
| Chord format mismatch (config `PrintScreen` vs portal `Print`)   | Low    | Pure converter fn + unit tests (Task 3); unknown tokens degrade to "no suggestion", binding still works        |
| Activation storms / stuck modifiers on some compositors          | Low    | Dispatch on `Activated` only, ignore `Deactivated` (mirrors Pressed-only rule in `drain_hotkeys`)              |

## Open Questions

- None blocking. (Spec-level questions resolved at `/spec`;
  plan-time corrections to the spec — `ShortcutsChanged` signal
  name, no-standalone-bind error — are already applied to
  `spec.md`.)
