# Task 005: Router skeleton — `run_window` with the Home route

**Description:** Introduce the router app: one eframe lifetime per
*window generation* with pages as internal routes. A new
`WindowApp` (editor-egui, e.g. `window.rs`) holds a route enum —
`Home` / `Editor` / `Loading` (the latter two land as stubs for
later tasks) — delegating `App::logic`/`App::ui` to the active
route, and issuing the transition viewport command batch
(`InnerSize` + `Resizable` + `Title`) when the route changes. A
single `run_window(start_route, home_bridge, page_bridge, …)`
entry replaces `run_home` in this task (the editor keeps
`run_editor_page` until Task 006). Home keeps its compact fixed
geometry (OQ-4 lean); the router swaps geometry per route. The app
crate adopts `run_window` for the home generation; the outer loop's
shape is unchanged this task — that is Task 006's shrink.

**Acceptance criteria:**
- [ ] `run_window` opens a generation starting on the Home route;
      home behavior byte-identical (capture click closes, Open/
      Paste route via the pending-editor close path, watchdog,
      directives, accelerators)
- [ ] Route-state transitions are pure where possible: a
      headless-testable `route_geometry(route) -> (size, resizable,
      title)` helper + unit tests; route-enum transition tests
- [ ] `EguiShell::run_home` removed (or delegates to `run_window`);
      `app_window.rs` home epoch spawns through `run_window`
- [ ] The router exposes its current route on close (restore
      parity input for Task 006) — e.g. a slot the shell reads
      after `run_native` returns

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui -p flintshot-app` green
- [ ] Full gate green (see Task 001's gate commands)
- [ ] Manual: `cargo run -- app` — home looks and behaves exactly
      as before (capture → close → editor epoch; Open → editor
      epoch; status line; focus ping)

**Dependencies:** Task 004 (editor route's stable geometry makes
per-route geometry constants meaningful)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/window.rs` (new) or
  `shell.rs`
- `crates/flintshot-editor-egui/src/lib.rs` (module + exports)
- `crates/flintshot-app/src/app_window.rs`

**Docs:**
- None expected (ADR lands at close-out, Task 010).

**Estimated scope:** Medium: 3 files
