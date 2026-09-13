# Spec: Remove pin-to-desktop

---
status: active
started: 2026-09-13
---

## Objective

Pin-to-desktop shipped in Phase 3 but never worked as intended on
Wayland: the always-on-top window hint is ignored by most compositors,
and the "real fix" (a layer-shell-based pin) is a significant
platform-specific investment. The feature is being **removed from the
product entirely** — not deferred. It is dropped from the vision, the
CLI surface, the IPC protocol, the editor toolbar, the tray/applet
menus, and the config schema.

This is a purge, not a refactor: dead code is deleted, not stubbed.
Anything the pin feature no longer justifies (temp-PNG composition
path, pin session tracking, pin-id counter) goes with it **only if**
no remaining output flow (Save / Copy / Upload) still needs it.

**User impact:** users lose `flintshot pin`, the editor Pin button,
tray "Pin last", and the `[pin]` config section. Existing config files
with a `[pin]` section are silently ignored (config parsing has no
`deny_unknown_fields`). No migration is provided.

**Success looks like:**

- `flintshot pin ...` no longer exists as a subcommand
- No Pin button in the editor toolbar or overlay toolbar
- No "Pin last" in tray or COSMIC applet menus
- `Request::PinLast` / `Event::Pinned` / `EditorOutcome::Pinned` /
  `PinConfig` gone from `flintshot-core`
- Save / Copy / Upload / OpenLast / capture / overlay flows unchanged
- Full CI gate green (fmt, clippy `-D warnings`, all tests, tray
  feature build, cosmic build)

## Removal inventory

Authoritative list of what gets touched. Order here is not
implementation order; see `plan.md` (created by `/build`).

### `crates/flintshot-core`

| File        | Removal                                                              |
| ----------- | -------------------------------------------------------------------- |
| `config.rs` | `PinConfig` struct + `Default` impl + tests; `Config.pin` field      |
| `ipc.rs`    | `Request::PinLast`; `Event::Pinned { id }`; related roundtrip tests |
| `shell.rs`  | `EditorOutcome::Pinned` variant; `EditorShell::run_pin` trait method |
| `lib.rs`    | `PinConfig` re-export                                                |

Removing `run_pin` changes the public `EditorShell` trait —
acceptable, pre-1.0, noted in CHANGELOG.

### `crates/flintshot-editor-egui`

| File        | Removal                                                                     |
| ----------- | --------------------------------------------------------------------------- |
| `pin.rs`    | Entire module (372 lines: `PinApp`, drag, aspect-lock, opacity, clipboard)  |
| `lib.rs`    | `mod pin` registration + doc mention                                        |
| `shell.rs`  | `with_pin_config` builder, `run_pin` impl / `run_pin_window`, `PinConfig` plumbing, pin tests |
| `toolbar.rs`| `ToolbarAction::Pin`, Pin button UI, `g_pin` glyph, tooltip/help-text mentions |
| `app.rs`    | `handle_pin`, `compose_and_write_pin`, `spawn_pin_worker` (+ their tests)   |
| `overlay.rs`| Pin chip (`overlay_pin`), `handle_pin`, `ToolbarAction::Pin` arm, helper imports |

### `crates/flintshot-app`

| File             | Removal                                                                        |
| ---------------- | ------------------------------------------------------------------------------ |
| `cli.rs`         | `Command::Pin(PinArgs)`, `PinArgs`, `run_pin` + `PinConfig` helper, `--opacity`/`--no-clipboard` overrides, `EditorOutcome::Pinned` match arms, help text |
| `main.rs`        | `Command::Pin` dispatch arm                                                    |
| `worker.rs`      | `PinIdSource`, `pin_ids` field, `Request::PinLast` dispatch arm, `pin_argv` builder, `Event::Pinned` emission, `"pin-last"` label, `register_pin` call site, dispatch tests |
| `ipc_server.rs`  | Stale doc comments referencing `PinLast` (dispatch itself lives in `worker.rs`) |
| `sessions.rs`    | `SessionKind::Pin`, `PinInfo`, `register_pin`, pin respawn/kill paths, tests   |
| `tray.rs`        | Nothing — the tray menu has no Pin item (earlier efforts removed it); matches were `libappindicator` false positives |
| `tray_helper.rs` | `Request::PinLast` entry in the `other_clicks_keep_the_helper_alive` test     |
| `prepare_gate.rs`| `pins_alive` gate condition + parameter + tests (pins are killed for capture; with no pins the condition is vacuous) |

Also scrub any remaining references in `app_window.rs`, `daemon.rs`,
`ambient.rs`, `single.rs`, `hotkeys*.rs`, `ipc_client.rs`,
`config_watcher.rs`, `tray_gate.rs` (mentions found by the final
sweep; most are doc comments).

### `crates/flintshot-cosmic`

| File     | Removal                                            |
| -------- | -------------------------------------------------- |
| `menu.rs`| `MenuMessage::PinLast`, `Request::PinLast` mapping, tests |
| `app.rs` | "Pin last" button, update test                      |
| `ipc.rs` | `Pinned` event status mapping + tests               |

### Living docs, tooling, records

| File                   | Change                                                                   |
| ---------------------- | ------------------------------------------------------------------------ |
| `README.md`            | Delete Wayland always-on-top note, both feature-list bullets             |
| `docs/VISION.md`       | Drop "Pin as floating always-on-top window"; drop `+ pin` from v1 ships  |
| `docs/ARCHITECTURE.md` | Remove pin window from component diagram, IPC enum samples, `run_pin` trait listing, `[pin]` config sample |
| `docs/ROADMAP.md`      | Annotate Phase 3 row: "(pin later removed, ADR-0049)" — keep history     |
| `docs/TEST_MATRIX.md`  | Remove pin rows                                                          |
| `justfile`             | Delete `pin` recipe                                                      |
| `CHANGELOG.md`         | New `## [Unreleased]` entry (history below stays untouched)              |
| `docs/adrs/ADR-0049-remove-pin-to-desktop.md` | New: Context · Decision · Consequences |

**Untouched (historical records):** prior CHANGELOG entries,
`docs/specs/phase-3-pin-to-desktop/`, `docs/specs/phase-3-pin-cosmic/`,
existing ADRs that mention pin.

## Wire-format break (accepted risk)

`Request` and `Event` are postcard-encoded **by variant index** (the
enum definitions explicitly append variants for this reason). Deleting
`PinLast` and `Pinned` re-indexes every variant after them — old and
new binaries cannot interoperate.

Accepted because every IPC client except the COSMIC applet is spawned
by the daemon itself (tray helper per ADR-0042, workers per ADR-0019)
and is therefore version-locked. The one real skew window is a
long-lived COSMIC applet surviving a daemon upgrade; worst case there
is a garbage decode → connection error. No tombstone variants, no
compat shim.

## Commands

From repo root (full CI parity):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p flintshot-app --features tray --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo build -p flintshot-cosmic          # opt-in; needs system deps
```

Linux system deps for the above are in AGENTS.md (xcap deps +
libgtk-3-dev / libayatana-appindicator3-dev for tray, cmake et al. for
cosmic).

## Project structure

No new source files; one new ADR, one spec dir (this one). Deletions
per the removal inventory above. Tests live where the code lives
(`#[cfg(test)]` modules) plus `crates/flintshot-core/tests/`; pin
tests are deleted alongside the code they cover, never converted to
`#[ignore]`.

## Code style

Unchanged — repo conventions per AGENTS.md and
`.opencode/tech/rust/coding-style.md`. The diff shape is:

- Delete enum variants, structs, and match arms outright; do not leave
  commented-out code or `// TODO(pin)` markers.
- Where a comment mentions pin incidentally (e.g. "Save / Copy / Pin /
  Upload"), rewrite the list without it — don't leave stale names.
- No new `#[allow(...)]`, no new dependencies.

## Testing strategy

- Existing suites must stay green after each removal step
  (`cargo test --workspace --all-targets`).
- Pin-specific tests are deleted with their code; no new tests are
  written for absence, **except** one core IPC roundtrip test remains
  meaningful: it must still exercise the surviving variants after
  `PinLast`/`Pinned` are gone (the existing roundtrip test is edited,
  not deleted, so wire coverage of the shrunken enums is kept).
- `flintshot-core` GUI-purity test
  (`tests/no_gui_types_in_core.rs`) must remain green — removal must
  not accidentally drop it or its subject.
- Verification of the user-visible surface: `cargo run -- --help`
  shows no `pin` subcommand; `cargo run -- app` and editor toolbar
  show no Pin button (manual smoke, recorded in the closeout task).

## Boundaries

- **Always:** run the full CI-parity command set before declaring
  done; update CHANGELOG `[Unreleased]`; write ADR-0049; keep
  historical records untouched; delete dead code rather than stubbing.
- **Ask first:** any change to surviving IPC variants' ordering
  (beyond the accepted re-index), any new dependency, touching CI
  config, any behavior change to Save / Copy / Upload / OpenLast.
- **Never:** rewrite CHANGELOG history or shipped specs; reintroduce
  pin via a stub or feature flag; silence clippy with `#[allow]`;
  remove or weaken the no-GUI-types-in-core test.

## Success criteria

1. `rg -in "pin"` over `crates/`, `justfile`, `README.md`,
   `docs/{VISION,ARCHITECTURE,ROADMAP,TEST_MATRIX}.md` returns only
   false positives (`std::pin`, `pin-project-lite` in `Cargo.lock`,
   "Snipping", harness docs under `.opencode/`, etc.) — zero
   feature references.
2. All Commands-section invocations exit 0.
3. `flintshot --help` lists no `pin`; `flintshot pin` errors as
   unknown subcommand.
4. Editor overlay + app window toolbars render without a Pin button
   (manual smoke).
5. ADR-0049 and CHANGELOG entry exist; ROADMAP Phase 3 row annotated.

## Open questions

None — scope confirmed with maintainer (living docs only; feature
gone for good; hard IPC removal; COSMIC included).
