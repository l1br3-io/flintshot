//! User-facing configuration for `flintshot`.
//!
//! Loaded from `<config_dir>/flintshot/config.toml` (where `<config_dir>`
//! is platform-appropriate via [`dirs::config_dir`]) and written back
//! with [`toml_edit`] so user comments and formatting are preserved
//! across edits.
//!
//! Phase 1 owns only the `[ui]` and `[capture]` tables. The full schema
//! lives in `docs/ARCHITECTURE.md` §Config; remaining sections
//! (`[hotkeys]`, `[upload]`, `[ipc]`) land alongside their phases.
//!
//! All public types are `#[non_exhaustive]` and every field carries
//! `#[serde(default)]`, so missing or partially-populated files load
//! cleanly and unknown fields are silently ignored (forward-compat with
//! newer config versions).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{FlintshotError, Result};
use crate::geom::Color;
use crate::serde_util::format_color;

/// Top-level config root. Maps to the entire `config.toml` file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct Config {
    /// User-interface defaults applied to the editor on open.
    #[serde(default)]
    pub ui: UiConfig,
    /// Capture behaviour defaults.
    #[serde(default)]
    pub capture: CaptureConfig,
}

/// Editor defaults: persist the user's last-used color, stroke width,
/// and font size so the editor opens with sane values next session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct UiConfig {
    /// Default annotation color.
    #[serde(default = "default_color")]
    pub default_color: Color,
    /// Default stroke width for outlined shapes, in physical pixels.
    #[serde(default = "default_stroke_width")]
    pub default_stroke_width: f32,
    /// Default font size for the text tool, in physical pixels.
    #[serde(default = "default_font_size")]
    pub default_font_size: f32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            default_color: default_color(),
            default_stroke_width: default_stroke_width(),
            default_font_size: default_font_size(),
        }
    }
}

/// Capture behaviour: whether the cursor is included and any capture
/// delay applied before grabbing the framebuffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct CaptureConfig {
    /// Include the mouse cursor in captured images.
    #[serde(default = "default_include_cursor")]
    pub include_cursor: bool,
    /// Delay before capturing, in milliseconds.
    #[serde(default = "default_delay_ms")]
    pub default_delay_ms: u32,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            include_cursor: default_include_cursor(),
            default_delay_ms: default_delay_ms(),
        }
    }
}

fn default_color() -> Color {
    Color::RED
}

fn default_stroke_width() -> f32 {
    3.0
}

fn default_font_size() -> f32 {
    16.0
}

fn default_include_cursor() -> bool {
    true
}

fn default_delay_ms() -> u32 {
    0
}

impl Config {
    /// Load config from the platform-appropriate location
    /// (`dirs::config_dir() / "flintshot" / "config.toml"`).
    ///
    /// Returns `Ok(Config::default())` when the file is missing — first
    /// run, or user hasn't customized anything yet.
    ///
    /// # Errors
    /// See [`Config::load_from`].
    pub fn load() -> Result<Self> {
        let Some(path) = config_path() else {
            tracing::warn!("no platform config_dir available; falling back to default config");
            return Ok(Self::default());
        };
        Self::load_from(&path)
    }

    /// Testable seam: load config from an explicit path.
    ///
    /// Missing file → `Ok(Config::default())`. Present-but-unparseable
    /// file → `Err`. This matches "first run" vs "user has a broken
    /// config" semantics.
    ///
    /// # Errors
    /// - [`FlintshotError::Io`] if reading the file fails for a reason
    ///   other than the file not existing.
    /// - [`FlintshotError::Config`] if the file exists but fails to
    ///   parse as valid TOML or fails to deserialize.
    pub fn load_from(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(contents) => match toml::from_str::<Self>(&contents) {
                Ok(cfg) => Ok(cfg),
                Err(e) => Err(FlintshotError::Config {
                    path: path.to_path_buf(),
                    detail: e.to_string(),
                }),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(FlintshotError::Io(e)),
        }
    }

    /// Save config to the platform-appropriate location, creating the
    /// parent directory if needed.
    ///
    /// # Errors
    /// See [`Config::save_to`].
    pub fn save(&self) -> Result<()> {
        match config_path() {
            Some(p) => self.save_to(&p),
            None => Err(FlintshotError::Config {
                path: PathBuf::new(),
                detail: "no platform config_dir available".into(),
            }),
        }
    }

    /// Testable seam: save config to an explicit path, preserving any
    /// comments in the existing file via [`toml_edit`]. Creates the
    /// parent directory if it doesn't exist.
    ///
    /// # Errors
    /// - [`FlintshotError::Io`] on filesystem write failure.
    /// - [`FlintshotError::Config`] if the existing file can't be parsed
    ///   by `toml_edit` (which is more lenient than `toml::from_str` —
    ///   it tolerates any syntactically valid TOML, including duplicate
    ///   keys with explicit array-of-tables semantics).
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(FlintshotError::Io)?;
        }

        let body = match std::fs::read_to_string(path) {
            Ok(existing) => self.merge_into_existing(&existing)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => self.fresh_toml()?,
            Err(e) => return Err(FlintshotError::Io(e)),
        };

        std::fs::write(path, body).map_err(FlintshotError::Io)
    }

    /// Render this config as a fresh TOML string (no existing-file
    /// context). Used when no file exists yet.
    fn fresh_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| FlintshotError::Config {
            path: PathBuf::new(),
            detail: format!("serialization failure: {e}"),
        })
    }

    /// Merge this config's known fields into an existing TOML document,
    /// preserving comments and key ordering. Unknown fields in the
    /// existing document are left untouched (forward-compat).
    fn merge_into_existing(&self, existing: &str) -> Result<String> {
        let mut doc: toml_edit::DocumentMut =
            existing
                .parse()
                .map_err(|e: toml_edit::TomlError| FlintshotError::Config {
                    path: PathBuf::new(),
                    detail: e.to_string(),
                })?;

        ensure_table(&mut doc, "ui");
        ensure_table(&mut doc, "capture");

        if let Some(ui) = doc.get_mut("ui").and_then(|i| i.as_table_mut()) {
            set_value(
                ui,
                "default-color",
                toml_edit::Value::from(format_color(&self.ui.default_color)),
            );
            set_value(
                ui,
                "default-stroke-width",
                toml_edit::Value::from(f64::from(self.ui.default_stroke_width)),
            );
            set_value(
                ui,
                "default-font-size",
                toml_edit::Value::from(f64::from(self.ui.default_font_size)),
            );
        }
        if let Some(cap) = doc.get_mut("capture").and_then(|i| i.as_table_mut()) {
            set_value(
                cap,
                "include-cursor",
                toml_edit::Value::from(self.capture.include_cursor),
            );
            set_value(
                cap,
                "default-delay-ms",
                toml_edit::Value::from(i64::from(self.capture.default_delay_ms)),
            );
        }

        Ok(doc.to_string())
    }
}

/// Ensure `doc[table_name]` exists as a table. Creates it if absent;
/// leaves existing tables (and their decor) untouched.
fn ensure_table(doc: &mut toml_edit::DocumentMut, table_name: &str) {
    if doc.get(table_name).is_some_and(toml_edit::Item::is_table) {
        return;
    }
    doc.insert(table_name, toml_edit::Item::Table(toml_edit::Table::new()));
}

/// Insert or update a table entry's value, preserving the existing
/// value's leading/trailing decor (so user comments survive). When the
/// key doesn't yet exist, inserts a fresh value with default decor.
fn set_value(table: &mut toml_edit::Table, key: &str, new_value: toml_edit::Value) {
    use toml_edit::{Entry, Item};
    match table.entry(key) {
        Entry::Occupied(mut occ) => {
            let existing = occ.get_mut();
            if let Item::Value(existing_value) = existing {
                let decor = existing_value.decor().clone();
                let mut replacement = new_value;
                *replacement.decor_mut() = decor;
                *existing_value = replacement;
            } else {
                *existing = Item::Value(new_value);
            }
        }
        Entry::Vacant(vac) => {
            vac.insert(Item::Value(new_value));
        }
    }
}

/// Resolve `<config_dir>/flintshot/config.toml`, or `None` if
/// `dirs::config_dir()` returns `None` (happens on platforms without
/// a clear convention).
#[must_use]
pub fn config_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("flintshot").join("config.toml"))
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn load_from_missing_returns_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing.toml");
        let cfg = Config::load_from(&path).expect("missing file should yield defaults");
        assert_eq!(cfg.ui.default_color, Color::RED);
        assert_eq!(cfg.ui.default_stroke_width, 3.0);
        assert_eq!(cfg.ui.default_font_size, 16.0);
        assert!(cfg.capture.include_cursor);
        assert_eq!(cfg.capture.default_delay_ms, 0);
    }

    #[test]
    fn save_to_then_load_from_roundtrips_values() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.ui.default_color = Color::rgb(10, 200, 90);
        cfg.ui.default_stroke_width = 7.5;
        cfg.ui.default_font_size = 22.0;
        cfg.capture.include_cursor = false;
        cfg.capture.default_delay_ms = 250;

        cfg.save_to(&path).unwrap();
        let back = Config::load_from(&path).unwrap();
        assert_eq!(back.ui.default_color, Color::rgb(10, 200, 90));
        assert_eq!(back.ui.default_stroke_width, 7.5);
        assert_eq!(back.ui.default_font_size, 22.0);
        assert!(!back.capture.include_cursor);
        assert_eq!(back.capture.default_delay_ms, 250);
    }

    #[test]
    fn save_to_preserves_existing_comments() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        // Existing file has user comments at the top, before a table,
        // and trailing after a value. toml_edit should keep all of them.
        let existing = "\
# my flintshot config
# configured 2025-01-01

[ui]
# this color is best
default-color = \"#00ff00\"

[capture]
include-cursor = true  # hide cursor when capturing presentations
";
        fs::write(&path, existing).unwrap();

        let cfg = Config::default();
        cfg.save_to(&path).unwrap();
        let written = fs::read_to_string(&path).unwrap();

        assert!(
            written.contains("# my flintshot config"),
            "top-level comment lost:\n{written}"
        );
        assert!(
            written.contains("# configured 2025-01-01"),
            "top-level comment lost:\n{written}"
        );
        assert!(
            written.contains("# this color is best"),
            "table header comment lost:\n{written}"
        );
        assert!(
            written.contains("# hide cursor when capturing presentations"),
            "trailing comment lost:\n{written}"
        );
    }

    #[test]
    fn save_to_creates_parent_directory() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("nested").join("deeper").join("config.toml");
        Config::default().save_to(&nested).unwrap();
        assert!(nested.exists());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "[unknown-section]\nfoo = \"bar\"\n[ui]\ndefault-color = \"#abcdef\"\n[experimental]\nflag = true\n",
        )
        .unwrap();

        let cfg = Config::load_from(&path).unwrap();
        assert_eq!(cfg.ui.default_color, Color::rgb(0xab, 0xcd, 0xef));
        // Unknown sections are simply absent from the typed view.
    }

    #[test]
    fn partial_toml_uses_defaults_for_missing_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        // Only one field set; the rest must come from defaults.
        fs::write(&path, "[ui]\ndefault-color = \"#112233\"\n").unwrap();

        let cfg = Config::load_from(&path).unwrap();
        assert_eq!(cfg.ui.default_color, Color::rgb(0x11, 0x22, 0x33));
        assert_eq!(
            cfg.ui.default_stroke_width, 3.0,
            "missing field must default"
        );
        assert_eq!(cfg.ui.default_font_size, 16.0);
        assert!(cfg.capture.include_cursor);
        assert_eq!(cfg.capture.default_delay_ms, 0);
    }

    #[test]
    fn hex_color_roundtrip_via_toml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.ui.default_color = Color::rgba(0xff, 0x00, 0x00, 0x80);
        cfg.save_to(&path).unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("default-color = \"#ff000080\""),
            "expected long-form hex with alpha, got:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert_eq!(back.ui.default_color, Color::rgba(0xff, 0x00, 0x00, 0x80));
    }

    #[test]
    fn broken_toml_surfaces_config_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "this is not = valid = toml =\n").unwrap();

        let err = Config::load_from(&path).unwrap_err();
        assert!(
            matches!(err, FlintshotError::Config { .. }),
            "expected Config error, got {err:?}"
        );
    }
}
