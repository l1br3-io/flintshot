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

use crate::ai::ProviderPreset;
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
    /// AI provider choice for Ask-the-shot (see [`AiConfig`]). Holds
    /// no secret material — the API key lives in the OS keyring.
    #[serde(default)]
    pub ai: AiConfig,
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
    /// need no re-press. `true` flips to Select after every eligible
    /// commit so the next press repositions what was just drawn
    /// (ADR-0040, superseded as the default by ADR-0058). Read once
    /// per editor session (matching the magnifier precedent).
    ///
    /// Legacy files may still carry the pre-rename `flip-to-move`
    /// key (values map 1:1); the serde alias reads it and the save
    /// path rewrites it as `flip-to-select`.
    #[serde(default = "default_flip_to_select", alias = "flip-to-move")]
    pub flip_to_select: bool,
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
    /// How many entries the recents index keeps (the home grid and
    /// `Recents` truncation consume the clamped form — see
    /// [`UiConfig::recent_captures_limit_clamped`]). Clamped 1–200
    /// at use; stored verbatim so a hand-edited out-of-range value
    /// survives re-saves.
    #[serde(default = "default_recent_captures_limit")]
    pub recent_captures_limit: u32,
    /// Right panel width in logical px, clamped to
    /// [`PANEL_MIN_WIDTH`]..=[`PANEL_MAX_WIDTH`] on read (see
    /// [`UiConfig::panel_width_clamped`]). Persisted so the editor
    /// reopens at the user's last width.
    #[serde(default = "default_panel_width")]
    pub panel_width: f32,
    /// Whether the right panel starts collapsed. Reopened with
    /// `Ctrl+B` (the collapsed state exposes no clickable
    /// affordance by design).
    #[serde(default)]
    pub panel_collapsed: bool,
}

impl UiConfig {
    /// The recents capacity actually enforced: the configured value
    /// clamped to 1–200.
    #[must_use]
    pub fn recent_captures_limit_clamped(&self) -> usize {
        // ≤ 200 by the clamp, so the fallback is unreachable on any
        // real target — but no unwrap outside tests.
        usize::try_from(self.recent_captures_limit.clamp(1, 200)).unwrap_or(1)
    }

    /// The panel width actually enforced: the configured value
    /// clamped to [`PANEL_MIN_WIDTH`]..=[`PANEL_MAX_WIDTH`]. Stored
    /// verbatim; every consumer (panel seed, worker fit) reads this.
    #[must_use]
    pub fn panel_width_clamped(&self) -> f32 {
        self.panel_width.clamp(PANEL_MIN_WIDTH, PANEL_MAX_WIDTH)
    }
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
            flip_to_select: default_flip_to_select(),
            show_statusbar: default_show_statusbar(),
            statusbar_size_mode: default_statusbar_size_mode(),
            animations: default_animations(),
            recent_captures_limit: default_recent_captures_limit(),
            panel_width: default_panel_width(),
            panel_collapsed: false,
        }
    }
}

/// The right panel's resize bounds, logical px: below 200 the panel
/// content breaks, above 400 it squeezes the canvas. Shared by the
/// editor shell's panel `size_range` and the clamp-on-read accessor
/// — one source of truth, toolkit-free f32.
pub const PANEL_MIN_WIDTH: f32 = 200.0;
/// See [`PANEL_MIN_WIDTH`].
pub const PANEL_MAX_WIDTH: f32 = 400.0;

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

/// AI provider defaults (Ask-the-shot). The `[ai]` section holds
/// only the *choice* of provider/model/base-url — the API key lives
/// in the OS keyring (entry `flinshot / ai:<provider>`), never in
/// `config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct AiConfig {
    /// Provider preset id: `"openai"`, `"xai"`, `"gemini"`, or
    /// `"custom"`. Stored verbatim; unknown values warn and resolve
    /// to the custom preset at use time (string-tolerant policy).
    #[serde(default = "default_ai_provider")]
    pub provider: String,
    /// Model id sent on the wire (e.g. `"gpt-4o-mini"`). Default
    /// empty — must be user-set; pages pre-fill from the preset's
    /// model hint.
    #[serde(default)]
    pub model: String,
    /// Base-URL override for the provider endpoint. Empty (default)
    /// = the preset's base URL; non-empty wins (`Ollama`, `LM
    /// Studio`, `OpenRouter`, ...).
    #[serde(default)]
    pub base_url: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: default_ai_provider(),
            model: String::new(),
            base_url: String::new(),
        }
    }
}

impl AiConfig {
    /// The provider preset for use: unknown ids warn and resolve to
    /// custom — the stored value survives re-saves (string-tolerant
    /// policy, `interaction-mode` precedent).
    #[must_use]
    pub fn preset(&self) -> &'static ProviderPreset {
        let preset = ProviderPreset::for_id(&self.provider);
        if preset.id != self.provider {
            tracing::warn!(
                "unknown ai.provider {:?}; resolving to the custom preset",
                self.provider
            );
        }
        preset
    }

    /// Base URL requests go to: the override when set, else the
    /// preset's. May be empty (custom with no override yet) — the
    /// config page treats that as "not configured".
    #[must_use]
    pub fn effective_base_url(&self) -> &str {
        if self.base_url.is_empty() {
            self.preset().base_url
        } else {
            &self.base_url
        }
    }
}

fn default_ai_provider() -> String {
    "openai".into()
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

fn default_flip_to_select() -> bool {
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
    /// pictures directory (platform standard — see
    /// [`OutputConfig::resolve_save_dir`]).
    #[serde(default)]
    pub save_dir: String,
    /// Whether flintshot's own copy-to-clipboard actions (the
    /// editor's Copy button, `capture.after-grab = "clipboard"`)
    /// also write the image to the resolved save directory so the
    /// copy lands in recents. A persist failure never fails the
    /// copy — the clipboard write already succeeded.
    #[serde(default = "default_save_clipboard_copies")]
    pub save_clipboard_copies: bool,
}

impl OutputConfig {
    /// The effective save directory: the configured `save-dir`, or
    /// the platform pictures dir plus `Flintshot` when unset
    /// (`~/Pictures/Flintshot` on Linux and macOS,
    /// `…\My Pictures\Flintshot` on Windows) — the fallback the
    /// settings row has documented since Phase 1. Falls back to
    /// `~/Pictures/Flintshot` via the home dir when the platform
    /// reports no pictures dir, and to a relative `Flintshot` only
    /// when no home exists either.
    #[must_use]
    pub fn resolve_save_dir(&self) -> PathBuf {
        let base = dirs::picture_dir().or_else(|| dirs::home_dir().map(|h| h.join("Pictures")));
        resolve_save_dir_with(&self.save_dir, base.as_deref())
    }
}

/// Testable core of [`OutputConfig::resolve_save_dir`]: a non-blank
/// `save_dir` passes through (whitespace-trimmed); a blank one joins
/// `Flintshot` onto `pictures_base` (or a relative `Flintshot` when
/// the platform offers no base — never empty, so callers always have
/// a directory to create). Pure.
#[must_use]
pub fn resolve_save_dir_with(save_dir: &str, pictures_base: Option<&Path>) -> PathBuf {
    let trimmed = save_dir.trim();
    if !trimmed.is_empty() {
        return PathBuf::from(trimmed);
    }
    pictures_base.map_or_else(|| PathBuf::from("Flintshot"), |base| base.join("Flintshot"))
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: default_output_format(),
            jpeg_quality: default_jpeg_quality(),
            filename_template: String::new(),
            save_dir: String::new(),
            save_clipboard_copies: default_save_clipboard_copies(),
        }
    }
}

fn default_save_clipboard_copies() -> bool {
    true
}

fn default_recent_captures_limit() -> u32 {
    50
}

fn default_panel_width() -> f32 {
    236.0
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
        ensure_table(&mut doc, "output");

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
        if let Some(out) = doc.get_mut("output").and_then(|i| i.as_table_mut()) {
            set_value(
                out,
                "format",
                toml_edit::Value::from(self.output.format.as_str()),
            );
            set_value(
                out,
                "jpeg-quality",
                toml_edit::Value::from(i64::from(self.output.jpeg_quality)),
            );
            set_value(
                out,
                "filename-template",
                toml_edit::Value::from(self.output.filename_template.as_str()),
            );
            set_value(
                out,
                "save-dir",
                toml_edit::Value::from(self.output.save_dir.as_str()),
            );
            set_value(
                out,
                "save-clipboard-copies",
                toml_edit::Value::from(self.output.save_clipboard_copies),
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
        // Legacy-key migration: a pre-rename `flip-to-move` entry
        // moves onto the new key — value in place below, and the
        // old key's leaf decor (any user comment) rides along;
        // toml_edit's remove/insert drops key decor unless it is
        // copied explicitly.
        if ui.contains_key("flip-to-move") && !ui.contains_key("flip-to-select") {
            let key_decor = ui
                .key_mut("flip-to-move")
                .map(|key| key.leaf_decor().clone());
            if let Some(item) = ui.remove("flip-to-move") {
                if let toml_edit::Entry::Vacant(vac) = ui.entry("flip-to-select") {
                    vac.insert(item);
                    if let (Some(decor), Some(mut key)) = (key_decor, ui.key_mut("flip-to-select"))
                    {
                        *key.leaf_decor_mut() = decor;
                    }
                }
            }
        }
        set_value(
            ui,
            "flip-to-select",
            toml_edit::Value::from(self.ui.flip_to_select),
        );
        set_value(
            ui,
            "show-statusbar",
            toml_edit::Value::from(self.ui.show_statusbar),
        );
        set_value(
            ui,
            "statusbar-size-mode",
            toml_edit::Value::from(self.ui.statusbar_size_mode.as_str()),
        );
        set_value(ui, "animations", toml_edit::Value::from(self.ui.animations));
        set_value(
            ui,
            "recent-captures-limit",
            toml_edit::Value::from(i64::from(self.ui.recent_captures_limit)),
        );
        set_value(
            ui,
            "panel-width",
            toml_edit::Value::from(f64::from(self.ui.panel_width)),
        );
        set_value(
            ui,
            "panel-collapsed",
            toml_edit::Value::from(self.ui.panel_collapsed),
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
    fn flip_to_select_defaults_false_when_absent() {
        let cfg = Config::load_from(&{
            let dir = tempdir().unwrap();
            let path = dir.path().join("config.toml");
            fs::write(&path, "[ui]\ndefault-color = \"#000000\"\n").unwrap();
            path
        })
        .unwrap();
        assert!(!cfg.ui.flip_to_select, "absent field must default false");
        assert!(!UiConfig::default().flip_to_select, "Default impl matches");
    }

    #[test]
    fn flip_to_select_true_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.ui.flip_to_select = true;
        cfg.save_to(&path).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("flip-to-select = true"),
            "flip-to-select not persisted:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert!(back.ui.flip_to_select);
    }

    #[test]
    fn flip_to_select_preserves_existing_comment_on_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let existing = "\
[ui]
# keep tools sticky while batching rects
flip-to-select = false
";
        fs::write(&path, existing).unwrap();

        let mut cfg = Config::default();
        cfg.ui.flip_to_select = false;
        cfg.save_to(&path).unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("# keep tools sticky while batching rects"),
            "comment on flip-to-select lost:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert!(!back.ui.flip_to_select, "user value must survive re-save");
    }

    /// Legacy `flip-to-move` files migrate silently: the value reads
    /// into `flip_to_select`, a re-save writes the new key (carrying
    /// any comment), and the old key disappears.
    #[test]
    fn legacy_flip_to_move_key_migrates_on_resave() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let legacy = "\
[ui]
# flip after every commit
flip-to-move = true
";
        fs::write(&path, legacy).unwrap();

        let cfg = Config::load_from(&path).unwrap();
        assert!(cfg.ui.flip_to_select, "legacy key value maps 1:1");

        cfg.save_to(&path).unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("flip-to-select = true"),
            "re-save writes the new key:\n{written}"
        );
        assert!(
            written.contains("# flip after every commit"),
            "the legacy key's comment rides along:\n{written}"
        );
        assert!(
            !written.contains("flip-to-move"),
            "the legacy key is gone after re-save:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert!(back.ui.flip_to_select, "value survives the round-trip");
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
        assert_eq!(from_asset.ui.flip_to_select, d.ui.flip_to_select);
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
        assert_eq!(
            from_asset.output.save_clipboard_copies,
            d.output.save_clipboard_copies
        );
        assert_eq!(
            from_asset.ui.recent_captures_limit,
            d.ui.recent_captures_limit
        );
        assert_eq!(from_asset.ui.panel_width, d.ui.panel_width);
        assert_eq!(from_asset.ui.panel_collapsed, d.ui.panel_collapsed);

        assert_eq!(from_asset.ai.provider, d.ai.provider);
        assert_eq!(from_asset.ai.model, d.ai.model);
        assert_eq!(from_asset.ai.base_url, d.ai.base_url);
    }

    // ── [ai] section (ai-transforms Task 003) ────────────────────

    #[test]
    fn ai_section_missing_partial_and_unknown_field_load_cleanly() {
        let missing: Config = toml::from_str("").unwrap();
        assert_eq!(missing.ai.provider, "openai");
        assert_eq!(missing.ai.model, "");
        assert_eq!(missing.ai.base_url, "");

        let partial: Config = toml::from_str("[ai]\nprovider = \"xai\"\n").unwrap();
        assert_eq!(partial.ai.provider, "xai");
        assert_eq!(partial.ai.model, "");
        assert_eq!(partial.ai.base_url, "");

        let unknown: Config = toml::from_str(
            "[ai]\nprovider = \"gemini\"\nmodel = \"gemini-2.0-flash\"\nsome-future-field = 7\n",
        )
        .unwrap();
        assert_eq!(unknown.ai.provider, "gemini");
        assert_eq!(unknown.ai.model, "gemini-2.0-flash");
    }

    #[test]
    fn ai_section_round_trips_with_kebab_case_keys() {
        let config: Config = toml::from_str(
            "[ai]\nprovider = \"custom\"\nmodel = \"llama3.2-vision\"\nbase-url = \"http://localhost:11434/v1\"\n",
        )
        .unwrap();
        let written = toml::to_string_pretty(&config).unwrap();
        let reloaded: Config = toml::from_str(&written).unwrap();
        assert_eq!(reloaded.ai.provider, "custom");
        assert_eq!(reloaded.ai.model, "llama3.2-vision");
        assert_eq!(reloaded.ai.base_url, "http://localhost:11434/v1");
    }

    #[test]
    fn ai_provider_unknown_value_resolves_custom_at_use_but_stores_verbatim() {
        let mut config = Config::default();
        config.ai.provider = "anthrope".into(); // typo-tolerant at use
        let preset = config.ai.preset();
        assert_eq!(preset.id, "custom", "unknown provider resolves custom");

        // ...and the stored value survives a save/load round-trip.
        let written = toml::to_string_pretty(&config).unwrap();
        let reloaded: Config = toml::from_str(&written).unwrap();
        assert_eq!(reloaded.ai.provider, "anthrope");
    }

    #[test]
    fn ai_effective_base_url_prefers_override_then_preset() {
        let mut config = Config::default();
        assert_eq!(
            config.ai.effective_base_url(),
            "https://api.openai.com/v1",
            "no override => preset default"
        );
        config.ai.base_url = "http://localhost:11434/v1".into();
        assert_eq!(config.ai.effective_base_url(), "http://localhost:11434/v1");
        config.ai.provider = "custom".into();
        config.ai.base_url.clear();
        assert_eq!(
            config.ai.effective_base_url(),
            "",
            "custom with no override is empty (not-configured signal)"
        );
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

    // ── Recent-captures-v2 Task 001 ───────────────────────────────

    /// The explicit `save-dir` passes through (whitespace-trimmed);
    /// an unset one joins `Flintshot` onto the injected pictures
    /// base, so the test never depends on the host env.
    #[test]
    fn resolve_save_dir_spans_explicit_empty_and_no_base() {
        use std::path::Path;
        assert_eq!(
            resolve_save_dir_with("/tmp/shots", Some(Path::new("/home/u/Pictures"))),
            PathBuf::from("/tmp/shots"),
            "the explicit path wins"
        );
        assert_eq!(
            resolve_save_dir_with("", Some(Path::new("/home/u/Pictures"))),
            PathBuf::from("/home/u/Pictures/Flintshot"),
            "empty resolves onto the base"
        );
        assert_eq!(
            resolve_save_dir_with("   ", Some(Path::new("/p"))),
            PathBuf::from("/p/Flintshot"),
            "whitespace-only resolves like unset"
        );
        assert_eq!(
            resolve_save_dir_with("", None),
            PathBuf::from("Flintshot"),
            "no base at all: a degraded relative dir, never empty"
        );
    }

    /// The method (host dirs) is pinned where the host allows it:
    /// explicit passthrough exactly, and the unset fallback always
    /// ends in `Flintshot`.
    #[test]
    fn resolve_save_dir_method_resolves_on_this_host() {
        let explicit = OutputConfig {
            save_dir: "/tmp/explicit-shots".into(),
            ..OutputConfig::default()
        };
        assert_eq!(
            explicit.resolve_save_dir(),
            PathBuf::from("/tmp/explicit-shots")
        );
        let resolved = OutputConfig::default().resolve_save_dir();
        assert!(
            resolved.ends_with("Flintshot"),
            "unset resolves to the platform pictures dir + Flintshot, got {resolved:?}"
        );
    }

    /// The new fields default correctly from a file that predates
    /// them (acceptance: old configs load with zero migration).
    #[test]
    fn recents_v2_fields_default_when_absent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[ui]\ndefault-color = \"#ff0000\"\n").unwrap();
        let cfg = Config::load_from(&path).unwrap();
        assert!(
            cfg.output.save_clipboard_copies,
            "persists copies by default"
        );
        assert_eq!(cfg.ui.recent_captures_limit, 50, "default capacity 50");
        assert!(OutputConfig::default().save_clipboard_copies);
        assert_eq!(UiConfig::default().recent_captures_limit, 50);
    }

    /// Both new fields survive a save/load round-trip.
    #[test]
    fn recents_v2_fields_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut cfg = Config::default();
        cfg.output.save_clipboard_copies = false;
        cfg.ui.recent_captures_limit = 100;
        cfg.save_to(&path).unwrap();
        let back = Config::load_from(&path).unwrap();
        assert!(!back.output.save_clipboard_copies);
        assert_eq!(back.ui.recent_captures_limit, 100);
    }

    /// The clamp the recents plumbing consumes: 1–200.
    #[test]
    fn recent_captures_limit_clamps_one_to_two_hundred() {
        let clamped = |v: u32| {
            UiConfig {
                recent_captures_limit: v,
                ..UiConfig::default()
            }
            .recent_captures_limit_clamped()
        };
        assert_eq!(clamped(0), 1);
        assert_eq!(clamped(500), 200);
        assert_eq!(clamped(50), 50);
    }

    /// The merge fix at the heart of the picker: saving **into an
    /// existing file** must write every `[output]` value and the
    /// `[ui]` keys the merge previously dropped — otherwise a
    /// Settings-driven change never reaches disk.
    #[test]
    fn save_into_existing_file_writes_output_and_late_ui_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "[ui]\ndefault-color = \"#ff0000\"\n[capture]\ninclude-cursor = true\n",
        )
        .unwrap();

        let mut cfg = Config::default();
        cfg.output.format = "jpeg".into();
        cfg.output.jpeg_quality = 75;
        cfg.output.filename_template = "{app}-{date}".into();
        cfg.output.save_dir = "/tmp/shots".into();
        cfg.output.save_clipboard_copies = false;
        cfg.ui.show_statusbar = false;
        cfg.ui.statusbar_size_mode = "exact-png".into();
        cfg.ui.animations = false;
        cfg.ui.recent_captures_limit = 100;
        cfg.save_to(&path).unwrap();

        let back = Config::load_from(&path).unwrap();
        assert_eq!(back.output.format, "jpeg");
        assert_eq!(back.output.jpeg_quality, 75);
        assert_eq!(back.output.filename_template, "{app}-{date}");
        assert_eq!(back.output.save_dir, "/tmp/shots");
        assert!(!back.output.save_clipboard_copies);
        assert!(!back.ui.show_statusbar);
        assert_eq!(back.ui.statusbar_size_mode, "exact-png");
        assert!(!back.ui.animations);
        assert_eq!(back.ui.recent_captures_limit, 100);
    }

    /// The merge preserves an existing `[output]` entry's comments
    /// while rewriting its value (`set_value` decor semantics).
    #[test]
    fn output_merge_preserves_existing_comments() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "[output]\n# where my shots go\nsave-dir = \"/old\"\n",
        )
        .unwrap();

        let mut cfg = Config::default();
        cfg.output.save_dir = "/new".into();
        cfg.save_to(&path).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("# where my shots go"),
            "comment on save-dir lost:\n{written}"
        );
        assert!(
            written.contains("save-dir = \"/new\""),
            "value updated in place:\n{written}"
        );
    }

    // ── sidebar-collapse-resize Task 001 ──────────────────────────

    /// The panel fields default (236 / false) in code and from a
    /// file that predates them (acceptance: old configs load with
    /// zero migration).
    #[test]
    fn panel_fields_default_when_absent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[ui]\ndefault-color = \"#ff0000\"\n").unwrap();
        let cfg = Config::load_from(&path).unwrap();
        assert_eq!(cfg.ui.panel_width, 236.0, "default width 236");
        assert!(!cfg.ui.panel_collapsed, "default expanded");
        assert_eq!(UiConfig::default().panel_width, 236.0);
        assert!(!UiConfig::default().panel_collapsed);
    }

    /// Both fields survive a save/load round-trip in kebab-case.
    #[test]
    fn panel_fields_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut cfg = Config::default();
        cfg.ui.panel_width = 320.0;
        cfg.ui.panel_collapsed = true;
        cfg.save_to(&path).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("panel-width = 320"),
            "panel-width not persisted kebab-case:\n{written}"
        );
        assert!(
            written.contains("panel-collapsed = true"),
            "panel-collapsed not persisted kebab-case:\n{written}"
        );

        let back = Config::load_from(&path).unwrap();
        assert_eq!(back.ui.panel_width, 320.0);
        assert!(back.ui.panel_collapsed);
    }

    /// The clamp the panel plumbing consumes: 200–400, stored
    /// verbatim so a hand-edited out-of-range value survives
    /// re-saves (the `recent_captures_limit` policy).
    #[test]
    fn panel_width_clamps_two_hundred_to_four_hundred() {
        let clamped = |v: f32| {
            UiConfig {
                panel_width: v,
                ..UiConfig::default()
            }
            .panel_width_clamped()
        };
        assert_eq!(clamped(5.0), 200.0);
        assert_eq!(clamped(9000.0), 400.0);
        assert_eq!(clamped(236.0), 236.0);
        // The raw field loads verbatim (clamp is at use, not on load).
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[ui]\npanel-width = 9000.0\n").unwrap();
        let cfg = Config::load_from(&path).unwrap();
        assert_eq!(cfg.ui.panel_width, 9000.0, "stored verbatim");
        assert_eq!(cfg.ui.panel_width_clamped(), 400.0, "clamped at use");
        // The bounds consts are the single source both crates share.
        assert_eq!(PANEL_MIN_WIDTH, 200.0);
        assert_eq!(PANEL_MAX_WIDTH, 400.0);
    }
}
