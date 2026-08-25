# Launcher desktop entry — task status

- [x] 001 — Assets: SVG icon + `flintshot.desktop` + autostart variant
- [x] 002 — Validation test: parse grammar + `Exec=` ↔ CLI sync
- [x] 003 — `just install` / `install-user` / `uninstall` recipes
- [x] 004 — Docs: CHANGELOG, TEST_MATRIX, final gate run

## Exit-criteria sweep (task 004 closeout, 2026-08-25)

| #  | Criterion                                                        | Status |
| -- | ---------------------------------------------------------------- | ------ |
| 1  | Launcher lists "Flintshot" + icon; activation starts daemon + tray; re-activation harmless | manual-pending — TEST_MATRIX rows 1–5. Caveat found at closeout: `just install` builds the default **no-tray** binary (`tray` is opt-in, Phase 2); tray rows need a `--features tray` build (workaround in the matrix setup). `FEATURES=` recipe passthrough flagged as follow-up. |
| 2  | Context-menu actions trigger capture without a terminal          | manual-pending — TEST_MATRIX rows 6–7 |
| 3  | `AUTOSTART=1 just install-user` + re-login → daemon + tray        | manual-pending — TEST_MATRIX rows 8–9 |
| 4  | `just uninstall` removes everything; reinstall idempotent         | manual-pending — TEST_MATRIX rows 10–11 (recipe mechanics + graceful tool-skips exercised in task 003; no desktop-session pass yet) |
| 5  | Validation test fails on bogus `Exec=`                           | verified — mutation re-run at closeout: `Exec=flintshot region` → `bogus-region` failed `exec_targets_resolve_to_real_cli_subcommands` naming the drift; reverted clean (`git diff` empty) |
| 6  | Standing gates green                                             | verified — `fmt --check`, `clippy --workspace --all-targets -D warnings`, `test --workspace --all-targets` (142+5 ok), doc tests, `lint-frontmatter.py` all green at closeout |

ADR check: none needed — all four decisions (daemon-is-the-app,
autostart opt-in, test-not-linter validation, SVG-only icon) stayed
minor and are recorded in the spec's Decisions section; nothing grew
architectural during implementation.
