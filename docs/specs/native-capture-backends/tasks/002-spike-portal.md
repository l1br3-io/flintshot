# Task 002: Spike B — portal Screenshot behavior per desktop

**Description:** Research task. Characterize the
`org.freedesktop.portal.Screenshot` call on every desktop this machine
can reach (GNOME, KDE, COSMIC, xdg-desktop-portal-wlr — whichever are
installed): does it prompt, how fast is it, what does the returned
image cover (one output vs full virtual desktop), is the cursor
included, and does the `interactive` option change any of that. Uses
`gdbus`/`busctl` one-liners plus a scratch zbus probe under
`/tmp/opencode/` — **no changes to the repo tree**. Findings gate
Task 6's scope (auto slot vs pin-only) and the crop-to-monitor
strategy.

**Acceptance criteria:**
- [ ] Per-portal-impl results recorded under `## Findings`: prompt
      behavior (none / first-call / every-call), end-to-end latency,
      returned URI format, image dimensions vs monitor count
- [ ] Cursor presence in the returned image verified (pixel-diff a
      with/without-cursor scenario or visual check)
- [ ] Coverage mapping: whether the image is one output, the primary
      monitor, or the whole virtual desktop — this decides how
      `capture_monitor(Some(n), ..)` crops
- [ ] Side-quest answered: does xcap *monitor enumeration* (not
      capture) ever trigger a portal prompt on Wayland?
- [ ] Verdict recorded: portal earns an `auto` slot, or `pin-only`;
      `spec.md` precedence amended if demoted

**Verification:**
- [ ] Findings contain per-impl concrete measurements
- [ ] Human confirms the verdict before Task 6 starts

**Dependencies:** None (parallelizable with Task 1, Task 3)

**Files likely touched:**
- `docs/specs/native-capture-backends/tasks/002-spike-portal.md` (findings appended)
- `docs/specs/native-capture-backends/spec.md` (only if precedence changes)

**Docs:**
- Findings feed Task 8's ADR.

**Estimated scope:** Small (1-2 files; scratch probe lives outside the tree)

## Findings

_(filled in during execution)_
