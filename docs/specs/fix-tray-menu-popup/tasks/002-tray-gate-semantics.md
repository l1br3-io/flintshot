# Task 002 — Tray-gate semantics retune (docs only)

**Status:** done (shipped in commit `1b12801`; backfilled)

## What

- `TrayAction`'s doc comment now states the real semantics: on
  Linux `Hide` unregisters the SNI item (dropping the `TrayIcon`,
  which also closes the panel's menu popup, ADR-0038) and
  `Show`/`ForceShow` re-register it; macOS/Windows remain
  `set_visible` flips plus the GLib pump (ADR-0031).
- `MENU_GRACE`'s rationale updated: the popup's removal is causal
  on Linux, but its completion is still unobservable (no API
  queries another process's popup state), so the ack keeps waiting
  the 600 ms blind, anchored at the click.

## Why

The gate's *decisions* are untouched — only the daemon's *how*
changed in task 001 — but the comments still described the old
status-flip mechanism, which would mislead the next reader into
"fixing" the timing.

## Verification

Docs-only; standing gates green.
