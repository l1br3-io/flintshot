# Task 003: `EditorOutcome::Pinned` + editor Pin button + pin-cache dir

**Description:** Add the editor *Pin to desktop* toolbar button — the fourth output path alongside Save and Copy. When clicked, it composes the final image (background + all committed annotations, via the existing `compose_final` source of truth at `render.rs`), writes it to a pin-cache file, publishes `EditorOutcome::Pinned(path)`, and closes the editor. The CLI's `run_editor_for_capture` then spawns `flintshot pin <path>` detached (reusing the Phase 2 `worker::CommandSpawner` pattern) and the pin persists as its own process. Also extend Task 002's `report_outcome` helper to send `ReportSaved` on `Pinned` so the daemon's `last_saved` tracks pins too.

**Acceptance criteria:**
- [ ] New variant `EditorOutcome::Pinned(PathBuf)` in `crates/flintshot-core/src/shell.rs` (alongside `Saved`/`CopiedToClipboard`/`Uploaded`/`Cancelled` at `:13-22`). Doc comment: "User pinned the composed image to the desktop; contains the pin-cache path the editor wrote."
- [ ] New helper `pin_cache_dir() -> Option<PathBuf>` in `crates/flintshot-core/src/config.rs` (alongside `config_path()` at `:377`). Resolves to `dirs::cache_dir()?.join("flintshot").join("pins")`. Doc comment notes the fallback (caller drops to `std::env::temp_dir().join("flintshot-pins")` when `None`).
- [ ] `pin_cache_dir()` unit test: honours a fake `DIRS_CACHE_HOME`-style override if the `dirs` crate exposes one (if not, test the composition shape with a seam — a `pin_cache_dir_in(base: Option<&Path>)` testable constructor, mirroring `ipc::default_unix_socket_in` at `ipc.rs:163`).
- [ ] Editor *Pin* button in `crates/flintshot-editor-egui/src/app.rs`: a new `ToolbarAction::Pin` variant (alongside `Save`/`Copy`/`None`), a "📍 Pin" button in the toolbar (mirror the Save/Copy button shape at `app.rs:760-764`), and a `handle_pin(&self, ctx)` method.
- [ ] `handle_pin` composes the final image via `compose_final(&self.state)` (selection-aware like Copy — use `compose_final_region` when a non-degenerate `state.selection` is active, mirroring `copy_to_clipboard` at `app.rs:878-883`), writes it to `pin_cache_dir().join(format!("pin-{timestamp}.png"))` (timestamp = `SystemTime::now().duration_since(UNIX_EPOCH).as_nanos()` for uniqueness), publishes `Ok(EditorOutcome::Pinned(path))`, and closes the window via `ViewportCommand::Close`.
- [ ] Write-failure path: if the cache dir can't be created or the file can't be written, publish `Err(FlintshotError::Shell { detail })` and close (matching `handle_save`'s failure shape at `app.rs:781-785`).
- [ ] `crates/flintshot-app/src/cli.rs::run_editor_for_capture`: extend the `match outcome` at `:264-278` with an explicit `EditorOutcome::Pinned(path)` arm that spawns `flintshot pin <path>` detached (via `worker::CommandSpawner` or a direct `std::process::Command::new(current_exe()).arg("pin").arg(&path).spawn()` with the same stdin/stdout null + reaper-thread pattern as `worker.rs:46-71`). Log the spawn.
- [ ] The `report_outcome` helper added in Task 002 is extended to also send `Request::ReportSaved { path }` on `EditorOutcome::Pinned(path)` (so the daemon's `last_saved` tracks pins, not just saves).
- [ ] The existing `#[non_exhaustive]` wildcard arm at `cli.rs:277` (`_ => tracing::warn!(?outcome, ...)`) is replaced by the explicit `Pinned` arm; the wildcard stays for future variants.
- [ ] `flintshot-core/src/lib.rs` re-exports `pin_cache_dir` if it's intended to be public (it is — the editor and the daemon's cleanup both call it). Add `pin_cache_dir` to the `pub use config::{...}` line at `:33`.

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets` — incl. new `EditorOutcome::Pinned` serde roundtrip (if `EditorOutcome` has one; add otherwise), `pin_cache_dir()` resolution test, and a unit test for the spawn-argv construction (`worker::pin_argv(path)` if you factor one out, or assert on the `Command` via a seam).
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual (needs display): `cargo run -- region`, draw an annotation, click *Pin* → editor closes, a borderless always-on-top pin window appears showing the composed image (background + annotation). The pin persists after the editor exits. Verify with multiple annotations + a selection-active pin (compose_final_region path).

**Dependencies:**
- Task 001 (`EguiShell::run_pin` / `flintshot pin` must exist so the spawn produces a visible window).
- Task 002 (extends the `report_outcome` helper to cover `Pinned`; without 002 the `Pinned` arm just skips the report, which is acceptable but loses daemon tracking — recommend 002 first).

**Files likely touched:**
- `crates/flintshot-core/src/shell.rs` — `EditorOutcome::Pinned(PathBuf)` variant + doc.
- `crates/flintshot-core/src/config.rs` — `pin_cache_dir()` + testable seam + unit test.
- `crates/flintshot-core/src/lib.rs` — re-export `pin_cache_dir` at `:33`.
- `crates/flintshot-editor-egui/src/app.rs` — `ToolbarAction::Pin`, button UI, `handle_pin` method.
- `crates/flintshot-app/src/cli.rs` — `Pinned` arm in `run_editor_for_capture`'s match; extend `report_outcome`.

**Docs:**
- `EditorOutcome::Pinned` doc comment notes the path is the pin-cache file (not the user's save path) and that the pin is its own process.
- `pin_cache_dir()` doc notes the opportunistic-cleanup contract (daemon reaps on start; files live in the OS cache dir which the OS may reap independently).
- A `// RATIONALE:` comment at the spawn site in `run_editor_for_capture` pointing to ADR-0023 (pin-as-process) — the editor deliberately exits and lets the pin live.
- If the timestamp-based filename has a collision risk (clock resolution, concurrent editors), note it as a `// GOTCHA:` — nanosecond precision + per-process pid suffix eliminates it in practice; record the choice.

**Estimated scope:** Medium (5 files; the editor button + handler is the substantive work, the core additions are small).

**Notes for the implementer:**
- The Pin button is closer to Save than to Copy in its lifecycle: it composes once, publishes, and **closes** the editor (Copy stays open for the X11 clipboard-owner reason documented at `app.rs:791-804`). Don't copy Copy's stay-open behaviour.
- The composed image for the pin is the same `compose_final` / `compose_final_region` output Save and Copy use — WYSIWYG holds by construction. Do not introduce a third composition path.
- The spawn uses `std::env::current_exe()` (not `PATH`) so the pin binary is the same `flintshot` as the editor — same pattern as `worker::CommandSpawner` at `worker.rs:47`. If a second caller of `CommandSpawner` appears (the daemon's `PinLast` in Task 004 is the third), the reuse is vindicated; otherwise a direct `Command::spawn` is fine for now.
- The pin-cache file is written **before** the spawn and **before** the editor closes. If the write fails, the pin never spawns and the editor surfaces the error. Order: compose → write cache file → publish `Pinned(path)` → close → (back in `run_editor_for_capture`) spawn pin from the published path.
