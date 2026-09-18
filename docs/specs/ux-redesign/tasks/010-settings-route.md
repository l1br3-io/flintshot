# Task 010: Settings route + page UI

**Description:** New `settings.rs` page + `Route::Settings` in the
router. Content per `reference/settings.html`: centered 640px column,
cutout section cards (Capture / Editor / Output & saving / Shortcuts /
System), `set-row` lines with `widgets` controls — wells, switches,
readonly wells. Every editable row reads + writes the Task 9 config
fields through the bridge; readonly rows (hide-from-shot 🔒, default
tool, shortcuts list, display server, version) render real values and
are visually dashed/locked. Route entry: the `⋯` overflow Settings
item (Task 5) activates; breadcrumb `Home ▸ Settings`; Escape/back
returns Home.

**Acceptance criteria:**
- [ ] `Route::Settings` reachable from every route's overflow; route
      transition + close choreography unchanged (ADR-0047 intact).
- [ ] All spec mapping-table rows render; editable rows apply to the
      in-memory config + persist (debounced save through the existing
      path); readonly rows show live values (display server detected,
      `CARGO_PKG_VERSION`, binding table, current default tool).
- [ ] Save location `Choose…` opens a directory picker and stores the
      path; filename template edits validate cheaply (no save on
      invalid chars).
- [ ] Topbar on settings: Capture enabled, Copy/Open disabled (mockup
      parity), no statusbar.
- [ ] Headless tests: section render, row→field wiring map, readonly
      distinctness; route tests updated.

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: toggle switches + wells → restart app → values persist;
      old TOML + new TOML both load; breadcrumb + Escape flow works

**Dependencies:** Tasks 5 (overflow entry), 7 (route matrix), 9
(schema)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/settings.rs` (new)
- `crates/flintshot-editor-egui/src/window.rs`
- `crates/flintshot-editor-egui/src/topbar.rs`
- `crates/flintshot-app/src/app_window.rs` (bridge plumbing if the
  config handle rides an existing bridge)

**Estimated scope:** Medium-Large (4 files — page is new but
self-contained)

**Docs:**
- GOTCHA: config write debounce + watcher interplay — inline comment
  at the save site.
