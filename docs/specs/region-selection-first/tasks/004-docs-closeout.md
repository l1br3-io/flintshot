# Task 004 — Docs: ADR-0035, CHANGELOG, TEST_MATRIX, spec closeout

> Status: complete
> Effort: [`region-selection-first`](../spec.md) · Plan decision 6

## Description

One docs task, per plan decision 6:

- `docs/adrs/ADR-0035-selection-first-editor-fallback.md` — the
  selection-first decision, revising ADR-0028's Wayland consequence
  ("Wayland users get the editor window until a layer-shell effort
  exists" → they get the spotlight in the editor window). Records
  the answered fullscreen-demotion question and the deferred
  opt-out escape hatch in its Open Questions section. The ADR index
  (`docs/adrs/README.md`) also back-fills the missing ADR-0033 /
  ADR-0034 rows.
- `CHANGELOG.md` `[Unreleased]` → **Changed**: `region` lands
  selection-first on every editor-shell path (default editor mode,
  `--editor`, Wayland `overlay` demotion); phase ends on first
  crop; Esc-cancel contract; selection-aware Save while live;
  fullscreen / open / true overlay unregressed.
- `docs/TEST_MATRIX.md`: new `region-selection-first` section —
  test script + matrix rows for the Wayland region flow, the
  X11/editor-mode region flow, and the fullscreen / open / overlay
  unregression, plus known caveats (mask-only chrome, decorated
  window, straddling-annotation parity, `[ui]` persistence on
  cancel). Cells left blank pending maintainer passes.
- `spec.md` frontmatter: `status: draft` → `shipped` with
  `shipped: 2026-08-31` (fast-forward merges skip the lifecycle
  flip; repo precedent does it in the closeout commit — see
  `df91c61`).

## Acceptance criteria

1. ADR-0035 exists, follows Context · Decision · Consequences,
   references ADR-0028, and records both open-question outcomes.
2. The ADR index lists every ADR file in `docs/adrs/` (0001–0035).
3. CHANGELOG `[Unreleased]` describes the user-visible change.
4. TEST_MATRIX has the effort's rows covering exit criteria 1–5's
   manual surface.
5. Spec frontmatter says `status: shipped`.
6. Standing gates green: `fmt`, `clippy -D warnings`, workspace
   tests, doc tests.

## Verification

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

Docs-only change; no code paths touched.
