# Task 001: Core `SessionStore` + temp-dir persistence

**Description:** The storage foundation in `flintshot-core`. New module
`session.rs` defining `SessionId` (monotonic u64), `SessionCapture`
(id, `created_at`, `image`, `anns`), and `SessionStore` — a thread-safe
(`Mutex` inner, after the `CaptureSessions` pattern) in-memory index
mirrored to `<dir>/<id>.png` + `<dir>/<id>.json`. Dir resolution:
`$XDG_RUNTIME_DIR/flintshot/session/`, fallback
`std::env::temp_dir()/flintshot/session/`. PNG encode runs on a
short-lived internal thread (captures are rare); the annotations JSON
rewrites synchronously (tiny payload, crash-safety by design). All IO
is advisory: failure logs a warn and degrades to memory-only — never an
error surfaced to the user. Re-export from `lib.rs`.

**Acceptance criteria:**
- [ ] `insert(image, now)` returns a fresh id, listable immediately; PNG lands on disk (thread) shortly after
- [ ] `update_anns(id, anns)` rewrites `<id>.json`; `remove(id)` drops memory entry + both files; missing files on remove are not errors
- [ ] `list()` returns most-recent-first (id/created_at ordering)
- [ ] `load(id)` decodes PNG + parses JSON back into a `SessionCapture`; corrupt/missing JSON → entry survives with empty `anns` (advisory degrade); missing PNG → `None`
- [ ] `clear()` removes the whole dir; `sweep_stale(dir)` deletes a pre-existing dir's contents (crash leftovers) without erroring on absence
- [ ] Dir resolution honors `XDG_RUNTIME_DIR` override and the temp fallback (testable via injected base / env-independent constructor)
- [ ] Serde types follow `#[serde(rename_all = "kebab-case")]` + `#[non_exhaustive]`; every public item documented (crate has `#![warn(missing_docs)]`)

**Verification:**
- [ ] `cargo test -p flintshot-core` — new unit tests (tempfile-injected dir), RED first
- [ ] `cargo fmt --all --check` + `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test -p flintshot-core --test no_gui_types_in_core` stays green

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/session.rs` (new)
- `crates/flintshot-core/src/lib.rs` (module + re-exports)

**Docs:**
- None yet (Task 007 writes the ADR)

**Estimated scope:** Medium
