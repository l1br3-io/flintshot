# ADR-0010: `interprocess` for IPC over D-Bus

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
