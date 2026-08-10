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
    /// Global hotkey chords (read by the daemon; see [`HotkeyConfig`]).
    #[serde(default)]
    pub hotkeys: HotkeyConfig,
    /// Pin-to-desktop window defaults (Phase 3; see [`PinConfig`]).
    #[serde(default)]
    pub pin: PinConfig,
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
    /// Default text-tool color (decoupled from `default_color` so a
    /// user can keep e.g. red arrows and white labels simultaneously).
    #[serde(default = "default_text_color")]
    pub default_text_color: Color,
    /// Default stroke width for outlined shapes, in physical pixels.
    #[serde(default = "default_stroke_width")]
    pub default_stroke_width: f32,
    /// Default font size for the text tool, in physical pixels.
    #[serde(default = "default_font_size")]
    pub default_font_size: f32,
    /// Modifier (as an egui key name) that, while held, reveals the
    /// pixel-zoom magnifier inset in the editor. Default `"Space"`.
    /// The egui shell parses this to an `egui::Key`; unknown values
    /// fall back to `Space` with a `tracing::warn!`.
    #[serde(default = "default_magnifier_modifier")]
    pub magnifier_modifier: String,
    /// When `true`, the magnifier inset is always shown (no modifier
    /// hold required). Useful for pixel-precise drawing workflows.
    #[serde(default)]
    pub magnifier_always_on: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            default_color: default_color(),
            default_text_color: default_text_color(),
            default_stroke_width: default_stroke_width(),
            default_font_size: default_font_size(),
            magnifier_modifier: default_magnifier_modifier(),
            magnifier_always_on: false,
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

/// Global-hotkey chords (X11/macOS/Windows). Wayland's portal-based
/// global hotkeys are Phase 6 — on Wayland the daemon logs the
/// compositor-keybind workaround and the `[hotkeys]` section is simply
/// unused. Chord strings use `global-hotkey`'s accelerator format
/// (`"Ctrl+Shift+Print"`, case-insensitive modifier names, `+`-joined).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct HotkeyConfig {
    /// Chord that captures a region (interactive selection), e.g.
    /// `"Ctrl+Shift+Print"`.
    #[serde(default = "default_region_hotkey")]
    pub region: String,
    /// Chord that captures the primary monitor fullscreen, e.g.
    /// `"Ctrl+Alt+Print"`.
    #[serde(default = "default_fullscreen_hotkey")]
    pub fullscreen: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            region: default_region_hotkey(),
            fullscreen: default_fullscreen_hotkey(),
        }
    }
}

fn default_region_hotkey() -> String {
    // global-hotkey's parser names the key `PrintScreen` (not `Print`);
    // ARCHITECTURE.md's example used the shorter form.
    "Ctrl+Shift+PrintScreen".into()
}

fn default_fullscreen_hotkey() -> String {
    "Ctrl+Alt+PrintScreen".into()
}

/// Pin-to-desktop window defaults (Phase 3). Mirrors the `[pin]` config
/// section. `default_position` is a free-form string: the special value
/// `"center"` centres the pin on the active monitor; any other value is
/// parsed as an `"X,Y"` coordinate pair by the CLI at resolve time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct PinConfig {
    /// Resting opacity of the pin window (the window snaps to full
    /// opacity while hovered). Range `0.0..=1.0`.
    #[serde(default = "default_pin_opacity")]
    pub default_opacity: f32,
    /// When `true`, the pin's image is copied to the clipboard when the
    /// window closes.
    #[serde(default = "default_pin_copy_on_close")]
    pub copy_on_close: bool,
    /// Default placement for a new pin. `"center"` → active-monitor
    /// centre; an `"X,Y"` pair → that point. CLI `--at` overrides this.
    #[serde(default = "default_pin_position")]
    pub default_position: String,
}

impl Default for PinConfig {
    fn default() -> Self {
        Self {
            default_opacity: default_pin_opacity(),
            copy_on_close: default_pin_copy_on_close(),
            default_position: default_pin_position(),
        }
    }
}

fn default_pin_opacity() -> f32 {
    0.5
}

fn default_pin_copy_on_close() -> bool {
    false
}

fn default_pin_position() -> String {
    "center".into()
}

fn default_color() -> Color {
    Color::RED
}

fn default_text_color() -> Color {
    Color::RED
}

fn default_stroke_width() -> f32 {
    3.0
}

fn default_font_size() -> f32 {
    16.0
}

fn default_magnifier_modifier() -> String {
    "Space".into()
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
        ensure_table(&mut doc, "hotkeys");
        ensure_table(&mut doc, "pin");

        if let Some(ui) = doc.get_mut("ui").and_then(|i| i.as_table_mut()) {
            set_value(
                ui,
                "default-color",
                toml_edit::Value::from(format_color(&self.ui.default_color)),
            );
            set_value(
                ui,
                "default-text-color",
                toml_edit::Value::from(format_color(&self.ui.default_text_color)),
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
            set_value(
                ui,
                "magnifier-modifier",
                toml_edit::Value::from(self.ui.magnifier_modifier.as_str()),
            );
            set_value(
                ui,
                "magnifier-always-on",
                toml_edit::Value::from(self.ui.magnifier_always_on),
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
        if let Some(hk) = doc.get_mut("hotkeys").and_then(|i| i.as_table_mut()) {
            set_value(
                hk,
                "region",
                toml_edit::Value::from(self.hotkeys.region.as_str()),
            );
            set_value(
                hk,
                "fullscreen",
                toml_edit::Value::from(self.hotkeys.fullscreen.as_str()),
            );
        }
        if let Some(pin) = doc.get_mut("pin").and_then(|i| i.as_table_mut()) {
            set_value(
                pin,
                "default-opacity",
                toml_edit::Value::from(f64::from(self.pin.default_opacity)),
            );
            set_value(
                pin,
                "copy-on-close",
                toml_edit::Value::from(self.pin.copy_on_close),
            );
            set_value(
                pin,
                "default-position",
                toml_edit::Value::from(self.pin.default_position.as_str()),
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

/// Resolve the pin-cache directory: `<cache_dir>/flintshot/pins/`.
///
/// The editor writes composed pin images here before spawning
/// `flintshot pin <path>`; the daemon may reap stale entries on start.
/// Returns `None` when `dirs::cache_dir()` returns `None` (platforms
/// without a cache convention) — callers fall back to
/// `std::env::temp_dir().join("flintshot-pins")`.
///
/// Opportunistic-cleanup contract: files here are safe to delete at any
/// time (a pin keeps no handle open after `image::open` reads it into
/// memory). The OS may reap the cache dir independently; the daemon's
/// own cleanup is best-effort.
#[must_use]
pub fn pin_cache_dir() -> Option<PathBuf> {
    pin_cache_dir_in(dirs::cache_dir().as_deref())
}

/// Pure, testable seam for [`pin_cache_dir`]: given a base cache dir (or
/// `None`), return `<base>/flintshot/pins`. Mirrors
/// `ipc::default_unix_socket_in`'s shape — keeps the platform `dirs`
/// lookup out of the composition logic so the path shape is unit-tested
/// without setting env vars.
///
/// Returns `None` when `base` is `None` (matching [`pin_cache_dir`]'s
/// contract for platforms without a cache convention).
#[must_use]
pub fn pin_cache_dir_in(base: Option<&Path>) -> Option<PathBuf> {
    Some(base?.join("flintshot").join("pins"))
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
    fn missing_hotkeys_section_uses_defaults() {
        // No [hotkeys] in the file → the default chords.
        let cfg = Config::load_from(&{
            let dir = tempdir().unwrap();
            let path = dir.path().join("config.toml");
            fs::write(&path, "[ui]\ndefault-color = \"#000000\"\n").unwrap();
            path
        })
        .unwrap();
        assert_eq!(cfg.hotkeys.region, "Ctrl+Shift+PrintScreen");
        assert_eq!(cfg.hotkeys.fullscreen, "Ctrl+Alt+PrintScreen");
        let d = Config::default();
        assert_eq!(d.hotkeys.region, "Ctrl+Shift+PrintScreen");
    }

    #[test]
    fn missing_pin_section_uses_defaults() {
        // No [pin] in the file → the built-in defaults.
        let cfg = Config::load_from(&{
            let dir = tempdir().unwrap();
            let path = dir.path().join("config.toml");
            fs::write(&path, "[ui]\ndefault-color = \"#000000\"\n").unwrap();
            path
        })
        .unwrap();
        assert_eq!(cfg.pin.default_opacity, 0.5);
        assert!(!cfg.pin.copy_on_close);
        assert_eq!(cfg.pin.default_position, "center");
        // Default impl matches.
        let d = PinConfig::default();
        assert_eq!(d.default_opacity, 0.5);
        assert!(!d.copy_on_close);
        assert_eq!(d.default_position, "center");
    }

    #[test]
    fn custom_pin_config_loads_from_toml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "[pin]\ndefault-opacity = 0.8\ncopy-on-close = true\ndefault-position = \"top-left\"\n",
        )
        .unwrap();
        let cfg = Config::load_from(&path).unwrap();
        assert_eq!(cfg.pin.default_opacity, 0.8);
        assert!(cfg.pin.copy_on_close);
        assert_eq!(cfg.pin.default_position, "top-left");
    }

    #[test]
    fn pin_fields_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.pin.default_opacity = 0.25;
        cfg.pin.copy_on_close = true;
        cfg.pin.default_position = "100,200".into();
        cfg.save_to(&path).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("[pin]"),
            "[pin] section missing:\n{written}"
        );
        assert!(
            written.contains("default-opacity = 0.25"),
            "default-opacity not persisted:\n{written}"
        );
        assert!(
            written.contains("copy-on-close = true"),
            "copy-on-close not persisted:\n{written}"
        );
        assert!(
            written.contains("default-position = \"100,200\""),
            "default-position not persisted:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert_eq!(back.pin.default_opacity, 0.25);
        assert!(back.pin.copy_on_close);
        assert_eq!(back.pin.default_position, "100,200");
    }

    #[test]
    fn custom_hotkeys_load_from_toml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "[hotkeys]\nregion = \"Ctrl+Shift+KeyA\"\nfullscreen = \"Super+F\"\n",
        )
        .unwrap();
        let cfg = Config::load_from(&path).unwrap();
        assert_eq!(cfg.hotkeys.region, "Ctrl+Shift+KeyA");
        assert_eq!(cfg.hotkeys.fullscreen, "Super+F");
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
    fn magnifier_defaults_when_absent() {
        let cfg = Config::load_from(&{
            let dir = tempdir().unwrap();
            let path = dir.path().join("config.toml");
            fs::write(&path, "[ui]\ndefault-color = \"#000000\"\n").unwrap();
            path
        })
        .unwrap();
        assert_eq!(cfg.ui.magnifier_modifier, "Space");
        assert!(!cfg.ui.magnifier_always_on);
        // Default impl matches.
        let d = UiConfig::default();
        assert_eq!(d.magnifier_modifier, "Space");
        assert!(!d.magnifier_always_on);
    }

    #[test]
    fn magnifier_fields_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.ui.magnifier_modifier = "Alt".into();
        cfg.ui.magnifier_always_on = true;
        cfg.save_to(&path).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("magnifier-modifier = \"Alt\""),
            "magnifier-modifier not persisted:\n{written}"
        );
        assert!(
            written.contains("magnifier-always-on = true"),
            "magnifier-always-on not persisted:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert_eq!(back.ui.magnifier_modifier, "Alt");
        assert!(back.ui.magnifier_always_on);
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

    #[test]
    fn pin_cache_dir_in_joins_flintshot_pins_under_base() {
        let path =
            pin_cache_dir_in(Some(Path::new("/home/user/.cache"))).expect("Some base → Some path");
        assert_eq!(
            path,
            PathBuf::from("/home/user/.cache/flintshot/pins"),
            "pin cache dir must be <base>/flintshot/pins"
        );
    }

    #[test]
    fn pin_cache_dir_in_none_returns_none() {
        assert!(
            pin_cache_dir_in(None).is_none(),
            "no base cache dir → no pin cache dir (caller falls back to temp)"
        );
    }

    #[test]
    fn pin_cache_dir_in_does_not_require_base_to_exist() {
        // A nonexistent base is still joined structurally — existence is
        // the caller's concern (the editor creates the dir on write).
        let path = pin_cache_dir_in(Some(Path::new("/nonexistent/cache"))).unwrap();
        assert_eq!(path, PathBuf::from("/nonexistent/cache/flintshot/pins"));
    }
}
