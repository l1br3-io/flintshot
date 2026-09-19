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
    /// Output defaults (settings route; see [`OutputConfig`]).
    #[serde(default)]
    pub output: OutputConfig,
}

/// Editor defaults: persist the user's last-used color, stroke width,
/// and font size so the editor opens with sane values next session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a settings table is naturally a flat row of on/off prefs"
)]
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
    /// When `false` (default), tools are sticky — committing an
    /// annotation keeps the active tool so rapid multi-draw workflows
    /// need no re-press. `true` flips to Move after every eligible
    /// commit so the next press repositions what was just drawn
    /// (ADR-0040, superseded as the default by ADR-0058). Read once
    /// per editor session (matching the magnifier precedent).
    #[serde(default = "default_flip_to_move")]
    pub flip_to_move: bool,
    /// When `true` (default), the editor shows the 30 px statusbar.
    /// The settings route's `ui.show-statusbar` toggle writes here.
    #[serde(default = "default_show_statusbar")]
    pub show_statusbar: bool,
    /// How the statusbar estimates the image's size:
    /// `"heuristic"` (default; `w × h × 4`) or `"exact-png"` (a
    /// PNG encode of the composed image). Unknown values resolve
    /// `heuristic` with a warning — same string-tolerant policy as
    /// `interaction-mode`.
    #[serde(default = "default_statusbar_size_mode")]
    pub statusbar_size_mode: String,
    /// When `true` (default), UI microinteractions (spring pop,
    /// section unfold, switch knob) run; `false` jumps every
    /// transition to its end state.
    #[serde(default = "default_animations")]
    pub animations: bool,
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
            show_statusbar: default_show_statusbar(),
            statusbar_size_mode: default_statusbar_size_mode(),
            animations: default_animations(),
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
    /// Which region the single Capture action grabs by default:
    /// `"region"` (default; interactive selection), `"fullscreen"`,
    /// or `"window"`. Unknown values resolve `region` with a
    /// warning — same string-tolerant policy as [`Self::backend`].
    #[serde(default = "default_default_mode")]
    pub default_mode: String,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            include_cursor: default_include_cursor(),
            default_delay_ms: default_delay_ms(),
            backend: default_capture_backend(),
            after_grab: default_after_grab(),
            default_mode: default_default_mode(),
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
    false
}

fn default_show_statusbar() -> bool {
    true
}

fn default_statusbar_size_mode() -> String {
    "heuristic".to_owned()
}

fn default_animations() -> bool {
    true
}

fn default_default_mode() -> String {
    "region".to_owned()
}

/// Output defaults for the settings route's `output.*` rows: the
/// save format, JPEG quality, and the direct-save filename
/// template / directory (used when a template is configured; the
/// plain Save action keeps its file dialog otherwise).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct OutputConfig {
    /// Saved-image format: `"png"` (default) or `"jpeg"`. Unknown
    /// values resolve `png` with a warning.
    #[serde(default = "default_output_format")]
    pub format: String,
    /// JPEG quality, 1–100 (default 90). Ignored for PNG.
    #[serde(default = "default_jpeg_quality")]
    pub jpeg_quality: u32,
    /// Direct-save filename template, e.g.
    /// `"{app}-{date}-{time}"`. Empty (default) = no direct save;
    /// the Save action shows its file dialog. See
    /// [`render_filename_template`].
    #[serde(default)]
    pub filename_template: String,
    /// Direct-save directory. Empty (default) = the user's
    /// pictures directory (platform standard).
    #[serde(default)]
    pub save_dir: String,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: default_output_format(),
            jpeg_quality: default_jpeg_quality(),
            filename_template: String::new(),
            save_dir: String::new(),
        }
    }
}

fn default_output_format() -> String {
    "png".to_owned()
}

fn default_jpeg_quality() -> u32 {
    90
}

/// The `capture.default-mode` vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DefaultMode {
    /// Interactive region selection.
    Region,
    /// The whole active monitor.
    Fullscreen,
    /// The focused window.
    Window,
}

/// Resolve the configured default mode; unknown values warn and
/// resolve [`DefaultMode::Region`] (string-tolerant policy).
#[must_use]
pub fn parse_default_mode(raw: &str) -> DefaultMode {
    match raw {
        "region" => DefaultMode::Region,
        "fullscreen" => DefaultMode::Fullscreen,
        "window" => DefaultMode::Window,
        other => {
            tracing::warn!("unknown capture.default-mode {other:?}; using \"region\"");
            DefaultMode::Region
        }
    }
}

/// The `output.format` vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutputFormat {
    /// Lossless PNG (default).
    Png,
    /// Lossy JPEG at the configured quality.
    Jpeg,
}

/// Resolve the configured output format; unknown values warn and
/// resolve [`OutputFormat::Png`].
#[must_use]
pub fn parse_output_format(raw: &str) -> OutputFormat {
    match raw {
        "png" => OutputFormat::Png,
        "jpeg" => OutputFormat::Jpeg,
        other => {
            tracing::warn!("unknown output.format {other:?}; using \"png\"");
            OutputFormat::Png
        }
    }
}

/// How the settings route's size readout estimates the image's
/// byte size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SizeMode {
    /// `w × h × 4` — the uncompressed-RGBA heuristic (default).
    Heuristic,
    /// A PNG encode of the composed image — exact on-disk bytes.
    ExactPng,
}

/// Resolve the configured size mode; unknown values warn and
/// resolve [`SizeMode::Heuristic`].
#[must_use]
pub fn parse_size_mode(raw: &str) -> SizeMode {
    match raw {
        "heuristic" => SizeMode::Heuristic,
        "exact-png" => SizeMode::ExactPng,
        other => {
            tracing::warn!("unknown ui.statusbar-size-mode {other:?}; using \"heuristic\"");
            SizeMode::Heuristic
        }
    }
}

/// Estimate the image's byte size behind one API: the heuristic is
/// `w × h × 4`; the exact mode PNG-encodes the composed image
/// (falling back to the heuristic when no composed image is
/// available — e.g. before the first render). Pure and
/// deterministic for a given image.
#[must_use]
pub fn estimate_size_bytes(
    dims: (u32, u32),
    mode: SizeMode,
    image: Option<&image::RgbaImage>,
) -> u64 {
    match mode {
        SizeMode::Heuristic => heuristic_bytes(dims),
        SizeMode::ExactPng => image.map_or_else(
            || heuristic_bytes(dims),
            |img| {
                let mut buf = Vec::new();
                if image::ImageEncoder::write_image(
                    image::codecs::png::PngEncoder::new(&mut std::io::Cursor::new(&mut buf)),
                    img.as_raw(),
                    img.width(),
                    img.height(),
                    image::ExtendedColorType::Rgba8,
                )
                .is_err()
                {
                    return heuristic_bytes(dims);
                }
                u64::try_from(buf.len()).unwrap_or(u64::MAX)
            },
        ),
    }
}

/// The RGBA heuristic: `w × h × 4`.
#[must_use]
pub fn heuristic_bytes(dims: (u32, u32)) -> u64 {
    u64::from(dims.0) * u64::from(dims.1) * 4
}

/// Substitute `{app}` / `{date}` / `{time}` in a filename template
/// and sanitize the result into a safe single-path-component
/// filename. Unknown `{...}` placeholders pass through untouched
/// (a typo stays visible in the saved name rather than silently
/// vanishing). Pure.
#[must_use]
pub fn render_filename_template(template: &str, app: &str, date: &str, time: &str) -> String {
    if template.is_empty() {
        // The direct-save gate reads emptiness: no template, no
        // direct save — an empty render, never a phantom "capture".
        return String::new();
    }
    let rendered = template
        .replace("{app}", app)
        .replace("{date}", date)
        .replace("{time}", time);
    sanitize_filename(&rendered)
}

/// Strip path separators, drive/prefix punctuation, and control
/// characters so a rendered template can never escape its
/// directory or produce an invalid name. Empty input maps to
/// `"capture"`. Pure.
#[must_use]
pub fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "capture".to_owned()
    } else {
        trimmed.to_owned()
    }
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

/// The fully-commented default config asset (the `VSCode` *Default
/// Settings* analogue), embedded at compile time. Rendered read-only
/// by the settings page's *View default config*, and written
/// verbatim as the template when [`ensure_user_config_at`]
/// materializes a missing user file. Pinned equal (field-by-field)
/// to [`Config::default`] by test, so the asset can never drift
/// from the code defaults.
#[must_use]
pub fn default_config_toml() -> &'static str {
    include_str!("config.default.toml")
}

/// Materialize the user's `config.toml` from the default asset when
/// — and only when — it does not exist yet, so the file on disk
/// always mirrors what Settings shows before the first *Edit config
/// file* click opens it. The asset's own header comment doubles as
/// the template's header. An existing file is a strict no-op: user
/// edits are never clobbered (pinned by test).
///
/// Returns whether the file was created.
///
/// # Errors
/// - [`FlintshotError::Io`] if the parent directory cannot be
///   created or the write fails.
/// - [`FlintshotError::Io`] for metadata failures other than
///   not-found.
pub fn ensure_user_config_at(path: &Path) -> Result<bool> {
    match std::fs::metadata(path) {
        Ok(_) => Ok(false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(FlintshotError::Io)?;
            }
            std::fs::write(path, default_config_toml()).map_err(FlintshotError::Io)?;
            Ok(true)
        }
        Err(e) => Err(FlintshotError::Io(e)),
    }
}

/// Platform-path wrapper over [`ensure_user_config_at`]: ensures the
/// user's `config.toml` exists (creating it from the template when
/// missing) and returns its path — `None` only when the platform
/// offers no config dir.
///
/// # Errors
/// See [`ensure_user_config_at`].
pub fn ensure_user_config() -> Result<Option<PathBuf>> {
    let Some(path) = config_path() else {
        return Ok(None);
    };
    ensure_user_config_at(&path)?;
    Ok(Some(path))
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
    fn settings_fields_roundtrip_through_save_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.capture.default_mode = "fullscreen".into();
        cfg.ui.show_statusbar = false;
        cfg.ui.statusbar_size_mode = "exact-png".into();
        cfg.ui.animations = false;
        cfg.output.format = "jpeg".into();
        cfg.output.jpeg_quality = 75;
        cfg.output.filename_template = "{app}-{date}-{time}".into();
        cfg.output.save_dir = "/tmp/shots".into();

        cfg.save_to(&path).unwrap();
        let back = Config::load_from(&path).unwrap();
        assert_eq!(back.capture.default_mode, "fullscreen");
        assert!(!back.ui.show_statusbar);
        assert_eq!(back.ui.statusbar_size_mode, "exact-png");
        assert!(!back.ui.animations);
        assert_eq!(back.output.format, "jpeg");
        assert_eq!(back.output.jpeg_quality, 75);
        assert_eq!(back.output.filename_template, "{app}-{date}-{time}");
        assert_eq!(back.output.save_dir, "/tmp/shots");
    }

    #[test]
    fn old_config_without_settings_fields_loads_with_defaults() {
        // A pre-settings-route TOML: none of the new fields exist.
        // Every one of them must deserialize to its default.
        let dir = tempdir().unwrap();
        let path = dir.path().join("old.toml");
        std::fs::write(
            &path,
            "[ui]\ndefault-stroke-width = 5.0\n[capture]\ninclude-cursor = false\n",
        )
        .unwrap();
        let cfg = Config::load_from(&path).expect("old config loads");
        assert_eq!(cfg.capture.default_mode, "region");
        assert!(cfg.ui.show_statusbar);
        assert_eq!(cfg.ui.statusbar_size_mode, "heuristic");
        assert!(cfg.ui.animations);
        assert_eq!(cfg.output.format, "png");
        assert_eq!(cfg.output.jpeg_quality, 90);
        assert!(cfg.output.filename_template.is_empty());
        assert!(cfg.output.save_dir.is_empty());
        // The old fields keep their own values.
        assert_eq!(cfg.ui.default_stroke_width, 5.0);
        assert!(!cfg.capture.include_cursor);
    }

    #[test]
    fn settings_parsers_reject_unknowns_to_defaults() {
        assert_eq!(parse_default_mode("region"), DefaultMode::Region);
        assert_eq!(parse_default_mode("fullscreen"), DefaultMode::Fullscreen);
        assert_eq!(parse_default_mode("window"), DefaultMode::Window);
        assert_eq!(parse_default_mode("banana"), DefaultMode::Region);

        assert_eq!(parse_output_format("png"), OutputFormat::Png);
        assert_eq!(parse_output_format("jpeg"), OutputFormat::Jpeg);
        assert_eq!(parse_output_format("webp"), OutputFormat::Png);

        assert_eq!(parse_size_mode("heuristic"), SizeMode::Heuristic);
        assert_eq!(parse_size_mode("exact-png"), SizeMode::ExactPng);
        assert_eq!(parse_size_mode("telepathy"), SizeMode::Heuristic);
    }

    #[test]
    fn filename_template_substitutes_known_and_keeps_unknown() {
        let out =
            render_filename_template("{app}-{date}-{time}", "flintshot", "2026-09-18", "12-30-00");
        assert_eq!(out, "flintshot-2026-09-18-12-30-00");
        // Unknown placeholders pass through untouched.
        assert_eq!(
            render_filename_template("shot-{zone}", "app", "d", "t"),
            "shot-{zone}"
        );
        // Empty template stays empty (the direct-save gate reads
        // emptiness, not the rendering).
        assert_eq!(render_filename_template("", "app", "d", "t"), "");
    }

    #[test]
    fn sanitize_filename_never_escapes_the_directory() {
        assert_eq!(sanitize_filename("a/b/c"), "abc");
        assert_eq!(sanitize_filename("..\\"), "capture");
        assert_eq!(sanitize_filename("shot:*.png"), "shot.png");
        assert_eq!(sanitize_filename("  .. "), "capture");
        assert_eq!(sanitize_filename("ok-name_1.2"), "ok-name_1.2");
    }

    #[test]
    fn size_estimator_spans_heuristic_and_exact_png() {
        let dims = (1920, 1080);
        assert_eq!(
            estimate_size_bytes(dims, SizeMode::Heuristic, None),
            1920 * 1080 * 4
        );
        // Exact: a real PNG encode of a solid image compresses far
        // below the heuristic but stays positive.
        let img = image::RgbaImage::from_pixel(64, 64, image::Rgba([255, 0, 0, 255]));
        let exact = estimate_size_bytes((64, 64), SizeMode::ExactPng, Some(&img));
        assert!(exact > 0 && exact < heuristic_bytes((64, 64)));
        // Exact without a composed image falls back to the heuristic.
        assert_eq!(
            estimate_size_bytes(dims, SizeMode::ExactPng, None),
            1920 * 1080 * 4
        );
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
    fn flip_to_move_defaults_false_when_absent() {
        let cfg = Config::load_from(&{
            let dir = tempdir().unwrap();
            let path = dir.path().join("config.toml");
            fs::write(&path, "[ui]\ndefault-color = \"#000000\"\n").unwrap();
            path
        })
        .unwrap();
        assert!(!cfg.ui.flip_to_move, "absent field must default false");
        assert!(!UiConfig::default().flip_to_move, "Default impl matches");
    }

    #[test]
    fn flip_to_move_true_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.ui.flip_to_move = true;
        cfg.save_to(&path).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("flip-to-move = true"),
            "flip-to-move not persisted:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert!(back.ui.flip_to_move);
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

    // ── Default-config asset (settings-improvement Task 001) ─────

    /// The asset deserializes to exactly the code defaults, field
    /// by field — the asset can never drift from `Config::default()`
    /// without this failing.
    #[test]
    fn default_config_asset_matches_code_defaults_field_by_field() {
        let from_asset = toml::from_str::<Config>(default_config_toml())
            .expect("the default asset must parse as a Config");
        let d = Config::default();

        assert_eq!(from_asset.ui.default_color, d.ui.default_color);
        assert_eq!(from_asset.ui.default_text_color, d.ui.default_text_color);
        assert_eq!(
            from_asset.ui.default_stroke_width,
            d.ui.default_stroke_width
        );
        assert_eq!(from_asset.ui.default_font_size, d.ui.default_font_size);
        assert_eq!(from_asset.ui.magnifier_modifier, d.ui.magnifier_modifier);
        assert_eq!(from_asset.ui.magnifier_always_on, d.ui.magnifier_always_on);
        assert_eq!(from_asset.ui.interaction_mode, d.ui.interaction_mode);
        assert_eq!(from_asset.ui.flip_to_move, d.ui.flip_to_move);
        assert_eq!(from_asset.ui.show_statusbar, d.ui.show_statusbar);
        assert_eq!(from_asset.ui.statusbar_size_mode, d.ui.statusbar_size_mode);
        assert_eq!(from_asset.ui.animations, d.ui.animations);

        assert_eq!(from_asset.capture.include_cursor, d.capture.include_cursor);
        assert_eq!(
            from_asset.capture.default_delay_ms,
            d.capture.default_delay_ms
        );
        assert_eq!(from_asset.capture.backend, d.capture.backend);
        assert_eq!(from_asset.capture.after_grab, d.capture.after_grab);
        assert_eq!(from_asset.capture.default_mode, d.capture.default_mode);

        assert_eq!(from_asset.hotkeys.capture, d.hotkeys.capture);
        assert_eq!(from_asset.upload.default_backend, d.upload.default_backend);

        assert_eq!(from_asset.output.format, d.output.format);
        assert_eq!(from_asset.output.jpeg_quality, d.output.jpeg_quality);
        assert_eq!(
            from_asset.output.filename_template,
            d.output.filename_template
        );
        assert_eq!(from_asset.output.save_dir, d.output.save_dir);
    }

    /// The asset carries a user-facing header: the materialized file
    /// names itself, so a user opening it cold knows what they hold.
    #[test]
    fn default_config_asset_opens_with_a_header_comment() {
        let first_line = default_config_toml().lines().next().unwrap_or_default();
        assert!(
            first_line.starts_with("# flintshot configuration"),
            "asset must open with a header comment, got: {first_line:?}"
        );
    }

    /// `ensure_user_config_at` on a missing file: creates it (parents
    /// included), header first, body that parses as a `Config`.
    #[test]
    fn ensure_user_config_at_creates_missing_file_with_header() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("config.toml");

        let created = ensure_user_config_at(&path).expect("creating the missing config");
        assert!(created, "a missing file must be created");
        assert!(path.exists(), "parents created too");

        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.starts_with("# flintshot configuration"),
            "the file opens with the asset's header:\n{written}"
        );
        assert_eq!(
            written,
            default_config_toml(),
            "the materialized file is the asset verbatim"
        );
        toml::from_str::<Config>(&written).expect("the written body parses");
    }

    /// `ensure_user_config_at` on an existing file: a strict no-op —
    /// user edits (even a hand-written sentinel) are never clobbered.
    #[test]
    fn ensure_user_config_at_never_clobbers_an_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let sentinel = "# my precious config\n[ui]\ndefault-color = \"#00ff00\"\n";
        fs::write(&path, sentinel).unwrap();

        let created = ensure_user_config_at(&path).expect("checking the existing config");
        assert!(!created, "an existing file must not be rewritten");

        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(after, sentinel, "byte-identical after the call");
    }
}
