//! Capability probing — what this machine can record with.
//!
//! Mirrors `flintshot-core`'s `backends` report: a
//! [`RecordCapability`] answers "which source, which encoders" so
//! callers (the `record` CLI's capability column, the engine's
//! prepare path) never build a pipeline that cannot run. Probing is
//! pure over an [`ElementRegistry`]; the real GStreamer-backed
//! registry is created here once the engine links it.

use crate::encoder::{ElementRegistry, EncoderKind, ENCODER_CHAIN};

/// A recordable screen source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordSourceKind {
    /// Wayland portal stream via `PipeWire` (`pipewiresrc`).
    PipeWire,
    /// X11 region grab (`ximagesrc`).
    X11,
}

/// What the probe learned about this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCapability {
    /// Best available source, `None` when neither `pipewiresrc` nor
    /// `ximagesrc` is installed. `PipeWire` wins ties: it is the
    /// Wayland-first platform decision.
    pub source: Option<RecordSourceKind>,
    /// Every chain encoder present, in fallback order (empty when
    /// no H.264 encoder is installed).
    pub encoders: Vec<EncoderKind>,
}

impl RecordCapability {
    /// The encoder a recording would use (first of
    /// [`Self::encoders`]).
    #[must_use]
    pub fn selected_encoder(&self) -> Option<EncoderKind> {
        self.encoders.first().copied()
    }

    /// Deduplicated, chain-ordered list of packages that would add
    /// missing capability: one encoder package when no encoder is
    /// present, plus both source packages when no source is (the
    /// session type — hence which source applies — is not known at
    /// probe time).
    #[must_use]
    pub fn missing_packages(&self) -> Vec<&'static str> {
        let mut packages: Vec<&'static str> = Vec::new();
        if self.encoders.is_empty() {
            packages.push(EncoderKind::VaaPi.package());
        }
        if self.source.is_none() {
            push_unique(&mut packages, PIPEWIRE_PACKAGE);
            push_unique(&mut packages, X11_PACKAGE);
        }
        packages
    }

    /// The actionable one-liner for "you cannot record here" —
    /// empty when recording is possible.
    #[must_use]
    pub fn install_hint(&self) -> String {
        let missing = self.missing_packages();
        if missing.is_empty() {
            String::new()
        } else {
            format!("install: {}", missing.join(" "))
        }
    }
}

/// Package that installs `pipewiresrc`.
const PIPEWIRE_PACKAGE: &str = "gstreamer1.0-pipewire";
/// Package that installs `ximagesrc`.
const X11_PACKAGE: &str = "gstreamer1.0-plugins-good";

fn push_unique<'a>(list: &mut Vec<&'a str>, item: &'a str) {
    if !list.contains(&item) {
        list.push(item);
    }
}

/// Probe source and encoder availability.
#[must_use]
pub fn probe_capabilities(registry: &dyn ElementRegistry) -> RecordCapability {
    let source = if registry.has_factory("pipewiresrc") {
        Some(RecordSourceKind::PipeWire)
    } else if registry.has_factory("ximagesrc") {
        Some(RecordSourceKind::X11)
    } else {
        None
    };
    let encoders = ENCODER_CHAIN
        .iter()
        .filter(|kind| registry.has_factory(kind.factory_name()))
        .copied()
        .collect();
    RecordCapability { source, encoders }
}

/// The real [`ElementRegistry`]: `GStreamer`'s own plugin registry,
/// behind the same trait the fake serves in tests.
///
/// [`gstreamer::init`] must run before any `GStreamer` API — undefined
/// behavior otherwise — so it runs once, lazily, on first probe. A
/// failed init (no usable `GStreamer` runtime) makes every factory
/// lookup report absent, and the resulting capability degrades to
/// its install hint instead of panicking.
#[derive(Debug, Default)]
pub struct GstElementRegistry;

impl GstElementRegistry {
    /// Run [`gstreamer::init`] once per process; `true` when the
    /// runtime came up.
    fn ensure_init() -> bool {
        static INIT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *INIT.get_or_init(|| gstreamer::init().is_ok())
    }
}

impl ElementRegistry for GstElementRegistry {
    fn has_factory(&self, name: &str) -> bool {
        Self::ensure_init() && gstreamer::ElementFactory::find(name).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::select_encoder;

    struct Fake(Vec<&'static str>);

    impl ElementRegistry for Fake {
        fn has_factory(&self, name: &str) -> bool {
            self.0.contains(&name)
        }
    }

    #[test]
    fn pipewire_wins_when_both_sources_present() {
        let cap = probe_capabilities(&Fake(vec!["pipewiresrc", "ximagesrc"]));
        assert_eq!(cap.source, Some(RecordSourceKind::PipeWire));
    }

    #[test]
    fn x11_source_when_pipewire_absent() {
        let cap = probe_capabilities(&Fake(vec!["ximagesrc"]));
        assert_eq!(cap.source, Some(RecordSourceKind::X11));
    }

    #[test]
    fn no_source_when_neither_installed() {
        let cap = probe_capabilities(&Fake(vec!["openh264enc"]));
        assert_eq!(cap.source, None);
    }

    #[test]
    fn encoders_listed_in_chain_order_not_registry_order() {
        let cap = probe_capabilities(&Fake(vec!["openh264enc", "vaapih264enc", "x264enc"]));
        assert_eq!(
            cap.encoders,
            vec![EncoderKind::VaaPi, EncoderKind::X264, EncoderKind::OpenH264]
        );
        assert_eq!(cap.selected_encoder(), Some(EncoderKind::VaaPi));
    }

    #[test]
    fn bare_machine_hint_names_encoder_and_both_source_packages() {
        let cap = probe_capabilities(&Fake(vec![]));
        assert_eq!(
            cap.missing_packages(),
            vec![
                "gstreamer1.0-vaapi",
                "gstreamer1.0-pipewire",
                "gstreamer1.0-plugins-good"
            ]
        );
        assert_eq!(
            cap.install_hint(),
            "install: gstreamer1.0-vaapi gstreamer1.0-pipewire gstreamer1.0-plugins-good"
        );
    }

    #[test]
    fn encoderless_machine_with_source_names_only_encoder_package() {
        let cap = probe_capabilities(&Fake(vec!["ximagesrc"]));
        assert_eq!(cap.missing_packages(), vec!["gstreamer1.0-vaapi"]);
        // A source exists, so no source package is suggested;
        // recording works once an encoder lands.
        assert!(!cap.install_hint().contains(X11_PACKAGE));
    }

    #[test]
    fn capable_machine_has_empty_hint() {
        let cap = probe_capabilities(&Fake(vec!["pipewiresrc", "x264enc"]));
        assert_eq!(cap.install_hint(), "");
        assert!(matches!(
            select_encoder(&Fake(vec!["x264enc"])),
            Ok(EncoderKind::X264)
        ));
        // Sanity: the fake covers a real chain member.
        assert!(ENCODER_CHAIN.iter().any(|k| k.factory_name() == "x264enc"));
    }

    #[test]
    fn real_registry_finds_a_base_plugin_element() {
        // videotestsrc ships in gst-plugins-base, which every
        // GStreamer dev setup (and CI) carries — and it is the very
        // element the pipeline integration tests stand in with.
        assert!(GstElementRegistry.has_factory("videotestsrc"));
    }

    #[test]
    fn real_registry_rejects_an_unknown_factory() {
        assert!(!GstElementRegistry.has_factory("flintshot-definitely-not-an-element"));
    }

    #[test]
    fn real_probe_reports_only_chain_members_as_encoders() {
        // Whatever this machine carries, the report must stay a
        // subset of the chain in chain order.
        let cap = probe_capabilities(&GstElementRegistry);
        let chain: Vec<_> = ENCODER_CHAIN.to_vec();
        assert!(cap.encoders.iter().all(|kind| chain.contains(kind)));
        let positions: Vec<usize> = cap
            .encoders
            .iter()
            .filter_map(|kind| chain.iter().position(|c| c == kind))
            .collect();
        let mut sorted = positions.clone();
        sorted.sort_unstable();
        assert_eq!(positions, sorted, "encoders must stay in chain order");
    }
}
