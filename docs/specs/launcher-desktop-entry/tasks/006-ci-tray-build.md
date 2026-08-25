# Task 006: CI `tray-build` job (compile the tray feature path)

**Description:** Nothing in CI compiles `flintshot-app` with
`--features tray`, so the tray code path (tray-icon + gtk) can rot
silently — the stale comment in `flintshot-app/Cargo.toml` even
references a daemon-build CI job that no longer exists. Add a small
`tray-build` job to `.github/workflows/ci.yml` that installs the GTK
stack and runs clippy with the feature enabled (compiles + lints in
one step, same `-D warnings` bar as the main lint job). Fix the
stale Cargo.toml comment to point at the real job.

**Acceptance criteria:**
- [ ] New `tray-build` job in `.github/workflows/ci.yml`: installs
      the xcap Linux deps (`XCAP_LINUX_DEPS` env already in the
      file) **plus** `libgtk-3-dev libayatana-appindicator3-dev`,
      then runs `cargo clippy -p flintshot-app --features tray
      --all-targets -- -D warnings`
- [ ] Job does not gate `flake8`-style extras: no test run, no
      workspace-wide scope — just the tray-feature compile/lint
- [ ] `crates/flintshot-app/Cargo.toml` tray-feature comment updated
      to reference the `tray-build` CI job (remove the stale
      "Task 012's dedicated daemon-build CI job" reference)
- [ ] AGENTS.md Linux-deps paragraph mentions that CI also exercises
      the tray feature in `tray-build` (one line, keep it accurate)
- [ ] CI YAML passes `actionlint` if available locally, else careful
      eyeball + push shows the job green

**Verification:**
- [ ] Local approximation:
      `cargo clippy -p flintshot-app --features tray --all-targets -- -D warnings`
      green on this machine (GTK dev libs present)
- [ ] `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml'))"`
      (or equivalent) — YAML parses
- [ ] Push branch → `tray-build` job appears and passes on GitHub

**Dependencies:** None (independent of Task 005; both land before
Task 007).

**Files likely touched:**
- `.github/workflows/ci.yml`
- `crates/flintshot-app/Cargo.toml` (comment only)
- `AGENTS.md` (one line)
- `CHANGELOG.md` (CI note)

**Docs:**
- None beyond the comment fixes above.

**Estimated scope:** Small: 4 files (one job + three one-liners)
