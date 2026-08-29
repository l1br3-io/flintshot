# Task 004: ADR-0031 GLib context pumping for the tray

**Description:** Write
`docs/adrs/ADR-0031-glib-context-pumping-for-tray.md` in the
established **Context · Decision · Consequences** format (numbered
to match `ls docs/adrs/` next free slot — 0031 unless taken).
Records why the daemon's platform loop must iterate the default
GLib main context (libappindicator's SNI registration and muda's
menu delivery are GLib-source-driven; the loop blocked on
`recv_timeout` so neither ever dispatched), the bounded-pump
design (non-blocking iterations + per-tick cap so a busy source
cannot wedge the loop; latency capped at `POLL_TIMEOUT`), why it
lives in `flintshot-app`'s daemon loop rather than a dedicated
GTK main thread (ADR-0018's architecture is preserved; smallest
seam), the glib-via-gtk re-export choice (zero new deps), and why
the `ksni` backend swap was rejected for this effort (pure-Rust
appeal acknowledged, but it is a rewrite gated behind hypothesis
disproof — spec boundary).

**Acceptance criteria:**

- [x] ADR file exists at the 0031 slot with Context / Decision /
      Consequences sections (incl. cost, alternatives rejected,
      link to TEST_MATRIX finding 3)
- [x] `docs/adrs/README.md` index row added (established pattern)
- [x] Claims consistent with code landed in Tasks 1–2

**Verification:**

- [x] Format matches a sampled recent ADR (e.g. ADR-0030)

**Dependencies:** Task 2 (records its decision)

**Files likely touched:**

- `docs/adrs/ADR-0031-glib-context-pumping-for-tray.md` (new)
- `docs/adrs/README.md`

**Docs:**

- This task *is* docs.

**Estimated scope:** Small: 2 files
