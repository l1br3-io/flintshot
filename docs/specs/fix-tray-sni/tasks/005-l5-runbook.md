# Task 005: L5 runbook for the maintainer + spec closeout

**Description:** Write the exact L5 verification protocol into
TEST_MATRIX (a "L5 runbook — fix-tray-sni" block near finding 3)
so the maintainer can execute it in one sitting and paste results
alongside. The runbook covers, in order: baseline probe
(`GetRegisteredStatusNotifierItems` before daemon start), start
`flintshot daemon` (tray-default build) → probe again → flintshot
item present + icon visible in cosmic-applet-status-area; menu
sweep (Capture region → editor opens once; Capture fullscreen;
per-monitor submenu entry; Bind shortcuts… → portal dialog; Open
config → `$EDITOR`; Quit → daemon exits); restart-ordering check
(kill watcher path or start daemon before the session's watcher →
icon appears when watcher becomes available); X11 session sanity
(icon shows); no-SNI-host sanity (daemon stays alive with the
warning). Also note which SNI interface the watcher consumed
(KDE vs Ayatana object path — spec open question 1). This task
does *not* record a pass — it records the protocol and marks every
row pending.

**Acceptance criteria:**

- [ ] Runbook block exists in TEST_MATRIX with copy-pasteable
      commands (gdbus calls, build command, per-check expected
      outcomes)
- [ ] Every affected row annotated pending-maintainer-L5 (no
      predicted results)
- [ ] Runbook cross-references launcher-desktop-entry task 007
      (rows 4/6/7 re-run happens on that effort after this lands)
- [ ] Spec open question 1 has a runbook step that answers it

**Verification:**

- [ ] A maintainer unfamiliar with the code could execute the
      runbook start-to-finish with only its contents

**Dependencies:** Task 3 (annotates the rows the runbook
completes)

**Files likely touched:**

- `docs/TEST_MATRIX.md`

**Docs:**

- This task *is* docs.

**Estimated scope:** Small: 1 file
