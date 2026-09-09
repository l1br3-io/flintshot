# Plan: Smart window hiding — capture self-exclusion + single-session replace

## Overview

The daemon learns the concept of an **active capture session**: a
registry of live capture/pin workers (process-exit truth, kill
handles), a **Replace** policy that supersedes any in-flight session
before spawning a new one (graceful close → bounded wait → hard
kill), and a generalized pre-grab handshake (`PrepareCapture` /
`CaptureReady`) that clears every flintshot surface — tray, app
window, pins — from the screen before any grab. `EditorOpened`
becomes a real window-open report instead of a spawn proxy.

Spec: `docs/specs/smart-window-hiding/spec.md`.

## Architecture decisions

- **Orchestration, not filtering** — close-then-reopen via the
  daemon; no xcap window filtering (impossible under Wayland portal
  captures). Extends the proven ADR-0036/0045/0046/0047 patterns.
- **Registry mirrors TrayGate's design** — pure state machine +
  thread-safe shell, injected clock, named grace consts, reaper as
  the crash-safe truth source (`sessions.rs`).
- **Graceful-before-kill** — superseded workers close via an IPC
  subscription + thread-safe egui `CloseHandle` (Context clone);
  hard kill is the bounded fallback. Prevents partial-file writes
  and SIGKILL data loss in the common case.
- **Daemon-first protocol growth** — new IPC variants are appended
  (postcard variant-index rule); the daemon intercepts and answers
  them before any client sends them, so intermediate tasks leave a
  working system.
- **Supersede lives at the entry points, not in `dispatch`** —
  capture triggers converge on a `dispatch_capture` helper
  (supersede → spawn) used by IPC server (async, `spawn_blocking`
  wait) and the sync daemon-loop sites (hotkey, portal). Keeps
  `dispatch` decision-only and testable.
- **`ReportAppHidden` means "window destroyed"** — the daemon adds
  a small compositor unmap grace after the ack before replying
  `CaptureReady` (the ADR-0047 lesson: close ≠ pixels gone).

## Task index

### Phase 1 — Foundation (Replace policy, kill-path first)
- [ ] Task 001: IPC schema variants → `tasks/001-ipc-schema.md`
- [ ] Task 002: Capture session registry (`sessions.rs`) → `tasks/002-session-registry.md`
- [ ] Task 003: Supersede-before-spawn at dispatch entry points → `tasks/003-supersede-dispatch.md`

### Checkpoint A — Replace works (kill path)
- [ ] `cargo test --workspace --all-targets` + clippy/fmt/doc gates green
- [ ] Manual: editor open + tray capture → old editor killed, one new editor

### Phase 2 — Graceful close + surface exclusion
- [ ] Task 004: Worker subscription + graceful editor/overlay close → `tasks/004-graceful-worker-close.md`
- [ ] Task 005: `PrepareCapture` daemon coordination (app-ack, pins, tray) → `tasks/005-prepare-capture-daemon.md`
- [ ] Task 006: App window hides for any capture + acks → `tasks/006-app-window-hide-ack.md`
- [ ] Task 007: Worker `CaptureGuard` (PrepareCapture → grab → ReportCaptured) → `tasks/007-worker-capture-guard.md`

### Checkpoint B — Both user scenarios pass
- [ ] Gates green
- [ ] Manual: tray capture while editor open → no flintshot chrome in shot, one editor
- [ ] Manual: app window open + hotkey capture → app window absent, returns after

### Phase 3 — Polish
- [ ] Task 008: Real `EditorOpened` (worker-reported) → `tasks/008-real-editor-opened.md`
- [ ] Task 009: Docs — ADR-0048, CHANGELOG, TEST_MATRIX → `tasks/009-docs.md`

### Checkpoint C — Done
- [ ] Spec exit criteria 1–8 verified; ready for review/merge

## Risks and mitigations

| Risk                                            | Impact | Mitigation                                                                   |
| :---------------------------------------------- | :----- | :--------------------------------------------------------------------------- |
| Supersede wait blocks the tokio IPC thread      | High   | Wait runs via `spawn_blocking`; sync loop sites call the blocking version    |
| Graceful close never arrives (hung worker)      | High   | Bounded supersede grace (~1.5 s) → hard kill via stored handle → reaper truth |
| Cross-thread egui close unreliable on Wayland   | Med    | `Context` clone + `send_viewport_cmd` is the documented thread-safe path; kill fallback covers the rest |
| Compositor unmap grace too short (window ghosts)| Med    | Ack + fixed post-ack grace; consts; TEST_MATRIX manual rows per compositor   |
| Wire skew during rolling upgrade                | Low    | Variants append-only; `HideTrayForCapture` still answered for old workers    |
| Reaper can't kill a `Child` it is `wait`ing on  | Med    | Reaper switches to `try_wait` polling so the kill handle stays reachable     |

## Open questions

- None blocking; spec open questions 1–3 (durable pins, orphaned
  pin registry, silent-replace UX) are documented limitations for
  follow-ups.
