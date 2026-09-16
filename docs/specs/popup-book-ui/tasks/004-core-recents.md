# Task 004: flintshot-core recents index + recording wiring

**Description:** New `flintshot-core::recents` module: ordered list
of image paths (+ mtime), capped at 20, persisted as
`<config_dir>/flintshot/recents.toml` (serde kebab-case, following
`config.rs` patterns). API: `load()`, `record(path)` (dedupes, moves
to front, saves), `paths()` — pure logic, tempfile-tested. Wire
recording in `flintshot-app` at: capture routed to editor page,
`open_image` success (home + editor bridges), and editor save
outcome. Paste (no file path) records nothing.

**Acceptance criteria:**
- [ ] Tempfile tests cover save/load round-trip, cap-at-20, dedupe +
      move-to-front, dead-path prune on load, corrupt-file → empty
      start
- [ ] `no_gui_types_in_core.rs` still passes (module is pure paths)
- [ ] Manual: open an image, restart the app — the path appears in
      `recents.toml` (grid rendering itself is Task 010)

**Verification:**
- [ ] `cargo test --workspace --all-targets` green
- [ ] `cat ~/.config/flintshot/recents.toml` after an open + save
      shows correct order

**Dependencies:** None (parallel-safe with Phase 1)

**Files likely touched:**
- `crates/flintshot-core/src/recents.rs` (new)
- `crates/flintshot-core/src/lib.rs` (module + re-export)
- `crates/flintshot-app/src/app_window.rs` (recording at routing
  points)
- `crates/flintshot-app/src/capture_flow.rs` (capture-landed record)

**Docs:**
- Module doc notes why recents is core (bridge-agnostic, testable)
  and why thumbnails are never persisted.

**Estimated scope:** Medium (4 files)
