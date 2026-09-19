# Decision Records

Compact ADRs capturing the "why" behind non-obvious technical choices.
Each decision is its own file, numbered `ADR-NNNN`. Superseded decisions
are kept for history — to supersede a decision, add a new
`ADR-NNNN-*.md` file that references the prior one; **never edit an
existing ADR**.

Format: **Context · Decision · Consequences**.

| ADR      | Decision                                                                                |
| -------- | --------------------------------------------------------------------------------------- |
| ADR-0001 | [Pure-Rust stack (eframe/egui) over Tauri](ADR-0001-pure-rust-stack.md)                 |
| ADR-0002 | [Editor-window first, live overlay later](ADR-0002-editor-window-first.md)               |
| ADR-0003 | [`xcap` for screen capture](ADR-0003-xcap-for-capture.md)                                |
| ADR-0004 | [Design the `Backend` upload trait up front](ADR-0004-backend-trait-up-front.md)         |
| ADR-0005 | [`libcosmic` for the COSMIC applet, as a separate binary](ADR-0005-libcosmic-cosmic-applet.md) |
| ADR-0006 | [Defer COSMIC applet implementation to Phase 3](ADR-0006-defer-cosmic-applet.md)         |
| ADR-0007 | [TOML for config](ADR-0007-toml-for-config.md)                                           |
| ADR-0008 | [MSRV pinned at 1.95](ADR-0008-msrv-1-95.md)                                             |
| ADR-0009 | [Dual `MIT OR Apache-2.0` license](ADR-0009-dual-license.md)                             |
| ADR-0010 | [`interprocess` for IPC over D-Bus](ADR-0010-interprocess-for-ipc.md)                    |
| ADR-0011 | [`postcard` for IPC message encoding](ADR-0011-postcard-for-ipc.md)                      |
| ADR-0012 | [Abstract UI toolkit via `Canvas` + `EditorShell` traits](ADR-0012-canvas-editorshell-traits.md) |
| ADR-0013 | [`FlintshotError` variants carry `detail`, not `source`](ADR-0013-error-field-detail-not-source.md) |
| ADR-0014 | [Synchronous `rfd::FileDialog`, not `AsyncFileDialog`](ADR-0014-synchronous-rfd-filedialog.md) |
| ADR-0015 | [`compose_final` is a separate render path from live `render()`](ADR-0015-compose-final-separate-render-path.md) |
| ADR-0016 | [Standing gate uses `--workspace` and scoped `--doc`](ADR-0016-workspace-scoped-standing-gate.md) |
| ADR-0017 | [`ab_glyph` over `cosmic-text` for Phase 1 text tool](ADR-0017-ab-glyph-over-cosmic-text-phase-1.md) |
| ADR-0018 | [Tokio side thread + main-thread platform loop, bridged by mpsc](ADR-0018-tokio-side-thread-main-event-loop-bridge.md) |
| ADR-0019 | [Daemon spawns worker processes, resolves binary via `current_exe`](ADR-0019-daemon-spawns-worker-processes.md) |
| ADR-0020 | [Single-instance lock scoped to the daemon role, with a `--replace` handshake](ADR-0020-single-instance-daemon-role-replace-handshake.md) |
| ADR-0021 | [Text-tool customization data model + Phase 2 multiline pulled forward](ADR-0021-text-tool-customization-data-model.md) |
| ADR-0022 | [Shared Marker/Pencil freehand path: Catmull-Rom in core + per-pixel-once polyline compositing](ADR-0022-shared-freehand-catmull-rom.md) |
| ADR-0023 | [Worker save reporting (`Request::ReportSaved`) + daemon last-saved state](ADR-0023-worker-report-saved.md) |
| ADR-0024 | [Upload trait rework: direct `keyring::Entry` credentials, no progress sink](ADR-0024-upload-trait-rework.md) |
| ADR-0025 | [Worker upload-result reporting (`Request::ReportUploaded`)](ADR-0025-worker-reports-upload-result.md) |
| ADR-0026 | [`reqwest` for upload HTTP](ADR-0026-reqwest-for-http.md) |
| ADR-0027 | [Painter-drawn toolbar glyphs over icon fonts or emoji](ADR-0027-painter-drawn-toolbar-glyphs.md) |
| ADR-0028 | [Capture-then-overlay (frozen frame) for the live overlay](ADR-0028-capture-then-overlay.md) |
| ADR-0029 | [Global hotkeys on Wayland via the xdg-desktop-portal (`ashpd`)](ADR-0029-ashpd-global-shortcuts.md) |
| ADR-0030 | [Ambient tokio runtime for window workers](ADR-0030-ambient-runtime-for-window-workers.md) |
| ADR-0031 | [GLib context pumping for the Linux tray](ADR-0031-glib-context-pumping-for-tray.md)      |
| ADR-0032 | [Poke-to-activate the SNI watcher](ADR-0032-poke-to-activate-sni-watcher.md)               |
| ADR-0033 | [Cloexec-safe single-instance lock (std abstract socket on Linux)](ADR-0033-cloexec-safe-single-instance-lock.md) |
| ADR-0034 | [Build the editor without eframe's accesskit feature](ADR-0034-editor-without-accesskit.md) |
| ADR-0035 | [Selection-first editor phase for `region` captures](ADR-0035-selection-first-editor-fallback.md) |
| ADR-0036 | [Worker-driven tray hiding for the capture grab window](ADR-0036-hide-tray-for-capture.md) |
| ADR-0037 | [Click-anchored menu grace for tray-launched captures](ADR-0037-menu-grace-for-tray-launches.md) |
| ADR-0038 | [Unregister the tray item to close the panel's menu](ADR-0038-unregister-tray-to-close-panel-menu.md) |
| ADR-0039 | [One long-lived tray registration — no drop/rebuild cycle](ADR-0039-one-tray-registration-no-rebuild.md) |
| ADR-0040 | [Flip to Move after every annotation commit](ADR-0040-flip-to-move-after-commit.md) |
| ADR-0041 | [The applet closes its own popup before capture requests](ADR-0041-applet-popup-closes-before-capture.md) |
| ADR-0042 | [The tray as a helper process whose exit closes the panel's popup](ADR-0042-tray-helper-process.md) |
| ADR-0043 | [Selection-phase tool lock — annotate only after the region is committed](ADR-0043-selection-phase-tool-lock.md) |
| ADR-0044 | [Persistent selection survives release, dies with every mutation — and is chrome-only](ADR-0044-persistent-selection-lifecycle.md) |
| ADR-0045 | [The app window as a subscription client — reaper-sourced `SessionEnded`, lock+focus-ping single instance](ADR-0045-app-window.md) |
| ADR-0046 | [New-capture from the editor re-captures in-process (the worker persists)](ADR-0046-editor-new-capture-inprocess.md) |
| ADR-0047 | [The app window closes for a capture — hiding is not a portable guarantee](ADR-0047-app-window-closes-for-capture.md) |
| ADR-0048 | [Capture sessions — Replace policy + a generalized pre-grab handshake](ADR-0048-capture-session-replace-and-prepare.md) |
| ADR-0049 | [Remove pin-to-desktop](ADR-0049-remove-pin-to-desktop.md) |
| ADR-0050 | [`Capturer` trait — `xcap` default behind a registry, portal native on Wayland](ADR-0050-capturer-trait-native-backends.md) |
| ADR-0051 | [One Capture action — the OS's selection UI, post-grab routing, clean break](ADR-0051-single-capture-action-os-selection.md) |
| ADR-0052 | [App-window presence by connection liveness](ADR-0052-app-window-presence-by-connection.md) |
| ADR-0053 | [Single-window app — routed pages, in-process grabs, forwarded triggers](ADR-0053-single-window-app.md) |
| ADR-0054 | [In-window routing — routes, not epochs; destruction reserved for captures](ADR-0054-in-window-routing.md) |
| ADR-0055 | [The side-panel pivot and chip-rect input gating](ADR-0055-pop-up-book-panel-pivot.md) |
| ADR-0056 | [The five-zone layout and action redistribution](ADR-0056-five-zone-layout.md) |
| ADR-0057 | [Additive settings schema with live apply](ADR-0057-settings-config-schema.md) |
| ADR-0058 | [Sticky tools by default](ADR-0058-sticky-tools-by-default.md) |
| ADR-0059 | [Painted vector glyphs, never font-fallback icons](ADR-0059-painted-glyphs-over-font-icons.md) |
| ADR-0060 | [A worker Copy lingers on Linux — process exit wipes the selection](ADR-0060-worker-copy-lingers-on-linux.md) |
| ADR-0061 | [Property-edit undo coalescing by edit kind](ADR-0061-property-edit-undo-coalescing.md) |
| ADR-0062 | [Committed-annotation write-through via lift-out sessions](ADR-0062-text-write-through.md) |
| ADR-0063 | [App workers carry `--standalone`](ADR-0063-app-workers-carry-standalone.md) |
| ADR-0064 | [The Select / Crop / Hand tool remap](ADR-0064-select-crop-hand-tool-remap.md) |
