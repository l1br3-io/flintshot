# Todo — Phase 3: Pin-to-desktop + COSMIC Applet

Flat status checklist. One line per task, mirroring
[`plan.md`](plan.md)'s task index. Per-task detail in
[`tasks/00N-*.md`](tasks/). This is the single status source the
project resumes from.

## Phase 3.A — Pin foundation
- [ ] Task 1 — PinConfig (`[pin]` section) + `flintshot pin` CLI scaffolding → [`tasks/001-pin-config-and-cli.md`](tasks/001-pin-config-and-cli.md)

**Checkpoint A** — `flintshot pin <path>` loads an image, reaches `run_pin`; `[pin]` config round-trips. Plumbing proven.

- [ ] Task 2 — Pin window end-to-end (`run_pin` real + `PinApp`, all 5 UX features) → [`tasks/002-pin-window.md`](tasks/002-pin-window.md)

**Checkpoint B** — `flintshot pin <path>` floats a borderless always-on-top window; drag/close/hover-opacity/resize/copy-on-close work.

## Phase 3.B — Daemon last-saved + PinLast/OpenLast
- [ ] Task 3 — Worker result reporting (`Request::ReportSaved`) + daemon last-saved state → [`tasks/003-worker-report-saved.md`](tasks/003-worker-report-saved.md)
- [ ] Task 4 — Daemon `PinLast` / `OpenLast` dispatch (spawn worker, emit `Event::Pinned`) → [`tasks/004-daemon-pin-last-open-last.md`](tasks/004-daemon-pin-last-open-last.md)

**Checkpoint C** — daemon responds to `PinLast`/`OpenLast` over IPC; `Event::Pinned { id }` broadcasts with monotonic ids.

## Phase 3.C — Editor Pin button
- [ ] Task 5 — Editor toolbar Pin button (compose → tempfile → spawn pin → close) → [`tasks/005-editor-pin-button.md`](tasks/005-editor-pin-button.md)

**Checkpoint D** — pin works from all three entry points (CLI, daemon, editor). Pin-to-desktop feature-complete and independently shippable.

## Phase 3.D — COSMIC applet
- [ ] Task 6 — COSMIC IPC client (framing + Request + Subscribe + daemon-absent) → [`tasks/006-cosmic-ipc-client.md`](tasks/006-cosmic-ipc-client.md)
- [ ] Task 7 — COSMIC applet shell + popup menu (actions, monitor submenu, config/quit) → [`tasks/007-cosmic-applet-popup.md`](tasks/007-cosmic-applet-popup.md)
- [ ] Task 8 — COSMIC event stream + daemon-absent state (Subscribe display, status line) → [`tasks/008-cosmic-event-stream.md`](tasks/008-cosmic-event-stream.md)

**Checkpoint E** — COSMIC applet works end-to-end; popup shows live status and degrades gracefully when the daemon is absent.

## Phase 3.E — Hardening
- [ ] Task 9 — Boundary test extension (reject cosmic/libcosmic/iced/slint) + CHANGELOG + ADR-0023 + TEST_MATRIX → [`tasks/009-hardening.md`](tasks/009-hardening.md)

**Checkpoint F** — Phase 3 exit criteria met; ready for review.
