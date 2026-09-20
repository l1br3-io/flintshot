# Task 006: Home "This session" section

**Description:** The visible surface. `HomeBridge::session_captures()
-> Vec<SessionCard>` (`SessionCard { id, png_path, dims, created_at }`
— the `<id>.png` path feeds the existing on-demand decode/downscale
thumbnail machinery, exactly like recents paths). `home.rs` renders a
"This session" section **above** the recents grid: 4-per-row cards in
the pop-up-book language, caption `WxH · age` + an "unsaved"
affordance (match recent-captures-v2 caption style), click = resume
(Task 005's `open_session_capture`), hover Discard chip with on-chip
two-step "Sure?" confirm (the Delete-chip pattern) →
`discard_session_capture` bridge method → `remove` + list refresh.
Section hidden when empty; scrolls on overflow like the recents grid.
List refreshes at the existing refresh points (construction, `Show`,
after a routed image).

**Acceptance criteria:**
- [ ] Cards render with live thumbnails for every store entry; empty store → no section
- [ ] Click resumes the editor (routes away from Home); Discard chip is two-step and removes the card
- [ ] Section scrolls when cards overflow the window (mirror the recents overflow test)
- [ ] Defaults keep test doubles compiling; no GUI types leak into core

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui --lib` (render + interaction tests following the `recents_grid_renders_a_real_thumbnail_and_reopens_it` pattern)
- [ ] Manual: `cargo run -- app` — capture, annotate, Home, resume, discard, save

**Dependencies:** Tasks 004, 005

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs` (section, cards, chips, bridge methods)
- `crates/flintshot-app/src/app_window.rs` (bridge impl: list/discard)

**Docs:**
- None (visual choices follow recent-captures-v2's shipped patterns)

**Estimated scope:** Medium
