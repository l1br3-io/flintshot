# Task 001: Author `dist.toml` (pinned cargo-dist config, plan green)

**Description:** Create the root `dist.toml` that scopes cargo-dist to
`flintshot-app` only, enables the tray feature, pins the five targets
(two macOS arches, Windows x64, Linux x64), installs the xcap+GTK apt
build deps in the Linux job, pins the Linux runner to `ubuntu-22.04`
for the glibc 2.35 floor, and sets `pr-run-mode = "upload"`. Verify
with `cargo dist plan` — no workflow generation yet (Task 2).

**Acceptance criteria:**
- [ ] `dist.toml` exists with `cargo-dist-version = "0.33.0"`,
      `packages = ["flintshot-app"]`, `features = ["tray"]`,
      `precise-builds = true`, the 4 targets, `installers =
      ["shell", "powershell", "msi"]`, `pr-run-mode = "upload"`
- [ ] `[dist.github-custom-runners]` pins
      `x86_64-unknown-linux-gnu = "ubuntu-22.04"`
- [ ] `[dist.dependencies.apt]` lists the xcap + tray build deps
      (matches `XCAP_LINUX_DEPS` in ci.yml plus
      `libgtk-3-dev`, `libayatana-appindicator3-dev`)
- [ ] `cargo dist plan` exits 0 and the plan JSON mentions only
      `flintshot-app` / binary `flintshot` — no `flintshot-cosmic`
- [ ] Plan shows shell + powershell + msi installers and sha256
      checksums for all archives

**Verification:**
- [ ] `cargo dist plan` (quiet, exit 0; grep output for `cosmic` →
      absent)
- [ ] `just ci` still green (no source changes, sanity)

**Dependencies:** None

**Files likely touched:**
- `dist.toml` (new)

**Docs:**
- Config keys verified against the cargo-dist 0.33 config reference
  (book/reference/config.html) — keys used here: `packages`,
  `features`, `precise-builds`, `github-custom-runners`,
  `dependencies`, `pr-run-mode`, `installers`. If a key is rejected
  by the pinned version, check the reference before inventing.

**Estimated scope:** Small (1 file; requires `cargo install
cargo-dist --locked --version 0.33.0`, ~10 min compile)
