# Task 002: Portal-first determinism

## Outcome (diagnosis, 2026-09-16)

**Verdict: stale-build / pre-routing path — no probe fault, no code
change** (the task's "build was stale" branch).

Evidence on the dev session (Wayland, COSMIC):

- `cargo run -- backends` → `session: wayland`, `portal: available
  (XDG Screenshot Portal)`, `xcap: available`, `auto selects:
  portal` — 5/5 consecutive runs identical (no transient D-Bus
  flakiness; the probe's zbus blocking connect + name-owner check +
  introspection is deterministic here).
- User config: `[capture] backend = "auto"` — no pin; auto walks
  `["portal", "xcap"]` and resolves portal.
- Timeline: the editor *page*'s `+` capture riding the in-process
  portal-first grab (`grab_for_app_window` → `grab_with_backend`'s
  `supports_selection()` arm) is 12 h old at spec time (a929437,
  2026-09-15 23:18). The observation quotes the popup's old
  Region/Fullscreen entries — a build that predates (or bypasses)
  that routing. Any `+` capture from such a build rode the worker
  recapture loop (`recapture_image` → `capture_prepared("recapture")`
  → `monitor_grab_prepared` in `cli.rs`), which **always**
  monitor-grabs regardless of portal availability — byte-for-byte
  the observed flow (monitor grab → editor with Select pre-armed).
- That worker recapture behavior still exists today by design for
  worker editor windows (spec: worker path unchanged this effort);
  the app window's page `+` (post-Task-001 popup Capture) rides the
  portal-first in-process grab.

The stderr `Ignoring a wl_output with version < 4` spam during
`backends` is libwayland noise from xcap's enumeration (probe is
bus-only; it never enumerates) — cosmetic, not a probe failure.

Manual residue for the PR checklist: `cargo run -- app` → editor `+`
→ Capture opens the compositor's selection UI, with `capture backend
resolved backend=portal` in the log.

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
