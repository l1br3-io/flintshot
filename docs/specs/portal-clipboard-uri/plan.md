# Plan: portal `clipboard:///` grab is read back, not rejected

## Overview

xdg-desktop-portal-cosmic's Copy-confirmed screenshot answers
`clipboard:///` (image on the system clipboard, no file). The grab
must be read back so every post-grab action (editor, clipboard,
upload, `--out`) proceeds. Effort history: Task 001 marker +
read-back (failed manual), Task 003 change-detect wait (still
failed manual). Root cause is now proven: **arboard 3.6.1 has no
native Wayland support unless the `wayland-data-control` feature
is enabled** — without it, every arboard call rides XWayland's
clipboard bridge, whose Wayland→X11 direction is lazy/focus-gated,
so reads fail nondeterministically (the portal's write itself
lands within ~1 s; `wl-paste` sees it fine).

## Architecture Decisions

- **Marker through the existing error channel** (Task 001):
  `PORTAL_GRAB_ON_CLIPBOARD` const in core, classified in the app
  layer — no trait-shape change.
- **Read-back waits for a changed image, not mere presence**
  (Task 003): pre-grab digest snapshot + ~5 s change-detect wait;
  byte-identical-at-deadline accepted. Stays — correct regardless
  of the arboard fix.
- **Enable `wayland-data-control` on the workspace arboard entry**
  (Task 004): one line; wl-clipboard-rs gives arboard native
  `zwlr_data_control_v1` (the same path `wl-paste` uses, proven
  working against the portal's selection on this COSMIC session).
  X11/other-OS behavior unchanged (feature gates the wayland
  backend; arboard picks per-session).
- Evidence trail: daemon.log timelines + the instrumented repro
  (Enter-injected portal capture; `wl-paste` vs arboard racing the
  same selection) recorded in
  `tasks/004-enable-arboard-wayland-data-control.md`.

## Task Index

### Phase 1: shipped (history)
- [x] Task 1: Portal marks `clipboard:///`; app reads the grab back → `tasks/001-marker-and-read-back.md`
- [x] Task 2: CHANGELOG entry + spec reconcile → `tasks/002-changelog.md`
- [x] Task 3: Clipboard wait outlasts the Response race — change-detect + 5 s budget → `tasks/003-clipboard-wait.md`

### Phase 2: reopen fix
- [x] Task 4: Enable arboard `wayland-data-control` → `tasks/004-enable-arboard-wayland-data-control.md`

### Checkpoint: Complete
- [x] Full finish gate passes (fmt, clippy, workspace tests, doc tests)
- [x] Manual COSMIC check (injected-Enter repro): capture → Copy →
      editor session confirmed live (35-thread editor process);
      human tray-path confirmation pending
- [ ] Review with human before merging

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| New transitive deps (wl-clipboard-rs, wayland-client) break a CI job | Med | Task 4 verifies all CI-shaped builds incl. `--features tray` and `FEATURES=''`; wayland-client's rust backend needs no system libs |
| Native-wayland **writes** (editor Copy, capture clipboard arm) now take the data-control path — selection lifetime after process exit depends on the compositor's clipboard manager | Med | Manual check: Copy in editor → close editor → paste elsewhere; keep the existing linger logic, revisit only if the check fails |
| `wayland-data-control` feature renamed/changed in future arboard | Low | Pinned `version = "3"`; feature verified present in 3.6.1 (registry Cargo.toml) |

## Open Questions

- None blocking. (Editor-egui's Linux-wide 60 s clipboard linger
  predates native-wayland arboard; revisit in a later effort if
  the manual Copy check surfaces anything.)
