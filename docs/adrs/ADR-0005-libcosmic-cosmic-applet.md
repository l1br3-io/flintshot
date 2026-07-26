# ADR-0005: `libcosmic` for the COSMIC applet, as a separate binary

**Context.** COSMIC desktop (System76, stable Dec 2025) uses `libcosmic`
(iced-based) for its applets. Our editor uses egui. We could:

1. Build a libcosmic applet as a separate binary that talks IPC to the
   daemon (chosen)
2. Reimplement the editor in libcosmic and skip egui entirely
   (rejected — loses live overlay flexibility)
3. Build a generic tray icon only and skip COSMIC panel integration
   (rejected — user explicitly asked for COSMIC applet support)

**Decision.** Add a `flintshot-cosmic` crate that produces a separate
binary. The applet is a panel button + popup that sends IPC commands to
the daemon. No shared UI code with the egui editor.

**Consequences.**
- Two GUI toolkits coexist without conflict
- The applet is small (~3-5 files of popup UI)
- We can reuse `flintshot-core::ipc` types directly
- Two GUI toolkits in one project — some duplicated theming work
- `libcosmic` brings system deps (cmake, fontconfig, freetype,
  xkbcommon) that complicate CI
