# Task 007: Wayland detection + editor-window fallback (+ force-flag semantics); macOS/Windows pass

**Description:** Complete the mode-resolution chain (plan decision 3):
CLI flag > config > **platform fallback**. The overlay's fullscreen
borderless window cannot reliably be raised above other windows or
grab input on all Wayland compositors (spec stance 2, ARCHITECTURE.md
risk table), so a Wayland session demotes a *config-resolved*
`overlay` to the editor window — honest UX, logged, never silent.

**Wayland detection** is environment-based (the same signals winit
keys off): a non-empty `WAYLAND_DISPLAY` (a compositor socket exists)
**or** `XDG_SESSION_TYPE = "wayland"` (case-insensitive; covers
session managers that only set the session type). `WAYLAND_DISPLAY`
wins when both are present — with a socket available, winit prefers
Wayland even inside an `XDG_SESSION_TYPE=x11` XWayland setup. The
decision is a pure function over the two values (unit-tested truth
table); a thin wrapper reads the real environment. macOS / Windows
have no Wayland, so the wrapper is `cfg`-gated to constant `false`
there — the overlay stays available and the "macOS/Windows pass" is
the manual TEST_MATRIX run (CI is Linux-only; rows land in Task 009).

**Force-flag semantics (the decision spec exit-criterion 4 defers to
this task): `--overlay` on Wayland is honored best-effort with a
`tracing::warn!` naming the risk.** Chosen over refusal because (a)
the fallback exists to protect the *default* path from
compositor-dependent behaviour, not to override an explicit,
informed request; (b) mainstream compositors (KWin, sway, Hyprland)
honour fullscreen windows well enough that refusing would break
users it would work for; (c) refusal turns a possibly-working
override into a dead flag with no upside. Config-resolved
`interaction_mode = "overlay"` on Wayland falls back to the editor
window with a `tracing::info!` naming the fallback (the daemon /
worker path included — workers inherit the session env and run this
same resolution code).

Resolution stays single-site: `run_capture_shell` resolves once and
logs mode + source, where a new `InteractionSource::PlatformFallback`
makes the fallback legible in every capture's decision log.

**Acceptance criteria:**
- [x] `wayland_from_env` truth table: non-empty `WAYLAND_DISPLAY` →
      true; `XDG_SESSION_TYPE = "wayland"` (any case) → true; both
      absent / empty / `"x11"` → false; `WAYLAND_DISPLAY` present
      beats `XDG_SESSION_TYPE = "x11"`.
- [x] `session_is_wayland` reads the real env on Unix and is
      constant `false` on macOS / Windows (cfg-gated); its wiring
      matches the pure function (env-var names pinned by test).
- [x] config `overlay` + Wayland → `(Editor, PlatformFallback)` +
      info log naming the fallback reason.
- [x] config `editor` + Wayland → `(Editor, Config)` — no fallback
      noise for the mode Wayland would have chosen anyway.
- [x] unknown config value + Wayland → `(Editor, Config)` (the
      existing `from_config` fallback; platform not involved).
- [x] `--overlay` + Wayland → `(Overlay, Flag)` — honored
      best-effort, warn log names the risk.
- [x] `--editor` flag + Wayland → `(Editor, Flag)` — unchanged.
- [x] Flag/config precedence on non-Wayland sessions is unchanged
      (existing Task 001 tests updated for the new signature and
      still pinning flag > config).
- [x] `--overlay` clap help documents the Wayland best-effort
      semantics (the "documented" half of the decision).
- [x] No changes in `flintshot-core` (detection is app-level;
      `no_gui_types_in_core.rs` unaffected by construction).

**Verification:**
- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace --all-targets` — incl. the new
      detection / resolution tests.
- [x] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual (Wayland, e.g. L2–L5): `flintshot region` with
      `interaction_mode = "overlay"` opens the editor window; the
      log names the Wayland fallback; `flintshot region --overlay`
      opens the overlay (best-effort) with a warn in the log;
      `flintshot region --editor` opens the editor.
- [ ] Manual (X11): `interaction_mode = "overlay"` still opens the
      overlay — no regression.
- [ ] Manual (macOS / Windows): overlay opens; no fallback log —
      recorded in TEST_MATRIX (Task 009 adds the rows).

**Dependencies:** Task 001 (the resolution helper this extends),
Task 002 (the overlay this gates).

**Files likely touched:**
- `crates/flintshot-app/src/platform.rs` — new: pure
  `wayland_from_env` + `session_is_wayland` (+ tests).
- `crates/flintshot-app/src/main.rs` — `mod platform;`.
- `crates/flintshot-app/src/cli.rs` —
  `InteractionSource::PlatformFallback`, `resolve_interaction_mode`
  wayland parameter + fallback/force-flag logs, `run_capture_shell`
  call site, `--overlay` help text, tests.
- `CHANGELOG.md` — Unreleased entry.

**Estimated scope:** Small–medium (~150 lines incl. tests, app-only).
