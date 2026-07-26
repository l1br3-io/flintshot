# ADR-0011: `postcard` for IPC message encoding

**Context.** IPC messages need a serialization format. Options:

- JSON — ubiquitous, human-debuggable, larger on the wire
- `bincode` — Rust-native, fast, no schema evolution story
- `postcard` — `serde`-compatible, compact, no_std-friendly, designed
  for embedded/CW

**Decision.** Use `postcard` for IPC. Length-prefixed frames. Keep JSON
as an option behind a feature flag for debugging if needed.

**Consequences.**
- Compact on the wire (matters for high-frequency event streams)
- Shares `serde` types with config and any future persistence
- Less debuggable than JSON without a tool — mitigated by `postcard`
  CLI debugger
- No schema evolution story; bump IPC version field if types change
