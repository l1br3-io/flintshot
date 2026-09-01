# Todo — Hide the tray during daemon-dispatched captures

- [x] 001: Core IPC variants (`HideTrayForCapture`, `ReportCaptured`,
      `Event::TrayHidden`) + round-trip tests
- [x] 002: Daemon `TrayGate`/`TrayVisibility` state machine, loop
      wiring, tray/hotkey pre-hide, IPC interception
- [x] 003: Worker handshake — bounded-wait helper + RAII guard around
      the grab in `capture_and_dispatch`
- [ ] 004: Docs — ADR-0036, CHANGELOG, TEST_MATRIX, spec closeout
