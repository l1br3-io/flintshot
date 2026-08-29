# Task 004: ADR-0032 — poke-to-activate the SNI watcher

**Description:** Write `docs/adrs/ADR-0032-poke-to-activate-sni-watcher.md`
in the repo's **Context · Decision · Consequences** shape and add it
to `docs/adrs/README.md`'s index. Content per the spec's Decisions
section: why a method call to an activatable well-known name (and
only that) triggers D-Bus activation while `NameOwnerChanged`
watching never does; the name pair and their order (standard first,
`com.system76.CosmicStatusNotifierWatcher` for activation on
COSMIC); the 30 s `NameHasOwner` poll over an event-driven
NameLost→poke design (same zbus surface, more lifecycle, for one
cheap bus-daemon call per interval); the dedicated-thread +
channel-shutdown topology (main loop must stay non-blocking,
ADR-0031); and the `zbus` direct-dep rationale — dep edge onto
ashpd's unified 5.18, not a new crate build, chosen over gio/GLib
DBus to avoid a second D-Bus stack; note the pre-existing zbus 4.4
under keyring as unrelated and unchanged.

**Acceptance criteria:**

- [x] ADR-0032 exists with Context / Decision / Consequences,
      numbered `0032`, slug-consistent filename
- [x] Covers: activation semantics (why only method calls activate),
      names + order, interval + poll-vs-event trade-off, thread +
      shutdown design, zbus-direct-dep rationale incl. the keyring
      zbus-4 note, and the OQ2 follow-up fence (back-off after
      futile activations, only if the idle-exit cycle is observed)
- [x] Cites the live evidence (L5 2026-08-30 session,
      `busctl … Peer Ping` activation observation) and the fix-tray-sni
      effort's registration finding
- [x] `docs/adrs/README.md` index row added

**Verification:**

- [x] `python3 .opencode/harness/scripts/check-refs.py` (or manual
      scan) — no dangling references from the ADR
- [x] Index lists ADR-0032 in order

**Dependencies:** Task 2 (records what was actually built, not what
was planned)

**Files likely touched:**

- `docs/adrs/ADR-0032-poke-to-activate-sni-watcher.md` (new)
- `docs/adrs/README.md` (index row)

**Docs:**

- This task *is* docs.

**Estimated scope:** Small: 2 files
