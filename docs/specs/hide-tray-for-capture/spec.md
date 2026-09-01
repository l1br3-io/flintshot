---
status: draft
started: 2026-09-01
---

# Spec — Hide the tray during daemon-dispatched captures

> Feature/fix effort. Stands on the Phase 2 daemon + tray
> (ADR-0019, ADR-0031) and revisits nothing — it closes a UX gap the
> selection-first effort surfaced live (L5, 2026-08-31): every
> daemon-triggered capture freezes the panel **with flintshot's own
> tray icon in the shot**.

## Objective

When a capture is grabbed (tray click, hotkey, IPC delegation, COSMIC
applet, desktop action — every path that ends in a `flintshot`
worker grabbing pixels), the daemon's tray icon must not be in the
frozen frame. It hides before the grab and returns right after it —
not for the editor's whole lifetime (the user may want the tray while
annotating).

**Success looks like:** click *Capture region* in the tray → the icon
vanishes at the click, the capture comes up without it, the icon is
back by the time the editor is interactable. Same from a hotkey or
the applet. A worker crash mid-grab still restores the icon (bounded).

## Mechanism (the decision)

**A worker→daemon handshake, not dispatch-site hiding.** The worker
is the only process that knows when the grab actually happens, so it
drives the window:

1. Worker (pre-grab): `Request::HideTrayForCapture` — request/reply
   (mirrors `BindShortcuts`' one-frame reply shape). The daemon hides
   the icon (SNI `Passive` via `tray_icon::TrayIcon::set_visible(false)`
   — the same mechanism its `Drop` uses), pumps GLib on Linux
   (ADR-0031), and replies `Event::TrayHidden` only after a
   **propagation grace** has elapsed (the panel removes the item
   asynchronously; an ack that doesn't mean "pixels gone" is useless).
2. Worker (post-grab): `Request::ReportCaptured` — fire-and-forget
   (mirrors `ReportSaved`). Sent whether the capture succeeded or
   failed (a RAII guard), so the tray returns the moment live pixels
   stop mattering.

Why not hide at the dispatch site only: the IPC server runs on the
tokio side thread and must not touch GTK; and no dispatch site knows
when its worker actually grabs. The handshake works for every trigger
source uniformly, including future ones.

Supporting decisions:

- **Depth counter** for concurrent captures; the icon returns when
  the last in-flight capture reports.
- **Pre-hide at click** on main-thread sources (tray menu drain,
  hotkey drain): the side effect only, no depth — the worker's
  handshake owns the depth. Kills one poll-interval of latency and
  gives instant visual feedback at click.
- **Safety restore**: if the icon stays hidden past a bound (killed
  worker, lost report), the daemon's loop force-restores and resets.
- **No tray / no-tray-feature daemon**: the handshake still replies
  promptly (hide is vacuous; no grace needed) — protocol identical.
- **Standalone worker, no daemon**: hide request finds no socket →
  skip silently; zero cost.
- Hide/restore is `set_visible` only — no unregister/rebuild, no new
  watcher churn.

## Scope (in / out)

**In:** core IPC schema (2 `Request` variants, 1 reply-only `Event`
variant, appended for postcard wire stability); daemon-side
`TrayGate` (thread-safe shell) + pure `TrayVisibility` state machine
(depth, grace, confirm epoch, safety) stepped by the main loop;
IPC-server interception; worker-side bounded-wait helper + RAII
restore guard wired into `capture_and_dispatch`; ADR-0036, CHANGELOG,
TEST_MATRIX rows.

**Out:** hiding other panels/apps (only our icon); config knob for
grace/safety (consts, tunable later); `--delay-ms` interaction (the
delay still runs pre-handshake; users hiding things themselves keep
that workflow); any editor-shell change.

## Deliverables

1. `flintshot-core::ipc`: `HideTrayForCapture`, `ReportCaptured`,
   `Event::TrayHidden` + round-trip tests.
2. `flintshot-app::tray_gate`: pure state machine + tests.
3. Daemon loop integration (`run_event_loop` steps the gate; tray /
   hotkey drains pre-hide; IPC intercepts both requests; bounded
   confirm wait) + tests.
4. Worker wiring in `capture_and_dispatch` (guard spans exactly the
   grab) + client helpers + tests.
5. Docs (ADR-0036, CHANGELOG `[Unreleased]` Fixed, TEST_MATRIX rows,
   spec closeout).

## Exit criteria

1. Tray *Capture region/fullscreen*: icon absent from the frozen
   frame; icon returns once the editor is up.
2. Hotkey + applet + CLI-delegated captures: same.
3. Esc-cancel / failed capture: icon still returns (guard reports on
   error paths).
4. Kill -9 a worker mid-grab: icon returns within the safety bound.
5. Daemon without tray feature: handshake completes, capture works.
6. Standalone (`--standalone`, no daemon): unchanged behaviour, no
   new latency.
7. Standing gates green (`fmt`, `clippy -D warnings`, workspace
   tests, doc tests).

## Open questions

1. Should the grace adapt per platform (native trays repaint faster
   than SNI round trips)? One const for now; revisit if a platform
   ghosts.
2. Should `--delay-ms` hide for the whole delay? Deferred — current
   semantics ("let the user clear the screen") are a deliberate
   user-controlled window.
