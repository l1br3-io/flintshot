# Task 004: App window — close state, not a mark

**Description:** `SharedInner::window_destroyed` (one-shot, bool)
becomes `window_open` (state, set by the outer loop on both sides of
`run_home`); the hide-ack waits for *currently closed* and consumes
nothing. Kills the stale-mark ack race structurally.

**Done:** state semantics + rewritten tests (truthful reads,
reopen re-arm, cross-thread wake).

## Verification

- [x] `cargo test -p flintshot-app app_window`
