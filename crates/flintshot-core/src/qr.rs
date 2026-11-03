//! QR: region → payloads, largest-first.
//!
//! The qr-decode contract behind both surfaces (editor tool, `qr`
//! CLI verb): the user frames a region, every code inside it
//! decodes, and the payloads come back ordered by area — largest
//! first, the code the user framed leads (SPEC-qr-decode's working
//! answer to area-vs-reading-order). One engine: `rqrr`, pure Rust
//! (barcode-beyond-QR stays rejected on record — every mature
//! barcode decoder is C/C++).
//!
//! The honesty split mirrors [`crate::ocr`]: nothing QR-shaped in
//! the region is a *friendly* empty ([`decode_region`] returns
//! `Ok(vec![])` — the cure is a closer re-capture, surfaced by the
//! caller); something QR-shaped that will not decode is a
//! [`QrError::Unreadable`] error, never a silent fake success.

use crate::geom::Rect;
use crate::image_ops::crop_region;

use image::RgbaImage;

/// One decoded QR code: its payload and pixel area (the
/// largest-first order key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrCode {
    payload: String,
    area: u64,
}

impl QrCode {
    /// Wrap a decoded `payload` with its `area` in square pixels.
    #[must_use]
    pub fn new(payload: impl Into<String>, area: u64) -> Self {
        Self {
            payload: payload.into(),
            area,
        }
    }

    /// The decoded payload, verbatim.
    #[must_use]
    pub fn payload(&self) -> &str {
        &self.payload
    }

    /// The code's on-image area in square pixels.
    #[must_use]
    pub fn area(&self) -> u64 {
        self.area
    }
}

/// QR decode failures. v1 has one shape: detection found
/// QR-shaped grids but none would decode — corrupt, too small, too
/// skewed. The cure is always the same and the [`Display`]
/// copy names it: a closer, larger re-capture.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum QrError {
    /// At least one QR-shaped grid was detected; none decoded.
    /// `detail` carries the lower-level failure verbatim.
    #[error("QR code detected but unreadable ({detail}) — re-capture the region closer or larger")]
    Unreadable {
        /// Lower-level decode-failure description.
        detail: String,
    },
}

/// One decode pass over a (possibly cropped) image — the
/// [`crate::ocr::OcrEngine`] seam, sized for QR's single engine so
/// callers and tests can substitute fakes.
pub trait QrEngine {
    /// Decode every QR code in `image`. `Ok` carries every code that
    /// decoded (possibly none — nothing QR-shaped is a friendly
    /// empty); `Err` means grids were detected but none decoded.
    ///
    /// # Errors
    /// [`QrError::Unreadable`] when detection found QR-shaped grids
    /// and none of them decoded.
    fn decode(&self, image: &RgbaImage) -> Result<Vec<QrCode>, QrError>;
}

/// The `rqrr`-backed engine: pure-Rust detect + decode. Detection
/// thresholds a grayscale *copy* (rqrr consumes the buffer), so the
/// caller's pixels are never mutated. Grids that detect but fail to
/// decode alongside readable siblings are skipped as false-positive
/// detections — only an all-failed frame is an error.
#[derive(Debug, Default)]
pub struct RqrrEngine;

impl QrEngine for RqrrEngine {
    fn decode(&self, image: &RgbaImage) -> Result<Vec<QrCode>, QrError> {
        if image.width() == 0 || image.height() == 0 {
            return Ok(Vec::new());
        }
        let mut prepared = rqrr::PreparedImage::prepare(image::imageops::grayscale(image));
        let grids = prepared.detect_grids();
        let mut codes = Vec::new();
        let mut failures = Vec::new();
        for grid in grids {
            match grid.decode() {
                Ok((_meta, payload)) => codes.push(QrCode::new(payload, quad_area(&grid.bounds))),
                Err(e) => failures.push(e.to_string()),
            }
        }
        if codes.is_empty() {
            if let Some(detail) = failures.first() {
                return Err(QrError::Unreadable {
                    detail: detail.clone(),
                });
            }
        }
        Ok(codes)
    }
}

/// Shoelace area of a grid's corner quad, in square pixels. rqrr
/// reports the four corners in image coordinates; the quadrilateral
/// area is the largest-first order key (skewed codes included).
fn quad_area([tl, tr, br, bl]: &[rqrr::Point; 4]) -> u64 {
    let cross = |a: &rqrr::Point, b: &rqrr::Point| {
        i64::from(a.x) * i64::from(b.y) - i64::from(b.x) * i64::from(a.y)
    };
    let doubled = cross(tl, tr) + cross(tr, br) + cross(br, bl) + cross(bl, tl);
    doubled.unsigned_abs() / 2
}

/// Decode every QR code inside `region` (`None` = the whole image),
/// ordered by area (largest first — the code the user framed leads).
/// Empty when none found; callers surface the friendly re-capture
/// copy, not a raw error.
///
/// # Errors
/// [`QrError::Unreadable`] — propagated from the engine when
/// QR-shaped grids were detected but none decoded.
pub fn decode_region(
    image: &RgbaImage,
    region: Option<Rect>,
    engine: &dyn QrEngine,
) -> Result<Vec<String>, QrError> {
    let pixels = match region {
        Some(rect) => crop_region(image, rect),
        None => image.clone(),
    };
    let mut codes = engine.decode(&pixels)?;
    codes.sort_by_key(|code| std::cmp::Reverse(code.area()));
    Ok(codes
        .into_iter()
        .map(|code| code.payload().to_owned())
        .collect())
}

/// Whether `payload` earns the result card's Open affordance: an
/// absolute http(s) URL and nothing else. The allowlist is a
/// security boundary — `xdg-open` dispatches by scheme, so any
/// other scheme (`file:`, `mailto:`, unregistered) would run an
/// arbitrary registered handler on untrusted screen content.
/// Control bytes never pass either (payloads are untrusted bytes
/// read off the screen).
#[must_use]
pub fn is_url(payload: &str) -> bool {
    let lowered = payload.to_ascii_lowercase();
    let Some(rest) = lowered
        .strip_prefix("https://")
        .or_else(|| lowered.strip_prefix("http://"))
    else {
        return false;
    };
    !rest.is_empty() && !rest.bytes().any(|byte| byte.is_ascii_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rendered QR code (quiet zone included) as RGBA — the
    /// deterministic fixture generator (dev-dep `qrcode`; never
    /// linked in release).
    fn code_image(payload: &str, module_size: u32) -> RgbaImage {
        let code = qrcode::QrCode::new(payload).expect("fixture payload encodes");
        let luma = code
            .render::<image::Luma<u8>>()
            .module_dimensions(module_size, module_size)
            .build();
        image::DynamicImage::ImageLuma8(luma).to_rgba8()
    }

    /// A white canvas with `code` pasted at `(x, y)`.
    fn canvas_with(code: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> RgbaImage {
        let mut canvas = RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));
        image::imageops::overlay(&mut canvas, code, i64::from(x), i64::from(y));
        canvas
    }

    /// The generated codes decode to their exact payloads — text, a
    /// URL, and a long string (byte-mode capacity shape).
    #[test]
    fn text_url_and_long_payloads_decode_exactly() {
        let long = "flintshot-qr-".repeat(15);
        for payload in [
            "hello flintshot",
            "https://flintshot.example/docs",
            long.as_str(),
        ] {
            let img = code_image(payload, 4);
            assert_eq!(
                decode_region(&img, None, &RqrrEngine).expect("a rendered code decodes"),
                vec![payload.to_owned()],
                "payload round-trips verbatim: {payload}"
            );
        }
    }

    /// A code flush against the image corner decodes inside a region
    /// that hugs the canvas edge.
    #[test]
    #[allow(clippy::cast_precision_loss, reason = "fixture dims are exact in f32")]
    fn code_flush_to_the_corner_decodes_inside_a_tight_region() {
        let code = code_image("edge case", 4);
        let (w, h) = code.dimensions();
        let canvas = canvas_with(&code, 0, 0, w * 2, h * 2);
        let region = Rect::from_xywh(0.0, 0.0, w as f32, h as f32);
        assert_eq!(
            decode_region(&canvas, Some(region), &RqrrEngine).expect("edge code decodes"),
            vec!["edge case".to_owned()]
        );
    }

    /// THE order pin: with several codes in frame, payloads come
    /// back largest-area first (the framed code leads).
    #[test]
    fn multiple_codes_order_largest_first() {
        let big = code_image("https://flintshot.example/qr/multi", 6);
        let small = code_image("S", 2);
        // Canvas sized to hold both pastes whole (a clipped code is
        // not a fixture — detection can't see what isn't there).
        let (bw, bh) = big.dimensions();
        let (sw, sh) = small.dimensions();
        let mut canvas = canvas_with(&big, 10, 10, bw + sw + 80, bh + sh + 20);
        image::imageops::overlay(
            &mut canvas,
            &small,
            i64::from(bw + 60),
            i64::from(bh - sh) / 2,
        );
        assert_eq!(
            decode_region(&canvas, None, &RqrrEngine).expect("both codes decode"),
            vec![
                "https://flintshot.example/qr/multi".to_owned(),
                "S".to_owned()
            ],
            "the larger code leads"
        );
    }

    /// Nothing QR-shaped in the region is a friendly empty, not an
    /// error — the caller surfaces the re-capture copy.
    #[test]
    fn a_blank_region_decodes_nothing() {
        let canvas = RgbaImage::from_pixel(120, 120, image::Rgba([255, 255, 255, 255]));
        assert_eq!(
            decode_region(&canvas, None, &RqrrEngine).expect("blank is friendly"),
            Vec::<String>::new()
        );
    }

    /// The region-crop rule: only the framed code's pixels reach the
    /// engine, so only its payload comes back; `None` decodes both.
    #[test]
    #[allow(clippy::cast_precision_loss, reason = "fixture dims are exact in f32")]
    fn only_the_framed_code_decodes() {
        let left = code_image("left-code", 4);
        let right = code_image("right-code", 4);
        let lw = left.dimensions().0;
        let mut canvas = canvas_with(&left, 0, 0, lw + right.dimensions().0 + 60, 120);
        image::imageops::overlay(&mut canvas, &right, i64::from(lw + 60), 10);

        let left_region = Rect::from_xywh(0.0, 0.0, lw as f32, 120.0);
        assert_eq!(
            decode_region(&canvas, Some(left_region), &RqrrEngine).expect("crop decodes"),
            vec!["left-code".to_owned()],
            "only the rect is decoded"
        );
        assert_eq!(
            decode_region(&canvas, None, &RqrrEngine)
                .expect("whole image decodes")
                .len(),
            2,
            "no region decodes both"
        );
    }

    /// The ordering logic pinned without any engine: a fake reports
    /// codes out of order, `decode_region` returns payloads by area.
    #[test]
    fn fake_engine_payloads_come_back_largest_first() {
        struct FakeEngine(Vec<QrCode>);
        impl QrEngine for FakeEngine {
            fn decode(&self, _image: &RgbaImage) -> Result<Vec<QrCode>, QrError> {
                Ok(self.0.clone())
            }
        }
        let engine = FakeEngine(vec![
            QrCode::new("small", 100),
            QrCode::new("big", 900),
            QrCode::new("mid", 400),
        ]);
        let img = RgbaImage::new(10, 10);
        assert_eq!(
            decode_region(&img, None, &engine).expect("fake codes flow through"),
            vec!["big".to_owned(), "mid".to_owned(), "small".to_owned()]
        );
    }

    /// The engine's failure propagates — never swallowed into an
    /// empty success.
    #[test]
    fn fake_engine_unreadable_propagates() {
        struct FakeEngine;
        impl QrEngine for FakeEngine {
            fn decode(&self, _image: &RgbaImage) -> Result<Vec<QrCode>, QrError> {
                Err(QrError::Unreadable {
                    detail: "fixture".to_owned(),
                })
            }
        }
        let img = RgbaImage::new(10, 10);
        assert_eq!(
            decode_region(&img, None, &FakeEngine),
            Err(QrError::Unreadable {
                detail: "fixture".to_owned()
            })
        );
    }

    /// A detected-but-damaged code is an error, never a silent
    /// empty: the finder patterns survive (detection still groups a
    /// grid) while the blacked-out data region breaks ECC — the
    /// spec's "never swallow decode errors into a fake success".
    #[test]
    fn a_damaged_code_surfaces_unreadable_not_empty() {
        let code = code_image("https://flintshot.example/damaged", 4);
        let mut img = code;
        let (w, h) = img.dimensions();
        for y in h / 3..2 * h / 3 {
            for x in w / 3..2 * w / 3 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }
        let result = decode_region(&img, None, &RqrrEngine);
        let Err(QrError::Unreadable { detail }) = &result else {
            panic!("a damaged-but-detected code must be Err, got {result:?}");
        };
        assert!(
            !detail.is_empty(),
            "the detail carries the lower-level failure"
        );
    }

    /// The Open affordance's allowlist: absolute http(s) URLs only.
    /// Other schemes never earn an Open button (`xdg-open` dispatches
    /// by scheme — an unknown scheme would run a registered handler
    /// on untrusted screen content), and control bytes never pass.
    #[test]
    fn is_url_allows_only_absolute_http_urls() {
        for (payload, expected) in [
            ("https://example.com/x?y=1#z", true),
            ("http://a.b", true),
            ("HTTPS://EXAMPLE.COM/PATH", true),
            ("example.com", false),
            ("www.example.com/x", false),
            ("file:///etc/passwd", false),
            ("javascript:alert(1)", false),
            ("mailto:a@b.c", false),
            ("https://", false),
            ("", false),
            ("https://ex\nample.com", false),
        ] {
            assert_eq!(is_url(payload), expected, "payload {payload:?}");
        }
    }
}
