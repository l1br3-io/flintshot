# Task 010: Welcome screen (hero + recents grid + drop)

**Description:** Redesign `home.rs`'s page per Gradia pattern #1 on
the pop-up-book skin: a large "Take a screenshot" hero cutout card
(forest-filled, spring pop-in), a recents grid below (paths from
`core::recents` via a new `HomeBridge::recent_images()` method
implemented by the app crate; thumbnails decoded + downscaled on
demand into an in-memory cache, letterboxed on cream; click →
existing `open_image` route; empty state shows a friendly card, not
blank space), Open / Paste as secondary cutout cards, and a
whole-page drag-and-drop image target (best-effort on Wayland —
open/paste/recents make the page complete regardless). Grain tiles
the welcome background. Status line and close-watchdog contract
(ADR-0047) untouched.

**Acceptance criteria:**
- [ ] Every welcome state renders content (with recents, without,
      no-config-dir) — never blank
- [ ] Recents persist across restarts, cap 20, click reopens via
      the existing open path; dead paths silently absent
- [ ] Hero capture card triggers the existing capture flow
      (window-close handshake unchanged — existing home tests pass)
- [ ] Drop of an image file routes to the editor (on winit-supported
      platforms); failure degrades silently, page stays complete

**Verification:**
- [ ] Full suite + clippy green; new bridge tests for
      `recent_images`
- [ ] Manual: fresh config → hero only; open a file → restart →
      thumbnail grid; drag-drop a PNG onto the window

**Dependencies:** Tasks 001, 002, 003, 004, 009

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs`
- `crates/flintshot-editor-egui/src/shell.rs` (`HomeBridge` method)
- `crates/flintshot-app/src/app_window.rs` (bridge impl)

**Docs:**
- GOTCHA if winit DND needs explicit viewport hints on Wayland.

**Estimated scope:** Large (3 files + bridge)
