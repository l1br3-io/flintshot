# Plan: Native capture backends

## Overview

Wrap all capture functionality in a `Capturer` trait inside
`flintshot-core`, keeping `xcap` as the default provider and universal
fallback, and add Linux-native backends that talk to the OS's own
screenshot services over D-Bus (`zbus`, blocking facade): GNOME Shell
Screenshot and the cross-desktop `org.freedesktop.portal.Screenshot`
portal. Selection is runtime auto-detect (session kind + bus-name
probes) with config `[capture] backend` / CLI `--backend` overrides.
Two spikes precede backend implementation because their findings can
drop or reorder a backend.

## Architecture Decisions

- **Trait lives in `flintshot-core::capture`** (module split, no new
  crate) — keeps `flintshot_core::monitors()` and the compat free
  functions stable for `flintshot-app` / `flintshot-cosmic`.
- **Sync trait methods** — every capture call site is synchronous
  worker code; D-Bus backends use `zbus::blocking` internally.
- **zbus direct, ashpd stays banned in core** — the no-GUI guard test
  deliberately forbids `use ashpd` (Phase 6: portal client lives in
  `flintshot-app`). Both native backends hand-roll D-Bus via zbus;
  the guard gains a path-scoped `use zbus` allowance under
  `src/capture/` only. (Amended into `spec.md` §Technologies during
  planning — the spec originally assumed ashpd.)
- **Enumeration always rides xcap; only the grab goes through the
  registry** — monitor listing feeds tray/applet menus and must stay
  cheap (no bus round-trip per menu open). Native backends delegate
  `monitors()` to xcap and map capture coordinates via monitor
  geometry, never via shell/portal index numbering (index semantics
  differ per service — spikes verify the mapping).
- **Preference resolution mirrors upload** — CLI `--backend` > config
  `[capture] backend` > `"auto"`; unknown id in *config* → warn +
  auto (string-tolerant policy, same as `interaction-mode`); unknown
  id on the *CLI* → hard error (same as upload's
  `validate_auth_backend`); known id whose availability probe fails →
  hard error with the probe detail. `"auto"` never fails — xcap is
  the terminal fallback.
- **Region capture stays a client-side crop** (not on the trait) —
  flintshot's UX is capture-monitor → select in overlay/editor.

## Task Index

### Phase 1: Spikes (parallelizable; findings can reshape Phase 3)
- [ ] Task 1: Spike A — GNOME Shell Screenshot service callable-ness → `tasks/001-spike-gnome-shell.md`
- [ ] Task 2: Spike B — portal Screenshot behavior per desktop → `tasks/002-spike-portal.md`

### Checkpoint: Spike findings reviewed
- [ ] Verdicts recorded in task files; precedence table confirmed or amended in `spec.md`
- [ ] Human approves backend scope before Phase 3

### Phase 2: Foundation (can start in parallel with Phase 1)
- [ ] Task 3: `Capturer` trait + registry + compat wrappers → `tasks/003-capturer-trait-registry.md`
- [ ] Task 4: Config/CLI backend preference + worker grab path → `tasks/004-preference-wiring.md`

### Checkpoint: Foundation green
- [ ] Full finish gate green (`cargo fmt --check`, clippy `-D warnings`, both test commands)
- [ ] xcap behavior byte-identical (smoke test path unchanged)

### Phase 3: Native backends (Tasks 5/6 parallelizable after their spikes + Task 3)
- [ ] Task 5: `GnomeShellCapturer` → `tasks/005-gnome-shell-backend.md`
- [ ] Task 6: `PortalScreenshotCapturer` → `tasks/006-portal-backend.md`

### Checkpoint: Native paths live
- [ ] Live-session manual capture through each implemented backend
- [ ] Full finish gate green; `no_gui_types_in_core.rs` still enforces the ashpd ban

### Phase 4: Surface + record
- [ ] Task 7: `flintshot backends` debug subcommand → `tasks/007-backends-subcommand.md`
- [ ] Task 8: ADR + docs (ARCHITECTURE, CHANGELOG, TEST_MATRIX, AGENTS) → `tasks/008-adr-docs.md`

### Checkpoint: Complete
- [ ] All spec success criteria met
- [ ] Ready for `/review`

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| GNOME 41+ restricts direct `org.gnome.Shell.Screenshot` callers | High — drops Task 5 | Spike A first; fallback is portal-only Wayland path (spec already allows) |
| Portal Screenshot prompts per call on some compositors | High — portal demoted to pin-only | Spike B measures; precedence amended, auto skips it |
| Monitor-index semantics differ between xcap and shell/portal | Med — wrong-monitor captures | Backends capture by monitor *rect* (geometry), never by service index |
| zbus version conflicts with ashpd's zbus in `flintshot-app` | Low/Med — duplicate D-Bus stacks | Pin same zbus major as ashpd's; cargo dedupes identical versions |
| Blocking D-Bus inside worker's CaptureGuard window deadlocks | Low — capture hang | Worker is sync, no runtime held during grab; timeouts on every D-Bus call |
| Portal/gnome write PNG to disk → decode latency (~50-150ms @ 4K) | Low | Acceptable vs portal-prompt pain; Spike B measures end-to-end |

## Open Questions

- Spike A verdict: implement GNOME backend, or drop to portal-only?
- Spike B verdict: does portal earn an `auto` slot, or pin-only?
- Does Wayland monitor *enumeration* (xcap) ever prompt? (Spike B side-quest; if yes, speccing a portal-based enumeration follow-up)
