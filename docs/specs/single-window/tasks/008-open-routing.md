# Task 008: `flintshot open` / `OpenLast` routing

**Description:** Open intents converge on the window too. `flintshot open
<path>` delegates `Request::OpenImage { path }` to the daemon when one is
running (no flags exist on `open`, so no standalone carve-outs; daemon
absent → today's in-process editor, unchanged). The daemon: window present
→ broadcast `Event::OpenRequested { path }` (no spawn); else spawn the
existing open worker. Tray `OpenLast` forwards the same event with the
last-saved path instead of spawning. The app-side pump handles
`OpenRequested`: decode → `PendingPage::Editor` → window close/reopen
(geometry change), replacing the current page if an image is already open.
A decode failure with a window present → status line, no route; without a
window → the worker path already errors clearly.

**Acceptance criteria:**
- [ ] With the window running: `flintshot open img.png` (shell) and tray
      *Open last* both surface the image in the single window, focused
- [ ] Without a window: both paths open the worker editor exactly as today
- [ ] Routing decision unit-tested with the recording-spawner pattern
      (forward vs spawn per presence, like Task 007's tests)

**Verification:**
- [ ] Full gate green
- [ ] Manual: both entry points × window-present / window-absent

**Dependencies:** Tasks 001, 004 (007's presence helper if shared — extract
  it rather than duplicating)

**Files likely touched:**
- `crates/flintshot-app/src/worker.rs` (dispatch arms for `OpenImage` /
  forwarded `OpenLast`)
- `crates/flintshot-app/src/cli.rs` (`run_open` delegation)
- `crates/flintshot-app/src/app_window.rs` (`OpenRequested` handling)

**Docs:** None expected beyond variant docs from Task 001.

**Estimated scope:** Medium: 3 files
