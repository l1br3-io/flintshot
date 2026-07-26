# ADR-0013: `FlintshotError` variants carry `detail`, not `source`

**Context.** `thiserror`'s `#[error]` derive treats any struct field
literally named `source` as the `std::error::Error::source()` chain link.
Such a field must satisfy `std::error::Error` (typically `dyn Error`),
which a plain `String` does not. Phase 1's capture-error variant was
specced as `FlintshotError::Capture { source: String }`; written
verbatim it fails to compile.

**Decision.** Name the message field `detail` across every
`FlintshotError` variant — `Capture { detail }`, `Config { path,
detail }`, `Shell { detail }`, … — and match downstream code on
`{ detail }`.

**Consequences.**

- Compiles cleanly; `thiserror` does not try to chain a `String` as an
  error source.
- One consistent field name across all variants; a single mental model.
- Error messages and the macOS "permission" substring contract are
  preserved verbatim — only the field name changed.
- If a real error-chain source is ever needed, add a dedicated
  `#[source] source: Box<dyn Error>` field alongside `detail`, rather
  than overloading the message field.
