# Task 005: ADR-0067 + CHANGELOG + finish gates

**Description:** Record the load-bearing decision and close out:
ADR-0067 (presence-gated forward-or-spawn for the toggle, and why
not broadcast-always — the resurrection race), a `CHANGELOG.md`
entry under `## [Unreleased]`, and the four finish gates from
`AGENTS.md`.

**Acceptance criteria:**
- [ ] `docs/adrs/ADR-0067-<slug>.md` exists in **Context ·
      Decision · Consequences** form: broadcast+always-spawn would
      race the exiting window's app-lock release; presence oracle
      (`CaptureDispatch::app_window_present`, ADR-0052) gates
      forward-vs-spawn on the `dispatch_open_blocking` seam;
      consequences include the known asymmetry (a future hotkey
      path through plain `dispatch` would be spawn-only) and the
      benign lost-click race when the window dies mid-forward
- [ ] `CHANGELOG.md` has the effort's entry under
      `## [Unreleased]` (user-visible: tray item swap)
- [ ] `cargo fmt --all --check` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] `cargo test --workspace --all-targets` green
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui` green

**Verification:**
- [ ] All four gates above, run in order, green in one session

**Dependencies:** Tasks 1–4 complete

**Files likely touched:**
- `docs/adrs/ADR-0067-presence-gated-window-toggle.md`
- `CHANGELOG.md`

**Docs:**
- The ADR *is* the doc.

**Estimated scope:** Small: 2 files
