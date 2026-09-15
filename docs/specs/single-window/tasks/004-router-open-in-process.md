# Task 004: Outer-loop page router + in-process Open

**Description:** Turn `app_window::run`'s outer loop into the page router.
`Page::{Home, Editor(EditorState)}` picks which epoch runs: `run_home` (as
today) or `run_editor_page` from Task 003 with an app-side bridge. The
**Open** button stops spawning `flintshot open` children: `HomeBridge::
open_image` decodes the picked file on the UI thread (after the existing
blocking `rfd` dialog), stores `PendingPage::Editor(state)` in `Shared`,
and closes the window; the outer loop runs the editor epoch. `Esc` from the
editor (`EditorPageExit::Home`) routes back to a Home epoch. New close
reasons in the `CloseReason` classification: `OpenImage(EditorState)` and
`GoHome`; `UserQuit` and the capture-intent machinery stay. Window-close
watchdog logic stays. `spawn_open_worker` and its argv test are deleted.

**Acceptance criteria:**
- [ ] `cargo run -- app`: Open… → file picker → same window reopens as the
      image-sized editor; Esc → same window reopens as three-button home;
      window X → process exits (daemon untouched)
- [ ] No child process spawns on the Open path (verify: no new `flintshot`
      in `ps` output)
- [ ] Close-reason unit tests cover the new intents (router picks the right
      next page per close reason; GoHome never misread as UserQuit)
- [ ] Unreadable image file → status line on home, window stays, no route

**Verification:**
- [ ] Full gate green (existing `Shared`/close tests updated, not deleted)
- [ ] Manual pass on the dev session: open a PNG, Esc, open a JPEG, quit

**Dependencies:** Task 003

**Files likely touched:**
- `crates/flintshot-app/src/app_window.rs` (`Page`, `PendingPage`, close
  reasons, router loop, `HomeBridge` impl)
- `crates/flintshot-editor-egui/src/shell.rs` (only if the epoch signature
  needs adjusting)

**Docs:** None expected; the module docs at the top of `app_window.rs` get
rewritten to describe the router (ADR-0047 references preserved).

**Estimated scope:** Medium: 2 files
