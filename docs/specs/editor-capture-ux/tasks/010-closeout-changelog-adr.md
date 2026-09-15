# Task 010: Close-out — CHANGELOG, ADR, final gate

## Outcome (2026-09-16)

- CHANGELOG `## [Unreleased]` rewritten for the effort (routing,
  stable editor, progress indicator, popup parity + strip removal,
  portal-first verification).
- ADR-0054 (`docs/adrs/ADR-0054-in-window-routing.md`) + index row;
  `docs/ARCHITECTURE.md` §single-window refreshed (routes in place,
  destruction reserved for captures, loading page, stable fit).
- Final full gate green: `fmt --check`, `clippy -D warnings`,
  855 workspace tests, doc tests.

## Spec §Success Criteria walkthrough

1. **`+` popup exactly Capture/Open…/Paste** — unit-tested
   (`plus_popup_*` mapping + worker-mode variant + no-variant test);
   labels/hints manual-pending.
2. **Portal-first on Wayland** — `backends` 5/5 portal +
   auto-selects-portal on the dev session (task 002's diagnosis);
   the live `+` → OS-overlay + `backend=portal` log line is the
   recorded manual residue for the PR checklist.
3. **Stable editor size, fitted/centered, scale-correct
   annotations** — `fit_scale` / round-trip / crop-refit / fit
   geometry unit tests; visual smoke (annotate, magnifier,
   move-chrome, selection phase) manual-pending.
4. **Transitions keep the window** — route state machine + no-close
   contracts + transition batch unit tests; WM-identity (taskbar /
   pinning) observation manual-pending.
5. **Post-confirm progress ≤ ~200 ms, in-place image swap, no
   stranded spinner** — delivery state machine unit tests
   (deliver-to-open, fast read-back, supersede folding,
   failure-while-loading, quiet exits); the perception budget is
   inherently manual.
6. **Full gate green** — verified above.

**Description:** Record the effort. CHANGELOG entries under
`## [Unreleased]` for every user-visible change (popup parity,
portal-first verification result, stable editor size + fit-to-view,
in-window routing, capture progress indicator, chrome strip if
removed). An ADR for the routing decision — numbered next after
ADR-0053, format **Context · Decision · Consequences** — covering:
one eframe lifetime per window generation, routes not epochs,
destruction reserved for captures (ADR-0047 constraint preserved),
and the fit-to-view enabler. Refresh `docs/ARCHITECTURE.md` only if
its window-lifecycle description is now stale. Final full gate.

**Acceptance criteria:**
- [ ] `CHANGELOG.md` updated under `## [Unreleased]` covering all
      shipped tasks
- [ ] ADR-0054 (routing) written in `docs/adrs/` in the
      Context · Decision · Consequences format
- [ ] `docs/ARCHITECTURE.md` window-lifecycle section accurate
- [ ] All spec success criteria checked against the build (spec
      §Success Criteria, items 1–6)
- [ ] Final full gate green:
      `cargo fmt --all --check` &&
      `cargo clippy --workspace --all-targets -- -D warnings` &&
      `cargo test --workspace --all-targets` &&
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`

**Verification:**
- [ ] The gate above, run once, clean
- [ ] Spec §Success Criteria walkthrough — each item verified or
      explicitly waived with the maintainer

**Dependencies:** All prior tasks

**Files likely touched:**
- `CHANGELOG.md`
- `docs/adrs/ADR-0054-in-window-routing.md` (new)
- `docs/ARCHITECTURE.md` (refresh, if stale)

**Docs:**
- This task *is* the docs task.

**Estimated scope:** Small: 3 files
