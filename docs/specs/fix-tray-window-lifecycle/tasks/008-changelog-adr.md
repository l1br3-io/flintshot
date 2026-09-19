# Task 008: CHANGELOG `[Unreleased]` + ADR-0066

**Description:** Record the effort: CHANGELOG entries under
`## [Unreleased]` (house style: bold lead sentence + mechanism +
pinned-by test refs) and ADR-0066 (window-as-liveness-anchor) in
`docs/adrs/` following the **Context · Decision · Consequences**
format of ADR-0065. Check `docs/adrs/README.md` for an index to
update if present.

**Acceptance criteria:**
- [ ] CHANGELOG `[Unreleased]`: Fixed — worker-editor Settings closes behind the handoff (no duplicate window); no-daemon fallback spawns carry `--standalone`; slow-prepare no longer double-begins the tray gate (30 s ghost); helper respawn churn backs off + truthful presence. Changed/Added — the app window self-heals a dead daemon (reconnect + rate-limited respawn + presence re-announce). Each entry names its pinning test(s)
- [ ] ADR-0066 records: daemon death was unrecoverable by design; decision — the app window is the liveness anchor (reconnect w/ capped backoff, rate-limited respawn, re-announce), supervisor/helper backoff, timeout≠unsupported split; consequences — no terminal degraded mode, bounded waits, manual-matrix dependence
- [ ] `docs/adrs/README.md` index updated if it lists ADRs
- [ ] Frontmatter check: `.opencode/harness/scripts/lint-frontmatter.py` passes (if wired); no spec-status flip here (status flips on merge per the lifecycle)

**Verification:**
- [ ] Full gate (four commands, quiet) — docs-only change must not break them
- [ ] Manual check: links/refs in CHANGELOG + ADR resolve (`python3 .opencode/harness/scripts/check-refs.py` if wired)

**Dependencies:** Tasks 001–007 (records their outcomes)

**Files likely touched:**
- `CHANGELOG.md`
- `docs/adrs/ADR-0066-app-window-liveness-anchor.md`
- `docs/adrs/README.md` (index, if maintained)

**Docs:**
- This task *is* docs.

**Estimated scope:** Small: 2–3 files
