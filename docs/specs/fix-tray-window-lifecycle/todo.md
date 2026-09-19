# Fix tray + window lifecycle — task checklist

- [x] 001 — Settings arm closes behind the handoff (ADR-0065 mirror)
- [x] 002 — No-daemon fallback spawns route through `worker::app_argv`
- [x] 003 — `Prepared` fallback split: timeout ≠ old-daemon `unsupported`
- [x] 004 — Tray helper daemon-watch bounded connect retry
- [x] 005 — TraySupervisor: spawn retry, churn backoff, truthful `is_present`
- [ ] 006 — Reconnect policy + daemon-spawn rate limiter (pure fns)
- [ ] 007 — Pump reconnect loop: reconnecting status, respawn, re-announce
- [ ] 008 — CHANGELOG `[Unreleased]` + ADR-0066
