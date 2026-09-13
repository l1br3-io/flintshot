# Plan: Native capture UX

## Overview

One Capture action on every surface (home window, tray, COSMIC
applet, CLI, hotkeys), with the OS's own selection UI doing the
region picking wherever the selected backend supports it: the portal
backend gains `interactive` mode; `xcap` keeps today's
monitor-grab → editor/overlay flow. Post-grab routing on the native
path is configurable (`[capture] after_grab`, default `editor`).
`flintshot capture` + `Request::Capture` replace `region` /
`fullscreen` end to end (clean break). Builds on the shipped
`native-capture-backends` foundation (`Capturer`, portal backend,
`select_backend`).

## Architecture Decisions

- **Capability-based flow, not session-based.** `Capturer` grows
  `supports_selection()` + `capture_selection()`; the worker picks
  the native flow when the *selected backend* has the capability.
  Consequence: pinning `--backend portal` on X11 also gets the
  native flow (xdg-desktop-portal runs there too) — a harmless
  generalization of the spec's "Wayland native path"; session kind
  still drives *auto* precedence, unchanged.
- **Additive first, purge second.** `Request::Capture` and the
  `capture` subcommand land additively alongside `region`/
  `fullscreen` (every commit works); a dedicated later task deletes
  the old subcommands, IPC variants, argv builders, and migrates the
  remaining senders mechanically. Avoids a broken transitional
  state where the daemon would spawn a deleted subcommand.
- **`after_grab` dispatch is a pure function** of (config value,
  CLI flag) → `editor | clipboard | upload`; unknown values warn and
  resolve `editor` (string-tolerant precedent). Clipboard via
  `arboard::ImageData`; upload reuses the existing upload path.
- **Cancellation is quiet.** The backend's `"cancelled"` error
  propagates as data; the worker exits 0 with one info log — no
  notification, no failure exit.
- **`--monitor` dies with the per-monitor surfaces.** Fallback flow
  grabs the primary monitor; selection belongs to the OS UI or the
  editor.

## Task Index

### Phase 1 — Verify + capability
- [ ] Task 1: Spike C — live-verify interactive portal (return shape, Escape, latency) → `tasks/001-spike-interactive.md`
- [ ] Task 2: `Capturer::capture_selection` + portal interactive impl → `tasks/002-capture-selection.md`

### Checkpoint: capability proven (HUMAN GATE — spike evidence reviewed before building on it)
- [ ] Full gate green; live interactive grab works via a scratch probe

### Phase 2 — Capture vertical
- [ ] Task 3: `flintshot capture` end-to-end (additive: subcommand + `Request::Capture` + `capture_argv` + dispatch + native/fallback flows + quiet cancel) → `tasks/003-capture-subcommand.md`
- [ ] Task 4: `after_grab` routing (config + `--after` + clipboard/upload dispatch) → `tasks/004-after-grab.md`

### Checkpoint: CLI vertical complete
- [ ] Human runs `flintshot capture` live on COSMIC: OS UI → editor; Escape quiet; `--backend xcap` fallback unchanged

### Phase 3 — Purge + surfaces
- [ ] Task 5: Purge `region`/`fullscreen` everywhere + migrate all senders → `tasks/005-purge-old-surfaces.md`
- [ ] Task 6: Home window — Capture / Open / hidden Paste → `tasks/006-home-window.md`
- [ ] Task 7: Tray — single Capture item, per-monitor submenu gone → `tasks/007-tray-menu.md`
- [ ] Task 8: COSMIC applet — single Capture action → `tasks/008-cosmic-applet.md`
- [ ] Task 9: Hotkeys — single `capture` binding, legacy warn, advice text → `tasks/009-hotkeys.md`

### Checkpoint: all surfaces collapsed
- [ ] Full manual matrix smoke on the live session; full gate green

### Phase 4 — Record
- [ ] Task 10: ADR + docs sweep + CHANGELOG breaking entries → `tasks/010-adr-docs.md`

### Checkpoint: complete
- [ ] Spec success criteria met; ready for PR

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Interactive portal returns a composite, not the selection (unverified) | High | Task 1 verifies live on COSMIC before anything builds on it; worst case the existing bbox-crop handles it |
| CLI surgery in `cli.rs` (2.4k lines, purge + flows) | Med | Additive landing (T3) separates new flows from the purge (T5); both scoped with pinned tests |
| Legacy `[hotkeys]` keys must *warn*, but serde ignores unknown fields silently | Med | Inspect the raw `[hotkeys]` table via `toml_edit` before deserializing (mechanism verified in T9; interaction-mode precedent) |
| GNOME/KDE/wlr interactive behavior unmeasured | Med | TEST_MATRIX rows; auto already falls to xcap on probe failure; `--backend` pin is the escape hatch |
| `flintshot-cosmic` builds only with opt-in system deps | Low | Its task verifies via `cargo build -p flintshot-cosmic` + clippy; CI's cosmic-build job covers it |

## Open Questions

- None blocking — Task 1 resolves the spec's open question #1
  (interactive return shape) with live evidence.
