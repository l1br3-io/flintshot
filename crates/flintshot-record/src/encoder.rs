//! The H.264 encoder fallback chain.
//!
//! Selection is a pure function over an [`ElementRegistry`] so the
//! chain order is unit-testable against a fake registry (CI machines
//! carry different encoder sets than end-user hardware). The real
//! GStreamer-backed registry lives in `probe`; this module never
//! touches `GStreamer` types.

use crate::RecordError;

/// One H.264 encoder candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderKind {
    /// Intel/AMD VA-API hardware encoder (`vaapih264enc`).
    VaaPi,
    /// V4L2 memory-to-memory encoder (`v4l2h264enc`), common on
    /// ARM `SoCs`.
    V4l2,
    /// libx264 software encoder (`x264enc`).
    X264,
    /// Cisco `OpenH264` software encoder (`openh264enc`).
    OpenH264,
}

/// The fallback order locked by the spec: hardware encoders first
/// (VA-API, then V4L2 M2M), software after (x264 before `OpenH264`).
///
/// Note for package `gstreamer1.0-plugins-good`: Ubuntu 24.04 builds
/// its `video4linux2` plugin without the M2M encoders, so
/// [`EncoderKind::V4l2`] resolves only on distros that ship them
/// (Debian does). The probe simply falls through on Ubuntu — order
/// and hint stay spec-locked either way (see the engine ADR).
pub const ENCODER_CHAIN: [EncoderKind; 4] = [
    EncoderKind::VaaPi,
    EncoderKind::V4l2,
    EncoderKind::X264,
    EncoderKind::OpenH264,
];

impl EncoderKind {
    /// The `GStreamer` factory name to probe for.
    #[must_use]
    pub const fn factory_name(self) -> &'static str {
        match self {
            Self::VaaPi => "vaapih264enc",
            Self::V4l2 => "v4l2h264enc",
            Self::X264 => "x264enc",
            Self::OpenH264 => "openh264enc",
        }
    }

    /// The Debian/Ubuntu plugin package that installs this encoder.
    /// Feeds the "what to install" half of
    /// [`RecordError::ElementMissing`].
    #[must_use]
    pub const fn package(self) -> &'static str {
        match self {
            Self::VaaPi => "gstreamer1.0-vaapi",
            Self::V4l2 => "gstreamer1.0-plugins-good",
            Self::X264 => "gstreamer1.0-plugins-ugly",
            Self::OpenH264 => "gstreamer1.0-plugins-bad",
        }
    }

    /// Short human label for reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::VaaPi => "VA-API (vaapih264enc)",
            Self::V4l2 => "V4L2 M2M (v4l2h264enc)",
            Self::X264 => "x264 (x264enc)",
            Self::OpenH264 => "OpenH264 (openh264enc)",
        }
    }
}

/// "Is this element factory present?" — abstracted so chain
/// selection is pure and testable; the GStreamer-backed
/// implementation lives in `probe`.
pub trait ElementRegistry {
    /// Whether an element factory named `name` exists on this
    /// installation.
    fn has_factory(&self, name: &str) -> bool;
}

/// Pick the first encoder in [`ENCODER_CHAIN`] order whose factory
/// the registry reports present.
///
/// # Errors
/// [`RecordError::ElementMissing`] naming the *package of the first
/// chain candidate* when nothing in the chain is available — the
/// caller cannot record without installing one, and the error says
/// which.
pub fn select_encoder(registry: &dyn ElementRegistry) -> Result<EncoderKind, RecordError> {
    for kind in ENCODER_CHAIN {
        if registry.has_factory(kind.factory_name()) {
            return Ok(kind);
        }
    }
    Err(RecordError::ElementMissing {
        plugin: EncoderKind::VaaPi.factory_name().into(),
        package: EncoderKind::VaaPi.package().into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Registry that only reports the given factory names present.
    struct Fake(Vec<&'static str>);

    impl ElementRegistry for Fake {
        fn has_factory(&self, name: &str) -> bool {
            self.0.contains(&name)
        }
    }

    #[test]
    fn selects_the_only_present_chain_member_whichever_it_is() {
        for kind in ENCODER_CHAIN {
            let reg = Fake(vec![kind.factory_name()]);
            assert_eq!(select_encoder(&reg), Ok(kind), "only {}", kind.label());
        }
    }

    #[test]
    fn prefers_earliest_in_chain_when_several_present() {
        // Both software encoders present: x264 wins (earlier).
        let reg = Fake(vec!["openh264enc", "x264enc"]);
        assert_eq!(select_encoder(&reg), Ok(EncoderKind::X264));
        // All four present: VA-API wins.
        let reg = Fake(vec![
            "x264enc",
            "v4l2h264enc",
            "openh264enc",
            "vaapih264enc",
        ]);
        assert_eq!(select_encoder(&reg), Ok(EncoderKind::VaaPi));
    }

    #[test]
    fn skips_absent_hardware_encoders_to_software() {
        // Typical Ubuntu CI box: no VA-API device, no v4l2 codecs.
        let reg = Fake(vec!["openh264enc"]);
        assert_eq!(select_encoder(&reg), Ok(EncoderKind::OpenH264));
    }

    #[test]
    fn none_present_names_the_first_chain_package_to_install() {
        let err = select_encoder(&Fake(vec![])).unwrap_err();
        assert_eq!(
            err,
            RecordError::ElementMissing {
                plugin: "vaapih264enc".into(),
                package: "gstreamer1.0-vaapi".into(),
            }
        );
        assert!(err.to_string().contains("gstreamer1.0-vaapi"));
    }

    #[test]
    fn off_chain_factories_do_not_count() {
        // Some other element exists, but no chain member.
        let reg = Fake(vec!["nvh264enc", "videotestsrc"]);
        assert!(select_encoder(&reg).is_err());
    }
}
