# Task 004 — Docs: ADR-0036, CHANGELOG, TEST_MATRIX, spec closeout

> Status: complete
> Effort: [`hide-tray-for-capture`](../spec.md) · Plan decision 5

## Description

- `docs/adrs/ADR-0036-hide-tray-for-capture.md`: the handshake
  decision (context constraints — worker-process split, main-thread
  GTK, async panel repaint, portal dialogs; rejected alternatives —
  dispatch-site hiding, fixed-delay restore, SNI re-registration,
  whole-session hiding), the bounds and their degradation contract,
  and the two open questions (per-platform grace, `--delay-ms`).
  ADR index row added.
- `CHANGELOG.md` `[Unreleased]` → **Fixed**: the icon-in-shot bug,
  the mechanism in one paragraph, the latency cost stated plainly.
- `docs/TEST_MATRIX.md`: new effort section — 11 rows across
  trigger sources, cancel/failure/dead-worker paths, standalone +
  no-tray-feature daemons, and the X11 registration regression;
  caveats name the ~250–400 ms grace cost and the 30 s ghost window.
  Cells blank pending maintainer passes.
- Spec frontmatter: `status: draft` → `shipped`
  (`shipped: 2026-09-01`), per the `df91c61` precedent.

## Acceptance criteria

1. ADR-0036 exists, follows Context · Decision · Consequences,
   references ADR-0018/0019/0031/0032 where it leans on them, and
   records both open questions.
2. The ADR index lists ADR-0036.
3. CHANGELOG `[Unreleased]` describes the fix user-visibly.
4. TEST_MATRIX covers spec exit criteria 1–6's manual surface.
5. Spec frontmatter says `status: shipped`.
6. Standing gates green in both feature configs.

## Verification

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p flintshot-app --features tray --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

Docs-only change; no code paths touched.
