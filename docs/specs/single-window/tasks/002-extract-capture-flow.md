# Task 002: Extract the grab flow from `cli.rs` into `capture_flow.rs`

**Description:** Pure refactor, zero behavior change. Move the
process-agnostic grab machinery out of `cli.rs` into a new
`flintshot-app/src/capture_flow.rs`: `native_grab_prepared`,
`capture_prepared`, `resolve_capture_backend`, the clipboard helpers
(`read_portal_clipboard_grab`, `await_changed_clipboard`, `rgba_digest`,
`clipboard_image_digest`, `copy_image_to_clipboard`,
`read_image_from_clipboard`), the classifiers
(`quiet_capture_exit`, `portal_grab_on_clipboard`), their constants
(`PORTAL_CLIPBOARD_*`), and their tests. `cli.rs` re-imports and keeps its
editor/opening halves (`run_capture_shell`, `run_editor_for_capture`,
recapture loop). Gate visibility `pub(crate)` so Task 006's app-window call
site compiles later. Do NOT move `CaptureGuard`, `SupersedeWatch`, or the
post-grab dispatch (`native_capture_and_dispatch`) — they stay in `cli.rs`
for now (Task 006 decides what the pump reuses).

**Acceptance criteria:**
- [ ] `cargo run -- capture` flows unchanged: same logs, same backend
      resolution order (CLI > config > auto), same portal-clipboard
      read-back wait
- [ ] All moved unit tests pass from their new home; no test logic changed
- [ ] `cli.rs` shrinks to dispatch + editor/session concerns only

**Verification:**
- [ ] Full gate green
- [ ] Manual: `cargo run -- backends` and one standalone
      `cargo run -- capture --standalone` on the dev session still work

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-app/src/capture_flow.rs` (new)
- `crates/flintshot-app/src/cli.rs`
- `crates/flintshot-app/src/main.rs` (module list)

**Docs:** If any move reveals a non-obvious coupling (e.g. a helper reading
a cli.rs const), a `// GOTCHA` comment at the new location.

**Estimated scope:** Medium: 3 files
