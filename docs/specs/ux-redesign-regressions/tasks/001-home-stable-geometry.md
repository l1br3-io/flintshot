# Task 001: Home route geometry matches the stable window geometry

**Description:** `route_geometry(&Route::Home)` still returns the
pre-redesign compact geometry (360×240, non-resizable) from
single-window OQ-4's three-button home. The redesigned home (topbar +
hero + hcards + recents) cannot fit — the topbar alone needs ~460 px,
so heroes overflow leftward past the brand on Home. Give Home the same
geometry as Editor/Loading/Settings: `EDITOR_PAGE_DEFAULT_SIZE`
(1024×640), resizable, title `flintshot`. `run_window` builds its
viewport from the same fn (shell.rs:266-270), so the initial window and
every Home↔Editor transition batch fix together. Home content should
lay out complete at 1024×640 and tolerate resize; if wide-stretch looks
wrong, cap the content column (~640) and center — decide in the manual
pass (plan open question).

**Acceptance criteria:**
- [ ] `route_geometry(&Route::Home)` returns size 1024×640,
      `resizable: true`, title `HOME_WINDOW_TITLE`
- [ ] The pinned test is updated: home uses the stable geometry (rename
      `home_route_keeps_its_compact_fixed_geometry`); Home→Editor
      transitions still issue exactly the 3-command batch, now
      identical geometry (only Title differs if at all)
- [ ] Home welcome body renders complete at 1024×640 — hero, three
      hcards, recents grid, status line all visible; the topbar never
      clips or wraps
- [ ] Resizing the home window smaller keeps content reachable
      (ScrollArea over the welcome body if the fixed heights overflow
      below ~600 px height)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` (quiet) — geometry + transition tests green
- [ ] `cargo fmt --all` && `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: `cargo run -- app` — Home opens 1024×640, resizable,
      nothing clipped; shrink/grow the window

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-editor-egui/src/window.rs` (route_geometry + Route::Home doc + tests)
- `crates/flintshot-editor-egui/src/home.rs` (welcome body scroll/column cap, only if needed)

**Docs:**
- The Route::Home doc comment's "compact, fixed geometry — OQ-4's
  lean" note updates in-place (the OQ-4 decision is superseded by the
  ux-redesign layout); no separate doc home needed.

**Estimated scope:** Small: 1-2 files

**Git Commit:** `git commit -m "fix(home): stable 1024×640 geometry — the redesign's home no longer fits the compact window"`
