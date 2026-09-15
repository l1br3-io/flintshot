# Task 005: Paste from home

**Description:** Ship the third button. In `home.rs`: flip `SHOW_PASTE` to
`true` (the pinned test flips with it — its contract becomes "Paste is
live") and drop the now-stale TODO. The app-side `HomeBridge::
request_paste` impl in `app_window.rs` reads the system clipboard via
`arboard` on the UI thread (the crate already depends on it — see the
clipboard helpers in the capture flow): an image decodes to
`PendingPage::Editor(state)` + window close (same router path as Open, Task
004); no image / unreadable clipboard → `SetStatus("clipboard has no
image")`, window stays. The paste accelerator (`P`) arms with the button.

**Acceptance criteria:**
- [ ] Copy an image anywhere → `cargo run -- app` → Paste (button or `P`)
      routes to the editor page with exactly those pixels
- [ ] Empty/text clipboard → status message on home, no window churn
- [ ] The `SHOW_PASTE` pinned test now asserts `true` and still fails on
      accidental reversion

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui`
- [ ] Full gate green
- [ ] Manual: `wl-copy < test.png` then paste; also with empty clipboard

**Dependencies:** Task 004

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs`
- `crates/flintshot-app/src/app_window.rs`

**Docs:** GOTCHA if arboard needs a fresh `Clipboard` per read on Wayland
(known arboard quirk) — comment at the read site.

**Estimated scope:** Small: 2 files
