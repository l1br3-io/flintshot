# ADR-0060: A worker Copy lingers on Linux — process exit wipes the selection

**Context.** Worker editor windows auto-closed after a successful
Copy, on the doc-commented assumption that "the compositor takes
ownership of the selection once set." A live probe
(ux-redesign-regressions spec §2) disproved that for arboard's
`wl-clipboard-rs` thread-serve model: the selection is served
from an in-process thread, and COSMIC does **not** adopt an
exited client's `zwlr_data_control` selection — the clipboard
empties the moment the process dies. This is exactly why
`wl-copy` forks a lingering server. X11 was already
owner-clipboard, so the defect was Wayland-visible but
Linux-general. macOS / Windows own the clipboard OS-side; exit is
safe there.

**Decision.**

- The close decision is one pure table, `close_after_copy(
  os_owns_clipboard) -> bool` (pinned by tests on any platform):
  worker Copy **never** auto-closes on Linux (either display
  server); macOS / Windows keep the close. Page (routed) mode
  never closes — the long-lived window satisfies the owner rule
  by existing.
- The serve thread outlives the user's paste by the user closing
  the window themselves (Esc / X) after pasting — no forked
  detached serve process, no daemon clipboard custody.

**Consequences.**

- Copy-then-paste works reliably from worker windows on Linux;
  the cost is one extra keypress (the close the user already
  knows from Esc) instead of a silently empty clipboard.
- The `close_after_copy` doc comment states the probe evidence,
  so the next window-lifecycle change starts from the corrected
  model, not the old assumption.
- Any future "auto-close after copy" proposal on Linux must
  first solve serve-thread custody (fork-and-linger or daemon
  handoff) — out of scope until demonstrated need.
