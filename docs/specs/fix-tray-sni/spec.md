---
status: shipped
started: 2026-08-29
shipped: 2026-08-29
---

# Spec — Fix tray SNI registration: icon never appears on COSMIC

> Bug-fix effort. Surfaced by the launcher effort's L5 pass
> (2026-08-28) and recorded as TEST_MATRIX "L5 pass findings" 3; it
> fails launcher row 4 and blocks launcher-desktop-entry exit
> criterion 1 (that effort stays `active` until this lands). This is
> the last of the three pre-existing bugs found that day — the other
> two shipped in `fix-delegated-capture` (2026-08-29).

## Objective

The tray icon appears and the tray menu works on a COSMIC (and any
SNI-host) desktop. Today the daemon (tray build) exports its bus
name and `/StatusNotifierItem` + `/org/ayatana/NotificationItem`
objects, but never completes `RegisterStatusNotifierItem` with the
`org.kde.StatusNotifierWatcher` host (cosmic-applet-status-area):
`RegisteredStatusNotifierItems` doesn't include flintshot and no
icon renders. Other apps (syncthing-gtk, pcloud) register fine on
the same watcher — the bug is ours.

Success looks like: after `flintshot daemon` starts (tray-default
build), the icon is present in the COSMIC status area, and clicking
menu items does what they say (Capture region opens the editor via
worker dispatch, Bind shortcuts… opens the portal dialog, Quit
exits) — with no regression where no SNI host exists or on X11.

## Diagnosis (leading hypothesis, to be confirmed live)

`daemon.rs`'s platform loop never iterates the GLib/GTK main
context. It calls `gtk::init()` + `TrayIconBuilder::build()` once
(`tray.rs`), then blocks on `recv_timeout(100 ms)` forever. On
Linux, `tray-icon`'s backend is libappindicator, which drives its
D-Bus lifecycle (including the `RegisterStatusNotifierItem` call
and watcher-owner tracking) from GLib main-context sources — and
muda delivers menu events through the same loop. With nobody
iterating the context, the objects get exported during construction
but the registration call never dispatches. The daemon's module doc
(daemon.rs:12-15) asserts "raw polling suffices on Linux (both
libraries pump the OS internally)" — that claim is true for
`global-hotkey`'s crossbeam receiver and false for the GTK-backed
tray half; this effort corrects both code and claim.

The alternative reading (registration races the watcher's
availability at session start) is subsumed: appindicator's retry /
`NameOwnerChanged` handling is *also* GLib-source-driven, so a
pumped loop fixes both the static and the racing cases. If live
debugging disproves the hypothesis, stop and re-decide (see
Boundaries).

## Technologies

No new dependencies. The fix uses the `gtk` crate's re-exported
`glib` (already behind the `tray` feature, Linux-scoped):
`glib::MainContext::default().iteration(false)` pumped per poll
tick. `tray-icon 0.24` + `muda` + libappindicator stay; replacing
them with `ksni` (pure-Rust SNI) is explicitly out of scope for
this effort absent disproof.

## Commands

Standing repo commands (AGENTS.md). Effort-specific:

```sh
cargo build -p flintshot-app --features tray   # the affected build
cargo test -p flintshot-app --features tray    # new pump unit tests
cargo check -p flintshot-app --no-default-features  # non-tray must stay clean
```

L5 probe commands (maintainer runs, agent guides — recorded in
TEST_MATRIX with output):

```sh
gdbus call --session --dest org.kde.StatusNotifierWatcher \
  --object-path /StatusNotifierWatcher \
  --method org.kde.StatusNotifierWatcher.GetRegisteredStatusNotifierItems
```

## Project structure

| Touch                         | Where                                        |
| ----------------------------- | -------------------------------------------- |
| GLib pump helper + loop wiring | `crates/flintshot-app/src/daemon.rs`        |
| Wrong "polling suffices" doc   | `crates/flintshot-app/src/daemon.rs` header |
| TEST_MATRIX row 4 + finding 3  | `docs/TEST_MATRIX.md`                       |
| CHANGELOG                      | `CHANGELOG.md`                               |
| ADR (pump decision)            | `docs/adrs/ADR-0031-*.md` (number at write)  |

`tray.rs`, `flintshot-core`, `flintshot-editor-egui` are not
expected to change.

## Code style

Standing conventions (AGENTS.md): `#![forbid(unsafe_code)]`, no
`unwrap()`/`expect()` outside tests, comments only for non-obvious
"why", `color_eyre` in bins. The pump is `#[cfg(all(feature =
"tray", target_os = "linux"))]` so non-tray and non-Linux builds
never see it; macOS run-loop needs stay future work.

## Testing strategy

- **Unit (headless, RED first):** the pump dispatches GLib sources.
  Schedule `glib::idle_add` setting an `AtomicBool` on the default
  main context; assert it never fires on an unpumped spin, fires
  after the pump helper runs. Pure glib needs no display — the test
  runs under `--features tray` without X11. Also: pump is bounded
  (returns even when the context would block) so the poll loop
  can't stall on a busy source.
- **Manual (L5, maintainer runs):** tray-default build on COSMIC —
  icon registers (`GetRegisteredStatusNotifierItems` includes the
  flintshot item), each menu path dispatches, daemon-started-before-
  watcher race covered by one restart-ordering check, no-SNI-host
  and X11 behavior unchanged. Results recorded in TEST_MATRIX with
  commit hash; no pass is simulated.
- **CI honesty:** the default test jobs can't run tray-feature
  tests (feature is opt-in); extending the `tray-build` job to also
  run them is a *ask-first* item, not assumed.

## Boundaries

- **Always:** pump bounded (non-blocking iterations only — a busy
  context must never wedge the daemon loop); fix the daemon module
  doc that this effort proves wrong; record the L5 evidence in
  TEST_MATRIX before declaring done.
- **Ask first:** CI changes (e.g. running tray-feature tests in the
  `tray-build` job); any new dependency; touching `tray.rs` menu
  structure; changes to POLL_TIMEOUT semantics.
- **Never:** switch tray backends (`ksni` or otherwise) inside this
  effort — if the live session disproves the no-pump hypothesis,
  stop and re-spec rather than fix-forward; `#[allow(...)]` without
  a justifying comment; unsafe code.

## Success criteria

1. On COSMIC with the watcher running, `flintshot daemon` (tray
   build) registers: its item appears in
   `RegisteredStatusNotifierItems` and the icon renders in the
   status area.
2. Every tray menu action works end-to-end: Capture region /
   fullscreen (incl. per-monitor submenu) spawn one worker via the
   dispatch table, Bind shortcuts… opens the portal dialog, Open
   config opens the editor, Quit exits the daemon.
3. Daemon started before the watcher (session-start ordering)
   still registers once the watcher appears.
4. No regression: no-SNI-host desktop keeps the warn-and-continue
   path (TEST_MATRIX row for tray-without-host); X11 tray still
   shows; non-tray builds compile and behave identically.
5. The pump unit tests fail if the pump is removed (regression
   seam, same spirit as the `--standalone` argv tests).

## Decisions

- **Pump the default GLib context inside the existing poll loop** —
  smallest seam that fixes registration *and* menu delivery; caps
  GLib dispatch latency at POLL_TIMEOUT (100 ms), imperceptible for
  tray interactions. Keeps ADR-0018's architecture (main-thread
  platform loop, tokio side thread) intact.
- **glib-only test seam, no GTK init in unit tests** — glib
  sources run without a display, so the pump is provable headless;
  the appindicator hypothesis link is documented and confirmed by
  the L5 pass instead of simulated.
- **Correct the false doc claim, not just the code** — daemon.rs's
  "raw polling suffices" header misled two efforts; the fix rewrites
  it to say what each event source actually needs.

## Open questions

1. Does cosmic-applet-status-area require the Ayatana
    (`/org/ayatana/NotificationItem`) interface, the KDE one, or
    accept both? Published objects suggest both are exported today;
    the L5 session should note which the watcher consumes.
    *Answered by the L5 pass: TEST_MATRIX's "L5 runbook —
    fix-tray-sni" step 3 records the object-path suffix the watcher
    registered (no code change expected either way).*
2. Does the `tray-build` CI job grow a test run (ask-first item) or
    stay compile-only? Decided when the unit tests land.
    *Resolved: stayed compile-only — no CI change landed; ADR-0031
    records the reasoning (the real-path proof is the manual L5
    run, not CI).*
