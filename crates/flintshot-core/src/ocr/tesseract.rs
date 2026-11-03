//! The opt-in `tesseract` engine: a CLI wrapper over the system
//! binary, like the record encoder probe (no linked dependency).
//!
//! Every installed language is available — that is this engine's
//! whole reason to exist beside the Latin-only built-in — at the
//! cost of requiring a user-installed binary. Recognition writes the
//! (already region-cropped) image to a temp PNG, runs
//! `tesseract <png> stdout -l <lang>`, and parses stdout; failures
//! are classified by shape: a vanished binary or missing language
//! data are [`OcrError::NoEngine`] carrying install guidance,
//! anything else is [`OcrError::EngineFailure`].

use std::path::PathBuf;
use std::process::Command;

use image::RgbaImage;

use super::{OcrEngine, OcrError, OcrOutput, TESSERACT_INSTALL_CURE};

/// Engine driving the system `tesseract` binary through its CLI.
///
/// The binary path is injectable for tests (and unusual installs);
/// [`TesseractEngine::new`] relies on `PATH` resolution at spawn
/// time, so a missing binary surfaces as [`OcrError::NoEngine`]
/// with the install cure rather than a construction failure.
#[derive(Debug, Clone)]
pub struct TesseractEngine {
    /// Binary to spawn; a bare name resolves via `PATH`.
    binary: PathBuf,
    /// Language code (`eng`, `deu`, …) passed as `-l`; empty means
    /// tesseract's own default. Tesseract-only — the built-in engine
    /// has no language switch.
    lang: String,
}

impl TesseractEngine {
    /// Engine using the `tesseract` binary found on `PATH`.
    #[must_use]
    pub fn new(lang: impl Into<String>) -> Self {
        Self {
            binary: PathBuf::from("tesseract"),
            lang: lang.into(),
        }
    }

    /// Same, with the binary path injected (tests, unusual installs).
    #[must_use]
    pub fn with_binary(binary: impl Into<PathBuf>, lang: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
            lang: lang.into(),
        }
    }

    /// The configured language code (empty = tesseract default).
    #[must_use]
    pub fn lang(&self) -> &str {
        &self.lang
    }
}

/// tesseract's stable "language data missing" stderr shape: the
/// `Error opening data file` line, with the `TESSDATA_PREFIX` hint
/// tesseract itself prints after it.
const MISSING_LANG_MARKERS: [&str; 2] = ["Error opening data file", "TESSDATA_PREFIX"];

/// Turn a failed pass's stderr into the right error: missing
/// language data is a user-curable install/config problem, not an
/// engine defect.
fn classify_failure(lang: &str, stderr: &str) -> OcrError {
    if MISSING_LANG_MARKERS.iter().any(|m| stderr.contains(m)) {
        OcrError::NoEngine {
            guidance: format!(
                "tesseract language data for '{lang}' is missing — \
                 install it (e.g. `apt install tesseract-ocr-{lang}`) \
                 or check TESSDATA_PREFIX"
            ),
        }
    } else {
        OcrError::EngineFailure {
            detail: stderr.trim().to_owned(),
        }
    }
}

/// Normalize tesseract stdout into the v1 text contract: the CLI
/// terminates its output with trailing newlines (and, historically,
/// a form feed) that are not part of the recognized text.
fn parse_stdout(out: &str) -> OcrOutput {
    OcrOutput::new(out.trim_end_matches(['\n', '\r', '\x0c']))
}

impl OcrEngine for TesseractEngine {
    fn recognize(&self, image: &RgbaImage) -> Result<OcrOutput, OcrError> {
        // Temp-file seam: tesseract reads images from paths, so the
        // (already region-cropped) pixels land in a NamedTempFile
        // that deletes itself when this pass ends.
        let mut png = tempfile::Builder::new()
            .prefix("flintshot-ocr-")
            .suffix(".png")
            .tempfile()
            .map_err(|e| OcrError::EngineFailure {
                detail: format!("create temp PNG: {e}"),
            })?;
        image
            .write_to(png.as_file_mut(), image::ImageFormat::Png)
            .map_err(|e| OcrError::EngineFailure {
                detail: format!("encode region PNG: {e}"),
            })?;

        let mut cmd = Command::new(&self.binary);
        cmd.arg(png.path()).arg("stdout");
        if !self.lang.is_empty() {
            cmd.arg("-l").arg(&self.lang);
        }
        let out = cmd.output().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => OcrError::NoEngine {
                guidance: format!("the tesseract binary was not found — {TESSERACT_INSTALL_CURE}"),
            },
            _ => OcrError::EngineFailure {
                detail: format!("spawn {}: {e}", self.binary.display()),
            },
        })?;

        if out.status.success() {
            Ok(parse_stdout(&String::from_utf8_lossy(&out.stdout)))
        } else {
            Err(classify_failure(
                &self.lang,
                &String::from_utf8_lossy(&out.stderr),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1×1 image is enough — the fake binaries ignore their input.
    fn tiny_image() -> RgbaImage {
        RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255]))
    }

    /// Write an executable fake `tesseract` and return its path.
    #[cfg(unix)]
    fn fake_binary(dir: &std::path::Path, script: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("fake-tesseract");
        std::fs::write(&path, script).expect("write fake binary");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake binary");
        path
    }

    /// The full path through a fake binary that prints two lines.
    #[cfg(unix)]
    #[test]
    fn recognize_parses_fake_stdout() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bin = fake_binary(dir.path(), "#!/bin/sh\nprintf 'hello\\nworld\\n'\n");
        let engine = TesseractEngine::with_binary(bin, "eng");
        let out = engine.recognize(&tiny_image()).expect("fake succeeds");
        assert_eq!(out.text(), "hello\nworld");
    }

    /// tesseract's trailing blank line / form feed are packaging,
    /// not text — they must not reach the clipboard as extra lines.
    #[cfg(unix)]
    #[test]
    fn recognize_trims_trailing_newlines_and_form_feed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bin = fake_binary(dir.path(), "#!/bin/sh\nprintf 'a\\nb\\n\\n\\014'\n");
        let engine = TesseractEngine::with_binary(bin, "eng");
        let out = engine.recognize(&tiny_image()).expect("fake succeeds");
        assert_eq!(out.text(), "a\nb");
    }

    /// A vanished binary is a `NoEngine` error carrying the install
    /// cure — resolution raced the uninstall, or `PATH` changed.
    #[test]
    fn missing_binary_reports_install_cure() {
        let engine = TesseractEngine::with_binary("/nonexistent/tesseract-ocr", "eng");
        let err = engine.recognize(&tiny_image()).expect_err("binary absent");
        let OcrError::NoEngine { guidance } = &err else {
            panic!("expected NoEngine, got {err:?}");
        };
        assert!(
            guidance.contains("tesseract"),
            "guidance must name the install cure: {guidance}"
        );
    }

    /// tesseract's missing-language stderr shape maps to `NoEngine`
    /// with guidance naming the configured language.
    #[cfg(unix)]
    #[test]
    fn missing_language_data_reports_lang_cure() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bin = fake_binary(
            dir.path(),
            "#!/bin/sh\necho 'Error opening data file /usr/share/tessdata/deu.traineddata' >&2\n\
             echo 'Please make sure the TESSDATA_PREFIX environment variable is set' >&2\nexit 1\n",
        );
        let engine = TesseractEngine::with_binary(bin, "deu");
        let err = engine
            .recognize(&tiny_image())
            .expect_err("lang data missing");
        let OcrError::NoEngine { guidance } = &err else {
            panic!("expected NoEngine, got {err:?}");
        };
        assert!(
            guidance.contains("deu") && guidance.contains("tesseract-ocr-deu"),
            "guidance must name the language cure: {guidance}"
        );
    }

    /// Any other nonzero exit with unrelated stderr is an engine
    /// failure — nothing the user can install their way out of by
    /// the same cure.
    #[cfg(unix)]
    #[test]
    fn other_failure_is_engine_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bin = fake_binary(dir.path(), "#!/bin/sh\necho 'corrupt image' >&2\nexit 1\n");
        let engine = TesseractEngine::with_binary(bin, "eng");
        let err = engine.recognize(&tiny_image()).expect_err("fake fails");
        assert!(
            matches!(err, OcrError::EngineFailure { ref detail } if detail.contains("corrupt image")),
            "expected EngineFailure with stderr detail, got {err:?}"
        );
    }
}
