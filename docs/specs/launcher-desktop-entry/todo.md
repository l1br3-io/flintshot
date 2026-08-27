# Launcher desktop entry — task status

- [x] 001 — Assets: SVG icon + `flintshot.desktop` + autostart variant
- [x] 002 — Validation test: parse grammar + `Exec=` ↔ CLI sync
- [x] 003 — `just install` / `install-user` / `uninstall` recipes
- [x] 004 — Docs: CHANGELOG, TEST_MATRIX, final gate run
- [x] 005 — Tray-default install builds (`FEATURES=` passthrough)
- [x] 006 — CI `tray-build` job (compiles tray feature path)
- [ ] 007 — Manual TEST_MATRIX pass + exit-criteria sweep
      (**blocked 2026-08-28** — L5 pass surfaced three pre-existing
      product bugs that fail exit criteria 1–2; see TEST_MATRIX
      "L5 pass findings". Rows 1/3/5/8/10/11/12 PASS on L5; rows
      4/6/7 FAIL; rows 2/9 still need the user's eyes. Re-run after
      the bugs are fixed in a follow-up effort.)

## Exit-criteria sweep (task 004 closeout, 2026-08-25; reopened by revision; task 007 run 2026-08-28)

| #  | Criterion                                                        | Status |
| -- | ---------------------------------------------------------------- | ------ |
| 1  | Launcher lists "Flintshot" + icon; activation starts daemon + tray; re-activation harmless | **partial-fail (L5, 2026-08-28)** — activation + re-activation PASS headlessly (rows 3/5); launcher-listing row 2 needs a visual check; **tray row 4 FAILs** (SNI item never registers with COSMIC's watcher). Blocker is a pre-existing tray bug, not install mechanics |
| 2  | Context-menu actions trigger capture without a terminal          | **fail (L5, 2026-08-28)** — rows 6/7: delegated captures fork-loop (~60–100 worker spawns/s), no editor opens; standalone fallback panics in zbus 5 (at-spi path). Both pre-existing product bugs (Phase 2 delegation design + Phase 6 zbus unification); see TEST_MATRIX "L5 pass findings" |
| 3  | `AUTOSTART=1 just install-user` + re-login → daemon + tray        | file mechanics PASS (row 8, 2026-08-28); re-login sweep deferred until 1–2 are fixed |
| 4  | `just uninstall` removes everything; reinstall idempotent         | verified (L5, 2026-08-28, rows 10–11) |
| 5  | Validation test fails on bogus `Exec=`                           | verified — mutation re-run at closeout: `Exec=flintshot region` → `bogus-region` failed `exec_targets_resolve_to_real_cli_subcommands` naming the drift; reverted clean (`git diff` empty) |
| 6  | Standing gates green                                             | verified — `fmt --check`, `clippy --workspace --all-targets -D warnings`, `test --workspace --all-targets` (142+5 ok), doc tests, `lint-frontmatter.py` all green at closeout |

ADR check: none needed — all four decisions (daemon-is-the-app,
autostart opt-in, test-not-linter validation, SVG-only icon) stayed
minor and are recorded in the spec's Decisions section; nothing grew
architectural during implementation. Revision decisions #5 (tray
recipe default) and #6 (CI tray job) likewise spec-recorded.

Task 007 disposition (2026-08-28): the manual pass ran on L5 and is
recorded honestly in TEST_MATRIX. The effort cannot ship its core
promise until the three pre-existing bugs (delegation fork loop,
zbus-5/at-spi editor crash, COSMIC SNI registration) are fixed —
recommended as a follow-up effort (`fix-delegated-capture` or
similar); fixing them here would violate this spec's "ask first:
touching flintshot-app code" boundary.
