//! Pipeline construction and the record lifecycle.
//!
//! One recording is one `GStreamer` pipeline:
//! `source → capsfilter → videoconvert → encoder → mp4mux → filesink`.
//! [`Recorder::prepare`] builds and links it (probing before every
//! factory use so a missing plugin is a typed
//! [`RecordError::ElementMissing`], never a build surprise),
//! [`Recorder::start`] moves it to `PLAYING` before returning, and
//! [`RecordHandle::stop`] sends EOS and waits for `mp4mux` to write
//! the `moov` box — the in-house [`crate::mp4`] walker verifies the
//! finalize before the path is handed back.
//!
//! Lifecycle state is shared behind an `Arc` so the duration-cap
//! thread and a caller's `stop()` race safely: whichever gets there
//! first performs the EOS/finalize inside a `OnceLock`; the other
//! observes the same outcome.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::thread;

use gstreamer as gst;
use gstreamer::prelude::*;

use crate::encoder::{select_encoder, ElementRegistry, EncoderKind};
use crate::mp4;
use crate::probe::GstElementRegistry;
use crate::{RecordConfig, RecordError, RecordSource};

/// Seconds the pipeline gets to reach `PLAYING` (link negotiation
/// and preroll) before start reports failure.
const START_TIMEOUT_SECS: u64 = 10;
/// Seconds EOS gets to become a written `moov` box before stop
/// reports [`RecordError::FinalizeTimeout`].
const FINALIZE_TIMEOUT_SECS: u64 = 10;

/// Elements this module builds that are not encoder-chain members,
/// mapped to the package that installs them. (Core elements ship
/// with the `GStreamer` runtime itself — this crate cannot link
/// without it — so their entry is a formality.)
const PACKAGE_HINTS: &[(&str, &str)] = &[
    ("videotestsrc", "gstreamer1.0-plugins-base"),
    ("videoconvert", "gstreamer1.0-plugins-base"),
    ("h264parse", "gstreamer1.0-plugins-bad"),
    ("capsfilter", "libgstreamer1.0-0"),
    ("mp4mux", "gstreamer1.0-plugins-good"),
    ("filesink", "libgstreamer1.0-0"),
    ("ximagesrc", "gstreamer1.0-plugins-good"),
    ("pipewiresrc", "gstreamer1.0-pipewire"),
];

fn package_hint(element: &str) -> &'static str {
    PACKAGE_HINTS
        .iter()
        .find(|(name, _)| *name == element)
        .map_or("gstreamer1.0-plugins-good", |(_, package)| *package)
}

/// A prepared, not-yet-started recording.
#[derive(Debug)]
pub struct Recorder {
    config: RecordConfig,
    pipeline: gst::Pipeline,
}

impl Recorder {
    /// What this machine can record with — best source, available
    /// encoders — straight from `GStreamer`'s plugin registry.
    #[must_use]
    pub fn probe() -> crate::RecordCapability {
        crate::probe_capabilities(&GstElementRegistry)
    }

    /// Build (but do not start) the pipeline for `cfg`.
    ///
    /// `async` by contract — the `Wayland` portal session the
    /// `Session` source drives is an async dance (see the portal
    /// task); the headless [`RecordSource::TestSrc`] path resolves
    /// immediately.
    ///
    /// # Errors
    /// [`RecordError::ElementMissing`] for every absent factory,
    /// [`RecordError::PipelineFailed`] for link/negotiation
    /// failures.
    #[allow(clippy::unused_async)] // portal task awaits inside
    pub async fn prepare(cfg: RecordConfig) -> Result<Self, RecordError> {
        let config = cfg.sanitized();
        let encoder_kind = select_encoder(&GstElementRegistry)?;

        let source = build_source(&config)?;
        let capsfilter = build_test_caps_filter(&config)?;
        let convert = make_element("videoconvert")?;
        let encoder = build_encoder(encoder_kind)?;
        // mp4mux only accepts avc/avc3 stream-format, while encoders
        // push byte-stream (and vaapih264enc won't renegotiate to
        // avc on downstream demand) — h264parse converts, and the
        // capsfilter pins the muxer's demanded format for every
        // encoder uniformly.
        let parser = make_element("h264parse")?;
        let avc_filter = build_avc_caps_filter()?;
        let mux = make_element("mp4mux")?;
        let sink = make_element("filesink")?;
        sink.set_property("location", config.out.to_string_lossy().to_string());

        let pipeline = gst::Pipeline::new();
        for element in [
            &source,
            &capsfilter,
            &convert,
            &encoder,
            &parser,
            &avc_filter,
            &mux,
            &sink,
        ] {
            pipeline
                .add(element)
                .map_err(|err| pipeline_error("pipeline", err))?;
        }
        gst::Element::link_many([
            &source,
            &capsfilter,
            &convert,
            &encoder,
            &parser,
            &avc_filter,
            &mux,
            &sink,
        ])
        .map_err(|err| pipeline_error("pipeline", err))?;

        Ok(Self { config, pipeline })
    }

    /// Move the pipeline to `PLAYING`, confirmed before returning.
    /// With [`RecordConfig::max_duration`] set, a watcher thread
    /// auto-stops (EOS + finalize) when the cap elapses.
    ///
    /// # Errors
    /// [`RecordError::PipelineFailed`] when the pipeline fails to
    /// reach `PLAYING` within the start budget.
    pub fn start(self) -> Result<RecordHandle, RecordError> {
        self.pipeline
            .set_state(gst::State::Playing)
            .map_err(|err| pipeline_error("pipeline", err))?;
        let (result, current, _) = self
            .pipeline
            .state(gst::ClockTime::from_seconds(START_TIMEOUT_SECS));
        match result {
            Ok(_) if current == gst::State::Playing => {}
            Ok(_) => {
                let detail = pending_bus_detail(&self.pipeline);
                let _ = self.pipeline.set_state(gst::State::Null);
                return Err(RecordError::PipelineFailed {
                    element: "pipeline".into(),
                    message: format!("did not reach PLAYING (state {current:?}){detail}"),
                });
            }
            Err(err) => {
                let detail = pending_bus_detail(&self.pipeline);
                let _ = self.pipeline.set_state(gst::State::Null);
                return Err(pipeline_error("pipeline", format!("{err}{detail}")));
            }
        }

        let core = Arc::new(LifecycleCore {
            pipeline: self.pipeline,
            out: self.config.out.clone(),
            done: OnceLock::new(),
        });
        if let Some(cap) = self.config.max_duration {
            let watcher = Arc::clone(&core);
            thread::spawn(move || {
                thread::sleep(cap);
                // The outcome lands in the shared cell; nobody is
                // listening on this thread, so drop the result.
                drop(LifecycleCore::stop_once(&watcher));
            });
        }
        Ok(RecordHandle { core })
    }
}

/// A running recording.
///
/// Call [`RecordHandle::stop`] to finalize; it is idempotent — the
/// duration cap may finalize first, and any number of later calls
/// return that same outcome. Dropping the handle does **not** stop
/// the recording (the cap, if any, still fires); v1 callers — the
/// `record` CLI — always stop explicitly.
pub struct RecordHandle {
    core: Arc<LifecycleCore>,
}

impl RecordHandle {
    /// Send EOS, wait for the finalize (`moov` written), and return
    /// the finished file's path.
    ///
    /// # Errors
    /// [`RecordError::FinalizeTimeout`] when EOS does not become a
    /// written `moov` within the finalize budget;
    /// [`RecordError::PipelineFailed`] when the bus reported an
    /// error instead.
    pub fn stop(&self) -> Result<PathBuf, RecordError> {
        LifecycleCore::stop_once(&self.core)
    }
}

/// State shared between the handle owner and the duration-cap
/// watcher; `done` makes the finalize happen exactly once.
struct LifecycleCore {
    pipeline: gst::Pipeline,
    out: PathBuf,
    done: OnceLock<Result<PathBuf, RecordError>>,
}

impl LifecycleCore {
    /// Perform (or observe) the single EOS → finalize → `Null`
    /// transition for this recording.
    fn stop_once(core: &Arc<Self>) -> Result<PathBuf, RecordError> {
        core.done
            .get_or_init(|| finalize_recording(&core.pipeline, &core.out))
            .clone()
    }
}

/// The one-way finalize: EOS, wait for the muxer to flush the
/// `moov`, tear the pipeline down, verify the box layout.
fn finalize_recording(
    pipeline: &gst::Pipeline,
    out: &std::path::Path,
) -> Result<PathBuf, RecordError> {
    pipeline.send_event(gst::event::Eos::new());
    let bus = pipeline.bus().ok_or_else(|| RecordError::PipelineFailed {
        element: "pipeline".into(),
        message: "pipeline has no bus".into(),
    })?;
    let message = bus.timed_pop_filtered(
        gst::ClockTime::from_seconds(FINALIZE_TIMEOUT_SECS),
        &[gst::MessageType::Eos, gst::MessageType::Error],
    );
    match message {
        None => Err(RecordError::FinalizeTimeout),
        Some(msg) if msg.type_() == gst::MessageType::Error => {
            let detail = match msg.view() {
                gst::MessageView::Error(err) => format!(
                    "{} ({})",
                    err.error(),
                    err.debug().map(String::from).unwrap_or_default()
                ),
                _ => "bus error without detail".into(),
            };
            let _ = pipeline.set_state(gst::State::Null);
            Err(RecordError::PipelineFailed {
                element: "pipeline".into(),
                message: detail,
            })
        }
        Some(_) => {
            let _ = pipeline.set_state(gst::State::Null);
            let finalized = std::fs::read(out).ok();
            match finalized {
                Some(bytes) if mp4::has_box(&bytes, b"moov") => Ok(out.to_path_buf()),
                _ => Err(RecordError::FinalizeTimeout),
            }
        }
    }
}

/// Build the source element the config selects.
///
/// [`RecordSource::Session`] is resolved by the X11 and portal
/// tasks; until they land it reports honestly instead of pretending.
fn build_source(config: &RecordConfig) -> Result<gst::Element, RecordError> {
    match config.source {
        RecordSource::TestSrc => {
            // Deliberately NOT is-live: a live source stays silent in
            // PAUSED, so the sink never prerolls and the pipeline
            // stalls below PLAYING. Non-live videotestsrc prerolls
            // immediately and stamps frames at the negotiated rate,
            // which is all the headless spine needs.
            make_element("videotestsrc")
        }
        RecordSource::Session => Err(RecordError::PipelineFailed {
            element: "source".into(),
            message: "session sources land in the X11/portal tasks".into(),
        }),
    }
}

/// The test spine's caps: fixed low resolution at the configured
/// frame rate, so headless runs are fast and deterministic.
fn build_test_caps_filter(config: &RecordConfig) -> Result<gst::Element, RecordError> {
    let caps = gst::Caps::builder("video/x-raw")
        .field("width", 320_i32)
        .field("height", 180_i32)
        .field(
            "framerate",
            gst::Fraction::new(i32::try_from(config.fps).unwrap_or(i32::from(30u16)), 1),
        )
        .build();
    let filter = make_element("capsfilter")?;
    filter.set_property("caps", caps);
    Ok(filter)
}

/// The muxer-side caps: mp4mux's H.264 sink pad demands
/// avc/avc3 stream format with au alignment.
fn build_avc_caps_filter() -> Result<gst::Element, RecordError> {
    let caps = gst::Caps::builder("video/x-h264")
        .field("stream-format", "avc")
        .field("alignment", "au")
        .build();
    let filter = make_element("capsfilter")?;
    filter.set_property("caps", caps);
    Ok(filter)
}

/// Build the chosen encoder with its per-kind tuning (the spec pins
/// `tune=zerolatency` for `x264enc`; the others run defaults).
fn build_encoder(kind: EncoderKind) -> Result<gst::Element, RecordError> {
    if !GstElementRegistry.has_factory(kind.factory_name()) {
        return Err(RecordError::ElementMissing {
            plugin: kind.factory_name().into(),
            package: kind.package().into(),
        });
    }
    let mut builder = gst::ElementFactory::make(kind.factory_name());
    if kind == EncoderKind::X264 {
        builder = builder.property_from_str("tune", "zerolatency");
    }
    builder
        .build()
        .map_err(|err| pipeline_error(kind.factory_name(), err))
}

/// Create an element, probing first so absence is a typed
/// what-to-install error rather than a build failure.
fn make_element(name: &str) -> Result<gst::Element, RecordError> {
    if !GstElementRegistry.has_factory(name) {
        return Err(RecordError::ElementMissing {
            plugin: name.into(),
            package: package_hint(name).into(),
        });
    }
    gst::ElementFactory::make(name)
        .build()
        .map_err(|err| pipeline_error(name, err))
}

fn pipeline_error(element: &str, err: impl std::fmt::Display) -> RecordError {
    RecordError::PipelineFailed {
        element: element.into(),
        message: err.to_string(),
    }
}

/// Drain already-queued bus error/warning messages (never blocking)
/// into a report suffix, so start failures name their element.
fn pending_bus_detail(pipeline: &gst::Pipeline) -> String {
    use std::fmt::Write as _;
    let Some(bus) = pipeline.bus() else {
        return String::new();
    };
    let mut detail = String::new();
    while let Some(message) =
        bus.timed_pop_filtered(gst::ClockTime::ZERO, &[gst::MessageType::Error])
    {
        if let gst::MessageView::Error(err) = message.view() {
            let source = message
                .src()
                .map_or_else(|| "unknown".into(), ToString::to_string);
            let _ = write!(
                detail,
                "; {source}: {} ({})",
                err.error(),
                err.debug().map(String::from).unwrap_or_default()
            );
        }
    }
    detail
}
