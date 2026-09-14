# Task 001: Clipboard arm copies, notifies, then routes through the shell

**Description:** Change `PostGrabAction::Clipboard` in
`native_capture_and_dispatch` (`crates/flintshot-app/src/cli.rs:528`)
so that after the existing copy + notification lines, the arm
performs the `editor` arm's shell invocation: supersede check, then
`run_capture_shell(img, "capture", interaction, watch)?`. The copy +
notify run unconditionally right after the grab (the requested
action); only the editor-open is supersede-gated. Update the two
in-code doc comments this makes stale:
`native_capture_and_dispatch`'s header ("editor shell, clipboard
copy, or upload" → clipboard copies *and then* opens the shell) and
`fire_capture_notification`'s header ("window-less capture paths …
the notification is the only feedback" → the notification is
immediate paste-ready feedback; the editor follows). Review the
fallback-branch GOTCHA (`cli.rs:443`) for coherence — it stays
substantively true ("only the native path dispatches on after_grab")
and should need no change.

**Acceptance criteria:**
- [ ] Clipboard arm order is: `copy_image_to_clipboard(&img)?` →
      `fire_capture_notification(...)` → info log → supersede check
      (quiet `Ok(())` exit) → `run_capture_shell(img, "capture",
      interaction, watch)?`
- [ ] Copy failure (`?` on `copy_image_to_clipboard`) returns before
      any window opens
- [ ] `Editor`, `Upload`, and `--out` arms byte-identical to before;
      fallback path (`run_capture`'s else branch) untouched
- [ ] `native_capture_and_dispatch` and `fire_capture_notification`
      doc comments describe the new flow (no stale "window-less")
- [ ] Existing unit tests still pass; no new pure seams introduced
      (none changed — dispatch body stays I/O-driven per spec)

**Verification:**
- [ ] `cargo build`
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual (Wayland session, human or live agent run):
      `cargo run -- capture --after clipboard` → clipboard holds the
      grab, notification fires, editor/overlay opens pre-loaded;
      `--after editor`, `--after upload`, `--out FILE`, and
      `--backend xcap` behave as before

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs` (only file)

**Docs:**
- Gotcha (X11 owner-clipboard lifetime of the up-front copy, only
  reachable via `--backend portal` pinned on X11): brief comment in
  the Clipboard arm explaining why the copy precedes the shell and
  its lifetime bound — code home, no `docs/gotchas.md` entry.

**Estimated scope:** Small (1 file)
