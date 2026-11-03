//! The built-in `ocrs` engine and its consent-gated model fetch.
//!
//! Models are never embedded (binary budget) and never fetched
//! silently: [`fetch_models_into`] refuses to touch its transport
//! unless consent was granted — test-pinned — and the first-use
//! surfaces (editor dialog, `--fetch-models`) are the only callers
//! that can grant it. A fetched model pair lands in the XDG cache
//! dir; a packaging pre-seed under `/usr/share/flintshot/ocrs` is
//! honored read-only, so distro builds skip the download entirely.
//!
//! Source-of-truth for the model artifacts: upstream `ocrs`-cli
//! pins the same two S3 URLs as its defaults (verified against the
//! `robertknight/ocrs` repo, `ocrs-cli/src/main.rs`).

use std::path::{Path, PathBuf};

use image::{RgbImage, RgbaImage};
use ocrs::{ImageSource, OcrEngine as OcrsModelEngine, OcrEngineParams};

use super::{
    models_present_in, OcrEngine, OcrError, OcrOutput, DETECTION_MODEL_FILE, OCRS_MODELS_CURE,
    OCRS_MODEL_PRESEED_DIR, RECOGNITION_MODEL_FILE,
};

/// Upstream source for the detection model (pinned with the
/// `ocrs`-CLI's own default; see module docs).
pub const DETECTION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx";
/// Upstream source for the recognition model.
pub const RECOGNITION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx";

/// The model pair the engine needs: local file name + source URL.
/// The consent dialog prints exactly these URLs (and the fetched
/// byte counts) — no hidden second source.
pub struct ModelFileSpec {
    /// File name inside the model dir.
    pub file_name: &'static str,
    /// Where a consented fetch downloads it from.
    pub url: &'static str,
}

/// Both model files, detection first.
pub const MODEL_SPECS: [ModelFileSpec; 2] = [
    ModelFileSpec {
        file_name: DETECTION_MODEL_FILE,
        url: DETECTION_MODEL_URL,
    },
    ModelFileSpec {
        file_name: RECOGNITION_MODEL_FILE,
        url: RECOGNITION_MODEL_URL,
    },
];

/// Transport seam for the consented fetch. The production
/// implementation (reqwest, already a flintshot-app dependency) is
/// wired at the CLI surface; tests inject fakes that never touch
/// the network.
pub trait ModelDownloader {
    /// Fetch `url` and return its full bytes. The `Err` string is
    /// user-facing context (surrounded by the engine-failure
    /// envelope on the way out).
    ///
    /// # Errors
    /// Any transport failure (DNS, TLS, HTTP status, I/O); the
    /// caller maps it into [`OcrError::EngineFailure`].
    fn fetch(&mut self, url: &str) -> Result<Vec<u8>, String>;
}

/// Model search order: a consented fetch's cache dir first, then
/// the packaging pre-seed. First dir holding *both* files wins.
pub(crate) fn model_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(base) = crate::ocr::cache_subdir_under(dirs::cache_dir()) {
        dirs.push(base);
    }
    dirs.push(PathBuf::from(OCRS_MODEL_PRESEED_DIR));
    dirs
}

/// The production fetch destination: the XDG cache dir's
/// `flintshot/ocrs` subdir (`None` where the platform has no cache
/// dir).
#[must_use]
pub fn model_cache_dir() -> Option<PathBuf> {
    super::cache_subdir_under(dirs::cache_dir())
}

/// The engine itself: models loaded, ready for recognition passes.
///
/// Construction is where models load; recognition never touches the
/// filesystem again.
pub struct OcrsEngine {
    engine: OcrsModelEngine,
}

impl OcrsEngine {
    /// Load models from the first search dir ([`crate::ocr`]'s cache
    /// and pre-seed order) that holds both files.
    ///
    /// # Errors
    /// [`OcrError::NoEngine`] naming the fetch cure when no dir has
    /// the pair; [`OcrError::EngineFailure`] when a present file
    /// fails to load (corrupt/partial download).
    pub fn load() -> Result<Self, OcrError> {
        Self::load_from(&model_search_dirs())
    }

    /// Injectable variant of [`Self::load`] (tests, tooling).
    ///
    /// # Errors
    /// Same shape as [`Self::load`]: [`OcrError::NoEngine`] when no
    /// dir holds the pair, [`OcrError::EngineFailure`] when a
    /// present file fails to load.
    pub fn load_from(dirs: &[PathBuf]) -> Result<Self, OcrError> {
        let Some(dir) = dirs.iter().find(|d| models_present_in(d)) else {
            return Err(OcrError::NoEngine {
                guidance: format!("the built-in OCR models are not installed — {OCRS_MODELS_CURE}"),
            });
        };
        let load = |file: &str| rten::Model::load_file(dir.join(file));
        let params = OcrEngineParams {
            detection_model: Some(load(DETECTION_MODEL_FILE).map_err(|e| load_error(&e))?),
            recognition_model: Some(load(RECOGNITION_MODEL_FILE).map_err(|e| load_error(&e))?),
            ..OcrEngineParams::default()
        };
        let engine = OcrsModelEngine::new(params).map_err(|e| OcrError::EngineFailure {
            detail: format!("ocrs engine init: {e:#}"),
        })?;
        Ok(Self { engine })
    }
}

/// A present-but-unloadable model is an engine defect, not a
/// missing-install problem.
fn load_error(e: &rten::LoadError) -> OcrError {
    OcrError::EngineFailure {
        detail: format!("model load: {e}"),
    }
}

/// ocrs speaks RGB8 HWC; the editor pipeline is RGBA. Alpha is
/// dropped (regions come from opaque screenshots).
fn to_rgb(image: &RgbaImage) -> RgbImage {
    let (w, h) = image.dimensions();
    RgbImage::from_fn(w, h, |x, y| {
        let p = image.get_pixel(x, y).0;
        image::Rgb([p[0], p[1], p[2]])
    })
}

impl OcrEngine for OcrsEngine {
    fn recognize(&self, image: &RgbaImage) -> Result<OcrOutput, OcrError> {
        let rgb = to_rgb(image);
        let source = ImageSource::from_bytes(rgb.as_raw(), rgb.dimensions()).map_err(|e| {
            OcrError::EngineFailure {
                detail: format!("image to ocrs: {e}"),
            }
        })?;
        let input = self
            .engine
            .prepare_input(source)
            .map_err(|e| OcrError::EngineFailure {
                detail: format!("ocrs preprocess: {e:#}"),
            })?;
        let text = self
            .engine
            .get_text(&input)
            .map_err(|e| OcrError::EngineFailure {
                detail: format!("ocrs recognize: {e:#}"),
            })?;
        Ok(OcrOutput::new(text.trim_end_matches(['\n', '\r', '\x0c'])))
    }
}

/// Consent-gated fetch into `dir` (the cache dir in production, a
/// temp dir in tests). Without consent this returns an error and
/// **must not call `downloader` at all** — the no-network-without-
/// consent rule, pinned by test.
///
/// # Errors
/// [`OcrError::NoEngine`] (consent refused, naming the cure);
/// [`OcrError::EngineFailure`] (transport or write failure).
pub fn fetch_models_into(
    dir: &Path,
    consent: bool,
    downloader: &mut dyn ModelDownloader,
) -> Result<(), OcrError> {
    if !consent {
        return Err(OcrError::NoEngine {
            guidance: format!("model download was not consented to — {OCRS_MODELS_CURE}"),
        });
    }
    std::fs::create_dir_all(dir).map_err(|e| OcrError::EngineFailure {
        detail: format!("create model dir {}: {e}", dir.display()),
    })?;
    for spec in &MODEL_SPECS {
        let bytes = downloader
            .fetch(spec.url)
            .map_err(|e| OcrError::EngineFailure {
                detail: format!("download {}: {e}", spec.url),
            })?;
        std::fs::write(dir.join(spec.file_name), &bytes).map_err(|e| OcrError::EngineFailure {
            detail: format!("write {}: {e}", spec.file_name),
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fake transport that records the URLs it was asked for. Any
    /// call without consent is a spec violation.
    struct FakeDownloader {
        calls: Vec<String>,
        bytes: Vec<u8>,
    }

    impl FakeDownloader {
        fn new() -> Self {
            Self {
                calls: Vec::new(),
                bytes: b"fake-model-bytes".to_vec(),
            }
        }
    }

    impl ModelDownloader for FakeDownloader {
        fn fetch(&mut self, url: &str) -> Result<Vec<u8>, String> {
            self.calls.push(url.to_owned());
            Ok(self.bytes.clone())
        }
    }

    /// THE consent pin: without consent, no transport call happens.
    #[test]
    fn fetch_without_consent_never_touches_the_network() {
        let mut dl = FakeDownloader::new();
        let dir = tempfile::tempdir().expect("tempdir");
        let err = fetch_models_into(dir.path(), false, &mut dl)
            .expect_err("unconsented fetch must refuse");
        assert!(
            matches!(err, OcrError::NoEngine { ref guidance } if guidance.contains("consent")),
            "expected consent-carrying NoEngine, got {err:?}"
        );
        assert!(
            dl.calls.is_empty(),
            "transport was called without consent: {:?}",
            dl.calls
        );
        // And nothing was written, either.
        assert!(
            !dir.path().join(DETECTION_MODEL_FILE).exists(),
            "unconsented fetch must not leave files"
        );
    }

    /// A consented fetch pulls both models from their pinned URLs
    /// into the target dir, in spec order.
    #[test]
    fn fetch_with_consent_writes_both_models() {
        let mut dl = FakeDownloader::new();
        let dir = tempfile::tempdir().expect("tempdir");
        fetch_models_into(dir.path(), true, &mut dl).expect("consented fetch succeeds");
        assert_eq!(
            dl.calls,
            vec![DETECTION_MODEL_URL, RECOGNITION_MODEL_URL],
            "fetch order must follow MODEL_SPECS"
        );
        assert!(dir.path().join(DETECTION_MODEL_FILE).is_file());
        assert!(dir.path().join(RECOGNITION_MODEL_FILE).is_file());
    }

    /// The cache location is the XDG cache dir + the flintshot/ocrs
    /// subdirectory — no home-dir guessing.
    #[test]
    fn cache_dir_is_xdg_cache_subdir() {
        use crate::ocr::OCRS_MODEL_CACHE_SUBDIR;
        let base = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            crate::ocr::cache_subdir_under(Some(base.path().to_owned())),
            Some(base.path().join(OCRS_MODEL_CACHE_SUBDIR))
        );
    }

    /// Loading with no model dir present reports the fetch cure —
    /// this is the offline "fresh machine" shape.
    #[test]
    fn load_from_missing_dir_reports_models_cure() {
        let dir = tempfile::tempdir().expect("tempdir");
        // let-else, not expect_err: the engine has no Debug (the
        // wrapped ocrs engine doesn't implement it).
        let Err(err) = OcrsEngine::load_from(&[dir.path().to_owned()]) else {
            panic!("no models present must fail");
        };
        let OcrError::NoEngine { guidance } = &err else {
            panic!("expected NoEngine, got {err:?}");
        };
        assert!(
            guidance.contains("--fetch-models"),
            "guidance must name the fetch cure: {guidance}"
        );
    }

    /// Model-gated: needs the real models (≈tens of MB) locally.
    /// Run manually after `flinshot ocr --fetch-models`:
    /// `cargo test -p flintshot-core ocr::ocrs -- --ignored`.
    /// Tracked as an L2 manual row in `docs/TEST_MATRIX.md`.
    #[test]
    #[ignore = "requires fetched ocrs models (flinshot ocr --fetch-models)"]
    fn recognizes_fixture_text_with_fetched_models() {
        let engine = OcrsEngine::load().expect("models must be fetched first");
        let fixture = image::load_from_memory(include_bytes!("testdata/ocr-fixture.png"))
            .expect("fixture decodes")
            .to_rgba8();
        let out = engine.recognize(&fixture).expect("recognition succeeds");
        let text = out.text();
        assert!(
            text.contains("FLINTSHOT") && text.contains("42"),
            "fixture text not recognized, got: {text:?}"
        );
    }
}
