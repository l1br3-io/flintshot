# Plan — Fix the app-window capture races

Fix-shaped (small, sequential): each task lands independently
testable, in dependency order. Verification per repo gate:
`cargo fmt --all --check` ·
`cargo clippy --workspace --all-targets -- -D warnings` ·
`cargo test --workspace --all-targets` ·
`cargo test --doc -p flintshot-core -p flintshot-editor-egui`, plus
the live TEST_MATRIX section for the behaviors a headless harness
cannot reach.

| #   | Task                                                          | Crate(s)              | Depends on |
| --- | ------------------------------------------------------------- | --------------------- | ---------- |
| 001 | Core: `Request::AnnounceAppWindow` (schema + roundtrip)       | core                  | —          |
| 002 | Daemon: presence in the prepare gate + interception + main-loop OR | app             | 001        |
| 003 | App window: presence connection in the pump                    | app                   | 001        |
| 004 | App window: `window_open` state replaces the one-shot mark     | app                   | —          |
| 005 | Home window: stuck-close watchdog                              | editor-egui           | —          |
| 006 | Native path: grab-only guard scope (`native_grab_prepared`)    | app                   | —          |
| 007 | Editor: Copy closes on Wayland                                | editor-egui           | —          |
| 008 | Docs: ADR-0052, TEST_MATRIX rows, CHANGELOG                    | docs                  | all        |

## Sequencing notes

- 004 and 002/003 are independent fixes for the same symptom
  (window-in-shot) via different races; both land — either alone
  leaves a reachable path to the bug.
- 005 and 007 address the "mess"/manual-close symptom from the two
  ends (armed intent stuck vs. editor lingering after Copy).
- 006 is a scope correction to ADR-0036 semantics that the native
  path violated; no protocol change.
