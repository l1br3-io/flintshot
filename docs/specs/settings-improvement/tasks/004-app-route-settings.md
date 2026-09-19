# Task 004: `app --route settings` + window routing + `SettingsFocus`

**Description:** Wire the route end to end: the CLI gains
`app --route <home|settings>`, the egui window router gains a
Settings start, and the app window's pump handles
`Event::SettingsFocus` (show/focus + route swap, in-window when
open, as the reopen start when closed).

**Acceptance criteria:**
- [ ] `flintshot app --route settings` parses (`AppArgs` with a
  validated `--route` value parser; bare `app` unchanged); a parse
  test pins both spellings and rejects `--route bogus`
- [ ] `app_window::run(initial_route)` threads the route: a new
  `GenerationStart::Settings` (and `Page::Settings` for the
  same-page restore) opens the window on Settings
- [ ] egui: `WindowStart::Settings` + `start_route` mapping +
  `WindowApp::new` constructs the settings route (settings page
  built from the home bridge's config snapshot, as `request_route`
  does today); `route_geometry(Settings)` unchanged (stable
  geometry)
- [ ] Pump: `Event::SettingsFocus` behaves like `AppFocus` in
  `HomeCore` (show only when idle, never over a live capture) **and**
  arms a route-settings intent in `Shared` (mirroring the
  `open_requested` pattern in `connected_loop`)
- [ ] In-window: `RouteHost::take_route_settings()` consumed by
  `WindowApp::logic_impl` → `request_route(Route::Settings)` (open
  window swaps page, no close)
- [ ] Closed window: `next_start_after_reopen` honors the intent →
  `GenerationStart::Settings`
- [ ] Unit tests: HomeCore's SettingsFocus arm (idle → Show,
  mid-capture → held), the Shared round-trip, the in-window swap,
  the reopen start, and `route_to_page(Route::Settings)` restoring
  Settings

**Verification:**
- [ ] `cargo test --workspace --all-targets` green
- [ ] `cargo run -- app --route settings` opens on Settings
  (manual, display session)

**Dependencies:** Tasks 2, 3

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs` (AppArgs, run_app)
- `crates/flintshot-app/src/main.rs` (Command::App args)
- `crates/flintshot-app/src/app_window.rs` (run signature,
  GenerationStart/Page, pump handling, bridge)
- `crates/flintshot-editor-egui/src/window.rs` (WindowStart,
  RouteHost, polls)
- `crates/flintshot-editor-egui/src/shell.rs` (start_route)

**Docs:**
- GOTCHA-level comments only where the intent/restore dance is
  non-obvious (mirror the `route_home` precedent's tone).

**Estimated scope:** Medium: 4–5 files
