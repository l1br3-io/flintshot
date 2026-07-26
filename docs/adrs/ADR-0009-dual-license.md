# ADR-0009: Dual `MIT OR Apache-2.0` license

**Context.** Three reasonable choices:

- MIT only — most permissive, single license
- Apache-2.0 only — permissive + patent grant
- `MIT OR Apache-2.0` — Rust ecosystem default (used by std, tokio,
  serde, etc.)

**Decision.** Dual `MIT OR Apache-2.0`. Include both `LICENSE-MIT` and
`LICENSE-APACHE` files; Cargo `license` field lists both.

**Consequences.**
- Maximum compatibility: downstream users pick whichever they prefer
- Patent grant from Apache-2.0 available to those who want it
- GPL-compatible (matters if we ever want to copy code from Flameshot,
  which is GPL)
- Slight license-text duplication
