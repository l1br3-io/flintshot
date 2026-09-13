# Task 008: ADR + docs

**Description:** Record the effort's decisions and update the
long-lived docs: a new ADR superseding-extends ADR-0003 (`xcap` for
capture → `xcap` as default provider behind the `Capturer` trait,
zbus-in-`src/capture/` exception with the ashpd ban preserved, spike
findings cited), plus ARCHITECTURE/CHANGELOG/TEST_MATRIX/AGENTS
updates. Spec gets a final consistency pass against what actually
shipped (precedence table vs spike verdicts, tech notes vs reality).

**Acceptance criteria:**
- [ ] `docs/adrs/ADR-00NN-capturer-trait-native-backends.md` written
      (Context · Decision · Consequences), numbered next in sequence,
      linked from ADR-0003 and the ADR index
- [ ] `docs/ARCHITECTURE.md`: capture section describes the trait +
      backends + selection rules; tech-stack table row updated;
      cross-platform notes reflect spike findings
- [ ] `CHANGELOG.md` under `## [Unreleased]`: user-visible entries
      (new `--backend` flag, `backends` subcommand, `[capture]
      backend` config, native Wayland path)
- [ ] `docs/TEST_MATRIX.md`: backends × session-kind section added
- [ ] `AGENTS.md` "Where things live" capture row points at
      `crates/flintshot-core/src/capture/`
- [ ] `spec.md` reconciled with shipped reality (no stale claims);
      frontmatter untouched (`status` flips at merge, not here)

**Verification:**
- [ ] `python3 .opencode/harness/scripts/check-refs.py` green (links
      resolve) and `lint-frontmatter.py` green
- [ ] Human reviews the ADR

**Dependencies:** Tasks 1-7 (records what shipped)

**Files likely touched:**
- `docs/adrs/ADR-00NN-capturer-trait-native-backends.md` (new)
- `docs/adrs/ADR-0003-xcap-for-capture.md` (superseded-by link)
- `docs/adrs/README.md` (index line)
- `docs/ARCHITECTURE.md`, `CHANGELOG.md`, `docs/TEST_MATRIX.md`, `AGENTS.md`,
  `docs/specs/native-capture-backends/spec.md`

**Docs:**
- This task *is* docs.

**Estimated scope:** Medium (5-8 files, docs-only)
