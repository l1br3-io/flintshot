# Task 003: Window-side toggle resolution — `HomeDirective::Quit` + `AppToggle` handling

**Description:** Make the app window resolve a forwarded
`Event::AppToggle` from its own surface state. Visible → a new
`HomeDirective::Quit` closes the viewport with X-close semantics
(process exits, daemon unaffected, mid-editor session discards —
identical to the window's X / Alt+F4). Capture-hidden → the existing
show path reopens it. The `Quit` variant mirrors `Hide`'s plumbing:
a `quit_requested` flag on `SharedInner` armed by
`push_directive`, translated to `ViewportCommand::Close` by both
directive-consuming hosts (home-route `window.rs`, editor-route
`app.rs`), classified by `close_reason` as a user quit — ordered
after pending-capture and `hide_requested` (a capture in progress
owns the close).

**Acceptance criteria:**
- [ ] `HomeDirective::Quit` added in `editor-egui/src/home.rs`
      (re-exported via `lib.rs`); both host consumers
      (`window.rs` ~line 368, `app.rs` ~line 1871) translate it to
      `ViewportCommand::Close`, headless-pinned the `Hide` way
      (assert a `Close` command was queued)
- [ ] `SharedInner` gains `quit_requested`;
      `push_directive(Quit)` arms it and queues the directive
      (mirror of `Hide`'s arm; `Show` drains any stale `Quit` like
      it drains `Hide`)
- [ ] `close_reason` ordering: pending capture > `hide_requested` >
      `quit_requested` > user quit — pins for all four, including
      "quit armed while a capture intent also armed → capture wins"
      (the `close_reason_capture_beats_hide` precedent) and "quit
      taken exactly once" (the `hide_request_is_taken_exactly_once`
      precedent)
- [ ] `HomeCore::on_event(IpcEvent::AppToggle)`: visible →
      `Some(HomeDirective::Quit)`; capture-hidden → the
      `app_focus()` result (Show when idle; suppressed while a
      capture is in flight or supersede-pending — same gate as
      `AppFocus`)
- [ ] The outer loop treats a quit-classified close as the user
      quit (break/exit, no reopen wait) — verified by the existing
      `CloseReason::UserQuit` path plus a classification test
- [ ] `flintshot-editor-egui` doc tests + headless tests green

**Verification:**
- [ ] `cargo test -p flintshot-app -p flintshot-editor-egui --all-targets` green
- [ ] `cargo test --doc -p flintshot-editor-egui` green
- [ ] `cargo build` green

**Dependencies:** Task 1 (the IPC variant exists); independent of Task 2

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs`
- `crates/flintshot-editor-egui/src/window.rs`
- `crates/flintshot-editor-egui/src/app.rs`
- `crates/flintshot-app/src/app_window.rs`

**Docs:**
- Why-comment on the ordering rule (capture beats hide beats quit)
  next to `close_reason`.

**Estimated scope:** Medium: 4 files
