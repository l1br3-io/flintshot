//! OCR: region → text, engine-abstracted.
//!
//! The `NormCap` contract behind both surfaces (editor tool, `ocr` CLI
//! verb): the user selects a region, an engine recognizes it, the
//! text lands on the clipboard. Two engines share one trait — the
//! honesty split from SPEC-ocr-region.md:
//!
//! - `ocrs` (default): pure-Rust crates, models fetched **with
//!   consent** only, Latin script;
//! - `tesseract` (opt-in): the system binary via a CLI wrapper, every
//!   installed language.
//!
//! This module owns the trait, [`OcrOutput`], auto-resolution over an
//! injectable [`OcrAvailabilityProbe`], and errors whose copy names
//! the cure (install/config guidance). Engine implementations live in
//! `ocrs.rs` and `tesseract.rs`.

pub mod ocrs;
pub mod tesseract;

pub use ocrs::{
    fetch_models_into, ModelDownloader, ModelFileSpec, OcrsEngine, DETECTION_MODEL_URL,
    MODEL_SPECS, RECOGNITION_MODEL_URL,
};
pub use tesseract::TesseractEngine;

use std::path::Path;

use image::RgbaImage;

/// One recognition pass's result.
///
/// v1's contract is plain text with per-line newlines. Word boxes are
/// a later extension (highlight-on-hover, history-search): they are
/// additive fields here, not a breaking change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrOutput {
    text: String,
}

impl OcrOutput {
    /// Wrap recognized `text` (per-line newlines, no trailing newline
    /// normalization — engines report what they recognized).
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    /// The recognized text, lines separated by `\n`.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// OCR failures, split by what the caller can do about them.
///
/// [`OcrError::NoEngine`] carries the actionable copy (install /
/// config guidance) and maps to the CLI's dedicated exit code;
/// [`OcrError::EngineFailure`] means an engine was selected and ran
/// but failed.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum OcrError {
    /// No usable engine for the requested choice. `guidance` names
    /// the cure(s): fetch the built-in models (consent-gated) and/or
    /// install tesseract.
    #[error("no OCR engine available: {guidance}")]
    NoEngine {
        /// Actionable install/config guidance.
        guidance: String,
    },
    /// The selected engine ran but failed. `detail` carries the
    /// lower-level message verbatim.
    #[error("OCR engine failure: {detail}")]
    EngineFailure {
        /// Lower-level error description.
        detail: String,
    },
}

/// One recognition pass over a (possibly cropped) image. Engines
/// return plain text with per-line newlines; word boxes are a later
/// extension, not part of v1's contract.
pub trait OcrEngine {
    /// Recognize `image` (already cropped to the user's region —
    /// only the rect reaches the engine) and return its text.
    ///
    /// # Errors
    /// [`OcrError::NoEngine`] when the engine's substrate (models,
    /// binary) vanished between selection and use;
    /// [`OcrError::EngineFailure`] when the pass itself fails.
    fn recognize(&self, image: &RgbaImage) -> Result<OcrOutput, OcrError>;
}

/// Config-facing engine selector (`[ocr] engine`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OcrEngineChoice {
    /// `ocrs` when its models are present, else `tesseract` when the
    /// binary is, else an error naming both cures.
    #[default]
    Auto,
    /// The built-in pure-Rust engine (models must be present or
    /// fetched with consent).
    Ocrs,
    /// The system `tesseract` binary (must be installed).
    Tesseract,
}

/// The concrete engine auto-/explicit resolution picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrEngineKind {
    /// `ocrs` (pure-Rust, consent-gated models).
    Ocrs,
    /// System `tesseract` binary wrapper.
    Tesseract,
}

/// What this machine has: the injectable seam behind
/// [`resolve_engine`], so resolution is unit-testable over a fake
/// (the record crate's encoder-probe pattern).
pub trait OcrAvailabilityProbe {
    /// Both `ocrs` model files exist (XDG cache or pre-seed dir).
    fn ocrs_models_present(&self) -> bool;
    /// A `tesseract` binary is reachable on `PATH`.
    fn tesseract_binary_present(&self) -> bool;
}

/// The real [`OcrAvailabilityProbe`]: filesystem and `PATH` checks.
#[derive(Debug, Default)]
pub struct SystemOcrProbe;

/// Upstream `ocrs` model file names inside a model dir. Pinned to
/// the `ocrs`-CLI's own defaults (`text-detection.onnx` /
/// `text-recognition.onnx` on the project's S3 bucket — see
/// `ocrs.rs`); `rten::Model::load_file` loads the ONNX form
/// directly.
pub(crate) const DETECTION_MODEL_FILE: &str = "text-detection.onnx";
/// See [`DETECTION_MODEL_FILE`].
pub(crate) const RECOGNITION_MODEL_FILE: &str = "text-recognition.onnx";
/// Model dir under `dirs::cache_dir()` — where a consented fetch
/// writes (`~/.cache/flintshot/ocrs`).
pub(crate) const OCRS_MODEL_CACHE_SUBDIR: &str = "flintshot/ocrs";
/// Packaging pre-seed location, honored read-only (deb/rpm/flatpak
/// decision, not this module's scope).
pub(crate) const OCRS_MODEL_PRESEED_DIR: &str = "/usr/share/flintshot/ocrs";

/// Cure copy shared by resolution errors and the consent dialog.
pub(crate) const OCRS_MODELS_CURE: &str =
    "download the built-in OCR models (consent prompt in the editor, or `flinshot ocr --fetch-models`)";
/// Cure copy for the tesseract opt-in.
pub(crate) const TESSERACT_INSTALL_CURE: &str =
    "install tesseract (e.g. `apt install tesseract-ocr`)";

/// Both model files present in `dir` (one file is not enough — the
/// engine needs detection *and* recognition).
pub(crate) fn models_present_in(dir: &Path) -> bool {
    dir.join(DETECTION_MODEL_FILE).is_file() && dir.join(RECOGNITION_MODEL_FILE).is_file()
}

/// The model cache dir under an XDG-style `base` (`None` when the
/// platform has no cache dir).
pub(crate) fn cache_subdir_under(base: Option<std::path::PathBuf>) -> Option<std::path::PathBuf> {
    base.map(|b| b.join(OCRS_MODEL_CACHE_SUBDIR))
}

impl OcrAvailabilityProbe for SystemOcrProbe {
    fn ocrs_models_present(&self) -> bool {
        let cached =
            cache_subdir_under(dirs::cache_dir()).is_some_and(|dir| models_present_in(&dir));
        cached || models_present_in(Path::new(OCRS_MODEL_PRESEED_DIR))
    }

    fn tesseract_binary_present(&self) -> bool {
        let Some(path) = std::env::var_os("PATH") else {
            return false;
        };
        std::env::split_paths(&path).any(|dir| dir.join("tesseract").is_file())
    }
}

/// Resolve `choice` against what the machine has.
///
/// `Auto` prefers `ocrs` when its models are present, falls back to
/// `tesseract` when only the binary is, and otherwise fails with an
/// error naming both cures. Explicit choices fail with their own
/// cure when their substrate is missing — never a silent cross-over
/// the user did not configure.
///
/// # Errors
/// [`OcrError::NoEngine`] when the requested engine's substrate is
/// absent; the `guidance` names the cure(s).
pub fn resolve_engine(
    choice: OcrEngineChoice,
    probe: &dyn OcrAvailabilityProbe,
) -> Result<OcrEngineKind, OcrError> {
    match choice {
        OcrEngineChoice::Auto => {
            if probe.ocrs_models_present() {
                Ok(OcrEngineKind::Ocrs)
            } else if probe.tesseract_binary_present() {
                Ok(OcrEngineKind::Tesseract)
            } else {
                Err(OcrError::NoEngine {
                    guidance: format!(
                        "no OCR engine is configured — {OCRS_MODELS_CURE}, or {TESSERACT_INSTALL_CURE}"
                    ),
                })
            }
        }
        OcrEngineChoice::Ocrs => {
            if probe.ocrs_models_present() {
                Ok(OcrEngineKind::Ocrs)
            } else {
                Err(OcrError::NoEngine {
                    guidance: format!(
                        "the built-in OCR models are not installed — {OCRS_MODELS_CURE}"
                    ),
                })
            }
        }
        OcrEngineChoice::Tesseract => {
            if probe.tesseract_binary_present() {
                Ok(OcrEngineKind::Tesseract)
            } else {
                Err(OcrError::NoEngine {
                    guidance: format!(
                        "the tesseract binary was not found on PATH — {TESSERACT_INSTALL_CURE}"
                    ),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fake probe: exactly the machine shape each test wants.
    struct FakeProbe {
        models: bool,
        tesseract: bool,
    }

    impl OcrAvailabilityProbe for FakeProbe {
        fn ocrs_models_present(&self) -> bool {
            self.models
        }
        fn tesseract_binary_present(&self) -> bool {
            self.tesseract
        }
    }

    #[test]
    fn auto_prefers_ocrs_when_models_present() {
        let probe = FakeProbe {
            models: true,
            tesseract: true,
        };
        assert_eq!(
            resolve_engine(OcrEngineChoice::Auto, &probe),
            Ok(OcrEngineKind::Ocrs)
        );
    }

    #[test]
    fn auto_falls_back_to_tesseract_when_models_missing() {
        let probe = FakeProbe {
            models: false,
            tesseract: true,
        };
        assert_eq!(
            resolve_engine(OcrEngineChoice::Auto, &probe),
            Ok(OcrEngineKind::Tesseract)
        );
    }

    #[test]
    fn auto_with_neither_names_both_cures() {
        let probe = FakeProbe {
            models: false,
            tesseract: false,
        };
        let err = resolve_engine(OcrEngineChoice::Auto, &probe)
            .expect_err("neither engine present must fail");
        let OcrError::NoEngine { guidance } = &err else {
            panic!("expected NoEngine, got {err:?}");
        };
        assert!(
            guidance.contains("--fetch-models"),
            "guidance must name the models cure: {guidance}"
        );
        assert!(
            guidance.contains("tesseract"),
            "guidance must name the tesseract cure: {guidance}"
        );
    }

    #[test]
    fn explicit_ocrs_with_models_resolves() {
        let probe = FakeProbe {
            models: true,
            tesseract: false,
        };
        assert_eq!(
            resolve_engine(OcrEngineChoice::Ocrs, &probe),
            Ok(OcrEngineKind::Ocrs)
        );
    }

    #[test]
    fn explicit_ocrs_without_models_names_the_fetch_cure() {
        let probe = FakeProbe {
            models: false,
            tesseract: true,
        };
        let err = resolve_engine(OcrEngineChoice::Ocrs, &probe)
            .expect_err("ocrs without models must fail, not cross over");
        let OcrError::NoEngine { guidance } = &err else {
            panic!("expected NoEngine, got {err:?}");
        };
        assert!(
            guidance.contains("--fetch-models") && !guidance.contains("apt install"),
            "guidance must name only the models cure: {guidance}"
        );
    }

    #[test]
    fn explicit_tesseract_without_binary_names_the_install_cure() {
        let probe = FakeProbe {
            models: true,
            tesseract: false,
        };
        let err = resolve_engine(OcrEngineChoice::Tesseract, &probe)
            .expect_err("tesseract without binary must fail, not cross over");
        let OcrError::NoEngine { guidance } = &err else {
            panic!("expected NoEngine, got {err:?}");
        };
        assert!(
            guidance.contains("tesseract"),
            "guidance must name the install cure: {guidance}"
        );
    }

    #[test]
    fn explicit_tesseract_with_binary_resolves() {
        let probe = FakeProbe {
            models: false,
            tesseract: true,
        };
        assert_eq!(
            resolve_engine(OcrEngineChoice::Tesseract, &probe),
            Ok(OcrEngineKind::Tesseract)
        );
    }

    /// The models check requires *both* files — one model is not a
    /// usable engine.
    #[test]
    fn models_present_requires_both_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(!models_present_in(dir.path()));
        std::fs::File::create(dir.path().join(DETECTION_MODEL_FILE)).expect("detection file");
        assert!(!models_present_in(dir.path()));
        std::fs::File::create(dir.path().join(RECOGNITION_MODEL_FILE)).expect("recognition file");
        assert!(models_present_in(dir.path()));
    }
}
