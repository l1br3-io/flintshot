# Plan: Worker editor — Home link navigates instead of closing

## Overview

Make the topbar breadcrumb's Home link honor its promise on worker
editor windows (the tray-capture fallback path): clicking it
delegates the daemon's `OpenApp` choreography (focus a running app
window / spawn `flintshot app`; no daemon → direct spawn), then
leaves the worker editor via the existing Esc contract. Mirrors the
proven `open_settings_from_worker` seam end to end. Spec:
`docs/specs/fix-worker-home-link/spec.md`.

## Architecture Decisions

- **Mirror, don't generalize.** A third hook (`with_open_home` /
  `set_open_home`) shape-mates `with_open_settings` rather than
  refactoring both into a generic "delegate" hook — the Settings
  twin stays untouched (regression risk ~0) and the diff reads as
  an additive change.
- **Delegate-then-close ordering.** The `NavigateHome` arm fires
  the hook *before* publishing `Cancelled` + `ViewportCommand::Close`:
  the app window's focus/spawn must be requested while the worker
  is still alive to ask, and the close is the worker's exit either
  way.
- **`Request::OpenApp`, not a new request.** The daemon's existing
  broadcast-first dispatch (`AppFocus` + spawn `app`) already
  implements focus-or-spawn; a running window focuses on its
  *current* route (no force-route to Home — matches spec acceptance
  criterion 2). No daemon-side changes.
- **No-daemon spawn mirrors `spawn_settings_app`** (detached
  `flintshot app`, no `--standalone`): the spawned app acquires the
  lock and auto-starts its own daemon via the pump, exactly like
  the Settings fallback.
- **Headless/no-hook fallback keeps today's behavior.** Hook absent
  (tests, any non-worker future caller) → publish `Cancelled` +
  close. Never a panic, never a dead click.

## Task Index

### Phase 1: The hook path (editor-egui)

- [x] Task 1: `open_home` hook seam + `NavigateHome` handler fires it → `tasks/001-open-home-hook.md`

### Phase 2: The real delegate (flintshot-app)

- [x] Task 2: Worker-side `open_home_from_worker` delegate + shell wiring → `tasks/002-worker-delegate.md`

### Checkpoint: Code complete

- [ ] `cargo fmt --all --check` clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo test --workspace --all-targets` green
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui` green
- [ ] Manual matrix (if a session is available): tray capture → Home link → app window on Home, worker editor closed; running-app case focuses it; no-daemon case spawns
- [ ] Review with human before docs

### Phase 3: Record

- [ ] Task 3: CHANGELOG `[Unreleased]` + ADR-0065 (worker-navigation stance) → `tasks/003-changelog-adr.md`

### Checkpoint: Complete

- [ ] All spec acceptance criteria met
- [ ] Ready for review / PR

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| `try_delegate` blocks the UI thread in the click handler (IPC round-trip) | Med | Precedented: `open_settings_from_worker` runs the identical call on the identical thread; accepted stance, revisit only if latency shows |
| Two flintshot windows alive briefly (worker editor closing + app window opening) | Low | By design (transient handoff); the worker closes immediately after the delegate returns |
| Router behavior change regressions | Low | App-window path untouched; `navigate_home_from_the_editor_routes_home` (window.rs) and the no-hook worker test stay as regression pins |
| Clippy pedantic on new hook code | Low | CI gate in every task's verify step |

## Open Questions

None — behavior and no-daemon fallback confirmed by the user
(2026-09-20, recorded in spec).
