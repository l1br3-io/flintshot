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
    /// Upload defaults (Phase 4; see [`UploadConfig`]).
    #[serde(default)]
    pub upload: UploadConfig,
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
    /// How a fresh capture is presented for annotation: `"editor"`
    /// (separate editor window; default) or `"overlay"` (fullscreen
    /// frozen-capture overlay, Phase 5). Stored as a free-form string
    /// like `magnifier-modifier`: the consuming shell parses it and
    /// falls back to `"editor"` with a `tracing::warn!` on unknown
    /// values, so a typo never bricks config loading and the user's
    /// value survives re-saves.
    #[serde(default = "default_interaction_mode")]
    pub interaction_mode: String,
    /// When `true` (default), committing an annotation flips the
    /// active tool to Move so the next press repositions what was
    /// just drawn. `false` restores sticky tools for rapid
    /// multi-draw workflows. Read once per editor session (matching
    /// the magnifier precedent).
    #[serde(default = "default_flip_to_move")]
    pub flip_to_move: bool,
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
            interaction_mode: default_interaction_mode(),
            flip_to_move: default_flip_to_move(),
        }
    }
}

/// Capture behaviour: cursor inclusion, pre-grab delay, and the
/// capture-backend selection.
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
    /// Capture backend id: `"auto"` (default; session-kind detection
    /// with `xcap` as terminal fallback), a registered backend id
    /// (`"xcap"`, `"portal"` on Linux), or anything else — unknown ids
    /// load verbatim, warn at selection time, and resolve like `auto`
    /// (same string-tolerant policy as `interaction-mode`, so a typo
    /// never bricks config loading).
    #[serde(default = "default_capture_backend")]
    pub backend: String,
    /// Post-grab action on the **native** capture path:
    /// `"editor"` (default; the grab opens in the editor),
    /// `"clipboard"` (image copied, no windows), or `"upload"`
    /// (default upload backend; URL → clipboard). The fallback
    /// (`xcap`) path always routes into the editor flow regardless.
    /// Unknown values load verbatim, warn at dispatch time, and
    /// resolve `editor` — same string-tolerant policy as
    /// [`Self::backend`].
    #[serde(default = "default_after_grab")]
    pub after_grab: String,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            include_cursor: default_include_cursor(),
            default_delay_ms: default_delay_ms(),
            backend: default_capture_backend(),
            after_grab: default_after_grab(),
        }
    }
}

/// Global-hotkey chord, X11/macOS/Windows (`global-hotkey`) and the
/// Wayland portal bind suggestion alike. One binding fires the single
/// Capture action (native-capture-ux; the former `region` /
/// `fullscreen` pair was removed — legacy keys warn at load via
/// [`Config::load_from`]'s raw-table inspection and bind nothing).
/// Chord strings use `global-hotkey`'s accelerator format
/// (`"Ctrl+Shift+Print"`, case-insensitive modifier names,
/// `+`-joined).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct HotkeyConfig {
    /// Chord that fires the single Capture action, e.g.
    /// `"Ctrl+Shift+PrintScreen"` (the former `region` default).
    #[serde(default = "default_capture_hotkey")]
    pub capture: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            capture: default_capture_hotkey(),
        }
    }
}

fn default_capture_hotkey() -> String {
    // global-hotkey's parser names the key `PrintScreen` (not `Print`);
    // ARCHITECTURE.md's example used the shorter form.
    "Ctrl+Shift+PrintScreen".into()
}

/// Legacy `[hotkeys]` keys from the pre-native-capture-ux config
/// (removed with the `region`/`fullscreen` subcommands).
const LEGACY_HOTKEY_KEYS: [&str; 2] = ["region", "fullscreen"];

/// The legacy keys present in a raw `[hotkeys]` table (pure detection
/// seam for the load-time warn).
#[must_use]
fn legacy_hotkey_keys(table: Option<&toml_edit::Table>) -> Vec<String> {
    match table {
        Some(table) => LEGACY_HOTKEY_KEYS
            .iter()
            .filter(|key| table.contains_key(key))
            .map(std::string::ToString::to_string)
            .collect(),
        None => Vec::new(),
    }
}

/// Warn once per legacy key found in the raw `[hotkeys]` table.
///
/// GOTCHA: serde ignores unknown fields silently, so a config still
/// carrying `region`/`fullscreen` chords would load with no signal —
/// and bind nothing (the fields no longer exist). Inspecting the
/// `toml_edit` document *before* deserializing is what makes the
/// rename visible (the string-tolerance policy's inspection
/// precedent, without the tolerance: these keys are dead, not
/// pass-through).
fn warn_legacy_hotkeys(doc: &toml_edit::DocumentMut) {
    for key in legacy_hotkey_keys(doc.get("hotkeys").and_then(toml_edit::Item::as_table)) {
        tracing::warn!(
            key = %key,
            "[hotkeys] `{key}` was removed with the region/fullscreen split; \
             set `[hotkeys] capture` instead — nothing binds from `{key}`"
        );
    }
}

/// Upload defaults (Phase 4). Mirrors the `[upload]` config section.
/// `default_backend` is the backend id used when `--backend` is not
/// passed on the CLI (`"imgur"` for Phase 4; `#[non_exhaustive]`-shaped
/// for future backends).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct UploadConfig {
    /// Backend id to use when `--backend` is not given (`"imgur"`,
    /// `"0x0"`, etc.).
    #[serde(default = "default_upload_backend")]
    pub default_backend: String,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            default_backend: default_upload_backend(),
        }
    }
}

fn default_upload_backend() -> String {
    "imgur".into()
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

fn default_interaction_mode() -> String {
    "editor".into()
}

fn default_flip_to_move() -> bool {
    true
}

fn default_include_cursor() -> bool {
    true
}

fn default_delay_ms() -> u32 {
    0
}

fn default_capture_backend() -> String {
    "auto".into()
}

fn default_after_grab() -> String {
    "editor".into()
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
            Ok(contents) => {
                // Legacy-key inspection first: `toml_edit` parses any
                // syntactically valid TOML (more lenient than the
                // serde pass), and a parse failure there simply falls
                // through to the real error below.
                if let Ok(doc) = contents.parse::<toml_edit::DocumentMut>() {
                    warn_legacy_hotkeys(&doc);
                }
                match toml::from_str::<Self>(&contents) {
                    Ok(cfg) => Ok(cfg),
                    Err(e) => Err(FlintshotError::Config {
                        path: path.to_path_buf(),
                        detail: e.to_string(),
                    }),
                }
            }
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
        ensure_table(&mut doc, "upload");

        if let Some(ui) = doc.get_mut("ui").and_then(|i| i.as_table_mut()) {
            self.merge_ui(ui);
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
            set_value(
                cap,
                "backend",
                toml_edit::Value::from(self.capture.backend.as_str()),
            );
        }
        if let Some(hk) = doc.get_mut("hotkeys").and_then(|i| i.as_table_mut()) {
            set_value(
                hk,
                "capture",
                toml_edit::Value::from(self.hotkeys.capture.as_str()),
            );
        }
        if let Some(upload) = doc.get_mut("upload").and_then(|i| i.as_table_mut()) {
            set_value(
                upload,
                "default-backend",
                toml_edit::Value::from(self.upload.default_backend.as_str()),
            );
        }

        Ok(doc.to_string())
    }

    /// Write every `[ui]` field into the existing `ui` table.
    fn merge_ui(&self, ui: &mut toml_edit::Table) {
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
        set_value(
            ui,
            "interaction-mode",
            toml_edit::Value::from(self.ui.interaction_mode.as_str()),
        );
        set_value(
            ui,
            "flip-to-move",
            toml_edit::Value::from(self.ui.flip_to_move),
        );
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
        // Includes a stale `[pin]` section (removed with the
        // pin-to-desktop feature): loading must still succeed.
        fs::write(
            &path,
            "[pin]\ndefault-opacity = 0.8\n[unknown-section]\nfoo = \"bar\"\n[ui]\ndefault-color = \"#abcdef\"\n[experimental]\nflag = true\n",
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
        assert_eq!(cfg.hotkeys.capture, "Ctrl+Shift+PrintScreen");
        let d = Config::default();
        assert_eq!(d.hotkeys.capture, "Ctrl+Shift+PrintScreen");
    }

    #[test]
    fn custom_hotkeys_load_from_toml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[hotkeys]\ncapture = \"Super+F\"\n").unwrap();
        let cfg = Config::load_from(&path).unwrap();
        assert_eq!(cfg.hotkeys.capture, "Super+F");
    }

    /// Pinned (native-capture-ux Task 009): a config still carrying
    /// the legacy `[hotkeys]` region/fullscreen chords loads — the
    /// warn fires (detection pinned below) — but nothing binds from
    /// them: `capture` falls back to its default.
    #[test]
    fn legacy_hotkeys_load_with_default_capture() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "[hotkeys]\nregion = \"Ctrl+Shift+KeyA\"\nfullscreen = \"Super+F\"\n",
        )
        .unwrap();
        let cfg = Config::load_from(&path).unwrap();
        assert_eq!(
            cfg.hotkeys.capture,
            HotkeyConfig::default().capture,
            "legacy keys must not seed the capture binding"
        );
    }

    /// The detection seam behind the load-time warn: exactly the two
    /// legacy keys, only when present.
    #[test]
    fn legacy_hotkey_keys_are_detected_from_the_raw_table() {
        let doc: toml_edit::DocumentMut =
            "[hotkeys]\nregion = \"X\"\nfullscreen = \"Y\"\ncapture = \"Z\"\n"
                .parse()
                .unwrap();
        let table = doc.get("hotkeys").and_then(toml_edit::Item::as_table);
        assert_eq!(
            legacy_hotkey_keys(table),
            vec!["region".to_string(), "fullscreen".to_string()]
        );
        let doc: toml_edit::DocumentMut = "[hotkeys]\ncapture = \"Z\"\n".parse().unwrap();
        let table = doc.get("hotkeys").and_then(toml_edit::Item::as_table);
        assert!(
            legacy_hotkey_keys(table).is_empty(),
            "no legacy keys, no warn"
        );
        assert!(
            legacy_hotkey_keys(None).is_empty(),
            "no [hotkeys] table at all"
        );
    }

    /// A save→load round-trip carries the single capture chord.
    #[test]
    fn hotkeys_capture_roundtrip_via_save() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.hotkeys.capture = "Ctrl+Alt+F9".into();
        cfg.save_to(&path).unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("capture = \"Ctrl+Alt+F9\""),
            "capture chord not persisted:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert_eq!(back.hotkeys.capture, "Ctrl+Alt+F9");
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
    fn interaction_mode_defaults_to_editor_when_absent() {
        let cfg = Config::load_from(&{
            let dir = tempdir().unwrap();
            let path = dir.path().join("config.toml");
            fs::write(&path, "[ui]\ndefault-color = \"#000000\"\n").unwrap();
            path
        })
        .unwrap();
        assert_eq!(cfg.ui.interaction_mode, "editor");
        let d = UiConfig::default();
        assert_eq!(d.interaction_mode, "editor");
    }

    #[test]
    fn interaction_mode_roundtrips_overlay() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.ui.interaction_mode = "overlay".into();
        cfg.save_to(&path).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("interaction-mode = \"overlay\""),
            "interaction-mode not persisted:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert_eq!(back.ui.interaction_mode, "overlay");
    }

    #[test]
    fn interaction_mode_unknown_value_loads_verbatim() {
        // String storage (same policy as magnifier-modifier): an
        // unknown value must neither break loading nor be rewritten
        // by a save — the consuming shell warns + falls back instead.
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[ui]\ninteraction-mode = \"hologram\"\n").unwrap();

        let cfg = Config::load_from(&path).unwrap();
        assert_eq!(cfg.ui.interaction_mode, "hologram");

        cfg.save_to(&path).unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("interaction-mode = \"hologram\""),
            "save must preserve the user's (unknown) value verbatim:\n{written}"
        );
    }

    #[test]
    fn flip_to_move_defaults_true_when_absent() {
        let cfg = Config::load_from(&{
            let dir = tempdir().unwrap();
            let path = dir.path().join("config.toml");
            fs::write(&path, "[ui]\ndefault-color = \"#000000\"\n").unwrap();
            path
        })
        .unwrap();
        assert!(cfg.ui.flip_to_move, "absent field must default true");
        assert!(UiConfig::default().flip_to_move, "Default impl matches");
    }

    #[test]
    fn flip_to_move_false_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.ui.flip_to_move = false;
        cfg.save_to(&path).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("flip-to-move = false"),
            "flip-to-move not persisted:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert!(!back.ui.flip_to_move);
    }

    #[test]
    fn flip_to_move_preserves_existing_comment_on_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let existing = "\
[ui]
# keep tools sticky while batching rects
flip-to-move = false
";
        fs::write(&path, existing).unwrap();

        let mut cfg = Config::default();
        cfg.ui.flip_to_move = false;
        cfg.save_to(&path).unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("# keep tools sticky while batching rects"),
            "comment on flip-to-move lost:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert!(!back.ui.flip_to_move, "user value must survive re-save");
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
    fn missing_upload_section_uses_defaults() {
        let cfg = Config::load_from(&{
            let dir = tempdir().unwrap();
            let path = dir.path().join("config.toml");
            fs::write(&path, "[ui]\ndefault-color = \"#000000\"\n").unwrap();
            path
        })
        .unwrap();
        assert_eq!(cfg.upload.default_backend, "imgur");
        let d = UploadConfig::default();
        assert_eq!(d.default_backend, "imgur");
    }

    #[test]
    fn custom_upload_config_loads_from_toml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[upload]\ndefault-backend = \"0x0\"\n").unwrap();
        let cfg = Config::load_from(&path).unwrap();
        assert_eq!(cfg.upload.default_backend, "0x0");
    }

    #[test]
    fn upload_fields_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.upload.default_backend = "imgur".into();
        cfg.save_to(&path).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("[upload]"),
            "[upload] section missing:\n{written}"
        );
        assert!(
            written.contains("default-backend = \"imgur\""),
            "default-backend not persisted:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert_eq!(back.upload.default_backend, "imgur");
    }

    #[test]
    fn upload_section_preserves_existing_comments() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let existing = "\
# upload config
[upload]
# which backend to use
default-backend = \"imgur\"
";
        fs::write(&path, existing).unwrap();

        Config::default().save_to(&path).unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("# upload config"),
            "upload comment lost:\n{written}"
        );
        assert!(
            written.contains("# which backend to use"),
            "upload field comment lost:\n{written}"
        );
    }

    #[test]
    fn capture_backend_defaults_to_auto_when_absent() {
        let cfg = Config::load_from(&{
            let dir = tempdir().unwrap();
            let path = dir.path().join("config.toml");
            fs::write(&path, "[capture]\ninclude-cursor = false\n").unwrap();
            path
        })
        .unwrap();
        assert_eq!(cfg.capture.backend, "auto");
        assert_eq!(CaptureConfig::default().backend, "auto");
    }

    #[test]
    fn capture_backend_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.capture.backend = "portal".into();
        cfg.save_to(&path).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("backend = \"portal\""),
            "capture backend not persisted:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert_eq!(back.capture.backend, "portal");
    }

    #[test]
    fn capture_backend_unknown_value_loads_and_resaves_verbatim() {
        // String-tolerant policy (interaction-mode precedent): a value
        // this build knows nothing about — e.g. a backend id a future
        // release removed — must neither break loading nor be
        // rewritten by a save. Selection warns + resolves auto.
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[capture]\nbackend = \"gnome-shell-legacy\"\n").unwrap();

        let cfg = Config::load_from(&path).unwrap();
        assert_eq!(cfg.capture.backend, "gnome-shell-legacy");

        cfg.save_to(&path).unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("backend = \"gnome-shell-legacy\""),
            "save must preserve the user's (unknown) value verbatim:\n{written}"
        );
    }

    #[test]
    fn after_grab_defaults_to_editor_when_absent() {
        let cfg = Config::load_from(&{
            let dir = tempdir().unwrap();
            let path = dir.path().join("config.toml");
            fs::write(&path, "[capture]\nbackend = \"auto\"\n").unwrap();
            path
        })
        .unwrap();
        assert_eq!(cfg.capture.after_grab, "editor");
        assert_eq!(CaptureConfig::default().after_grab, "editor");
    }

    #[test]
    fn after_grab_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.capture.after_grab = "clipboard".into();
        cfg.save_to(&path).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("after-grab = \"clipboard\""),
            "after-grab not persisted:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert_eq!(back.capture.after_grab, "clipboard");
    }

    #[test]
    fn after_grab_unknown_value_loads_and_resaves_verbatim() {
        // Same string-tolerant policy as `backend`: an unknown action
        // (future release, typo) loads verbatim, never bricks config,
        // and a save must not rewrite it. Dispatch warns + resolves
        // `editor`.
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[capture]\nafter-grab = \"print-it\"\n").unwrap();

        let cfg = Config::load_from(&path).unwrap();
        assert_eq!(cfg.capture.after_grab, "print-it");

        cfg.save_to(&path).unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("after-grab = \"print-it\""),
            "save must preserve the user's (unknown) value verbatim:\n{written}"
        );
    }

    #[test]
    fn capture_backend_preserves_existing_comment_on_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let existing = "\
[capture]
# portal until the cursor gap is fixed
backend = \"portal\"
";
        fs::write(&path, existing).unwrap();

        let mut cfg = Config::default();
        cfg.capture.backend = "portal".into();
        cfg.save_to(&path).unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("# portal until the cursor gap is fixed"),
            "comment on backend lost:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert_eq!(
            back.capture.backend, "portal",
            "user value must survive re-save"
        );
    }
}
