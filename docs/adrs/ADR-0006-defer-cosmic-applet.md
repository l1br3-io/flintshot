# ADR-0006: Defer COSMIC applet implementation to Phase 3

**Context.** The applet depends on the IPC protocol from Phase 2
(daemon + tray) being in place. Implementing it earlier means rework.

**Decision.** Ship the `flintshot-cosmic` crate skeleton in Phase 0 (so
CI verifies it builds). Defer the actual popup / IPC-client logic to
Phase 3, alongside pin-to-desktop.

**Consequences.**
- Phase 0 surfaces libcosmic system-deps issues early
- Phase 3 implementation lands after IPC is stable
- The COSMIC crate sits as a stub for Phases 1 and 2 — minor context
  cost for new contributors
