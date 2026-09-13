# Task 007: `flintshot backends` debug subcommand

**Description:** Add a `flintshot backends` subcommand that prints,
for every registered backend: id, display name, availability (+ detail
string on failure), and which backend `auto` would select right now.
Powered by the same probe/registry API as `select_backend` (expose a
`capture::backend_report()`-style function taking the injectable seam
so output formatting is unit-testable headless). Always exits 0 —
it's a diagnostic, not a health gate.

**Acceptance criteria:**
- [ ] Output lists all registered backends with per-backend status
      lines and a final `auto selects: <id>` line
- [ ] Report logic is a pure function of (session kind, probe
      results) — unit-tested with fakes; the subcommand is a thin
      printer around it
- [ ] `--help` text mentions the config/CLI override keys
- [ ] No capture is performed by the subcommand (probes only — never
      grabs pixels or writes files)

**Verification:**
- [ ] `cargo test -p flintshot-app --quiet` green (printer unit tests)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] Manual: `cargo run -- backends` on a live session; output
      matches expectations for that session kind

**Dependencies:** Task 3 (probe/registry API); fullest output after
Tasks 5-6, but works with xcap-only registration

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs`
- `crates/flintshot-core/src/capture/mod.rs` (report fn)

**Docs:**
- None beyond CLI help.

**Estimated scope:** Small (2 files)
