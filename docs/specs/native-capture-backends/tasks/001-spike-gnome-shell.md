# Task 001: Spike A — GNOME Shell Screenshot service callable-ness

**Description:** Research task. Determine whether a plain session-bus
client can call GNOME Shell's screenshot service on the current GNOME
installation, what the method surface looks like, and how monitor
selection maps. Builds a scratch probe (standalone cargo project under
`/tmp/opencode/`, zbus blocking) — **no changes to the repo tree**.
Findings gate Task 5's scope and may amend the spec's precedence table.

**Acceptance criteria:**
- [ ] Probe results recorded under `## Findings` below: method surface
      at `/org/gnome/Shell/Screenshot` (and the legacy path), incl.
      `Screenshot` / `ScreenshotArea` / `ScreenshotMonitor` presence
      and args (cursor, flash, filename)
- [ ] Callable verdict: does an unrestricted session-bus caller
      succeed, or is the interface portal/allowlist-restricted?
- [ ] Monitor mapping: how the service's monitor selector (index or
      rect) corresponds to xcap's enumeration order — verified by
      comparing a capture's dimensions against `MonitorInfo` geometry,
      including on a scaled display if available
- [ ] Timing: end-to-end ms for one `ScreenshotArea`-style call
      (D-Bus + file write + PNG decode), cursor-inclusion honored
- [ ] Verdict recorded: `implement` / `drop-to-portal`, and if it
      changes anything, `spec.md` §Auto-detection precedence amended

**Verification:**
- [ ] Findings section contains concrete numbers/paths from the live
      session (not just documentation quotes)
- [ ] Human confirms the verdict before Task 5 starts

**Dependencies:** None (parallelizable with Task 2, Task 3)

**Files likely touched:**
- `docs/specs/native-capture-backends/tasks/001-spike-gnome-shell.md` (findings appended)
- `docs/specs/native-capture-backends/spec.md` (only if precedence changes)

**Docs:**
- Findings feed Task 8's ADR; cite this task file from the ADR.

**Estimated scope:** Small (1-2 files; scratch probe lives outside the tree)

## Findings

_(filled in during execution)_
