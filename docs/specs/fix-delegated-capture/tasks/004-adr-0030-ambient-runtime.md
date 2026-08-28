# Task 004: ADR-0030 ambient runtime for window workers

**Description:** Write `docs/adrs/ADR-0030-ambient-runtime-for-window-workers.md`
in the established **Context · Decision · Consequences** format
(numbered, matches `ls docs/adrs/` → next free number). Records why
window workers carry an ambient tokio runtime (zbus 5 via
accesskit/at-spi requires one; Phase 6 unified zbus to 5.18 for
ashpd), the runtime-per-short-lived-worker trade-off, why the guard
wraps only the `EguiShell` call (drops before ipc_client's one-shot
runtimes → no nesting), and why zbus 4 / async-io downgrades were
rejected (workspace `Cargo.toml` pins ashpd tokio deliberately).

**Acceptance criteria:**

- [x] ADR file exists at `docs/adrs/ADR-0030-ambient-runtime-for-window-workers.md`
- [x] Sections cover: context, decision, consequences (incl. cost,
      alternatives rejected, link to TEST_MATRIX finding 2)
- [x] `docs/adrs/README.md` index updated if that's the established
      pattern (check first; skip if ADRs aren't indexed there)

**Verification:**

- [x] Format matches a sampled recent ADR (e.g. ADR-0029)
- [x] Claims consistent with the code landed in Task 2

**Dependencies:** Task 2 (records its decision)

**Files likely touched:**

- `docs/adrs/ADR-0030-ambient-runtime-for-window-workers.md` (new)
- `docs/adrs/README.md` (only if it indexes ADRs)

**Docs:**

- This task *is* docs.

**Estimated scope:** Small: 1–2 files
