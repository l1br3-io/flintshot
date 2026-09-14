# ADR-0051: One Capture action — the OS's selection UI, post-grab routing, clean break

**Context.** flintshot's capture UX had forked per mode and per
surface: `region` vs `fullscreen` subcommands (plus `--monitor`
plumbing), `Request::CaptureRegion`/`CaptureFullscreen` on the wire,
two hotkey chords, three tray capture items (region, fullscreen,
per-monitor submenu), two home-window buttons plus a monitor picker,
and two COSMIC-applet buttons plus per-monitor entries. Meanwhile
ADR-0050 shipped the `Capturer` trait with the XDG Screenshot Portal
native on Wayland, and Spike B (`docs/specs/native-capture-backends/
tasks/002-spike-portal.md`) had measured the portal's
`interactive: true` mode — COSMIC's own full-screen select-and-confirm
UI — dismissing it with "not usable … we have our own editor". Spike C
(`docs/specs/native-capture-ux/tasks/001-spike-interactive.md`)
re-measured it as the *user-facing* selection mechanism: it returns
exactly the selected region (dims track the drag, no composite crop
needed), Enter captures, Escape maps to the existing `"cancelled"`
error — with recorded caveats on the installed COSMIC frontend
(`xdg-desktop-portal-cosmic` 0.1.0pop1: Escape sometimes misreports as
code 0, sometimes wedges with no Response — hence a long Response
budget with a quiet hard timeout, and read-then-delete for files that
land in `~/Pictures/Screenshots` rather than `/tmp`).

Maintaining flintshot's own region-selection windows duplicates what
the compositor already does better (it is guaranteed to layer, focus,
and compose correctly on its own session), while the editor — the
actual value-add — stays as the default post-grab destination.

**Decision.**

- **One Capture action on every surface.** `flintshot capture`
  (carrying `--delay-ms`, `--backend`, `--after`, `--out`,
  `--standalone`, and the interaction flags) replaces `region` /
  `fullscreen` **with no aliases** (0.x schema: all consumers ship
  together; a clean break beats a deprecation shim nobody can age).
  `Request::Capture { delay_ms }` replaces the two IPC variants;
  `region_argv`/`fullscreen_argv` collapse into `capture_argv`.
  `--monitor` dies: selection is the OS UI's job (native) or the
  editor's (fallback); the fallback grabs the primary monitor.
- **Capability-based flow.** `Capturer` gained
  `supports_selection()` + `capture_selection()`. The worker runs
  the native flow (OS selection UI) when the *selected* backend has
  the capability — session kind still drives `auto` precedence
  (unchanged from ADR-0050), but a pinned `--backend portal` on X11
  also gets the native flow: harmless generalization, capability is
  the honest contract.
- **`after_grab` routing on the native path only.**
  `[capture] after_grab = "editor" (default) | "clipboard" | "upload"`
  (CLI `--after`), a pure dispatch; unknown values warn and resolve
  `editor` (string-tolerant policy). The fallback path ignores it —
  selection happens in the editor/overlay there, and a raw
  primary-monitor dump to the clipboard is never what the user
  meant.
- **Cancellation is quiet.** The backend's `"cancelled"` /
  interactive-timeout errors propagate as data; the worker exits 0
  with one info log — no notification, no failure exit.
- **Surfaces collapse mechanically.** Tray: one *Capture* item
  (per-monitor submenu removed). COSMIC applet: one *Capture* action.
  Home window: *Capture* + *Open* (file dialog → `flintshot open`
  worker in its own process — native event loops do not nest), plus
  a fully-built *Paste* control hidden behind a compile-time const
  for the future clipboard→editor feature. Hotkeys:
  `[hotkeys] capture = "…"` (the former `region` default); legacy
  `region`/`fullscreen` keys warn at load via a `toml_edit`
  raw-table inspection *before* serde (which would silently drop
  them) and bind nothing.
- **Fallback byte-identical.** The `xcap` path keeps today's
  monitor-grab → editor/overlay flow, selection-first landing
  included (internal mode label `"region"`), so X11 / portal-down /
  pinned users see no change.

**Consequences.**

- Wayland users get their compositor's own selection UI from every
  surface (hotkey, tray, applet, app window, CLI) with Enter
  returning exactly the selection; Escape is a quiet exit. The OS UI
  cannot be restyled by flintshot and its exact shape varies per
  compositor (GNOME/KDE/wlr unmeasured — `docs/TEST_MATRIX.md` rows;
  `--backend xcap` is the escape hatch).
- Breaking changes across CLI, IPC, and config in one release:
  `flintshot region` / `flintshot fullscreen` now fail to parse
  (pinned by test), old IPC variants no longer compile, and legacy
  hotkey keys must be renamed by hand (the load warning says exactly
  that). Pre-collapse portal shortcut bindings under `region` /
  `fullscreen` stop dispatching until re-bound.
- flintshot no longer needs a region-selection window for the native
  path, but the editor's selection-first phase stays (the fallback
  path and recaptures still use it) — no code was deleted there.
- The editor remains the default post-grab destination everywhere;
  `after_grab = "clipboard"` is the window-less flow (notification
  as the only feedback). Superseded (2026-09-14,
  `docs/specs/clipboard-opens-editor/`): `clipboard` now copies and
  opens the editor too — only `upload` stays window-less.
- Spike B's "we have our own editor" note is superseded *as a UX
  stance* (the OS UI is the better selector); its protocol findings
  (prompt-free non-interactive path, client-lifetime gotcha) stand.

Supersedes the UX stance of Spike B; extends ADR-0050 (the `Capturer`
trait and backend registry this builds on).
