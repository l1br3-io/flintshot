# ADR-0020: Single-instance lock scoped to the daemon role, with a `--replace` handshake

**Status.** Accepted.

**Context.** Phase 2 introduces a long-running `flintshot daemon`, but
`flintshot` also runs short-lived workers (`flintshot region`,
`fullscreen`) spawned by that daemon, and standalone invocations from a
terminal. Two questions follow:

1. *Which* `flintshot` processes must be mutually exclusive? Clearly the
   daemon — two trays / two IPC servers / two hotkey registrations on
   the same chords would be chaos. But workers and standalone captures
   must be allowed to run concurrently (the daemon spawns a worker per
   capture; a user can run `flintshot region` while the daemon runs).
2. How does `flintshot daemon --replace` take over from a running daemon
   cleanly — without leaving two daemons fighting over the socket, and
   without forcing the user to kill the old one by hand?

**Decision.**

- **The single-instance lock is acquired only in the daemon role.**
  `flintshot daemon` takes it on start and holds it for its lifetime;
  `flintshot region` / `fullscreen` / workers never touch it. So a
  daemon + N editors coexist, but two daemons cannot.
- **`--replace` is an IPC handshake, not a lock fight.** The new daemon,
  before trying to take the lock:
  1. connects to the running daemon's IPC socket and sends
     `Request::Quit`;
  2. waits (bounded, ~3 s, polling) for the IPC socket file to
     disappear — the old daemon's listener is dropped on shutdown, and
     `try_overwrite` reclaims the file;
  3. acquires the single-instance lock with a short retry — there's a
     small window between the old daemon's socket-file removal and its
     (abstract-socket) lock release, so a single immediate acquire can
     race; the retry absorbs it.

  On the receiving side, `Request::Quit` (via the worker dispatch
  table) fires the daemon-wide shutdown signal, which stops both the IPC
  accept loop and the heartbeat ticker; the ticker dropping the channel
  sender makes the main loop exit, the lock drops, and the process ends.

- A `--replace` with **no** daemon running is a no-op: the connect fails
  (no socket), the new daemon simply starts fresh. A second `flintshot
  daemon` **without** `--replace` fails fast with "already running".

**Consequences.**

- Exactly-one-daemon invariant holds; workers and standalone captures
  are never blocked by it.
- `--replace` is cooperative: the old daemon shuts down gracefully
  (unregisters hotkeys, closes the socket, drops the tray), so no
  half-dead tray / leaked hotkey registration. No `SIGKILL`, no orphans.
- The lock is an OS primitive (abstract Unix socket on Linux, flock on
  macOS, mutex on Windows via `single-instance`), so a daemon that
  crashes still releases it (process death frees the resource) — the
  next start isn't blocked by a ghost.
- The `--replace` handshake's two waits (socket-free + lock-retry) cap
  the takeover latency at a few hundred ms in practice; both have
  bounded timeouts so a stuck old daemon surfaces as an error rather
  than an indefinite hang.
