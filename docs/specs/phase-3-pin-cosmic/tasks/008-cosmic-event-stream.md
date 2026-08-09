# Task 8: COSMIC event stream + daemon-absent state

**Description:** Add the live feedback layer to the COSMIC popup: a
persistent `Subscribe` connection (Task 6's `subscribe()`) that reads
the daemon's `Event` stream and renders the latest status line in the
popup (*Idle* / *Capturing…* / *Editor opened* / *Saved <path>* /
*Pinned #N* / error). Plus the daemon-absent state: when
`probe_daemon()` returns `NotRunning` (or the stream drops), the popup
shows a clear *"daemon not running"* state with a *Start daemon* hint
(it does **not** auto-start — out of scope). This is what makes the
applet feel native instead of "fire-and-forget buttons."

**Acceptance criteria:**
- [ ] A `Subscribe` connection is maintained for the applet's lifetime
      (Task 6's `subscribe()`); incoming `Event` frames update a shared
      status state that the popup renders each frame.
- [ ] Status line mapping: `CaptureStarted` → *"Capturing…"*;
      `EditorOpened` → *"Editor opened"*; `Saved { path }` →
      *"Saved <basename>"*; `Pinned { id }` → *"Pinned #<id>"*;
      `Uploaded { url }` → *"Uploaded <url>"* (Phase 4, but render it
      gracefully); `Error { code, message }` → *"Error: <message>"*
      (styled distinctly). Idle (no events yet) → *"Idle"*.
- [ ] `Lagged` handling: log + skip (mirror `ipc_server.rs:296`), no
      panic, status unchanged.
- [ ] Daemon-absent: on startup, `probe_daemon()`; if `NotRunning`, the
      popup shows *"daemon not running — start with `flintshot daemon`"*
      in place of the status line, and the action buttons are disabled
      (or send-no-op with a tooltip). On stream disconnect mid-session,
      transition to the same absent state.
- [ ] Reconnect: if the stream drops, periodically re-probe
      (configurable interval, e.g. 2s) and return to the live state when
      the daemon reappears — so restarting the daemon doesn't require
      restarting the applet.
- [ ] The `tokio` task driving the subscribe loop is spawned on the
      applet's runtime (libcosmic uses tokio; the `tokio` feature is
      already on in `Cargo.toml:22`); events cross to the UI thread via
      a channel (libcosmic's message type).

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy -p flintshot-cosmic -- -D warnings`
- [ ] `cargo build -p flintshot-cosmic` green on Linux.
- [ ] `cargo test -p flintshot-cosmic` — incl. a status-mapping unit
      test (given each `Event`, the status string is correct — pure fn,
      no UI); a reconnect-state test (stream drops → absent → re-probe
      succeeds → live).
- [ ] Manual: `flintshot daemon` + `flintshot-cosmic`; popup shows
      *Idle*; trigger a capture from the popup → status goes *Capturing…*
      → *Editor opened* → save → *Saved <basename>*. Stop the daemon
      (`flintshot daemon --replace` or Quit) → popup shows *"daemon not
      running"*; restart daemon → popup returns to live within ~2s.

**Dependencies:** Task 6 (the `subscribe()` / `probe_daemon()` client)
+ Task 7 (the popup shell to render into).

**Files likely touched:**
- `crates/flintshot-cosmic/src/app.rs` — status state + render; daemon-absent UI; reconnect logic.
- `crates/flintshot-cosmic/src/ipc.rs` — possibly a `status_for_event(event) -> String` pure helper (testable headless).
- `crates/flintshot-cosmic/Cargo.toml` — only if a new dep is needed (shouldn't be; `tokio` already present).

**Docs:**
- `CHANGELOG.md` `[Unreleased]` → Added: COSMIC applet live event
  stream + daemon-absent state.
- If the reconnect/absent UX has a non-obvious gotcha (e.g.
  libcosmic's message-passing quirk), a `// GOTCHA:` comment at the
  site.

**Estimated scope:** Medium (2–3 files; the status mapping + reconnect logic is the substantive part; UI rendering builds on Task 7's popup).
