# Task 002: Ambient tokio runtime guard for window workers

**Description:** On at-spi desktops, eframe → accesskit → zbus 5
needs an ambient tokio reactor; window-opening workers
(`region`/`fullscreen` → editor or overlay, `open` → editor,
`pin` → pin window) have none and panic before any window appears.
Add a small `ambient.rs` module in `flintshot-app` with
`with_ambient_runtime(f)`: build a multi-thread `Runtime`, hold an
`enter()` guard, run `f`, drop the guard. Wire it inside
`run_editor_with_config`, `run_overlay_with_config`, and
`run_pin_window` around the `EguiShell` call only — narrowest seam
covering every window path, and the guard is dropped before
`report_editor_outcome` fires `ipc_client`'s own one-shot `block_on`
runtimes (no runtime nesting).

**Acceptance criteria:**

- [ ] New module (e.g. `crates/flintshot-app/src/ambient.rs`) with
      documented `with_ambient_runtime` helper; `main.rs` wires the
      `mod`
- [ ] `tokio::runtime::Handle::try_current()` fails on a plain
      thread, succeeds inside `with_ambient_runtime`, and does not
      leak after it returns (guard dropped)
- [ ] The helper propagates the closure's return value and maps
      runtime-construction failure to an `eyre` error (no
      `unwrap`/`expect`)
- [ ] Editor, overlay, and pin paths call `EguiShell` under the
      guard (both egui and headless builds compile; headless
      fallbacks unchanged — they open no window)
- [ ] `run_upload` unchanged (already owns a runtime; never reaches
      eframe)
- [ ] `flintshot-core` / `flintshot-editor-egui` untouched

**Verification:**

- [ ] `cargo test -p flintshot-app` green (new guard unit tests)
- [ ] Full standing gates green
- [ ] `cargo check -p flintshot-app --no-default-features` clean
      (headless fallbacks still compile)

**Dependencies:** None (independent of Task 1; ordered after it only
for commit narrative)

**Files likely touched:**

- `crates/flintshot-app/src/ambient.rs` (new)
- `crates/flintshot-app/src/main.rs` (mod declaration)
- `crates/flintshot-app/src/cli.rs` (three call sites + tests)

**Docs:**

- Module doc comment states the why (zbus 5 via accesskit; TEST_MATRIX
  L5 finding 2). Full rationale lands as ADR-0030 in Task 4.

**Estimated scope:** Medium: 3 files
