# TODO — Phase 2: Daemon, Tray, Hotkeys, IPC

> Flat status index — the single status source. One line per task +
> checkpoint; mirrors [`plan.md`](plan.md). Per-task acceptance criteria,
> verification, dependencies, and files live in
> [`tasks/00N-*.md`](tasks/). Check off by completing the acceptance
> criteria in the task file, not by writing code.

## Phase 2.A — Foundation

- [ ] 001 — IPC protocol types in `flintshot-core::ipc` → [`tasks/001-ipc-protocol-types.md`](tasks/001-ipc-protocol-types.md)
- [ ] 002 — Workspace deps + MSRV gate → [`tasks/002-workspace-deps.md`](tasks/002-workspace-deps.md)
- [ ] **Checkpoint A** — foundation types + deps in place; workspace builds clean

## Phase 2.B — Daemon event-loop de-risk (high risk)

- [ ] 003 — Daemon event-loop bridge spike + single-instance lock → [`tasks/003-daemon-loop-bridge.md`](tasks/003-daemon-loop-bridge.md)
- [ ] **Checkpoint B** — bridge approach proven; ADR-0018 reviewed before building on it

## Phase 2.C — IPC server + worker model

- [ ] 004 — IPC server transport + postcard framing → [`tasks/004-ipc-server-transport.md`](tasks/004-ipc-server-transport.md)
- [ ] 005 — Worker spawn module + dispatch table → [`tasks/005-worker-spawn-module.md`](tasks/005-worker-spawn-module.md)
- [ ] 006 — Wire capture requests + `--replace` → [`tasks/006-wire-capture-requests.md`](tasks/006-wire-capture-requests.md)
- [ ] **Checkpoint C** — IPC end-to-end; external client observes the Event stream; `--replace` swaps daemons

## Phase 2.D — System UI

- [ ] 007 — System tray menu → [`tasks/007-system-tray-menu.md`](tasks/007-system-tray-menu.md)
- [ ] 008 — Multi-monitor tray submenu → [`tasks/008-monitor-submenu.md`](tasks/008-monitor-submenu.md)
- [ ] 009 — Global hotkeys from `[hotkeys]` config → [`tasks/009-global-hotkeys.md`](tasks/009-global-hotkeys.md)
- [ ] **Checkpoint D** — full daemon works on X11: tray + hotkeys + IPC + single-instance

## Phase 2.E — Config hot-reload + CLI delegation

- [ ] 010 — Config hot-reload via `notify` → [`tasks/010-config-hot-reload.md`](tasks/010-config-hot-reload.md)
- [ ] 011 — CLI capture subcommands detect-and-delegate → [`tasks/011-cli-detect-delegate.md`](tasks/011-cli-detect-delegate.md)
- [ ] **Checkpoint E** — config edit applies a new hotkey without restart; `flintshot region` delegates / falls back

## Phase 2.F — Hardening

- [ ] 012 — Boundary test + ADRs + CHANGELOG + cross-platform docs → [`tasks/012-hardening.md`](tasks/012-hardening.md)
- [ ] **Checkpoint F** — Phase 2 exit criteria met; ready for review
