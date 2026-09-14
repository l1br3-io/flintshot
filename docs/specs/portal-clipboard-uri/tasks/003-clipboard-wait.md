# Task 003: The clipboard wait outlasts the Response race

**Description:** The Task-001 reopen (~1 s retry) still failed in
the wild (daemon.log 2026-09-14 18:31Z: 21 attempts, every read
"not available in the requested format or the clipboard is empty"),
yet the portal's write *does* land — the image is on the clipboard
and arboard-readable minutes later. Root cause, from
xdg-desktop-portal-cosmic's own `Msg::Capture` (`src/screenshot.rs`):
the `clipboard:///` Response is `tokio::spawn`ed before the update
fn returns its `Task::batch`, and the `clipboard::write_data` task
is queued **behind the overlay layer-surface destroys** — the gap
from Response to readable selection is unbounded and machine-
dependent, observed >1 s. A second latent bug rode the same seam:
if an image already sat on the clipboard (the user's previous
Copy), the first retry could succeed with the **stale** screenshot
— the "silently wrong image" the spec forbids.

Fix in `crates/flintshot-app/src/cli.rs`:

1. `clipboard_image_digest()` snapshots the clipboard's image
   digest **before** the OS UI opens (`rgba_digest` over
   width/height/bytes; `None` when no image).
2. `native_grab_prepared` narrows the prepare guard's scope to the
   grab itself, then routes the marker arm to
   `read_portal_clipboard_grab(pre_grab)`.
3. `read_portal_clipboard_grab` polls `await_changed_clipboard` —
   `PORTAL_CLIPBOARD_WAIT_SLOTS`, 50 × 100 ms ≈ 5 s — accepting
   only an image whose digest **differs** from the snapshot. An
   image identical to the snapshot at the deadline is accepted
   anyway (two identical screenshots back to back must not read as
   "never landed"; byte-identical is correct content). Nothing
   landing in the window stays a hard error naming the wait.
4. The old `read_with_retries` + `CLIPBOARD_READ_RETRY_DELAYS`
   (single caller) are removed; the pure wait driver
   (`await_changed_clipboard`, probe/digest injected, zero-length
   delays keep tests sleep-free) carries the unit tests.

**Acceptance criteria:**
- [x] A changed image wins on the first probe it appears in; a
      stale image never wins while a fresh one is coming
- [x] The attempt policy is one probe per slot plus a final
      (`delays.len() + 1`), pinned by test
- [x] A deadline image byte-identical to the snapshot is accepted
      (`stale_accepted`), never a timeout
- [x] Nothing landing within the window is a hard error naming the
      ~5 s wait after the `clipboard:///` response
- [x] The prepare guard no longer covers the wait: the tray
      restores at pixel-time even while the write is pending
- [x] No `Capturer` trait change, no core change, no new deps

**Verification:**
- [x] `cargo test -p flintshot-app clipboard` (6 tests)
- [x] `cargo build`
- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace --all-targets`
- [x] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual (COSMIC session, human): tray → Capture → confirm with
      Copy → editor opens; daemon.log shows
      "portal clipboard write landed" (or the stale-accept info)

**Dependencies:** Task 001 (marker + read-back seam)

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs`
- `CHANGELOG.md`
- `docs/specs/portal-clipboard-uri/` (spec, todo, this task)

**Estimated scope:** Small (1 code file + docs)
