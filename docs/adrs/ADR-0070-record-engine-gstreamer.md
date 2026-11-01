# ADR-0070: Record engine — GStreamer behind a prepare/start/stop contract in `flintshot-record`

**Context.** The screen-recording initiative (spec
`git log --grep='record-engine'`, tasks E1–E7) needs an engine that
turns "record" into a finalized MP4: Wayland via the XDG ScreenCast
portal + PipeWire, X11 via a region grab, H.264 into `mp4mux`. The
screenshot path (`flintshot-core::capture`) already owns portals for
still images; video needs a media pipeline, encoder fallbacks, and a
finalize lifecycle that stills never had. Short-snips profile
(seconds–minutes), video-only, Linux-first.

**Decision.**

1. **GStreamer (gstreamer-rs 0.23) in a new crate,
   `flintshot-record`.** It is the only GStreamer surface in the
   workspace — `flintshot-core` stays GStreamer-free and *lends* the
   engine its geometry (`geom::Rect`) and session detection
   (`SessionKind`), mirroring the core/app split of ADR-0069. Build
   cost: `libgstreamer1.0-dev` + plugin packages (installed by
   `just install`'s `record_deps` and CI's `RECORD_LINUX_DEPS`).
2. **Contract:** `Recorder::prepare(cfg) → start() →
   RecordHandle::stop()`. `prepare` is async (the portal dance);
   `start` confirms `PLAYING` within a budget and spawns the
   duration-cap watcher; `stop` sends EOS, waits for the `moov`
   (budgeted; else `FinalizeTimeout`), tears down, and *verifies the
   box layout* before returning the path. **Deviation from the spec
   sketch:** `stop(&self)` instead of `stop(self)` — the sketch's
   own idempotence clause ("a second call returns the same
   outcome") is unimplementable on a consumed `self`, and the
   duration-cap thread needs to race a caller's stop into one
   shared `Arc<OnceLock>` outcome.
3. **Encoder fallback chain, probed at runtime:** `vaapih264enc →
   v4l2h264enc → x264enc` (`tune=zerolatency`) `→ openh264enc`,
   hardware before software. Chain order is a pure function over an
   `ElementRegistry` trait so it is truth-table tested. Distro
   findings: `openh264enc` ships in `plugins-bad` (not a separate
   package), and **Ubuntu 24.04 builds `plugins-good`'s
   `video4linux2` plugin without the M2M encoders** — the V4L2 slot
   resolves only on distros that ship them (Debian does); the probe
   falls through by design and the hint stays `plugins-good`.
4. **`h264parse` between encoder and muxer, always.** `mp4mux`
   accepts only `avc`/`avc3` stream-format; `x264enc` renegotiates
   to avc on downstream demand but `vaapih264enc` pushes
   byte-stream and will not — the pipeline stalls below `PLAYING`
   with the muxer rejecting caps against `EMPTY`. A bare avc
   capsfilter cannot convert; `h264parse` does, uniformly for every
   encoder. Corollary war story: a failed negotiation still EOSes
   into a **`moov`-only ~600-byte file** — "the file exists" is not
   success; assert size/`mdat` (the in-house MP4 walker plus a
   size-growth assertion pin this in the integration tests).
5. **Finalize-on-stop, not fragmented MP4.** `moov` written on EOS;
   a crash mid-recording loses the file. fMP4 survives crashes but
   some players dislike it — deferred with evidence required
   (spec open question 2).
6. **Portal restore token lives in the config, caller-owned.** The
   token is in/out on the *contract*
   (`RecordConfig::portal_restore_token` in,
   `Recorder::restore_token()` out) — the engine never stores
   anything. Home is `[record]` in config.toml (a token is not a
   credential; the keyring stance of ADR-0024 is untouched).
   Enabling ashpd's `screencast` feature was the spec's ask-first
   item: verified zero new crates (`serde_repr` already in the
   lock), approved with E6 after an in-band ask went unanswered.
7. **Headless testability spine:** a hidden `RecordSource::TestSrc`
   swaps in `videotestsrc` so engine tests run in CI with no
   display, portal, or PipeWire. Gotchas pinned: `videotestsrc`
   must **not** be `is-live` (a live source stays silent in
   `PAUSED`, so the sink never prerolls — the pipeline stalls at
   `Ready` with no bus error), and the source capsfilter is
   mode-dependent (fixed 320×180 for the spine, framerate-only for
   real grabs whose geometry must flow). `pipewiresrc` takes the
   portal fd via its `fd` property with the `OwnedFd` kept alive in
   the `Recorder` for the whole recording.

**Consequences.**

- The workspace gains a heavy dep set (GStreamer link + plugin
  runtime); paid knowingly at install time, and the probe degrades
  to typed `ElementMissing` errors naming apt packages rather than
  panicking on sparse machines.
- Every failure mode is typed (`PortalDenied`, `ElementMissing`,
  `NoDisplay`, `PipelineFailed`, `FinalizeTimeout`) with start
  failures draining bus errors into the message — the module's
  tests double as the error-text contract.
- Graphical truths stay manual: portal round-trip, deny path, X11
  grab, restore-token re-record are `#[ignore]`d tests + checklist
  rows (`cargo test -p flintshot-record --test portal_session --
  --ignored`), same posture as the ai-streaming manual pass.
- Adding audio or live-casting later means revisiting finalize
  (fMP4) and the muxer, not the contract.
