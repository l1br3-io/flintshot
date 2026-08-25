# Task 005: Tray-default install builds (`FEATURES=` passthrough)

**Description:** Make `just install` / `just install-user` build the
release binary with the tray feature enabled by default, per the spec
revision (2026-08-26). `FEATURES=` overrides the set (`FEATURES=""` =
minimal no-tray build, no GTK build deps). When the default tray
build fails because GTK dev libs are missing, print an actionable
hint instead of a bare compiler error. Sync `docs/TEST_MATRIX.md`
(remove the now-obsolete `--features tray` workaround) and add the
CHANGELOG entry.

**Acceptance criteria:**
- [x] `just install` and `just install-user` run
      `cargo build --release --features tray` by default (visible in
      `just -n` dry-run output)
- [x] `FEATURES="" just install-user` builds the minimal binary
      (no `--features` flag passed to cargo at all — avoid the
      `--features ""` edge)
- [x] `FEATURES="tray,egui"` (or any custom set) passes through
      verbatim
- [x] On build failure with `tray` in the feature set, the recipe
      prints a hint naming the GTK dev-lib prerequisite and the
      `FEATURES=""` escape hatch, then exits non-zero; the hint does
      NOT fire when `tray` is absent from the feature set
- [x] Bash parameter expansion uses `${FEATURES-tray}` semantics
      (unset → `tray`; set-but-empty → empty), not `${FEATURES:-tray}`
      (which would make `FEATURES=""` impossible)
- [x] `uninstall` recipe untouched (it never builds)
- [x] TEST_MATRIX launcher section: workaround block removed, row 4
      parenthetical dropped, caveat rewritten, `FEATURES=""` minimal
      row added
- [x] CHANGELOG `[Unreleased]` entry for the recipe behavior change

**Verification:**
- [x] `just -n install-user` dry-run shows the tray-default cargo
      command
- [x] Real run: `just install-user` on this machine (ayatana +
      gtk3 dev libs present) → installed binary is a tray build
      (`~/.local/bin/flintshot daemon` shows tray; or check
      `ldd ~/.local/bin/flintshot | grep appindicator`)
- [x] `FEATURES="" just install-user` succeeds →
      `ldd ~/.local/bin/flintshot | grep -c appindicator` = 0
- [x] Failure hint: in a clean container without GTK dev libs
      (e.g. `podman run --rm -vi .:/w:Z ubuntu:24.04` + rustup),
      the default install fails with the hint; skip if no container
      runtime — then note it as pending for Task 007
- [x] Standing gates: `cargo fmt --all --check`,
      `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace --all-targets`

**Corrections found while verifying (2026-08-26):**

- The ldd discriminator is **libgtk-3, not appindicator**: the
  tray feature links `libgtk-3.so.0` at build time; tray-icon's
  appindicator backend is dlopened at runtime, so it never appears
  in ldd. Default install → `grep -c libgtk-3` = 1; minimal → 0.
  TEST_MATRIX setup section records this.
- The failure hint was verified via an equivalent forced failure
  (`FEATURES='tray,nonexistent'` → hint + exit 1; `FEATURES='nonexistent'`
  → bare error, no hint) instead of a fresh rustup container — same
  code path (cargo failure + tray-in-set). A real GTK-less container
  run can ride along with task 007 if desired; docker is present.

**Dependencies:** None (Task 006 is independent; Task 007 depends on
this).

**Files likely touched:**
- `justfile` (install / install-user recipes + header comments)
- `docs/TEST_MATRIX.md` (launcher section)
- `CHANGELOG.md`

**Docs:**
- The `${FEATURES-tray}` vs `${FEATURES:-tray}` distinction gets a
  one-line code comment at the expansion site (why = non-obvious).

**Estimated scope:** Small: 3 files (one recipe pair + two docs)
