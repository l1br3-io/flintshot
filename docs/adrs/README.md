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
