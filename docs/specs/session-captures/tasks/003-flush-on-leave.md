# Task 003: Flush-on-leave + active-session plumbing

**Description:** Make leaving the editor preserve live annotations —
the fix for the snapshot-vs-live trap. In egui `window.rs`,
`request_route` gains a guard: when the current route is
`Route::Editor` and the target is not, it calls the new
`RouteHost::editor_route_left(&EditorState)` **with the live state
before the swap** (the route slot's construction snapshot — the source
of today's documented loss at `app_window.rs:2062` — must not be
used). The app's `RouteHostBridge` impl upserts into the store
(update `anns`, ensure entry exists). For window-closing exits the
editor route never passes through `request_route` — the outer loop's
close handling (`app_window.rs:~850`, `exit.route` is Editor) flushes
the same way for `NewCapture` / hidden-by-event closes (user quit is
covered by Task 002's `clear()`). Track the open editor's id in
`Shared.active_session: Option<SessionId>` — set when the editor route
opens (resume or fresh), cleared on leave — for Task 4's save removal.
`RoutedEditor` (egui) carries `session_id` alongside `state` so the
plumbing survives the PendingEditor↔RoutedEditor mapping
(`to_routed` / `route_to_page`).

**Acceptance criteria:**
- [ ] Editor → Home (topbar) / Settings / Esc-Cancelled in-window route swap flushes live `anns` into the store entry (unit: bridge upsert called with the live state; egui test: route swap through the host)
- [ ] `NewCapture` close (`+`/Ctrl+N) stashes current annotations before the capture proceeds — the "Win11-style discard" note (`shell.rs:29`) is now stale; update that doc comment
- [ ] Hidden-by-event close with the editor open flushes on the outer-loop path
- [ ] `Shared.active_session` set on editor-route open, cleared on leave; `to_routed`/`route_to_page` round-trip `session_id`
- [ ] Editor-chrome Open/Paste swap (page replaced via close) flushes the outgoing image's annotations before the replacement arms

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui --lib` (route-leave flush test, host double) + `cargo test -p flintshot-app --lib` (upsert + outer-loop flush)
- [ ] Full gate commands

**Dependencies:** Task 002

**Files likely touched:**
- `crates/flintshot-editor-egui/src/window.rs` (`request_route` guard, `RoutedEditor.session_id`, `RouteHost` calls)
- `crates/flintshot-editor-egui/src/shell.rs` (`RouteHost` trait def, if it lives there)
- `crates/flintshot-app/src/app_window.rs` (`RouteHostBridge` upsert, outer-loop flush, `active_session`, `to_routed`/`route_to_page`)
- `crates/flintshot-core/src/shell.rs` (stale doc comment on `NewCapture`)

**Docs:**
- GOTCHA comment at `request_route`: the route slot is a construction snapshot; flush must use the live editor state

**Estimated scope:** Medium
