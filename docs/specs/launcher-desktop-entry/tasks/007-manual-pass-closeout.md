# Task 007: Manual TEST_MATRIX pass + exit-criteria sweep

**Description:** Exit criteria 1–4 require a real desktop session —
the only part of this effort a test can't cover. After Tasks 005–006
land, run the launcher TEST_MATRIX script on both an X11 and a
Wayland session: launcher discovery, activation (tray now included
by default), desktop actions, autostart re-login, uninstall +
idempotent reinstall, and the `FEATURES=""` minimal path. Fill the
matrix cells, resolve the container-based failure-hint check if it
was deferred in Task 005, and update the todo.md exit-criteria
sweep. This task closes the effort.

**Acceptance criteria:**
- [ ] TEST_MATRIX launcher rows 1–11 filled for at least one X11 and
      one Wayland session (L1–L5 / W1–W2 columns as available)
- [ ] Row 4 (tray icon appears) verified against the *default*
      install — no manual `--features tray` rebuild step
- [ ] `FEATURES="" just install-user` minimal path row filled
- [ ] Missing-GTK failure hint verified in a clean container (or
      explicitly deferred with a note in Known caveats)
- [ ] `todo.md` exit-criteria sweep updated: criteria 1–4 move from
      manual-pending to verified (with session names/dates)
- [ ] Spec "Effort status" remaining-work list emptied / annotated
      done
- [ ] Any launcher/DE quirks found are recorded in TEST_MATRIX
      Known caveats

**Verification:**
- [ ] Every matrix cell filled or explicitly marked N/A with reason
- [ ] Standing gates green one final time (`fmt --check`, clippy
      `-D warnings`, workspace tests, doc tests)
- [ ] Ready-for-review declared — `/review` next, then `/ship`
      (merge commit flips spec to `shipped`)

**Dependencies:** Task 005 (tray-default recipes — the matrix tests
the real install path). Task 006 desirable (CI job green before
closeout) but not blocking.

**Files likely touched:**
- `docs/TEST_MATRIX.md` (matrix cells + caveats)
- `docs/specs/launcher-desktop-entry/todo.md` (sweep)
- `docs/specs/launcher-desktop-entry/spec.md` (Effort status section)

**Docs:**
- DE-specific gotchas → TEST_MATRIX Known caveats (they are test
  knowledge, not code knowledge).

**Estimated scope:** Small: 3 files (mostly filling in results)

**Note:** Human-in-the-loop task — needs login sessions (autostart
re-login especially). The agent can drive everything except
re-login and visual launcher checks; pair with the user for those.
