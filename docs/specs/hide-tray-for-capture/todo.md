# Todo — Hide the tray during daemon-dispatched captures

- [ ] 001: Core IPC variants (`HideTrayForCapture`, `ReportCaptured`,
      `Event::TrayHidden`) + round-trip tests
- [ ] 002: Daemon `TrayGate`/`TrayVisibility` state machine, loop
      wiring, tray/hotkey pre-hide, IPC interception
- [ ] 003: Worker handshake — bounded-wait helper + RAII guard around
      the grab in `capture_and_dispatch`
- [ ] 004: Docs — ADR-0036, CHANGELOG, TEST_MATRIX, spec closeout
