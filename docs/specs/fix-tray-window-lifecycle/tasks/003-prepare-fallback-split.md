# Task 003: `Prepared` fallback split — timeout ≠ old-daemon `unsupported`

**Description:** A worker whose `PrepareCapture` reply exceeds the
2 s client bound falls back to a second (tray-only) handshake — but
the daemon already `begin_capture`d for the prepare, so the fallback
begins *again* and the worker's single `ReportCaptured` leaves
`hide_depth` at 1: the 30 s tray ghost (spec §Diagnosis A2). Split
the internal `Prepared` outcome: a **timeout** means the daemon
already armed the gate (`ArmedSlow` → `Registered { acked: false }`,
no second handshake, the one owed `ReportCaptured` releases the
gate); only an explicit `unsupported` error reply (a known-old daemon
that never began) keeps `Fallback` → legacy tray-only hide. Spec
AC 5.

**Acceptance criteria:**
- [ ] RED first: fake-server test — a server that accepts the `PrepareCapture` connection and stays silent past the bound yields `PrepareCaptureOutcome::Registered { acked: false }` AND receives **no second connection** within a bounded observation window (current code sends `HideTrayForCapture` on a fresh connection → RED)
- [ ] `Prepared` gains the `ArmedSlow` variant; the timeout arm maps to it; the `Err(_)` timeout arm no longer routes through `hide_tray_for_capture_to`
- [ ] `unsupported` reply still maps to `Fallback` → legacy tray-only hide sent (existing `prepare_capture_falls_back_to_tray_on_unsupported` stays green; un-ignore it if it proves runnable headless, keeping the run instruction otherwise)
- [ ] Absent-socket `NoDaemon` semantics unchanged (`prepare_capture_absent_socket_reports_no_daemon`)
- [ ] `Registered { acked: false }` still owes exactly one `ReportCaptured` (document on the variant; no code change — the caller's guard is unchanged)

**Verification:**
- [ ] New silent-server test green: `cargo test -p flintshot-app -q prepare` (plus `-- --ignored` if the socket tests carry the ignore attr; try headless first — local sockets need no display)
- [ ] Full gate (four commands, quiet)
- [ ] Manual check deferred to the manual matrix (slow-prepare tray return within ~1–2 s)

**Verification note:** mirror the fake-server pattern from
`prepare_capture_falls_back_to_tray_on_unsupported`
(tempdir + `interprocess` tokio listener + spawn_blocking client +
socket-exists bridge). Assert "no second connection" with a bounded
`tokio::time::timeout` around a second `listener.accept()` that must
elapse — never an unbounded wait.

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-app/src/ipc_client.rs` (`Prepared` enum, timeout arm, tests)

**Docs:**
- The `ArmedSlow` variant's doc comment carries the why (server began
  → second begin leaks hide_depth → 30 s ghost).

**Estimated scope:** Medium: 1 file (test-heavy)
