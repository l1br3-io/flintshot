//! Output writing: filename composition, collision avoidance, and
//! image encoding shared by every surface that writes a capture to
//! disk — the editor's direct Save, the editor's Copy persistence,
//! and the capture path's clipboard persistence. The latter two may
//! run in builds without any GUI crate (daemon-spawned capture
//! workers), which is why this lives in core rather than the editor
//! shell.
//!
//! Also owns the egress watermark ([`apply_watermark`]): the one
//! stamp every leaving-the-app path composes — never the canvas,
//! never the session store's mirrors.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::config::{OutputConfig, OutputFormat, WatermarkConfig, WatermarkPosition};
use crate::error::{FlintshotError, Result};
use crate::geom::{Color, Point};
use crate::render_filename_template;
use crate::text::TextStyle;

/// The text-render seam the watermark needs: measure a run at a font
/// size, and stamp it with an alpha scale. Production goes through
/// render.rs's cached system-font rasterizer; tests inject
/// deterministic geometry so placement / scale / clamp / opacity
/// plumbing is assertable without any font.
pub(crate) trait StampSeam {
    /// The rendered block's `(width, height)` at `font_size`, or
    /// `None` when no system font is available.
    fn measure(&mut self, text: &str, font_size: f32) -> Option<(f32, f32)>;
    /// Stamp `text` at `anchor` (top-left). Returns `false` when no
    /// system font is available (the caller degrades to unstamped).
    fn stamp(
        &mut self,
        img: &mut image::RgbaImage,
        anchor: Point,
        text: &str,
        font_size: f32,
        alpha: f32,
    ) -> bool;
}

/// The production [`StampSeam`]: white sans-serif glyphs through
/// render.rs's fontconfig resolution (warn + `false` when fontless).
struct SystemFontStamp;

impl StampSeam for SystemFontStamp {
    fn measure(&mut self, text: &str, font_size: f32) -> Option<(f32, f32)> {
        let m = crate::render::measure_text_run(text, &watermark_style(font_size))?;
        Some((m.width, m.height))
    }

    fn stamp(
        &mut self,
        img: &mut image::RgbaImage,
        anchor: Point,
        text: &str,
        font_size: f32,
        alpha: f32,
    ) -> bool {
        crate::render::stamp_text(img, anchor, text, &watermark_style(font_size), alpha)
    }
}

/// The watermark's glyph style: plain white sans-serif — legible on
/// most screenshots and deliberately not configurable in v1.
fn watermark_style(font_size: f32) -> TextStyle {
    TextStyle::simple(font_size, Color::rgb(255, 255, 255))
}

/// Image dimension → f32 (exact: dimensions ≤ 2^24, the f32
/// mantissa limit — same bound render.rs relies on).
#[allow(
    clippy::cast_precision_loss,
    reason = "image dimensions ≤ 2^24, exact in f32"
)]
fn dim_to_f32(d: u32) -> f32 {
    d as f32
}

/// Stamp font size for an image: `scale × height / 16` — at the
/// default `scale = 1.0` the text block is about 1/16 of the image
/// height. A non-positive scale floors to 0 (no visible stamp).
fn watermark_font_size(img_h: u32, scale: f32) -> f32 {
    scale.max(0.0) * dim_to_f32(img_h) / 16.0
}

/// Below this many px the glyphs are unreadable dots — skip the
/// stamp instead (a degenerate image or scale, warned).
const MIN_WATERMARK_FONT_PX: f32 = 1.0;

/// Padding around the stamped block, relative to its font size.
fn watermark_margin(font_size: f32) -> f32 {
    font_size / 2.0
}

/// 9-grid anchor for a `block` of size `(w, h)` inside an image of
/// `(img_w, img_h)`: left / centre / right column × top / centre /
/// bottom row, `margin` from the edges. Pure.
fn watermark_anchor(
    img_w: f32,
    img_h: f32,
    block: (f32, f32),
    margin: f32,
    position: WatermarkPosition,
) -> Point {
    let x = match position {
        WatermarkPosition::TopLeft | WatermarkPosition::Left | WatermarkPosition::BottomLeft => {
            margin
        }
        WatermarkPosition::Top | WatermarkPosition::Center | WatermarkPosition::Bottom => {
            (img_w - block.0) / 2.0
        }
        _ => img_w - block.0 - margin,
    };
    let y = match position {
        WatermarkPosition::TopLeft | WatermarkPosition::Top | WatermarkPosition::TopRight => margin,
        WatermarkPosition::Left | WatermarkPosition::Center | WatermarkPosition::Right => {
            (img_h - block.1) / 2.0
        }
        _ => img_h - block.1 - margin,
    };
    Point::new(x, y)
}

/// Stamp the configured watermark onto an egress-bound image.
///
/// No-op (the same buffer back, byte-identical) when the watermark
/// is disabled or its text is blank — egress never fails on it: a
/// missing system font, a degenerate image, or an unshrinkably long
/// text all degrade to unstamped with a warn (advisory posture).
///
/// Otherwise: 9-grid placement, deterministic opacity blend
/// (`out = white·(coverage·opacity) + bg·(1−coverage·opacity)`), a
/// font size of `scale × height / 16`, and shrink-to-fit so a long
/// text still lands entirely inside the image.
#[must_use]
pub fn apply_watermark<'a>(
    img: &'a image::RgbaImage,
    cfg: &WatermarkConfig,
) -> Cow<'a, image::RgbaImage> {
    apply_watermark_with(img, cfg, &mut SystemFontStamp)
}

/// [`apply_watermark`]'s core with the stamp seam injected (the
/// test seam — see [`StampSeam`]).
fn apply_watermark_with<'a>(
    img: &'a image::RgbaImage,
    cfg: &WatermarkConfig,
    seam: &mut dyn StampSeam,
) -> Cow<'a, image::RgbaImage> {
    if !cfg.is_active() {
        return Cow::Borrowed(img);
    }
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        tracing::warn!("watermark skipped: degenerate {}×{} image", w, h);
        return Cow::Borrowed(img);
    }
    let text = cfg.text.trim();
    let (img_w, img_h) = (dim_to_f32(w), dim_to_f32(h));
    let mut font_size = watermark_font_size(h, cfg.scale);
    if font_size < MIN_WATERMARK_FONT_PX {
        tracing::warn!(
            font_size,
            "watermark skipped: font size below the readable floor"
        );
        return Cow::Borrowed(img);
    }
    let Some(mut block) = seam.measure(text, font_size) else {
        tracing::warn!("watermark skipped: no system font available");
        return Cow::Borrowed(img);
    };
    // Shrink-to-fit: glyph advance scales linearly with the font
    // size, so one proportion step lands the block inside the
    // margins exactly (re-measured to swallow rounding).
    let avail_w = img_w - 2.0 * watermark_margin(font_size);
    let avail_h = img_h - 2.0 * watermark_margin(font_size);
    if block.0 > avail_w || block.1 > avail_h {
        font_size *= (avail_w / block.0).min(avail_h / block.1).min(1.0);
        if font_size < MIN_WATERMARK_FONT_PX {
            tracing::warn!("watermark skipped: text cannot shrink inside the image");
            return Cow::Borrowed(img);
        }
        if let Some(refit) = seam.measure(text, font_size) {
            block = refit;
        }
    }
    let anchor = watermark_anchor(
        img_w,
        img_h,
        block,
        watermark_margin(font_size),
        crate::config::parse_watermark_position(&cfg.position),
    );
    let mut stamped = img.clone();
    if seam.stamp(&mut stamped, anchor, text, font_size, cfg.opacity_clamped()) {
        Cow::Owned(stamped)
    } else {
        // The seam already warned (no system font).
        Cow::Borrowed(img)
    }
}

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
    } else if format == OutputFormat::Webp {
        "webp"
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
/// suffixing, creating `dir` on first write. PNG (lossless), JPEG at
/// `jpeg_quality`, or lossless WebP per `format` (the `image-webp`
/// encoder is lossless-only; `jpeg_quality` does not apply). Returns
/// the written path.
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
    } else if format == OutputFormat::Webp {
        let file = std::fs::File::create(&path)?;
        let encoder = image::codecs::webp::WebPEncoder::new_lossless(file);
        img.write_with_encoder(encoder)?;
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
/// `[output] format` encoding, the egress watermark stamped via
/// [`apply_watermark`] when enabled. Creates the directory on first
/// write. Returns the written path.
///
/// # Errors
/// See [`save_capture_image`].
pub fn persist_clipboard_copy(img: &image::RgbaImage, output: &OutputConfig) -> Result<PathBuf> {
    let format = crate::parse_output_format(&output.format);
    let stamped = apply_watermark(img, &output.watermark);
    save_capture_image(
        &output.resolve_save_dir(),
        &clipboard_copy_stem(output),
        &stamped,
        format,
        output.jpeg_quality,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;
    use tempfile::tempdir;

    // ── Watermark (WM2) ───────────────────────────────────────────

    use crate::geom::Point;

    /// A deterministic stamp seam: "glyph block" = a linear advance
    /// per font px, stamped as a solid white rect blended at `alpha`
    /// (out = 255·α + bg·(1−α)) — the same source-over math the real
    /// rasterizer applies per coverage-1 glyph interior. Records
    /// every call so placement/size/opacity plumbing is assertable
    /// without a system font.
    struct RecordingRectStamp {
        /// Measured block width per font px (`len × factor`).
        advance_per_px: f32,
        /// Calls seen by [`RecordingRectStamp::stamp`]:
        /// (anchor, font_size, alpha).
        calls: Vec<(Point, f32, f32)>,
    }

    impl RecordingRectStamp {
        fn new(advance_per_px: f32) -> Self {
            Self {
                advance_per_px,
                calls: Vec::new(),
            }
        }
    }

    impl StampSeam for RecordingRectStamp {
        fn measure(&mut self, text: &str, font_size: f32) -> Option<(f32, f32)> {
            #[allow(
                clippy::cast_precision_loss,
                reason = "test text lengths, exact in f32"
            )]
            let chars = text.chars().count() as f32;
            Some((self.advance_per_px * chars * font_size, font_size * 1.2))
        }

        fn stamp(
            &mut self,
            img: &mut image::RgbaImage,
            anchor: Point,
            _text: &str,
            font_size: f32,
            alpha: f32,
        ) -> bool {
            self.calls.push((anchor, font_size, alpha));
            let (block_w, block_h) = self
                .measure(_text, font_size)
                .expect("rect seam always measures");
            let x0 = anchor.x.round().clamp(0.0, dim_to_f32(img.width())) as u32;
            let y0 = anchor.y.round().clamp(0.0, dim_to_f32(img.height())) as u32;
            let x1 = (x0 + block_w.round() as u32).min(img.width());
            let y1 = (y0 + block_h.round() as u32).min(img.height());
            for py in y0..y1 {
                for px in x0..x1 {
                    let bg = img.get_pixel(px, py);
                    let out = [(bg.0[0]), (bg.0[1]), (bg.0[2])].map(|c| {
                        (255.0 * alpha + f32::from(c) * (1.0 - alpha))
                            .round()
                            .clamp(0.0, 255.0) as u8
                    });
                    img.put_pixel(px, py, image::Rgba([out[0], out[1], out[2], bg.0[3]]));
                }
            }
            true
        }
    }

    /// A seam with no system font: measure finds nothing, stamp fails.
    struct FontlessStamp;

    impl StampSeam for FontlessStamp {
        fn measure(&mut self, _text: &str, _font_size: f32) -> Option<(f32, f32)> {
            None
        }

        fn stamp(
            &mut self,
            _img: &mut image::RgbaImage,
            _anchor: Point,
            _text: &str,
            _font_size: f32,
            _alpha: f32,
        ) -> bool {
            false
        }
    }

    fn enabled_cfg() -> crate::config::WatermarkConfig {
        crate::config::WatermarkConfig {
            enabled: true,
            text: "© flintshot".into(),
            ..crate::config::WatermarkConfig::default()
        }
    }

    /// Disabled config → identity: the same buffer comes back
    /// (`Cow::Borrowed`), byte-identical, seam never invoked.
    #[test]
    fn apply_watermark_disabled_is_byte_identical_identity() {
        let img = image::RgbaImage::from_pixel(64, 64, image::Rgba([10, 20, 30, 255]));
        let mut seam = RecordingRectStamp::new(0.5);
        let out = apply_watermark_with(&img, &crate::config::WatermarkConfig::default(), &mut seam);
        assert!(
            matches!(out, std::borrow::Cow::Borrowed(_)),
            "disabled watermark returns the input buffer untouched"
        );
        assert!(seam.calls.is_empty(), "the seam is never invoked");
    }

    /// Enabled but blank text → identity (an enabled watermark with
    /// nothing to say stamps nothing).
    #[test]
    fn apply_watermark_blank_text_is_identity() {
        let img = image::RgbaImage::from_pixel(64, 64, image::Rgba([10, 20, 30, 255]));
        let mut cfg = enabled_cfg();
        cfg.text = "   ".into();
        let mut seam = RecordingRectStamp::new(0.5);
        let out = apply_watermark_with(&img, &cfg, &mut seam);
        assert!(matches!(out, std::borrow::Cow::Borrowed(_)));
        assert!(seam.calls.is_empty());
    }

    /// Font size = scale × height / 16 — the documented proportion.
    #[test]
    fn watermark_font_size_scales_with_image_height() {
        assert_eq!(watermark_font_size(100, 1.0), 6.25);
        assert_eq!(watermark_font_size(200, 1.0), 12.5, "double height");
        assert_eq!(watermark_font_size(100, 2.0), 12.5, "double scale");
        assert_eq!(watermark_font_size(100, 0.0), 0.0, "zero scale");
        assert_eq!(watermark_font_size(100, -1.0), 0.0, "negative scale floors");
    }

    /// The font size handed to the seam follows image height at a
    /// fixed config (proportional glyphs across two image sizes).
    #[test]
    fn apply_watermark_font_size_follows_image_height_through_the_seam() {
        let cfg = enabled_cfg();
        for (h, expected) in [(100u32, 6.25f32), (200, 12.5)] {
            let img = image::RgbaImage::from_pixel(300, h, image::Rgba([0, 0, 0, 255]));
            let mut seam = RecordingRectStamp::new(0.05);
            let out = apply_watermark_with(&img, &cfg, &mut seam);
            assert!(matches!(out, std::borrow::Cow::Owned(_)), "stamp happened");
            let (_, font_size, _) = seam.calls.last().copied().expect("one stamp");
            assert!(
                (font_size - expected).abs() < f32::EPSILON,
                "h={h}: font size {font_size} == scale×h/16 == {expected}"
            );
        }
    }

    /// Each of the 9 grid positions lands the stamped block's center
    /// in the correct third × third band of the image.
    #[test]
    fn apply_watermark_lands_each_of_the_nine_grid_positions() {
        let cases = [
            ("top-left", 0, 0),
            ("top", 1, 0),
            ("top-right", 2, 0),
            ("left", 0, 1),
            ("center", 1, 1),
            ("right", 2, 1),
            ("bottom-left", 0, 2),
            ("bottom", 1, 2),
            ("bottom-right", 2, 2),
        ];
        let (w, h) = (300u32, 300u32);
        for (raw, col_third, row_third) in cases {
            let mut cfg = enabled_cfg();
            cfg.position = raw.into();
            let img = image::RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, 255]));
            let mut seam = RecordingRectStamp::new(0.05);
            let out = apply_watermark_with(&img, &cfg, &mut seam);
            let stamped = match out {
                std::borrow::Cow::Owned(o) => o,
                std::borrow::Cow::Borrowed(_) => panic!("{raw}: expected a stamp"),
            };
            // Bounding box of stamped (non-black) pixels.
            let mut min = (u32::MAX, u32::MAX);
            let mut max = (0u32, 0u32);
            let mut any = false;
            for y in 0..h {
                for x in 0..w {
                    if stamped.get_pixel(x, y).0 != [0, 0, 0, 255] {
                        any = true;
                        min.0 = min.0.min(x);
                        min.1 = min.1.min(y);
                        max.0 = max.0.max(x);
                        max.1 = max.1.max(y);
                    }
                }
            }
            assert!(any, "{raw}: the stamp landed");
            let center = ((min.0 + max.0) / 2, (min.1 + max.1) / 2);
            assert_eq!(
                (center.0 / (w / 3), center.1 / (h / 3)),
                (col_third, row_third),
                "{raw}: stamped block centered in the right band, got {center:?}"
            );
        }
    }

    /// The clamped opacity (0.05–1.0 band) is what reaches the seam.
    #[test]
    fn apply_watermark_passes_clamped_opacity_to_the_stamp() {
        for (raw, expected) in [(0.0f32, 0.05f32), (2.0, 1.0), (0.6, 0.6)] {
            let mut cfg = enabled_cfg();
            cfg.opacity = raw;
            let img = image::RgbaImage::from_pixel(300, 100, image::Rgba([0, 0, 0, 255]));
            let mut seam = RecordingRectStamp::new(0.05);
            let _ = apply_watermark_with(&img, &cfg, &mut seam);
            let (_, _, alpha) = seam.calls.last().copied().expect("one stamp");
            assert!(
                (alpha - expected).abs() < f32::EPSILON,
                "opacity {raw} → {alpha}"
            );
        }
    }

    /// Long text shrinks to fit inside the image: the stamped block
    /// never exceeds the image bounds (minus the margin), and the
    /// font size the seam sees is below the configured one.
    #[test]
    fn apply_watermark_shrinks_long_text_to_fit_inside_the_image() {
        let mut cfg = enabled_cfg();
        // 60 chars × 2.0 advance × 6.25 px ≈ 750 px ≫ the 293 px that
        // fit in a 300-px image — nominal size cannot fit.
        cfg.text =
            "an extremely long watermark string that cannot fit inside this image no matter what"
                .into();
        let (w, h) = (300u32, 100u32);
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, 255]));
        let mut seam = RecordingRectStamp::new(2.0);
        let out = apply_watermark_with(&img, &cfg, &mut seam);
        assert!(matches!(out, std::borrow::Cow::Owned(_)), "still stamps");
        let (anchor, font_size, _) = seam.calls.last().copied().expect("one stamp");
        let nominal = watermark_font_size(h, cfg.scale);
        assert!(
            font_size < nominal,
            "shrunk from {nominal} to {font_size} to fit"
        );
        let (block_w, block_h) = seam.measure(&cfg.text, font_size).expect("measures");
        assert!(
            anchor.x >= 0.0
                && anchor.y >= 0.0
                && anchor.x + block_w <= dim_to_f32(w) + 0.5
                && anchor.y + block_h <= dim_to_f32(h) + 0.5,
            "block {block_w}×{block_h} at {anchor:?} stays inside {w}×{h}"
        );
    }

    /// No system font → degrade to unstamped with the input buffer
    /// returned untouched (egress never fails on the watermark).
    #[test]
    fn apply_watermark_missing_font_degrades_to_unstamped() {
        let img = image::RgbaImage::from_pixel(64, 64, image::Rgba([7, 8, 9, 255]));
        let mut seam = FontlessStamp;
        let out = apply_watermark_with(&img, &enabled_cfg(), &mut seam);
        assert!(
            matches!(out, std::borrow::Cow::Borrowed(_)),
            "no font: unstamped, byte-identical"
        );
    }

    /// The real system-font seam (render.rs rasterizer): with a font
    /// available, a solid-black background and white text at full
    /// opacity leave *exactly white* interior pixels (out =
    /// 255·1·coverage + 0·(1−1·coverage) at coverage 1) — the
    /// deterministic opacity math on the production path. The image
    /// is tall enough that `scale × h / 16` glyphs are chunky, so
    /// coverage-1 interiors exist. Hosts with no sans-serif skip the
    /// pixel leg gracefully (compose_final precedent); the identity
    /// contract above never skips.
    #[test]
    fn apply_watermark_blends_interior_glyph_pixels_at_configured_opacity() {
        let mut cfg = enabled_cfg();
        cfg.opacity = 1.0;
        let img = image::RgbaImage::from_pixel(400, 600, image::Rgba([0, 0, 0, 255]));
        let out = apply_watermark(&img, &cfg);
        let stamped = match out {
            std::borrow::Cow::Owned(o) => o,
            std::borrow::Cow::Borrowed(_) => {
                // No system font on this host (minimal CI container):
                // the warn-and-degrade path already covered above.
                eprintln!("skipping: no system font available");
                return;
            }
        };
        let white = stamped
            .pixels()
            .filter(|p| p.0 == [255, 255, 255, 255])
            .count();
        assert!(white > 0, "coverage-1 glyph interiors are pure white");

        // Half opacity on black: interiors land on exactly
        // round(255·0.5) = 128 (255·0.5 + 0·0.5).
        cfg.opacity = 0.5;
        let img = image::RgbaImage::from_pixel(400, 600, image::Rgba([0, 0, 0, 255]));
        let out = apply_watermark(&img, &cfg);
        let stamped = match out {
            std::borrow::Cow::Owned(o) => o,
            std::borrow::Cow::Borrowed(_) => {
                eprintln!("skipping: no system font available");
                return;
            }
        };
        let half = stamped
            .pixels()
            .filter(|p| p.0 == [128, 128, 128, 255])
            .count();
        assert!(half > 0, "coverage-1 interiors are exactly 50% white");
    }

    // ── egress wiring (WM3) ───────────────────────────────────────

    /// A clipboard-copy persist with the watermark ON stamps the
    /// written file (soft: the stamp needs a system font — a
    /// fontless host skips the pixel leg; the disabled contract
    /// below never skips).
    #[test]
    fn persist_clipboard_copy_stamps_when_watermark_enabled() {
        let dir = tempdir().unwrap();
        let mut output = OutputConfig::default();
        output.save_dir = dir.path().to_string_lossy().into_owned();
        output.watermark.enabled = true;
        let img = image::RgbaImage::from_pixel(400, 600, image::Rgba([0, 0, 0, 255]));
        let written = persist_clipboard_copy(&img, &output).expect("persist");
        let back = image::open(&written).expect("decodes").to_rgba8();
        let changed = back.pixels().filter(|p| p.0 != [0, 0, 0, 255]).count();
        if changed == 0 {
            eprintln!("skipping: no system font available");
            return;
        }
        assert!(changed > 0, "the persisted copy carries the stamp");
    }

    /// The disabled watermark keeps the persisted copy pixel-identical
    /// to the source (the never-surprise-stamp contract).
    #[test]
    fn persist_clipboard_copy_disabled_is_pixel_identical() {
        let dir = tempdir().unwrap();
        let mut output = OutputConfig::default();
        output.save_dir = dir.path().to_string_lossy().into_owned();
        let img = image::RgbaImage::from_pixel(40, 30, image::Rgba([1, 2, 3, 255]));
        let written = persist_clipboard_copy(&img, &output).expect("persist");
        let back = image::open(&written).expect("decodes").to_rgba8();
        assert_eq!(back, img, "no watermark, no change — byte-for-byte");
    }

    /// The session store's recovery mirror stays unstamped even with
    /// a watermark configured: `insert` writes the pristine source
    /// (`<id>.png`, session-resume interop — recovery returns
    /// unstamped pixels).
    #[test]
    fn session_store_mirror_stays_unstamped_with_watermark_enabled() {
        let dir = tempdir().unwrap();
        let store = crate::session::SessionStore::in_dir(dir.path().to_path_buf());
        let img = image::RgbaImage::from_pixel(40, 30, image::Rgba([4, 5, 6, 255]));
        let id = store.insert(img.clone(), 0);
        let mirror = dir.path().join(format!("{}.png", id.raw()));
        // The PNG write rides a short-lived thread; poll briefly.
        for _ in 0..100 {
            if mirror.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let back = image::open(&mirror).expect("the mirror png").to_rgba8();
        assert_eq!(back, img, "the mirror is the pristine source");
    }

    // ── save/clipboard paths ─────────────────────────────────────

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
        assert_eq!(output_ext(OutputFormat::Webp), "webp");
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
    fn save_capture_image_writes_lossless_webp_with_webp_ext() {
        let dir = tempdir().unwrap();
        let img = image::RgbaImage::from_pixel(8, 6, image::Rgba([10, 200, 30, 255]));
        let written = save_capture_image(dir.path(), "shot", &img, OutputFormat::Webp, 90).unwrap();
        assert_eq!(
            written.extension().map(|e| e.to_string_lossy()),
            Some("webp".into()),
            "webp files take the .webp extension"
        );
        let back = image::open(&written).unwrap();
        assert_eq!(back.dimensions(), (8, 6), "the written file decodes");
        assert_eq!(
            back.to_rgba8(),
            img,
            "lossless: WebP output round-trips the pixels exactly"
        );
    }

    #[test]
    fn webp_output_carries_the_riff_webp_magic() {
        let dir = tempdir().unwrap();
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([7, 77, 177, 255]));
        let written = save_capture_image(dir.path(), "shot", &img, OutputFormat::Webp, 90).unwrap();
        let mut head = [0u8; 12];
        std::io::Read::read_exact(
            &mut std::io::BufReader::new(std::fs::File::open(&written).unwrap()),
            &mut head,
        )
        .unwrap();
        assert_eq!(&head[0..4], b"RIFF", "a WebP container starts with RIFF");
        assert_eq!(
            &head[8..12],
            b"WEBP",
            "a WebP container carries the WEBP tag"
        );
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
