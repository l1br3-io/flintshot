---
status: active
started: 2026-09-14
---

# Spec: clipboard post-grab opens the editor

## Objective

`after_grab = clipboard` (config `[capture] after_grab` or `--after
clipboard`) behaves differently per backend today:

- **Native path** (Wayland portal — the only backend with
  `supports_selection() = true`): the grab is copied straight to the
  clipboard and a notification fires. No editor window ever opens
  (`crates/flintshot-app/src/cli.rs`, `native_capture_and_dispatch`'s
  `PostGrabAction::Clipboard` arm).
- **Fallback path** (X11/xcap and every backend without selection):
  `after_grab` is ignored; every grab lands in the editor, where the
  user copies via the Copy button.

Same flag, different behavior per backend. This effort makes the
native clipboard route **copy AND open the editor**: the image is
written to the clipboard immediately (the user's explicit "save to
clipboard" intent — paste-ready right away), then the editor opens
pre-loaded with the same grab so it can be refined and re-copied.
Every capture that can show a window now does.

User story: *As a Wayland user who triggers a capture with
`after_grab = clipboard`, I want the grab on my clipboard immediately
AND the editor open, so that my backend doesn't silently change what
happens after a grab — and I can still annotate before pasting.*

### Acceptance criteria

1. `flintshot capture --after clipboard` on the portal (Wayland)
   backend copies the grab to the clipboard (unchanged from today),
   fires the "Capture copied to clipboard" notification (unchanged),
   **and** opens the editor in plain `"capture"` mode (selection
   already happened in the OS UI — same mode label and shell path as
   the `editor` arm).
2. Clipboard-copy failure remains a hard error: the editor must not
   open if the copy failed (the user asked for the clipboard and
   nothing happened — existing semantics preserved).
3. `after_grab = upload` is unchanged: direct window-less upload.
4. `after_grab = editor` and the `--out` PNG path are unchanged.
5. The fallback (non-selection) path is untouched — it already always
   opens the editor.
6. Supersede semantics: the copy + notification happen unconditionally
   after a successful grab (they are the requested action); the
   editor-open is skipped with a quiet `Ok(())` exit if the session
   was superseded in between — mirroring the existing `editor` arm's
   post-grab supersede check.
7. All entry points converge: CLI-direct, daemon-delegated, tray, and
   hotkey captures run through `native_capture_and_dispatch`, so all
   of them get the new behavior from the one change.

## Technologies

No new dependencies. Pure Rust, existing crates:

- `flintshot-app` (`cli.rs`) — the only file with behavior changes.
- `arboard` (existing) — clipboard write, via the existing
  `copy_image_to_clipboard` helper.
- `notify-rust` (existing) — via the existing
  `fire_capture_notification` helper.

## Commands

```sh
cargo build                                    # compile (default-members)
cargo test --workspace --all-targets           # unit + integration
cargo test -p flintshot-app clipboard          # targeted unit tests (cli.rs)
cargo fmt --all --check                        # formatting gate
cargo clippy --workspace --all-targets -- -D warnings   # lint gate
cargo run -- capture --after clipboard         # manual check (Wayland session)
```

## Project Structure

No new files. Touched:

```
crates/flintshot-app/src/cli.rs   # native_capture_and_dispatch Clipboard arm
CHANGELOG.md                      # user-visible change → [Unreleased]
docs/specs/clipboard-opens-editor/  # this spec
```

The change site is `native_capture_and_dispatch`
(`crates/flintshot-app/src/cli.rs:528`): the `Clipboard` arm gains the
`editor` arm's shell invocation (`run_capture_shell(img, "capture",
interaction, watch)`) after the existing copy + notify lines. If the
match arms' shared body grows awkward, extract a small helper — but
at this size inline is fine.

## Code Style

Follows repo conventions (`AGENTS.md`): `#![forbid(unsafe_code)]`,
no `unwrap()`/`expect()` outside tests, pedantic clippy clean,
comments only for non-obvious "why". The changed arm, target shape:

```rust
match post_grab {
    PostGrabAction::Editor => {
        if watch.superseded() {
            tracing::info!("native capture superseded before the shell; exiting cancelled");
            return Ok(());
        }
        run_capture_shell(img, "capture", interaction, watch)?;
    }
    PostGrabAction::Clipboard => {
        // Copy first, unconditionally: the copy is the requested
        // action; superseding must not swallow it.
        copy_image_to_clipboard(&img)?;
        fire_capture_notification("Capture copied to clipboard");
        tracing::info!("native capture copied to clipboard");
        // Then the editor, mirroring the Editor arm, so every
        // backend lands in the editor after a grab.
        if watch.superseded() {
            tracing::info!("native capture superseded before the shell; exiting cancelled");
            return Ok(());
        }
        run_capture_shell(img, "capture", interaction, watch)?;
    }
    PostGrabAction::Upload => {
        upload_capture_and_report(&img)?;
    }
}
```

## Testing Strategy

Repo-standard: unit tests colocated in `cli.rs`'s `#[cfg(test)] mod
tests`, run via `cargo test --workspace --all-targets`. The dispatch
body is I/O-driven (clipboard, windows, D-Bus), so it is not directly
unit-testable headless; the pure seams this change relies on
(`resolve_post_grab`, `session_next`) already have tests. Add/extend
tests only where a pure seam changes; otherwise verify by build +
clippy + the manual Wayland check. No new integration tests — the
portal path requires a live session bus and compositor.

## Boundaries

- **Always:** run the four finish-gate commands (fmt, clippy, test,
  doc-tests) before declaring done; update `CHANGELOG.md` under
  `## [Unreleased]`; keep the GOTCHA comment block at the fallback
  branch (`cli.rs:443`) coherent with the new behavior.
- **Ask first:** any change to `PostGrabAction` semantics beyond the
  Clipboard arm; touching the fallback path; new dependencies.
- **Never:** route `upload` through the editor in this effort; weaken
  the clipboard-failure-is-an-error rule; edit the daemon protocol.

## Success Criteria

- [ ] `cargo run -- capture --after clipboard` on a Wayland/portal
      session: clipboard holds the grab, notification fires, editor
      window opens pre-loaded, ready to refine/re-copy.
- [ ] `--after editor`, `--after upload`, `--out`, and the fallback
      path behave byte-identically to before.
- [ ] `cargo fmt --all --check`,
      `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace --all-targets`, and
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
      all pass.
- [ ] `CHANGELOG.md` updated under `## [Unreleased]`.

## Open Questions

None — resolved at spec time: copy happens before the editor opens
(paste-ready immediately); upload stays window-less; the notification
stays.
