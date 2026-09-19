# ADR-0016: Standing gate uses `--workspace` and scoped `--doc`

**Context.** The workspace sets `default-members = ["crates/flintshot-app"]`
so a casual `cargo build` is fast. Consequently a bare
`cargo test --all-targets` exercises only `flintshot-app` (which has no
tests), and `cargo test --doc` errors on bin crates that have no library
target (`flintshot-app`, `flintshot-cosmic`). The original standing gate
as written ran both bare and thus silently skipped `flintshot-core`'s
entire test suite (its unit tests plus the `no_gui_types_in_core`
integration test) and aborted the doc-test step on the first bin crate.

**Decision.** The standing gate and CI scope with `--workspace` and
explicit package selectors:

- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo test --doc -p flintshot-core -p flintshot-editor-egui`

Doc tests are scoped to the library crates only; binary crates are
excluded.

**Consequences.**

- Every crate's tests and lints run on every gate invocation; local
  results match CI. (The justfile later added `-q` to quiet local
  output — raw logs now differ cosmetically, results do not.)
- `flintshot-cosmic` (opt-in, heavy system deps) is built and linted in
  CI's dedicated `cosmic-build` job, not the default local gate.
- Any new library crate must be added to the `--doc` package selector
  list to get its doc tests collected.
- Documented in `AGENTS.md` Commands and `CONTRIBUTING.md` so
  contributors copy the correct commands.
