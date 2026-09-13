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

**Executed:** 2026-09-13. No GNOME session on this machine
(COSMIC/Wayland here), so this spike is source-level research against
gnome-shell upstream (GitLab, `main` + `gnome-48` + `gnome-42`
branches), corroborated by user reports. Live-D-Bus verification was
not possible; confidence below reflects that.

### Method surface at `/org/gnome/Shell/Screenshot`

From `data/dbus-interfaces/org.gnome.Shell.Screenshot.xml` and
`js/ui/screenshot.js` (`main`, GNOME 49-era):

| Method | Args | Restricted? |
| --- | --- | --- |
| `Screenshot` | `in(include_cursor:b, flash:b, filename:s) → out(success:b, filename_used:s)` | yes |
| `ScreenshotArea` | `in(x:i, y:i, width:i, height:i, flash:b, filename:s) → out(success:b, filename_used:s)` | yes |
| `ScreenshotWindow` | `in(include_frame:b, include_cursor:b, flash:b, filename:s) → out(success:b, filename_used:s)` | yes |
| `InteractiveScreenshot` | `→ out(success:b, uri:s)` — opens GNOME's Screenshot UI | yes |
| `SelectArea` | `→ out(x:i, y:i, width:i, height:i)` — interactive rubber band | yes |
| `PickColor` | `→ out(result:a{sv})` | **no** (`_createScreenshot(invocation, false, false)`) |
| `FlashArea` | `in(x:i, y:i, width:i, height:i)` | n/a (no pixels) |

- **No `ScreenshotMonitor` method exists** — the spec's
  `Screenshot vs ScreenshotMonitor` question resolves to: only
  `ScreenshotArea(x, y, w, h)` grabs a rect, in stage/logical px.
  Monitor selection would have to be rect-based.

### Callable verdict: **restricted — plain session-bus callers denied**

Since GNOME 41 the service wraps every grab method in
`DBusSenderChecker.checkInvocation`
(`js/misc/util.js:344-410`), which throws
`Gio.DBusError.ACCESS_DENIED` ("`<method> is not allowed`") unless:

1. the shell runs in `unsafe_mode` (debug flag, not a production
   path), or
2. the caller's unique bus name maps to an allowlisted well-known
   name.

Allowlist contents, verified from branch sources:

- **GNOME 41–48** (`gnome-42`, `gnome-48` branches):
  `org.gnome.SettingsDaemon.MediaKeys`,
  `org.freedesktop.impl.portal.desktop.gtk`,
  `org.freedesktop.impl.portal.desktop.gnome`,
  `org.gnome.Screenshot` (the gnome-screenshot tool).
- **GNOME 49** (`main` as of 2026-09): shrunk to
  `org.gnome.SettingsDaemon.MediaKeys` +
  `org.freedesktop.impl.portal.desktop.gnome` only — gnome-screenshot
  and the gtk portal impl lost access (matches Arch forum reports of
  gnome-screenshot breaking on 49).

Corroborating user reports: Qt forum ("stopped working after Gnome
40"), Fedora/Red Hat bugzilla (gnome-screenshot broke on 41),
askubuntu 1411874.

A flintshot worker owns no allowlisted name, so every grab call
returns `ACCESS_DENIED` on stock GNOME 41+. There is no
non-sandboxed/same-user exemption — the check is purely
name-allowlist-based.

### Monitor mapping

Moot under the drop verdict, but for the record: the service has no
monitor-index selector; the only rect grab is `ScreenshotArea` in
logical (stage) px. Mapping to xcap physical px would need scale
factors per monitor — the classic scaled-display trap the plan flags.

### Timing

Not measurable live (no GNOME session on this box). The GNOME portal
impl (`xdg-desktop-portal-gnome`) calls the same internal shell API
the service exposes, so Task 2's portal latency is the practical
upper bound for the same code path.

### Verdict: `drop-to-portal`

GNOME Shell Screenshot is not callable by unprivileged session-bus
clients on any supported GNOME (41+), and the allowlist is shrinking
(49 dropped two entries). Implementing `GnomeShellCapturer` would
produce a backend that hard-fails `available()`/capture on every
stock GNOME — dead code.

Consequences for this effort:

- Task 5 shrinks to its documented fallback shape: no
  `gnome_shell.rs`, no registry slot; the zbus/no-GUI-test scaffolding
  lands via Task 6 (portal) instead.
- `spec.md` §Auto-detection precedence: the `Wayland + GNOME Shell
  svc` row is deleted; GNOME Wayland rides the general
  `portal → xcap` row.
- Spike B's portal measurements now carry the whole Wayland-native
  story — its verdict gates whether Wayland native capture exists at
  all (vs xcap-only + portal pin-only).

Sources:
- `js/ui/screenshot.js` @ `main` (allowlist, `_createScreenshot`
  gating: lines ~2842-2905; method impls 3012-3190)
- `js/misc/util.js` @ `main` (`DBusSenderChecker`, 344-410)
- `data/dbus-interfaces/org.gnome.Shell.Screenshot.xml` @ `main`
- `js/ui/screenshot.js` @ `gnome-48` (allowlist w/ `org.gnome.Screenshot`)
  and `gnome-42` (same)
