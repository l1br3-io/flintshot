# Task 005: L5 runbook + spec closeout

**Description:** CI cannot simulate bus-daemon activation, so the
live proof is a maintainer runbook — a "L5 runbook —
fix-tray-watcher-resurrect" block in `docs/TEST_MATRIX.md` next to
the Task 3 rows, mirroring the fix-tray-sni runbook precedent
(commit `dcaca59`). One-sitting protocol from the spec's Commands
section: baseline probe (watcher owned, icon visible), kill the
tray host (`pkill -f cosmic-applet-status-area` or `systemctl --user
stop …`) with the daemon left running → icon returns within ~30 s;
fresh daemon on a watcher-dead session → icon appears at startup;
healthy-session regression row (one probe per interval, no log spam
above debug, no extra processes). Includes the OQ2 check: after the
run, verify the COSMIC watcher stays resident (no idle-exit spawn
cycle). Spec closeout: confirm `todo.md` fully checked, note any
deviations from the plan in the spec's Open Questions (OQ1 resolved
fire-and-forget; OQ2 answered by the run's observation), and leave
`spec.md` at `active` for the merge commit to flip to `shipped`.

**Acceptance criteria:**

- [x] Runbook block in TEST_MATRIX: numbered steps, exact commands,
      per-step expected outcome (incl. the
      `busctl --user get-property … RegisteredStatusNotifierItems`
      assertion listing `tray_app_<pid>_1`), covering exit criteria
      1–3 plus the OQ2 residency check
- [x] X11 sanity note: watchdog is additive there (no activatable
      COSMIC name; failed standard poke logs at debug and life goes
      on) — one row so a regression can't hide
- [x] Runbook cites the code commit hash (Task 2's) for
      `PASS <commit> <date>` bookkeeping
- [x] `todo.md` all-checked; spec Open Questions annotated with
      resolutions; `spec.md` status left `active` (merge flips it)

**Verification:**

- [x] Maintainer executes the runbook on L5 in one sitting; results
      land in the Task 3 rows as `PASS <commit> <YYYY-MM-MM-DD>`
- [x] Any FAIL → issue per TEST_MATRIX's template; fix-forward only
      after re-planning, not silently

**Dependencies:** Tasks 1–4

**Files likely touched:**

- `docs/TEST_MATRIX.md` (runbook block)
- `docs/specs/fix-tray-watcher-resurrect/{spec.md,todo.md}`
  (closeout annotations)

**Docs:**

- This task *is* docs.

**Estimated scope:** Small: 3 files
