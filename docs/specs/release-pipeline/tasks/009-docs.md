# Task 009: Docs — ADR, AGENTS.md commands, CHANGELOG

**Description:** Record the pipeline decisions durably and make the
next agent's life easy: an ADR for the release architecture
(cargo-dist + custom jobs split, glibc pin, signing deferral), an
AGENTS.md section for the release commands and boundaries, and the
CHANGELOG `## [Unreleased]` entry.

**Acceptance criteria:**
- [ ] `docs/adrs/ADR-0068-release-pipeline.md` — Context · Decision
      · Consequences: cargo-dist-native vs custom-job artifact split
      (+ why: extra-artifacts are arch-independent-only,
      universal-apple-darwin unsupported), ubuntu-22.04 glibc pin,
      signing deferred with the native drop-in hooks named
      (`ssldotcom-windows-sign`, `azure-windows-sign`, Apple
      notarization TODO)
- [ ] `AGENTS.md`: a "Release" row/section in Commands + the
      boundaries (release.yml is generated — regenerate, never
      hand-edit; version bumps only via `just release`)
- [ ] `CHANGELOG.md` `## [Unreleased]` → **Added** entry describing
      the pipeline (artifacts, channels, `just` commands)
- [ ] ADR numbering checked against existing `docs/adrs/` (next free
      NNNN — 0068 assumed, verify)

**Verification:**
- [ ] `just ci` green (docs-only change; the standing gate)
- [ ] `python3 .opencode/harness/scripts/check-refs.py` (if it
      covers docs links) or manual link check of the new ADR

**Dependencies:** Tasks 1-8 (documents what they decided; write
after fallback outcomes are known)

**Files likely touched:**
- `docs/adrs/ADR-0068-release-pipeline.md` (new)
- `AGENTS.md`
- `CHANGELOG.md`

**Docs:** This task *is* docs.

**Estimated scope:** Small (3 files)
