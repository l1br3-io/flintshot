# Task 009: Settings config schema (additive fields + plumbing)

**Description:** Extend `flintshot-core::config` with the settings
route's fields — additive, kebab-case, serde-defaulted so every
existing TOML keeps loading: `capture.default-mode`
(region/fullscreen/window), `ui.show-statusbar` (default true),
`ui.statusbar-size-mode` (heuristic | exact-png), `ui.animations`
(default true), `output.format` (png | jpeg), `output.jpeg-quality`
(1-100, default 90), `output.filename-template` (default empty),
`output.save-dir` (default empty). Plus the pure pieces the route
needs: template substitution (`{app}`, `{date}`, `{time}` — unit-
tested) and the humanized/exact byte-size estimator. No GUI types —
`no_gui_types_in_core.rs` stays green.

**Acceptance criteria:**
- [ ] All fields round-trip through the existing save/load; absent
      fields deserialize to defaults (old-config test).
- [ ] `default-mode`/`format` parse strict-ish (unknown → default +
      warning, matching existing config tolerance).
- [ ] Template substitution pure + tested (`{app}` `{date}` `{time}`,
      unknown placeholders pass through, sanitized filename).
- [ ] Byte estimator: heuristic (dims × 4) + exact (PNG encode)
      behind one `SizeMode` API; unit-tested.
- [ ] Doc comments on all public items; `#[non_exhaustive]` where
      extension is plausible; clippy/fmt clean.

**Verification:**
- [ ] `cargo test --workspace --all-targets` (config + estimator +
      template tests)
- [ ] `cargo test --doc -p flintshot-core`
- [ ] Manual: load a pre-existing config.toml — loads unchanged with
      defaults for new fields

**Dependencies:** Task 1 (module conventions only — parallelizable
with Tasks 2-8)

**Files likely touched:**
- `crates/flintshot-core/src/config.rs`
- `crates/flintshot-core/src/lib.rs` (re-exports if needed)

**Estimated scope:** Medium (1-2 files, core crate)

**Docs:**
- GOTCHA if the config watcher fights rapid successive writes —
  inline comment at the write site.
