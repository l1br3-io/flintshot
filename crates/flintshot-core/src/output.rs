//! Output writing: filename composition, collision avoidance, and
//! image encoding shared by every surface that writes a capture to
//! disk — the editor's direct Save, the editor's Copy persistence,
//! and the capture path's clipboard persistence. The latter two may
//! run in builds without any GUI crate (daemon-spawned capture
//! workers), which is why this lives in core rather than the editor
//! shell.

use std::path::{Path, PathBuf};

use crate::config::{OutputConfig, OutputFormat};
use crate::error::{FlintshotError, Result};
use crate::render_filename_template;

/// `YYYY-MM-DD` for the `{date}` placeholder (local-time notation,
/// UTC via std — good enough for a filename stamp; no chrono dep).
#[must_use]
pub fn chrono_date() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let days = now.as_secs() / 86_400;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `HH-MM-SS` for the `{time}` placeholder (UTC, filename-safe).
#[must_use]
pub fn chrono_time() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() % 86_400;
    format!(
        "{:02}-{:02}-{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// Days-since-epoch → civil `(y, m, d)` (Howard Hinnant's
/// algorithm; UTC only).
fn civil_from_days(days: u64) -> (i64, u32, u32) {
    let z = i64::try_from(days).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (
        if m <= 2 { y + 1 } else { y },
        u32::try_from(m).unwrap_or(1),
        d,
    )
}

/// The output format's file extension.
#[must_use]
pub fn output_ext(format: OutputFormat) -> &'static str {
    if format == OutputFormat::Jpeg {
        "jpg"
    } else {
        "png"
    }
}

/// The next free path: `name.ext`, then `name-2.ext`, `name-3.ext`,
/// … skipping anything that already exists (the collision rule).
/// Pure modulo the filesystem probe.
#[must_use]
pub fn next_free_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path.file_stem().map_or_else(
        || "capture".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    let ext = path
        .extension()
        .map_or_else(|| "png".to_owned(), |s| s.to_string_lossy().into_owned());
    let parent = path.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    for n in 2u32.. {
        let candidate = parent.join(format!("{stem}-{n}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("u32 collisions exhausted")
}

/// Write `img` to `dir/stem.ext` with `-2`-style collision
/// suffixing, creating `dir` on first write. PNG (lossless) or JPEG
/// at `jpeg_quality` per `format`. Returns the written path.
///
/// # Errors
/// Surfaces [`FlintshotError::Io`] on dir creation or the file
/// write, and [`FlintshotError::Image`] on encode failure.
pub fn save_capture_image(
    dir: &Path,
    stem: &str,
    img: &image::RgbaImage,
    format: OutputFormat,
    jpeg_quality: u32,
) -> Result<PathBuf> {
    let path = next_free_path(dir.join(format!("{stem}.{}", output_ext(format))));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(FlintshotError::Io)?;
    }
    if format == OutputFormat::Jpeg {
        let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
        let file = std::fs::File::create(&path)?;
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
            file,
            u8::try_from(jpeg_quality.clamp(1, 100)).unwrap_or(90),
        );
        rgb.write_with_encoder(encoder)?;
    } else {
        img.save_with_format(&path, image::ImageFormat::Png)?;
    }
    Ok(path)
}

/// The clipboard-copy filename stem: the rendered filename template
/// when one is configured, else the built-in `flintshot-{date}-
/// {time}` scheme — the same collision and directory rules as
/// direct saves.
#[must_use]
pub fn clipboard_copy_stem(output: &OutputConfig) -> String {
    if output.filename_template.is_empty() {
        return format!("flintshot-{}-{}", chrono_date(), chrono_time());
    }
    render_filename_template(
        &output.filename_template,
        env!("CARGO_PKG_VERSION"),
        &chrono_date(),
        &chrono_time(),
    )
}

/// Persist one clipboard copy per `output` (the
/// `save-clipboard-copies` feature): [`clipboard_copy_stem`]
/// naming, the resolved save directory
/// ([`OutputConfig::resolve_save_dir`]), collision suffixing,
/// `[output] format` encoding. Creates the directory on first
/// write. Returns the written path.
///
/// # Errors
/// See [`save_capture_image`].
pub fn persist_clipboard_copy(img: &image::RgbaImage, output: &OutputConfig) -> Result<PathBuf> {
    let format = crate::parse_output_format(&output.format);
    save_capture_image(
        &output.resolve_save_dir(),
        &clipboard_copy_stem(output),
        img,
        format,
        output.jpeg_quality,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;
    use tempfile::tempdir;

    #[test]
    fn date_time_renderers_are_filename_safe() {
        assert_eq!(chrono_date().len(), 10, "YYYY-MM-DD");
        assert!(
            chrono_date()
                .chars()
                .all(|c| c.is_ascii_digit() || c == '-'),
            "digits and dashes only: {}",
            chrono_date()
        );
        assert_eq!(chrono_time().len(), 8, "HH-MM-SS");
        assert!(
            chrono_time()
                .chars()
                .all(|c| c.is_ascii_digit() || c == '-'),
            "digits and dashes only: {}",
            chrono_time()
        );
    }

    #[test]
    fn output_ext_maps_formats() {
        assert_eq!(output_ext(OutputFormat::Png), "png");
        assert_eq!(output_ext(OutputFormat::Jpeg), "jpg");
    }

    #[test]
    fn next_free_path_appends_collision_suffix() {
        let dir = tempdir().unwrap();
        let first = next_free_path(dir.path().join("shot.png"));
        assert_eq!(
            first,
            dir.path().join("shot.png"),
            "free path passes through"
        );
        std::fs::write(&first, b"x").unwrap();
        let second = next_free_path(dir.path().join("shot.png"));
        assert_eq!(
            second,
            dir.path().join("shot-2.png"),
            "first collision gets -2"
        );
        std::fs::write(&second, b"x").unwrap();
        let third = next_free_path(dir.path().join("shot.png"));
        assert_eq!(
            third,
            dir.path().join("shot-3.png"),
            "second collision gets -3"
        );
    }

    #[test]
    fn save_capture_image_creates_a_missing_dir_and_writes_png() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("nested").join("deeper");
        let img = image::RgbaImage::from_pixel(8, 6, image::Rgba([10, 20, 30, 255]));
        let written = save_capture_image(&target, "shot", &img, OutputFormat::Png, 90).unwrap();
        assert_eq!(written, target.join("shot.png"));
        let back = image::open(&written).unwrap();
        assert_eq!(back.dimensions(), (8, 6), "the written file decodes");
    }

    #[test]
    fn save_capture_image_writes_jpeg_with_quality_and_jpg_ext() {
        let dir = tempdir().unwrap();
        let img = image::RgbaImage::from_pixel(8, 6, image::Rgba([200, 30, 30, 255]));
        let written = save_capture_image(dir.path(), "shot", &img, OutputFormat::Jpeg, 90).unwrap();
        assert_eq!(
            written.extension().map(|e| e.to_string_lossy()),
            Some("jpg".into()),
            "jpeg files take the .jpg extension"
        );
        let back = image::open(&written).unwrap();
        assert_eq!(back.dimensions(), (8, 6));
    }

    #[test]
    fn save_capture_image_collisions_get_suffixed() {
        let dir = tempdir().unwrap();
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]));
        let first = save_capture_image(dir.path(), "shot", &img, OutputFormat::Png, 90).unwrap();
        assert_eq!(first, dir.path().join("shot.png"));
        let second = save_capture_image(dir.path(), "shot", &img, OutputFormat::Png, 90).unwrap();
        assert_eq!(second, dir.path().join("shot-2.png"));
    }

    #[test]
    fn clipboard_copy_stem_spans_template_and_builtin() {
        let with_template = OutputConfig {
            filename_template: "{app}-{date}".into(),
            ..OutputConfig::default()
        };
        let stem = clipboard_copy_stem(&with_template);
        assert!(
            stem.starts_with(env!("CARGO_PKG_VERSION")),
            "the template renders with the app version as {{app}}: {stem}"
        );
        assert!(
            stem.contains(&chrono_date()),
            "the template renders {{date}}: {stem}"
        );

        let builtin = clipboard_copy_stem(&OutputConfig::default());
        assert!(
            builtin.starts_with("flintshot-"),
            "no template: the built-in flintshot-{{date}}-{{time}} scheme, got {builtin}"
        );
        assert!(
            builtin.contains(&chrono_date()),
            "built-in carries the date"
        );
    }

    #[test]
    fn persist_clipboard_copy_writes_the_resolved_dir() {
        let dir = tempdir().unwrap();
        let output = OutputConfig {
            save_dir: dir.path().to_string_lossy().into_owned(),
            ..OutputConfig::default()
        };
        let img = image::RgbaImage::from_pixel(5, 5, image::Rgba([9, 9, 9, 255]));
        let written = persist_clipboard_copy(&img, &output).unwrap();
        assert!(
            written.starts_with(dir.path()),
            "the copy lands in the resolved dir: {written:?}"
        );
        assert!(written.exists(), "the file is on disk");
        assert!(
            written
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("flintshot-")),
            "built-in naming when no template: {written:?}"
        );
    }
}
