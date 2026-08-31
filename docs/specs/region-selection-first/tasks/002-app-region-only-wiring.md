# Task 002 — Region-only wiring in `flintshot-app`

> Status: complete
> Effort: [`region-selection-first`](../spec.md) · Plan decision 4

## Description

Wire task 001's `EguiShell::with_selection_first()` into the app
crate as a single decision point:

- `run_editor_with_config` gains a `selection_first: bool` parameter
  and calls `.with_selection_first()` on the shell when set (the
  `open` path passes `false` explicitly).
- The decision lives beside the interaction-mode resolution in
  `run_capture_shell`: selection-first iff the capture mode label is
  `region` **and** the resolved shell is the editor window — covering
  the default `editor` mode on all platforms, `--editor`, and the
  Wayland demotion of config-set `overlay` (the demoted editor *is*
  region's editor path).
- Fullscreen is untouched: even its Wayland overlay-demotion stays
  the plain editor (spec Open Question 1, lean no — fullscreen's
  contract is "the whole monitor is the shot").
- The interaction-mode decision log now names `selection_first`.
- Daemon-spawned workers need nothing: they spawn bare `region
  --standalone` argv (worker.rs `region_argv`), which re-enters
  `run_region` and inherits the wiring.

## Acceptance criteria

1. `selection_first_for("region", Editor)` → `true`; `("region",
   Overlay)` → `false` — region's editor path (any source: flag,
   config, platform fallback) is selection-first, the true overlay
   never is (unit tests).
2. `("fullscreen", Editor)` and `("open", Editor)` → `false` —
   including fullscreen's Wayland demotion composition with
   `resolve_interaction_mode(None, "overlay", true)` (unit tests).
3. `run_editor_for_capture` (the `open` path) passes `false`; only
   `run_capture_shell` can request the phase (code inspection — the
   builder call is unreachable from `open`/`pin`).
4. The `run_capture_shell` decision log carries a `selection_first`
   field.
5. Standing gates green: `fmt`, `clippy -D warnings`, workspace
   tests, doc tests.

## Verification

```sh
cargo test -p flintshot-app cli
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```
