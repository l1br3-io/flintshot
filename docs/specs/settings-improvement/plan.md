# Plan: settings-improvement — Settings as the single preferences surface

## Overview

Make **Settings** the one preferences surface, VSCode-style: the tray
menu swaps *Open config* / *Bind shortcuts…* for a single **Settings**
item that opens (or focuses) the app window routed to its settings
page; the settings page gets its layout bug fixed (content under the
44 px topbar), gains *Edit config file* / *View default config* links
over a new documented default-config asset, and a Wayland-only
*Rebind shortcuts…* row; the COSMIC applet mirrors the swap. One new
IPC pair (`Request::OpenSettings` / `Event::SettingsFocus`), appended
per the postcard variant-index rule and modeled on the `OpenApp` /
`AppFocus` precedent.

## Architecture decisions

- **`OpenSettings` mirrors `OpenApp` exactly** (broadcast-first, then
  spawn `app --route settings`): a running window shows + routes
  itself via the broadcast; without one, the spawn creates the
  window on Settings directly. No new choreography; the tray-item
  exit rules (`exits_after_action == false`) and pre-hide rules
  (`spawns_capture_worker == false`) follow for free.
- **The route flag is `app --route settings`** (spec's proposal):
  a `--route <home|settings>` clap arg on the `app` subcommand,
  threaded to `app_window::run` as the initial `GenerationStart`,
  and mirrored in the worker table's `app_argv`.
- **Defaults view is an inline collapsible monospace card** in a new
  *Config file* section of the settings page (not a popup): matches
  the existing `section_card` idiom, stays headless-testable, and
  keeps the VSCode *Default/User settings* pair on one surface.
- **The default config is a checked-in asset** (`config.default.toml`,
  embedded via `include_str!`): *View default config* renders it
  read-only; a test pins it field-by-field equal to
  `Config::default()` so the asset can never drift. First *Edit
  config file* click materializes the file from the template
  (header comment + asset body) — only when missing, never
  clobbering (toml_edit round-trip untouched).
- **Settings-page actions drain through the existing bridge shape**:
  `SettingsApp` stays pure data + a drained action queue
  (`EditConfigFile` / `RebindShortcuts`); the window router services
  the queue through new default-bodied `HomeBridge` methods, so test
  doubles keep compiling and headless click tests pin the wiring.
- **Tray ids retire, never recycle**: `flintshot.open-config` and
  `flintshot.bind-shortcuts` are pinned dead by test (the
  `legacy_capture_ids_are_gone` precedent); `flintshot.settings` is
  the new item.

## Task index

### Phase 1: Foundation (core, no behavior change yet)

- [ ] Task 1: Default config asset + template writer → `tasks/001-default-config-asset.md`
- [ ] Task 2: IPC schema append (`OpenSettings` / `SettingsFocus`) → `tasks/002-ipc-open-settings.md`

### Checkpoint: Foundation
- [ ] `cargo test -p flintshot-core` green; workspace still builds

### Phase 2: The Settings flow (IPC → daemon → window → tray/applet)

- [ ] Task 3: Worker-table dispatch for `OpenSettings` → `tasks/003-worker-open-settings-dispatch.md`
- [ ] Task 4: `app --route settings` + window routing + `SettingsFocus` → `tasks/004-app-route-settings.md`
- [ ] Task 5: Tray menu swap (Settings replaces Open config / Bind shortcuts) → `tasks/005-tray-settings-swap.md`
- [ ] Task 6: COSMIC applet mirrors the swap → `tasks/006-cosmic-settings-swap.md`

### Checkpoint: Settings flow end-to-end
- [ ] Tray **Settings** click: window focuses on Settings; no window → spawns on Settings
- [ ] `flintshot app --route settings` cold-starts there

### Phase 3: The settings page itself

- [ ] Task 7: Shared open-in-editor module (`config_edit.rs`) → `tasks/007-config-edit-module.md`
- [ ] Task 8: Settings layout fix (CentralPanel; overlap regression RED test first) → `tasks/008-settings-layout-fix.md`
- [ ] Task 9: Config-file links + read-only defaults view → `tasks/009-settings-config-links.md`
- [ ] Task 10: Wayland-gated *Rebind shortcuts…* row → `tasks/010-settings-rebind-row.md`

### Checkpoint: Settings page complete
- [ ] Links click through; defaults view renders the embedded TOML read-only
- [ ] Scrolling never renders content under the topbar (machine-checked)

### Phase 4: Closeout

- [ ] Task 11: CHANGELOG + full finish gates → `tasks/011-changelog-closeout.md`

## Risks and mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Overlap regression test fights egui internals (clip rects vs shape bounds) | Med | Precedents exist (`topbar.rs` central-top test, `widgets.rs` shape scans); the task allows reformulating the assertion while keeping it machine-checked |
| `WindowStart`/`Page`/`GenerationStart` trio grows a Settings variant in three crates | Med | Task 4 touches all in one commit; route_geometry already treats Settings as stable-geometry |
| `$EDITOR` from a GUI process has no controlling terminal (carried over) | Low | Logged on spawn failure (today's tray behavior); follow-up per spec, not this effort |
| COSMIC crate needs system deps to build | Low | Task 6 verified with `cargo build -p flintshot-cosmic` (opt-in, CI mirrors) |

## Open questions

- None blocking — both spec questions are decided above (`--route
  settings` spelling; inline card for the defaults view). Both are
  local, reversible choices.
