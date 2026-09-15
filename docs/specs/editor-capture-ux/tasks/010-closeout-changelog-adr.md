# Task 010: Close-out — CHANGELOG, ADR, final gate

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
