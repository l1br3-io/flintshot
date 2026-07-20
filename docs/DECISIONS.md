# Decision Records

Compact ADRs capturing the "why" behind non-obvious technical choices.
Each is numbered; superseded decisions are kept for history.

Format: **Context · Decision · Consequences**.

---

## ADR-0001: Pure-Rust stack (eframe/egui) over Tauri

**Context.** The project's initial `tech.md` declared
`rust + typescript + react`, which pointed at Tauri. Tauri is mature and
battle-tested. However:

- The user wants a Flameshot-like tool with real-time annotation drawing
  over a captured screen.
- A live overlay with low-latency pen/mouse input on a transparent
  fullscreen surface is fundamentally awkward in HTML5 canvas — feasible
  but not native.
- The user explicitly chose to drop TS/React and update `tech.md` to
  pure Rust.

**Decision.** Use `eframe` (egui + winit + glow) for the editor and
overlay UI. No web frontend, no JavaScript toolchain.

**Consequences.**
- Single-language codebase, easier contribution
- Immediate-mode GUI is a natural fit for drawing tools and per-frame
  overlays
- Smaller binary size (no webview runtime)
- Loses the React/TS ecosystem (rich text editing components,
  accessibility widgets)
- egui's text rendering quality requires `cosmic-text` to be wired in
- egui's look-and-feel is distinctive; users wanting a native GTK/Qt
  look will notice

---

## ADR-0002: Editor-window first, live overlay later

**Context.** Flameshot's signature UX is the live overlay — fullscreen
transparent window where you select and annotate over the actual screen.
This is hard cross-platform:

- On Wayland, compositors won't let arbitrary clients grab fullscreen
  input.
- On macOS, it requires the screen-recording permission path plus
  careful window level management.
- On Windows, it requires opaque fullscreen windows with layered
  drawing.

The editor-window model (Satty-style: capture → open normal window
containing the image → annotate → save) works everywhere with no special
privileges.

**Decision.** Ship editor-window as the only interaction model in
Phase 1. Add live overlay in Phase 5 with explicit Wayland fallback.

**Consequences.**
- Phase 1 ships on every platform with no platform-specific surprises
- Same `EditorState` model serves both modes — Phase 5 is mostly a
  different surface, not a different feature set
- Phase 1 UX is less "magical" than Flameshot
- Some users will request live overlay and we have to communicate
  "Phase 5" politely

---

## ADR-0003: `xcap` for screen capture

**Context.** Screen capture options in Rust:

- `xcap` — most adopted, supports X11/Wayland/macOS/Windows, maintained
  by the `pot-app` team
- `screenshots` (formerly `screenshots-rs`) — predecessor to `xcap`,
  deprecated in favor of it
- `scrap` — lower-level, more about continuous capture for streaming
- Custom per-platform bindings via `x11rb`/`xcb`, `core-graphics`,
  `DXGI` — maximum control, maximum work

**Decision.** Use `xcap`. Re-evaluate if its ~25ms-per-capture latency
on Wayland becomes a user-visible problem.

**Consequences.**
- Cross-platform capture out of the box
- Active maintenance, broad adoption
- On Wayland, capture goes through `xdg-desktop-portal`, which shows a
  permission dialog on first use
- We're at the mercy of `xcap`'s update cadence for new platform quirks

---

## ADR-0004: Design the `Backend` upload trait up front

**Context.** Imgur is the only upload target for v1. We could hardcode
Imgur until another backend is requested.

**Decision.** Define the `Backend` trait in Phase 0 and ship only
`ImgurBackend` in Phase 4. The trait is cheap; the rewrite is not.

**Consequences.**
- Adding 0x0, SFTP, or custom backends later is purely additive
- Forces us to think about credentials, progress, and deletion up front
- Slight over-engineering for v1 (we ship one impl)
- The trait shape might need revision once we hit a backend it doesn't
  fit

---

## ADR-0005: `libcosmic` for the COSMIC applet, as a separate binary

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

---

## ADR-0006: Defer COSMIC applet implementation to Phase 3

**Context.** The applet depends on the IPC protocol from Phase 2
(daemon + tray) being in place. Implementing it earlier means rework.

**Decision.** Ship the `flintshot-cosmic` crate skeleton in Phase 0 (so
CI verifies it builds). Defer the actual popup / IPC-client logic to
Phase 3, alongside pin-to-desktop.

**Consequences.**
- Phase 0 surfaces libcosmic system-deps issues early
- Phase 3 implementation lands after IPC is stable
- The COSMIC crate sits as a stub for Phases 1 and 2 — minor context
  cost for new contributors

---

## ADR-0007: TOML for config

**Context.** Standard formats: JSON (no comments), YAML (ambiguous,
indentation-sensitive), TOML (Rust ecosystem default), RON (Rust-specific,
niche).

**Decision.** Use TOML via `serde` + `toml_edit`. Use `notify` to
hot-reload on file changes.

**Consequences.**
- Conventional for Rust CLI tools; users will guess correctly
- `toml_edit` lets us write back without losing comments
- Deeply-nested config gets awkward in TOML; we keep the schema flat

---

## ADR-0008: MSRV pinned at 1.95

**Context.** As of mid-2026, Rust is at ~1.97. Pinning MSRV too tight
excludes users; too loose complicates CI.

**Decision.** MSRV = 1.95 (two stable releases behind current). CI
verifies with `cargo +1.95 build`. Bump on minor releases as needed.

**Consequences.**
- Conservative, inclusive of users on slightly older distros
- Two-release buffer gives us latest-stable idioms without forcing the
  very newest
- We might occasionally have to bump on a transitive dep's MSRV

---

## ADR-0009: Dual `MIT OR Apache-2.0` license

**Context.** Three reasonable choices:

- MIT only — most permissive, single license
- Apache-2.0 only — permissive + patent grant
- `MIT OR Apache-2.0` — Rust ecosystem default (used by std, tokio,
  serde, etc.)

**Decision.** Dual `MIT OR Apache-2.0`. Include both `LICENSE-MIT` and
`LICENSE-APACHE` files; Cargo `license` field lists both.

**Consequences.**
- Maximum compatibility: downstream users pick whichever they prefer
- Patent grant from Apache-2.0 available to those who want it
- GPL-compatible (matters if we ever want to copy code from Flameshot,
  which is GPL)
- Slight license-text duplication

---

## ADR-0010: `interprocess` for IPC over D-Bus

**Context.** Flameshot uses D-Bus on Linux. We considered:

1. D-Bus via `zbus` — Linux/Unix-native, integrates with desktop
   services, more complex
2. Local socket via `interprocess` — cross-platform (Unix socket +
   named pipe), simpler, no desktop integration
3. HTTP on localhost — overkill for a single-machine tool

**Decision.** Use `interprocess` with a local socket (Unix domain
socket on Linux/macOS, named pipe on Windows). Single protocol across
all platforms.

**Consequences.**
- One IPC code path for all platforms
- No D-Bus dependency on Linux
- No integration with desktop services that speak D-Bus (KDE/GNOME
  might notice us less)
- If we ever want to expose flintshot to other apps via D-Bus, we'd add
  a `zbus` adapter later

---

## ADR-0011: `postcard` for IPC message encoding

**Context.** IPC messages need a serialization format. Options:

- JSON — ubiquitous, human-debuggable, larger on the wire
- `bincode` — Rust-native, fast, no schema evolution story
- `postcard` — `serde`-compatible, compact, no_std-friendly, designed
  for embedded/CW

**Decision.** Use `postcard` for IPC. Length-prefixed frames. Keep JSON
as an option behind a feature flag for debugging if needed.

**Consequences.**
- Compact on the wire (matters for high-frequency event streams)
- Shares `serde` types with config and any future persistence
- Less debuggable than JSON without a tool — mitigated by `postcard`
  CLI debugger
- No schema evolution story; bump IPC version field if types change

---

## ADR-0012: Abstract UI toolkit via `Canvas` + `EditorShell` traits

**Context.** The default UI toolkit is egui (per ADR-0001). But egui is
not the only option — iced (used by libcosmic), slint, and gtk-rs all
exist. Without an abstraction, swapping egui for another toolkit would
require rewriting every annotation tool, the editor state model, and
the pin window. Worse, the data model would leak egui types (`Pos2`,
`Color32`, `Rect`, `Stroke`) into non-UI code, coupling capture/IPC
logic to a particular GUI library.

We considered:

1. No abstraction — egui types in core, accept the coupling (rejected —
   user explicitly asked for swappability; locks us in)
2. Higher-level "feature" trait (`EditorUI::run(state)`) — too coarse,
   doesn't really abstract anything (rejected)
3. Full immediate-mode canvas trait — re-invents a 2D drawing API,
   leaky (rejected as primary; partially adopted for annotations only)
4. Two-trait bridge: own types + logic in core; `Canvas` for drawing
   primitives; `EditorShell` for window lifecycle (chosen)

**Decision.** Define geometry types (`Point`, `Rect`, `Color`,
`Stroke`, `Key`) in `flintshot-core::geom`. All shared data model types
(`Annotation`, `EditorState`) use these, never egui's. Define two
traits in core:

- `Canvas` — drawing primitives for annotations; has an associated
  `Image` type for toolkit-specific texture handles
- `EditorShell` — lifecycle ("open editor with this state, return
  outcome")

`flintshot-editor-egui` implements both. `flintshot-app` picks the
implementation via Cargo feature flag (`default = ["egui"]`). Blur and
pixelate are computed in core and emitted to the canvas as image-blit
instructions, so all toolkits produce identical output.

**Consequences.**
- Swapping egui for iced/slint is purely additive: implement two
  traits, flip a feature flag
- Annotation logic (hit-testing, undo/redo, blur) lives in one place —
  bug fixes propagate to all toolkits
- Geometry types are minimal — no `RotatedRect`, `Affine`, `Path`
  until needed; grow as features demand
- The associated `Image` type on `Canvas` makes the trait
  non-object-safe (no `dyn Canvas`); acceptable, we always know the
  concrete type at the call site
- Some toolkit-specific features (egui's per-frame texture caching)
  need care to map cleanly; mitigated by making `upload_image`
  explicit
- Text rendering stays toolkit-specific (each toolkit uses its own
  text shaping); the `Canvas::draw_text` trait method takes plain
  text + position + size + color
