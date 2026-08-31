# ADR-0033: CLOEXEC-safe single-instance lock on Linux

**Context.** Live on L5 (COSMIC, 2026-08-30): after the daemon
exited, every subsequent `flintshot daemon` — including
`daemon --replace` — failed with *"another flintshot daemon is
already running"* while `pgrep` showed no daemon process. The
journal showed a `--replace` at 12:41 whose takeover never came
up, and by 12:44 the IPC socket was stale (no listener) with the
lock still "held". Controlled reproduction: start the daemon,
trigger one capture (spawning a `region --standalone` editor
worker), kill the daemon — `flintshot daemon` now fails until the
editor is closed, then starts instantly.

Root cause: `single-instance` 0.3's Linux lock binds an abstract
unix-domain socket whose fd is created **without `FD_CLOEXEC`**
(`nix::sys::socket::socket` with empty `SockFlag`). Abstract names
live exactly as long as any open fd referencing the socket, and
`std::process::Command` children inherit non-CLOEXEC fds across
`exec`. The daemon spawns workers per capture (ADR-0019), so every
editor inherited the lock fd; when the daemon died, the name
stayed alive — held by whichever editors were still open. The
user-visible failure mode is total: no daemon → no tray, no
delegation, and `flintshot region` silently falls back to
standalone (with the accesskit panic noise below), which reads as
"nothing works".

Alternatives considered:

- **Patch / replace the crate upstream** — correct long-term, but
  out of our control and latency-bound on a release cycle.
- **Close the fd post-acquire via `fcntl`** — the crate keeps
  `maybe_sock: Option<RawFd>` private; no access without forking
  the crate.
- **`Command`'s `pre_exec` to close inherited fds in children** —
  needs `unsafe` (forbidden crate-wide) and treats the symptom in
  every spawn site instead of the cause.

**Decision.** Linux implements the lock directly in
`crates/flintshot-app/src/single.rs`: bind an abstract socket via
std's `UnixListener::bind_addr(SocketAddr::from_abstract_name(..))`
(`AddrInUse` ⇒ already running — identical semantics to the
crate). std creates every fd close-on-exec, so the lock dies with
the daemon and *only* the daemon; spawned editors cannot
ghost-hold it. macOS/Windows keep the `single-instance` crate
(flock file / named mutex; no fd-inheritance hazard there —
Windows mutex handles are not inherited by default, and macOS
flock ghost-holding is theoretical for our spawn shape and left to
the crate). Verified live post-fix: `daemon --replace` acquires
the lock ~50 ms after the old daemon quits, and a fresh daemon
starts cleanly beside a live orphaned editor.

One subtlety surfaced by the regression test: a **concurrent fork
transiently duplicates the fd table** (before `exec` applies
CLOEXEC), so the name can be busy for microseconds-to-milliseconds
around any `Command::spawn`. That window is benign in production —
`--replace` already retries the acquire for 3 s — and the test
reacquires with a bounded 1 s retry, which cleanly separates the
transient window from the bug it guards (a lifetime ghost-hold).

**Consequences.** One less Linux dependency in the daemon path
(`single-instance` remains only for non-Linux targets, so the
workspace dep stays). The lock module owns ~40 lines of platform
code with the same test seams as before, plus a regression test
(`lock_not_inherited_by_spawned_children`) that fails against the
old crate's behavior. Known unrelated finding from the same
session: standalone captures print a scary but non-fatal
accesskit/zbus panic (accesskit_unix spawns a plain thread while
zbus 5.18 is feature-unified onto its tokio executor; the a11y
thread dies, the editor opens) — fixed later the same day by
building the editor without eframe's accesskit feature
(ADR-0034).
