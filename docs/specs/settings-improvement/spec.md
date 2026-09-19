# Spec: settings-improvement

---
status: active
started: 2026-09-19
---

## Objective

Make **Settings** the single preferences surface and align the config
file with it, VSCode-style.

Today preferences are fragmented: the tray menu exposes *Open config*
(runs `$EDITOR`) and *Bind shortcuts…* (portal dialog), while the real
settings page lives behind the topbar overflow. The settings page also
has a layout bug — its `ScrollArea` is drawn without a `CentralPanel`,
so scrolled content overlaps the 44 px topbar.

This effort:

1. **Tray menu becomes** `Open app window / Capture / Settings / Quit`.
   *Open config* and *Bind shortcuts…* items are removed. Clicking
   **Settings** opens (or focuses) the single app window routed to the
   settings page — the same open-or-focus flow as *Open app window*,
   carrying the destination route.
2. **Fix the settings page layout**: content clips below the topbar;
   scrolling never slides under it. Follow with a frontend-design-guided
   polish pass (`.opencode/skills/frontend-design/SKILL.md`) within the
   existing theme tokens.
3. **Add config-file links on the settings page**:
   *Edit config file* opens the user's `config.toml` in `$EDITOR`
   (fallback `$VISUAL`; print the path when neither is set — today's
   tray behavior). First click on a missing file writes a commented
   template first, so the file on disk always mirrors what Settings
   shows.
   *View default config* renders the embedded, fully-commented default
   TOML read-only in-app — the VSCode *Default Settings* / *User
   Settings* pair.
4. **Config model (VSCode-style)**: a documented **default config**
   (checked-in asset, embedded via `include_str!`, rendered by *View
   default config*) plus the user's `config.toml` holding their values.
   The settings GUI remains the editor for the user file; merge
   semantics unchanged (serde `#[serde(default)]` per field, unknown
   fields ignored).
5. **Bind shortcuts stays GUI-reachable**: the settings page's
   Shortcuts section gains a *Rebind shortcuts…* action (Wayland-only
   row; hidden on X11 exactly as the tray item was gated). The
   `flintshot hotkeys bind` CLI path is untouched.
6. **COSMIC applet mirrors the swap**: its *Open config* item becomes
   *Settings*, sending the same IPC request.

**Users**: Linux desktop users of flintshot who today must know where
`config.toml` lives to change anything, and who see broken scrolling on
the settings page.

## Technologies

Unchanged from the workspace: pure-Rust Cargo workspace, egui/eframe
for the app window, `tray-icon`/muda for the tray, postcard IPC
(`flintshot-core::ipc`), `toml_edit` for config writes, libcosmic for
the applet. No new dependencies.

## Commands

From the repo root (full list in `AGENTS.md`):

```sh
cargo build                                   # default members
cargo run -- app                              # the app window
cargo run -- app --route settings             # cold-start on Settings (new)
cargo test --workspace --all-targets          # unit + integration
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p flintshot-cosmic               # applet (opt-in; system deps)
```

## Project Structure

Touch points (no new crates):

```
crates/flintshot-core/src/config.rs            # default_config_toml() + asset; template writer
crates/flintshot-core/src/config.default.toml # NEW: commented default config asset
crates/flintshot-core/src/ipc.rs               # append Request::OpenSettings, Event::SettingsFocus
crates/flintshot-app/src/tray.rs               # menu items + TrayAction swap
crates/flintshot-app/src/tray_helper.rs        # drop OpenConfig path
crates/flintshot-app/src/config_edit.rs        # NEW: shared open-in-editor (editor_argv moves here)
crates/flintshot-app/src/daemon.rs             # OpenSettings dispatch (broadcast + spawn)
crates/flintshot-app/src/worker.rs             # worker-table row + app argv with route flag
crates/flintshot-app/src/cli.rs                # `app --route settings`
crates/flintshot-app/src/app_window.rs         # SettingsFocus handler; settings bridge callbacks
crates/flintshot-editor-egui/src/settings.rs   # layout fix; header links; rebind row; defaults view
crates/flintshot-editor-egui/src/window.rs     # CentralPanel wrap; bridge wiring
crates/flintshot-cosmic/src/{menu,app}.rs      # Settings swap; drop local open_config
docs/specs/settings-improvement/               # this effort
```

## Code Style

Repo conventions (`AGENTS.md` + `.opencode/tech/rust/coding-style.md`)
apply: `#![forbid(unsafe_code)]`, `clippy::pedantic` clean, doc
comments on public items in core, `#[non_exhaustive]`,
`#[serde(rename_all = "kebab-case")]`, no `unwrap()`/`expect()` outside
tests, comments only for non-obvious *why*.

Representative snippet — IPC variants are **appended** (postcard
encodes the variant index; inserting would break wire compat), as
every prior effort did:

```rust
// ipc.rs — appended (postcard variant-index rule)
/// Client → daemon: open the app window on its settings page.
/// Mirrors [`Request::OpenApp`] (broadcast + app-worker spawn) but
/// carries the destination route.
OpenSettings,
```

Tray ids stay short + namespaced; removed ids are pinned dead by test
(the `legacy_capture_ids_are_gone` precedent):

```rust
const ID_SETTINGS: &str = "flintshot.settings";
// "flintshot.open-config" / "flintshot.bind-shortcuts" must resolve
// to None — pinned in tests, never reused.
```

## Testing Strategy

Same levels as the workspace: unit tests in-crate (`#[cfg(test)]`),
headless egui tests via `ctx.run_ui` (the settings/topbar/window
precedents), integration tests in `flintshot-core/tests/`. Key pins:

- **Tray**: `action_for` maps `flintshot.settings` →
  `Request::OpenSettings`; `flintshot.open-config` and
  `flintshot.bind-shortcuts` resolve to `None` (legacy-id pin);
  `exits_after_action(OpenSettings) == false` (helper stays alive,
  like `OpenApp`).
- **IPC/daemon**: worker-table dispatch for `OpenSettings` broadcasts
  `Event::SettingsFocus` and spawns the app worker with the route flag
  (table-name + argv pins, the `OpenApp` precedent).
- **App window**: `SettingsFocus` shows the window and routes to
  Settings; `app --route settings` cold-starts there (router test).
- **Layout regression**: with the topbar + settings route rendered
  headlessly at a small screen height, every painted shape's bounds
  sit at/below `TOPBAR_HEIGHT` — the overlap bug's RED test first.
- **Config asset**: the checked-in `config.default.toml` parses and
  deserializes to a `Config` equal (field-by-field) to
  `Config::default()` — the asset can never drift from code defaults.
- **Template write**: `ensure_user_config()` on a temp dir creates the
  file with the header comment and a body that parses; on an existing
  file it is a no-op (never clobbers user edits).
- **Editor split**: `editor_argv("code --wait") == ["code", "--wait"]`
  (moved test), blank value → path printed.
- **Settings page**: the two links render and report through the
  bridge (headless click tests); the defaults view shows the embedded
  TOML text and registers no editable widget; the rebind row renders
  iff the bridge reports Wayland.
- **Cosmic**: `action_for(Settings)` → `Request::OpenSettings`;
  `OpenConfig` variant removed; `request_spawns_capture` unchanged.

## Boundaries

- **Always**: append IPC variants (never insert/reorder); keep
  `flintshot-core`'s public API free of GUI types
  (`tests/no_gui_types_in_core.rs` must stay green); run the four
  finish commands (fmt / clippy / test / doc-test) before declaring
  done; update `CHANGELOG.md` under `## [Unreleased]`; preserve user
  comments in `config.toml` on every write (toml_edit).
- **Ask first**: new dependencies (none expected); CI changes;
  renaming the user-facing config path or schema fields; changing the
  tray-item exit choreography (ADR-0036/0037/0042 handshake).
- **Never**: `unsafe`; `#[allow]` without a justifying comment; reuse
  of retired menu ids (`flintshot.open-config`,
  `flintshot.bind-shortcuts`); breaking postcard wire bytes of
  existing variants; auto-overwriting an existing user config with the
  template.

## Success Criteria

1. Tray menu offers exactly *Open app window*, *Capture*, *Settings*,
   *Quit*; the old two items are gone and their ids pinned dead.
2. Tray/applet **Settings** click: running window focuses and routes
   to the settings page; no window → one spawns directly on Settings.
3. Scrolling the settings page never renders content overlapping the
   topbar (machine-checked); the page passes the frontend-design
   polish pass (spacing, hierarchy, link affordances consistent with
   the theme tokens).
4. *Edit config file*: missing file → commented template written
   first; opens in `$EDITOR`/`$VISUAL`, else prints the path. Saved
   settings keep writing through the debounce + toml_edit round-trip.
5. *View default config* shows the embedded commented defaults
   read-only; the asset is pinned equal to `Config::default()` by test.
6. *Rebind shortcuts…* available in Settings on Wayland sessions,
   absent on X11; drives the same portal path as the old tray item.
7. COSMIC applet menu shows *Settings* and sends the IPC request;
   local `open_config` code removed.
8. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
   -- -D warnings`, both test commands: green.

## Open Questions

- Cold-start flag spelling (`app --route settings` proposed) and the
  defaults-view presentation (inline monospace card vs popup) —
  decide in the plan phase; both are local, reversible choices.
- `$EDITOR` from a GUI process has no controlling terminal (carried
  over from today's tray behavior; logged on spawn failure). If it
  bites, an `xdg-open`/portal fallback is a follow-up, not this
  effort.
