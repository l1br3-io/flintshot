# Task 1: PinConfig (`[pin]` section) + `flintshot pin` CLI scaffolding

**Description:** Lay the data-flow foundation for pin-to-desktop before
the window exists. Add a `[pin]` config section (`PinConfig`: default
opacity, copy-on-close, default position) to `flintshot-core::config`,
mirroring how `HotkeyConfig` was added in Phase 2. Wire the real
`flintshot pin <path>` CLI body: parse new flags (`--at X,Y`, `--size
WxH`, `--opacity N`, `--no-clipboard`), load the image from disk, merge
config + CLI overrides into a resolved `PinConfig` + window geometry,
and call `EguiShell::run_pin(img, rect)` — which is still a `NotImplemented`
stub after this task (the window is Task 2). The goal: prove the
plumbing (config round-trips, image loads, geometry computes, the call
reaches `run_pin`) so Task 2 only implements the viewport.

**Acceptance criteria:**
- [ ] `PinConfig` struct in `crates/flintshot-core/src/config.rs` with
      `#[serde(default)]` fields: `default_opacity: f32` (default
      `0.5`), `copy_on_close: bool` (default `false`), `default_position:
      String` (default `"center"`). `#[non_exhaustive]`, `kebab-case`,
      `Default` impl — mirror `HotkeyConfig` (`config.rs:116`).
- [ ] `Config` struct gains `#[serde(default)] pub pin: PinConfig`
      (`config.rs:29`); `Config::load` / `Config::save` round-trip the
      `[pin]` section without disturbing `[ui]`/`[capture]`/`[hotkeys]`
      (comment-preserving via existing `toml_edit` path).
- [ ] `flintshot_core::PinConfig` re-exported from `lib.rs`.
- [ ] `PinArgs` (`cli.rs:112`) gains `--at <X,Y>` (parsed to a `Point`),
      `--size <WxH>` (parsed to a `(u32,u32)`), `--opacity <N>` (f32),
      `--no-clipboard` (flag). Unknown/ malformed values error clearly
      via `clap` value parsers.
- [ ] `run_pin` (`cli.rs:322`) real body: load the image
      (`image::open` → `to_rgba8`), resolve the window geometry
      (image natural size by default; `--size` overrides; position =
      active-monitor center by default, `--at` overrides), build the
      `Rect` (min = position, max = position + size), load `Config`,
      apply CLI overrides to the `PinConfig`, call
      `flintshot_editor_egui::EguiShell::new().run_pin(img, rect)`.
- [ ] After this task, `flintshot pin /path/to.png` reaches `run_pin`
      and returns the `NotImplemented` error gracefully (logged, not
      panicked). A missing file errors with the existing "no such file"
      message (`cli.rs:329`).
- [ ] Active-monitor-center resolution: use `flintshot_core::monitors()`
      (already public, used by the tray at `tray.rs:115`); pick the
      primary (or first) monitor's center. Zero monitors → fall back to
      `(0, 0)` with a `tracing::warn!`.

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets` — incl. new `PinConfig`
      round-trip test (load a TOML with `[pin]`, assert fields; load
      without `[pin]`, assert defaults) mirroring the `HotkeyConfig`
      tests.
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual: `cargo run -- pin /path/to/existing.png` → reaches
      `run_pin`, logs `NotImplemented`, exits cleanly (no panic).
      `cargo run -- pin /nonexistent` → clear "no such file" error.
      `cargo run -- config` → output includes a `[pin]` section.

**Dependencies:** None (foundation task).

**Files likely touched:**
- `crates/flintshot-core/src/config.rs` — new `PinConfig` struct + field on `Config` + default fns.
- `crates/flintshot-core/src/lib.rs` — re-export `PinConfig`.
- `crates/flintshot-app/src/cli.rs` — extend `PinArgs`; real `run_pin` body; geometry/position helpers.

**Docs:**
- The `[pin]` section is documented in `docs/ARCHITECTURE.md` §Config
  (the table already lists planned sections). Add the three fields with
  defaults. No `docs/gotchas.md` entry — the config has a code home.

**Estimated scope:** Medium (3 files; config is small, CLI body is the substantive part).
