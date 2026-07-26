# ADR-0004: Design the `Backend` upload trait up front

**Context.** Imgur is the only upload target for v1. We could hardcode
Imgur until another backend is requested.

**Decision.** Define the `Backend` trait in Phase 0 and ship only
`ImgurBackend` in Phase 4. The trait is cheap; the rewrite is not.

**Consequences.**
- Adding 0x0, SFTP, or custom backends later is purely additive
- Forces us to think about credentials, progress, and deletion up front
- Slight over-engineering for v1 (we ship one impl)
- The trait shape might need revision once we hit a backend it doesn't
  fit
