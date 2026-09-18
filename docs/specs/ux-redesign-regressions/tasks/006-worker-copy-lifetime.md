# Task 006: Worker Copy no longer closes the window on Linux

**Description:** ASK-FIRST (window/close choreography, ADR-0047
territory) — approved via this plan's review. Today a successful Copy
in a **worker** editor window on Wayland publishes the outcome and
closes (`close_after_copy_window` → `close_after_copy(wayland) =
true`), which ends the process — and the probe (spec §2) proved COSMIC
does **not** adopt an exited client's zwlr_data_control selection: the
clipboard empties. The doc comment's compositor-ownership assumption
is false for arboard's wl-clipboard-rs thread-serve model (the serve
thread lives in-process only; wl-copy forks for exactly this reason).
X11 was already handled by lingering (owner-based clipboard). New
policy: a worker Copy never auto-closes on Linux (either display
server) — it shows the Task-4 "Copied" state and the user closes via
Esc/X (the outcome is pre-published, so the eventual close still
reports the copy). macOS/Windows keep the close (OS-owned clipboard),
expressed platform-neutrally in the decision fn.

**Acceptance criteria:**
- [ ] `close_after_copy` decision table: Linux X11 → false, Linux
      Wayland → false, OS-owned platforms → true (pure fn, unit
      tested; the existing `copy_closes_on_wayland_and_lingers_on_x11`
      test flips to the new table)
- [ ] `handle_copy`'s doc comment is rewritten to the probe evidence:
      compositor adoption is not guaranteed (COSMIC), so the serve
      thread must outlive the user's paste — the window lingers
- [ ] Worker Copy keeps the window open and shows the Copied state
      (headless: worker EditorApp, copy outcome armed, no Close
      command issued)
- [ ] Esc / window-X after a copy still reports `CopiedToClipboard`
      (existing pre-publish semantics; existing test stays green)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` (quiet)
- [ ] `cargo fmt --all` && `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: `cargo run -- open <img> --standalone` → Copy → window
      stays, "Copied" shows; `wl-paste --list-types` lists `image/png`;
      paste into another app works

**Dependencies:** Task 4 (Copied state is the lingering worker's visible affordance)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs` (close_after_copy, close_after_copy_window, handle_copy docs, tests)

**Docs:**
- The corrected `handle_copy` doc comment carries the GOTCHA (probe
  evidence + why lingering is the only portable guarantee). The ADR in
  Task 7 records the policy change.

**Estimated scope:** Small: 1 file

**Git Commit:** `git commit -m "fix(editor): worker Copy lingers on Linux — process exit was wiping the clipboard"`
