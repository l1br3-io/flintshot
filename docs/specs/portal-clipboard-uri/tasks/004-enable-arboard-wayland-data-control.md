# Task 004: Enable arboard `wayland-data-control`

**Description:** Tasks 001+003 were correct but insufficient: the
read-back still failed every manual attempt. Instrumented repro
(2026-09-15, this session) proved the portal's clipboard write
lands ~0.5–1 s after the `clipboard:///` Response and `wl-paste`
(native `zwlr_data_control_v1`) reads it fine — while arboard
fails against the *same* selection in the same 250 ms window, in
both directions (`get_image` fails; `set_image` "succeeds" but
`wl-paste` sees nothing). Cause: arboard 3.6.1 carries no wayland
crates without the optional `wayland-data-control` feature — its
Linux backend is X11-only (`x11rb`), riding XWayland's clipboard
bridge, whose Wayland→X11 sync is lazy/focus-gated (hence the
nondeterministic "sometimes reads" behavior). Fix: add the feature
to the workspace `arboard` entry so every crate using
`arboard = { workspace = true }` (app clipboard arm, editor Copy,
overlay) gets the native data-control backend; arboard still picks
X11 on X11 sessions. Proof of fix already recorded: an arbtest
build with the feature reads a real 2211×1205 Wayland selection
(`OK: 2211x1205 10657020 bytes`) that the featureless build cannot.

**Acceptance criteria:**
- [x] `[workspace.dependencies] arboard` declares
      `features = ["wayland-data-control"]` (with `image-data`,
      `default-features = false` as today); Cargo.lock gains
      `wl-clipboard-rs` + wayland crates
- [x] One adjacent pre-existing fix: the `#[cfg(not(feature = "egui"))]`
      `run_overlay_with_config` stub's stale signature (missing `watch`)
      — without it `--no-default-features` still failed to compile — Task 003's wait/digest machinery and
      all call sites untouched
- [x] Finish gate green: `cargo fmt --all --check`,
      `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace --all-targets`,
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [x] CI-shaped builds still compile: default, `--features tray`,
      and `FEATURES=''` (no new system deps on Linux)

**Verification:**
- [x] All gate commands above pass
- [x] Manual (injected-Enter repro, this session): tray →
      Capture → confirm with Copy → editor opens; daemon.log shows
      `portal clipboard write landed` (no 5 s timeout error)
- [x] Manual regression (partial): editor Copy → paste into another app
      still works (native-wayland write path)
- [x] Deploy via `just install-user --refresh`

**Dependencies:** Task 003 (the wait machinery this fix makes
effective)

**Files likely touched:**
- `Cargo.toml` (workspace `arboard` entry — one line)
- `Cargo.lock` (regenerated)
- `CHANGELOG.md` (amend the effort's Fixed entry)
- `docs/specs/portal-clipboard-uri/` (todo tick, spec note)

**Docs:**
- GOTCHA comment on the workspace `arboard` entry: without
  `wayland-data-control`, arboard silently falls back to XWayland
  on Wayland sessions — clipboard reads/writes become
  focus-gated and flaky.

**Estimated scope:** Small (1 line + lock + docs)
