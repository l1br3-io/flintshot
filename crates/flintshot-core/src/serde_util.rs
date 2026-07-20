//! Serde helper modules for non-obvious (de)serialization conventions
//! used across `flintshot-core`.
//!
//! Currently houses [`color_hex`], which serializes [`crate::Color`] as
//! a CSS-style hex string (`"#rrggbb"` when opaque, `"#rrggbbaa"` when
//! the alpha channel is not 255). Reading is permissive: both lengths
//! are accepted, with or without a leading `#`, and uppercase /
//! lowercase hex digits both parse.

use serde::{Deserializer, Serializer};

use crate::geom::Color;

/// Hex-string (de)serialization for [`Color`].
///
/// On serialize:
/// - Alpha of `255` → `"#rrggbb"` (6 digits).
/// - Any other alpha → `"#rrggbbaa"` (8 digits).
///
/// On deserialize:
/// - Accepts `"#rrggbb"`, `"#rrggbbaa"`, `"rrggbb"`, `"rrggbbaa"`.
/// - Accepts mixed-case hex digits.
pub mod color_hex {
    use serde::Deserialize;

    use super::{format_color, parse_color, Color, Deserializer, Serializer};

    /// Serialize a [`Color`] as a hex string.
    ///
    /// # Errors
    /// Propagates the serializer's error if writing the string fails.
    pub fn serialize<S: Serializer>(color: &Color, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&format_color(color))
    }

    /// Deserialize a [`Color`] from a hex string.
    ///
    /// # Errors
    /// Returns the deserializer's error type if the input is not a
    /// 6- or 8-digit hex string (with optional `#` prefix).
    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Color, D::Error> {
        let s = String::deserialize(de)?;
        parse_color(&s).map_err(serde::de::Error::custom)
    }
}

/// Format a [`Color`] as a hex string, picking the short form when
/// the color is fully opaque.
#[must_use]
pub fn format_color(color: &Color) -> String {
    if color.a == 255 {
        format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            color.r, color.g, color.b, color.a
        )
    }
}

/// Parse a hex color string. Accepts `#rrggbb`, `#rrggbbaa`, and the
/// same forms without the leading `#`. Uppercase and lowercase digits
/// are both accepted. Alpha defaults to 255 when not specified.
///
/// # Errors
/// Returns a string describing the parse failure for any other shape.
pub fn parse_color(s: &str) -> Result<Color, String> {
    let trimmed = s.trim().trim_start_matches('#');
    let bytes = trimmed.as_bytes();

    let (red, green, blue, alpha) = match bytes.len() {
        6 => (
            hex2(&bytes[0..2])?,
            hex2(&bytes[2..4])?,
            hex2(&bytes[4..6])?,
            255,
        ),
        8 => (
            hex2(&bytes[0..2])?,
            hex2(&bytes[2..4])?,
            hex2(&bytes[4..6])?,
            hex2(&bytes[6..8])?,
        ),
        _ => return Err(format!("expected 6 or 8 hex digits, got {}", bytes.len())),
    };
    Ok(Color::rgba(red, green, blue, alpha))
}

/// Decode two hex characters into a `u8`.
fn hex2(pair: &[u8]) -> Result<u8, String> {
    if pair.len() != 2 {
        return Err(format!("expected 2 hex chars, got {}", pair.len()));
    }
    Ok((hex1(pair[0])? << 4) | hex1(pair[1])?)
}

/// Decode one hex character into a `u8` nibble.
fn hex1(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("invalid hex digit: {byte:#x}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_opaque_is_short_form() {
        assert_eq!(format_color(&Color::rgb(255, 0, 0)), "#ff0000");
        assert_eq!(format_color(&Color::rgb(0, 0, 0)), "#000000");
        assert_eq!(format_color(&Color::rgb(255, 255, 255)), "#ffffff");
    }

    #[test]
    fn format_with_alpha_is_long_form() {
        assert_eq!(format_color(&Color::rgba(255, 0, 0, 128)), "#ff000080");
        assert_eq!(format_color(&Color::rgba(0, 0, 0, 0)), "#00000000");
    }

    #[test]
    fn parse_short_with_hash() {
        assert_eq!(parse_color("#ff0000").unwrap(), Color::rgb(255, 0, 0));
    }

    #[test]
    fn parse_long_with_hash() {
        assert_eq!(
            parse_color("#ff000080").unwrap(),
            Color::rgba(255, 0, 0, 128)
        );
    }

    #[test]
    fn parse_without_hash() {
        assert_eq!(parse_color("00ff00").unwrap(), Color::rgb(0, 255, 0));
        assert_eq!(
            parse_color("00ff00ff").unwrap(),
            Color::rgba(0, 255, 0, 255)
        );
    }

    #[test]
    fn parse_uppercase() {
        assert_eq!(parse_color("#FFAABB").unwrap(), Color::rgb(255, 170, 187));
    }

    #[test]
    fn parse_rejects_wrong_length() {
        assert!(parse_color("#ff").is_err());
        assert!(parse_color("#ffffff0").is_err());
        assert!(parse_color("").is_err());
    }

    #[test]
    fn parse_rejects_non_hex() {
        assert!(parse_color("#gg0000").is_err());
        assert!(parse_color("hello!").is_err());
    }

    #[test]
    fn parse_trims_whitespace() {
        assert_eq!(parse_color("  #ff0000  ").unwrap(), Color::rgb(255, 0, 0));
    }

    #[test]
    fn roundtrip_opaque() {
        let c = Color::rgb(123, 200, 17);
        assert_eq!(parse_color(&format_color(&c)).unwrap(), c);
    }

    #[test]
    fn roundtrip_with_alpha() {
        let c = Color::rgba(10, 20, 30, 200);
        assert_eq!(parse_color(&format_color(&c)).unwrap(), c);
    }
}
