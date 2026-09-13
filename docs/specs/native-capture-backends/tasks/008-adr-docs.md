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

**Outcome (executed 2026-09-13):** ADR-0050 written (Context ·
Decision · Consequences, spike findings cited, supersedes ADR-0003
via a back-link; indexed in `docs/adrs/README.md`).
ARCHITECTURE.md: `backends` in the subcommand list, `backend =`
in the config sample, a new capture section (trait + registry +
selection rules + the path-scoped zbus rule), tech-stack row, and
the Wayland bullet rewritten around auto→portal. TEST_MATRIX.md:
"Capture backends" section (7-row matrix incl. the wedged-portal
and cursor-gap rows; notes the `#[ignore]` live test).
AGENTS.md: `cargo run -- backends` + the capture-registry row in
"Where things live". CHANGELOG entries landed with tasks 4/6/7.
spec.md reconciled: gnome-shell examples/rows/structure/illustration
removed, ashpd→zbus corrections, seam names updated
(`select_backend_with`/`backend_report_with`), no-GUI resolution
recorded. check-refs green except the pre-existing
`launcher-desktop-entry` ref; frontmatter lint green. Frontmatter
`status` untouched (flips at merge).
