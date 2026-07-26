# ADR-0007: TOML for config

**Context.** Standard formats: JSON (no comments), YAML (ambiguous,
indentation-sensitive), TOML (Rust ecosystem default), RON (Rust-specific,
niche).

**Decision.** Use TOML via `serde` + `toml_edit`. Use `notify` to
hot-reload on file changes.

**Consequences.**
- Conventional for Rust CLI tools; users will guess correctly
- `toml_edit` lets us write back without losing comments
- Deeply-nested config gets awkward in TOML; we keep the schema flat
