# Task 002: Portal-first determinism

**Description:** The user observed the `+` capture resolving to the
xcap fallback flow (monitor grab → editor with Select pre-armed),
which means `grab_with_backend` took the `!supports_selection()` arm
— either the portal availability probe failed at grab time or the
tested build predated the in-process grab (commit a929437).
Diagnose on the dev session, fix whatever is found, and pin the
expected behavior. Auto resolution already prefers portal on
Wayland (`candidate_ids`); the fix, if any, is in probe robustness
or its call sites — not in reordering precedence.

**Acceptance criteria:**
- [ ] Diagnosis recorded (in the task file's outcome or the PR
      description): `cargo run -- backends` output on the dev
      session + the `capture backend resolved` log line from a live
      `+` capture
- [ ] If the probe is at fault (e.g. a transient D-Bus race under
      `zbus` blocking connection): fixed with the minimal robust
      change, unit-tested where headless-possible
- [ ] If the build was stale: the behavioral verification below is
      the deliverable — no code change
- [ ] On the dev Wayland session, `+` → Capture opens the
      compositor's own selection UI; the fallback editor flow runs
      only when the portal probe genuinely fails

**Verification:**
- [ ] `cargo run -- backends` shows portal available + auto selects
      portal on the dev session
- [ ] Manual: `+` → Capture opens the OS selection overlay; log
      shows `backend=portal`
- [ ] Full gate green (see Task 001's gate commands)

**Dependencies:** None (independent of Task 001; both are Phase 1)

**Files likely touched:**
- `crates/flintshot-core/src/capture/portal.rs` (probe fix, if
  any)
- `crates/flintshot-core/src/capture/mod.rs` (selection tests, if
  any)

**Docs:**
- GOTCHA discovered during diagnosis → inline `// GOTCHA` comment
  at the probe site.

**Estimated scope:** Small: 0–2 files
