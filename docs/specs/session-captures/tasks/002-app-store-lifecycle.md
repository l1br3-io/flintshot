# Task 002: App ownership, lifecycle, insert-on-arrival

**Description:** Wire the store into the app window. `Shared` gains a
`session: SessionStore` (created in `run_app_window` after
`sweep_stale` + init of the resolved dir; `clear()` at the function
tail after the outer loop breaks — `app_window.rs:~893` — so every
graceful exit path cleans up). Add one choke-point helper
`arm_editor(shared, state, selection_first)` that **inserts into the
store** and arms `PendingEditor`; route all six arming sites through it
(pump grab delivery `:714`, forwarded open `:901`, Home bridge
Open/Paste `:1574/:1594`, editor-chrome Open/Paste `:1833/:1853`).
`PendingEditor` gains `session_id: Option<SessionId>` — `None` on new
arrivals (insert), `Some(id)` on resume (Task 005 reuses, never
re-inserts).

**Acceptance criteria:**
- [ ] Startup sweeps a stale session dir; graceful loop exit (user quit, pump death, error) calls `clear()` — verify via the pure seams (sweep/clear are store methods; outer-loop wiring asserted in tests where the loop pieces are unit-testable)
- [ ] Every existing `set_pending_editor` call site in the app window goes through the arm helper; a store entry exists after arming
- [ ] `PendingEditor.session_id` is `None` for all six current sites; existing tests updated for the new field (mechanical)
- [ ] No behavior change visible to the user yet (entries accumulate invisibly; no removal wiring in this task)

**Verification:**
- [ ] `cargo test -p flintshot-app --lib` (new unit tests: arm→insert, sweep-on-start helper)
- [ ] Full gate: fmt + clippy + `cargo test --workspace --all-targets`

**Dependencies:** Task 001

**Files likely touched:**
- `crates/flintshot-app/src/app_window.rs` (`Shared`, arm helper, six call sites, outer-loop tail)

**Docs:**
- GOTCHA comment at the arm helper: resume must pass `Some(id)` or it double-inserts

**Estimated scope:** Medium
