# Task 5: Editor toolbar Pin button

**Description:** Add the third pin entry point — a **Pin** button in the
editor toolbar. On click, it composes the final image (background +
annotations, via the existing `flintshot_core::compose_final`,
`render.rs:650`), writes it to a temp file, spawns `flintshot pin
<tmpfile>` as a detached child (resolving the binary via
`current_exe`, mirroring `worker::CommandSpawner`), and closes the
editor. This reuses the Task 1/2 CLI path end-to-end and gives users
the "pin what I just annotated" flow without leaving the editor. Marks
pin-to-desktop feature-complete from all three entry points.

**Acceptance criteria:**
- [ ] New `ToolbarAction::Pin` variant (`app.rs:122` enum), a "📌 Pin"
      button in the toolbar alongside Save/Copy (`app.rs:683`/`:689`),
      and a `handle_pin` method mirroring `handle_save`/`handle_copy`.
- [ ] `handle_pin`: call `compose_final(&self.state)` (or
      `compose_final_region` if a selection is active — mirror Copy's
      selection-awareness at `app.rs:850`), write the PNG to a temp
      file via `tempfile` (promoted from dev-dep to dep in
      `flintshot-app/Cargo.toml`), spawn `flintshot pin <tmpfile>` as a
      detached child (`Stdio::null` for stdin/stdout, reaped like
      `CommandSpawner`), then publish `Ok(EditorOutcome::Cancelled)` —
      or a new `EditorOutcome::Pinned` variant if the
      `#[non_exhaustive]` enum should name it (recommend adding it for
      clarity; the daemon/Cosmic path can then distinguish). The editor
      window closes after spawning.
- [ ] The temp file is written under the OS temp dir with a stable name
      (e.g. `flintshot-pin-<pid>-<n>.png`) and left on disk for the pin
      worker to read (the pin worker opens it once at startup; the file
      can be reaped on pin close, or left — document the choice).
- [ ] If `compose_final` or the temp write fails, publish
      `Err(FlintshotError)` and close (mirror the render-error pattern).
- [ ] Spawn failures (binary not resolvable) publish an error rather
      than panicking; the editor stays usable.
- [ ] A selection-active pin pins just the selection (parity with the
      Copy fix at `app.rs:850`).

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets` — incl. a unit test that
      `handle_pin`'s compose+write path produces a valid PNG (headless:
      construct an `EditorState`, call `compose_final`, assert the
      bytes decode); the spawn itself is `#[ignore]`'d (needs the
      binary + display).
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual: `flintshot region` → annotate → click Pin → editor
      closes, a pin window floats showing the annotated image. With a
      selection active → pin shows just the selection. Multiple
      Pin clicks (across editor sessions) → multiple pins coexist.

**Dependencies:** Task 2 (needs the `flintshot pin <path>` window to
exist). Soft-depends on Task 4 only if `EditorOutcome::Pinned` should
round-trip through the daemon (not required; the editor Pin button
spawns the worker directly).

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs` — `ToolbarAction::Pin`, button, `handle_pin`.
- `crates/flintshot-core/src/shell.rs` — (optionally) `EditorOutcome::Pinned` variant.
- `crates/flintshot-app/Cargo.toml` — promote `tempfile` from `[dev-dependencies]` to `[dependencies]`.
- `crates/flintshot-app/src/cli.rs` — handle `EditorOutcome::Pinned` in the outcome match (`cli.rs:264`) if the variant is added.

**Docs:**
- `CHANGELOG.md` `[Unreleased]` → Added: editor Pin toolbar button.
- If `EditorOutcome::Pinned` is added, note the new variant (affects
  every `EditorOutcome` match arm with a wildcard — `#[non_exhaustive]`).

**Estimated scope:** Medium (3–4 files; the compose+spawn+close wiring is the substantive part; reuses existing `compose_final`).
