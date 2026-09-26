//! The topbar — zone 1 of the five-zone window: a 48 px forest-deep
//! cutout bar with the flame brand + the `Flintshot | Home > Editor`
//! breadcrumb
//! identity (the breadcrumb's past stages are links home, the
//! current stage gold + bold), exactly three hero ghost buttons —
//! **Capture** and **Copy** with the gold hero-key border,
//! **Open** plain — and the dots overflow chip.
//!
//! One component serves every routed surface ([`Surface`]): the
//! heroes stay, the overflow contents vary by route (editor:
//! Save/Upload/Paste; home/loading: about + note; all: the Settings
//! entry, inert until the settings route lands). Worker editor
//! windows draw the same bar themselves.

use egui::{Color32, Stroke};

use crate::theme;
use crate::widgets;

/// Topbar height, logical px — the mockup's zone contract.
pub(crate) const TOPBAR_HEIGHT: f32 = 48.0;

/// The topbar's horizontal inner margin, logical px — the frame's
/// gutter and the overflow chip's right inset.
const BAR_MARGIN: i8 = 14;

/// The overflow chip's edge — and the overflow menu's row height:
/// the bar's compact control size, one step past [`theme::CONTROL_H`]
/// for the hero-height click targets the density bump carries.
const OVERFLOW_EDGE: f32 = theme::CONTROL_H + 4.0;

/// An overflow row's glyph-slot centre and label start, from the
/// row's left edge: 12 px pad + 8 px half-slot, then an 8 px gap.
const ROW_GLYPH_AT: f32 = 20.0;
const ROW_LABEL_AT: f32 = 36.0;

/// Which surface the topbar serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Surface {
    /// The welcome page.
    Home,
    /// An editor surface (worker window or routed editor).
    Editor,
    /// The capture-progress page.
    Loading,
    /// The settings page (constructed when its route lands,
    /// Task 010; the enable matrix already pins it).
    #[allow(dead_code)]
    Settings,
}

/// What a topbar interaction fired this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopbarAction {
    /// The hero Capture button.
    Capture,
    /// The hero Copy button.
    Copy,
    /// The hero AI button — opens the Ask-the-shot dialog (the
    /// `I` shortcut's pointer twin).
    Ai,
    /// The hero Open button.
    Open,
    /// The overflow's Save item.
    Save,
    /// The overflow's Upload item.
    Upload,
    /// The overflow's Paste item.
    Paste,
    /// The overflow's Resize item (editor surface only). Opens the
    /// resize popover (see [`resize_popover`]); the shell owns the
    /// open/closed state and applies through
    /// [`EditorState::resize`](flintshot_core::EditorState::resize).
    Resize,
    /// The overflow's Settings entry.
    Settings,
    /// The breadcrumb's Home link — leave the current stage for
    /// home (route home in-window; worker windows close, matching
    /// Esc).
    NavigateHome,
}

/// Which topbar buttons are live this frame, as a compact bitmask —
/// disabled buttons render in the cream-dim style and swallow input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Enable(u8);

impl Enable {
    /// The hero Capture button.
    pub(crate) const CAPTURE: u8 = 1;
    /// The hero Copy button.
    pub(crate) const COPY: u8 = 1 << 1;
    /// The hero AI button.
    pub(crate) const AI: u8 = 1 << 2;
    /// The hero Open button.
    pub(crate) const OPEN: u8 = 1 << 3;
    /// The overflow's Save / Upload / Paste items (editor routes).
    pub(crate) const EDITOR_ITEMS: u8 = 1 << 4;

    /// All actions live (the routed editor surface).
    pub(crate) const ALL: Enable =
        Enable(Self::CAPTURE | Self::COPY | Self::AI | Self::OPEN | Self::EDITOR_ITEMS);

    /// The worker editor: Open / Paste have no bridge, so they stay
    /// disabled; Capture / Copy / Save / Upload are local. AI is
    /// live too — the worker shell is the same `EditorApp` as the
    /// routed page, so the AI run bridge (dialog + conversation +
    /// transforms refresh) is lifecycle-independent (spec Open
    /// Question 1, resolved by inspection).
    pub(crate) const WORKER: Enable = Enable(Self::CAPTURE | Self::COPY | Self::AI);

    /// Nothing live.
    pub(crate) const NONE: Enable = Enable(0);

    /// Whether `flag` is enabled.
    #[must_use]
    pub(crate) const fn has(self, flag: u8) -> bool {
        self.0 & flag != 0
    }
}

/// The per-route enablement matrix (spec §Routes): Home wires
/// Capture + Open (Copy has no image to copy — the mockup's
/// disabled tooltip explains); Editor is fully live; Settings keeps
/// Capture only; Loading disables everything while a grab is in
/// flight.
pub(crate) fn enable_for(surface: Surface) -> Enable {
    match surface {
        Surface::Home => Enable(Enable::CAPTURE | Enable::OPEN),
        Surface::Editor => Enable::ALL,
        Surface::Settings => Enable(Enable::CAPTURE),
        Surface::Loading => Enable::NONE,
    }
}

/// Draw the topbar across the top of `ui` and report what fired.
/// `copied` arms the Copy hero's "Copied" state (check glyph, gold
/// label) — the caller owns the ~2 s window (see `EditorApp`'s
/// `copied_visible`). `resize_live` dims the overflow's Resize item
/// when the image can't shrink (≤ 1 px wide).
pub(crate) fn show(
    ui: &mut egui::Ui,
    surface: Surface,
    status: Option<&str>,
    enable: Enable,
    copied: bool,
    resize_live: bool,
) -> Option<TopbarAction> {
    use crate::theme::palette as p;
    let mut fired = None;
    egui::Panel::top("flintshot_topbar")
        .resizable(false)
        .exact_size(TOPBAR_HEIGHT)
        .frame(
            egui::Frame::new()
                .fill(p::FOREST_DEEP)
                .inner_margin(egui::Margin::symmetric(BAR_MARGIN, 0)),
        )
        .show(ui, |ui| {
            ui.set_min_height(TOPBAR_HEIGHT - 4.0);
            ui.horizontal_centered(|ui| {
                brand(ui, surface, &mut fired);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    overflow(ui, surface, status, enable, resize_live, &mut fired);
                    ui.add_space(theme::SPACE_SCALE[1]);
                    hero(
                        ui,
                        "topbar-open",
                        g_open,
                        "Open",
                        false,
                        enable.has(Enable::OPEN),
                        false,
                        &mut fired,
                    );
                    let ai_resp = hero(
                        ui,
                        "topbar-ai",
                        g_spark_gold,
                        "AI",
                        false,
                        enable.has(Enable::AI),
                        false,
                        &mut fired,
                    );
                    if surface == Surface::Home && !enable.has(Enable::AI) {
                        ai_resp.on_hover_text("No image is open — AI is available in the editor");
                    }
                    let copy_resp = hero(
                        ui,
                        "topbar-copy",
                        crate::panel::g_copy,
                        if copied { "Copied" } else { "Copy" },
                        true,
                        enable.has(Enable::COPY),
                        copied,
                        &mut fired,
                    );
                    if surface == Surface::Home && !enable.has(Enable::COPY) {
                        copy_resp
                            .on_hover_text("No image is open — copy is available in the editor");
                    }
                    hero(
                        ui,
                        "topbar-capture",
                        g_capture,
                        "Capture",
                        true,
                        enable.has(Enable::CAPTURE),
                        false,
                        &mut fired,
                    );
                });
            });
            // The quiet cream bottom edge, per the mockup.
            let rect = ui.max_rect();
            ui.painter().line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                egui::Stroke::new(2.0, p::with_alpha(p::CREAM_TEXT, 36)),
            );
        });
    fired
}

/// The flame badge + `Flintshot` title + `|` + breadcrumb. The
/// route lives only in the breadcrumb (the old `| Editor` suffix
/// duplicated it and read as a second, confusing label).
fn brand(ui: &mut egui::Ui, surface: Surface, fired: &mut Option<TopbarAction>) {
    use crate::theme::palette as p;
    // The gold flame badge, 24 px.
    let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(8),
        p::GOLD,
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "🔥",
        egui::FontId::proportional(theme::TYPE_SM),
        p::FOREST_INK,
    );
    ui.add_space(4.0);
    // `Flintshot` in the display weight, then the dim `|` separator.
    let name = ui.painter().layout_no_wrap(
        "Flintshot".to_owned(),
        egui::FontId::new(
            theme::TYPE_MD,
            egui::FontFamily::Name(theme::FAMILY_DISPLAY.into()),
        ),
        p::CREAM_TEXT,
    );
    let (rect, _) = ui.allocate_exact_size(name.size(), egui::Sense::hover());
    ui.painter().galley(rect.min, name, p::CREAM_TEXT);
    ui.add_space(theme::SPACE_SCALE[0]);
    let sep = ui.painter().layout_no_wrap(
        "|".to_owned(),
        egui::FontId::proportional(theme::TYPE_MD),
        p::CREAM_DIM,
    );
    let (rect, _) = ui.allocate_exact_size(sep.size(), egui::Sense::hover());
    ui.painter().galley(rect.min, sep, p::CREAM_DIM);
    ui.add_space(theme::POD_GAP);
    breadcrumb(ui, surface, fired);
}

/// The breadcrumb (`Home > Editor`, painted chevrons): past
/// stages are links back
/// home — gold + underline on hover, a click reports
/// [`TopbarAction::NavigateHome`] — while the current stage is
/// static gold + bold. Links are live only on surfaces that can
/// navigate (Editor / Settings); Loading is mid-grab (inert) and
/// Home is its own current stage.
fn breadcrumb(ui: &mut egui::Ui, surface: Surface, fired: &mut Option<TopbarAction>) {
    use crate::theme::palette as p;
    let (stages, current) = match surface {
        Surface::Home => (&["Home"][..], 0usize),
        Surface::Editor => (&["Home", "Editor"][..], 1),
        Surface::Loading => (&["Home", "Loading"][..], 1),
        Surface::Settings => (&["Home", "Settings"][..], 1),
    };
    let can_navigate = matches!(surface, Surface::Editor | Surface::Settings);
    for (i, stage) in stages.iter().enumerate() {
        let cur = i == current;
        let font = if cur {
            egui::FontId::new(
                theme::TYPE_SM,
                egui::FontFamily::Name(theme::FAMILY_DISPLAY.into()),
            )
        } else {
            egui::FontId::proportional(theme::TYPE_SM)
        };
        let galley = ui.painter().layout_no_wrap(
            (*stage).to_owned(),
            font,
            if cur { p::GOLD } else { p::CREAM_DIM },
        );
        let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
        if can_navigate && !cur {
            let resp = ui
                .interact(
                    rect,
                    egui::Id::new(("topbar-crumb", i)),
                    egui::Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.hovered() {
                ui.painter()
                    .galley_with_override_text_color(rect.min, galley, p::GOLD);
                ui.painter().line_segment(
                    [
                        egui::pos2(rect.left(), rect.bottom() + 1.0),
                        egui::pos2(rect.right(), rect.bottom() + 1.0),
                    ],
                    egui::Stroke::new(1.0, p::GOLD),
                );
            } else {
                ui.painter().galley(rect.min, galley, p::CREAM_DIM);
            }
            if resp.clicked() {
                *fired = Some(TopbarAction::NavigateHome);
            }
        } else {
            ui.painter()
                .galley(rect.min, galley, if cur { p::GOLD } else { p::CREAM_DIM });
        }
        if i + 1 < stages.len() {
            // The stage separator: a painted chevron (the text
            // glyph is tofu in the font chain, U+25B8 — cmap scan).
            let (sep_rect, _) =
                ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            g_chevron(ui.painter(), sep_rect, p::with_alpha(p::CREAM_DIM, 128));
            ui.add_space(2.0);
        }
    }
}

/// One hero ghost button; registers under `id` and reports clicks
/// through `fired`. `copied` passes the Copy hero's "Copied" state
/// through to [`widgets::ghost_button`].
// Flat args by design: each hero call reads as one line and the
// per-hero differences (icon / label / key / gate / copied) stay
// visible at the call site — a spec struct would bury them.
#[allow(clippy::too_many_arguments)]
fn hero(
    ui: &mut egui::Ui,
    id: &'static str,
    glyph: crate::rail::Glyph,
    label: &str,
    hero_key: bool,
    enabled: bool,
    copied: bool,
    fired: &mut Option<TopbarAction>,
) -> egui::Response {
    let action = match id {
        "topbar-capture" => TopbarAction::Capture,
        "topbar-copy" => TopbarAction::Copy,
        "topbar-ai" => TopbarAction::Ai,
        _ => TopbarAction::Open,
    };
    let resp = ui
        .scope(|ui| {
            if !enabled {
                ui.disable();
            }
            widgets::ghost_button(ui, egui::Id::new(id), glyph, label, hero_key, copied)
        })
        .inner;
    if enabled && resp.clicked() {
        *fired = Some(action);
    }
    resp
}

/// The overflow chip's dots glyph: three centered vector dots.
/// Painted,
/// never text — the embedded font chain's cmaps lack U+22EF, so
/// the text glyph renders as tofu (ux-redesign-regressions spec's
/// cmap scan).
fn more_dots(p: &egui::Painter, rect: egui::Rect, c: Color32) {
    for x in [-5.0_f32, 0.0, 5.0] {
        p.circle_filled(crate::rail::pt(rect, x, 0.0), 1.75, c);
    }
}

// --- the topbar glyph kit (ux-redesign-regressions task 002) ---
//
// Every icon the topbar renders is a painted vector glyph, never
// text: the embedded font chain's cmaps lack U+22EF, U+29C9,
// U+25B8, U+2913, U+21E7 and U+2713 (the spec's cmap scan), so
// those codepoints render as tofu — and emoji-fallback codepoints
// render inconsistently across hosts. Copy / save / upload / check
// reuse `panel`'s existing `pub(crate)` glyphs; the rest live here.
// Glyph space is ±7 with +Y down (`rail::pt`).

/// Capture: a plus in a ring — the hero's "new capture" mark.
fn g_capture(p: &egui::Painter, r: egui::Rect, c: Color32) {
    p.circle_stroke(crate::rail::pt(r, 0.0, 0.0), 6.0, egui::Stroke::new(2.0, c));
    crate::rail::seg(p, r, (-3.0, 0.0), (3.0, 0.0), c, 2.0);
    crate::rail::seg(p, r, (0.0, -3.0), (0.0, 3.0), c, 2.0);
}

/// Open: a folder — tabbed top edge, closed body.
fn g_open(p: &egui::Painter, r: egui::Rect, c: Color32) {
    p.add(egui::Shape::line(
        vec![
            crate::rail::pt(r, -7.0, -5.0),
            crate::rail::pt(r, -2.0, -5.0),
            crate::rail::pt(r, 0.0, -3.0),
            crate::rail::pt(r, 7.0, -3.0),
            crate::rail::pt(r, 7.0, 5.0),
            crate::rail::pt(r, -7.0, 5.0),
            crate::rail::pt(r, -7.0, -5.0),
        ],
        egui::Stroke::new(2.0, c),
    ));
}

/// Paste: a clipboard sheet with its clip peeking above.
fn g_paste(p: &egui::Painter, r: egui::Rect, c: Color32) {
    p.rect(
        egui::Rect::from_min_size(crate::rail::pt(r, -5.0, -4.0), egui::vec2(10.0, 11.0)),
        egui::CornerRadius::same(2),
        Color32::TRANSPARENT,
        egui::Stroke::new(2.0, c),
        egui::StrokeKind::Middle,
    );
    p.rect(
        egui::Rect::from_min_size(crate::rail::pt(r, -2.0, -6.0), egui::vec2(4.0, 2.5)),
        egui::CornerRadius::same(1),
        c,
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
}

/// About: an information dot and stem.
fn g_about(p: &egui::Painter, r: egui::Rect, c: Color32) {
    p.circle_filled(crate::rail::pt(r, 0.0, -5.0), 1.6, c);
    crate::rail::seg(p, r, (0.0, -1.5), (0.0, 6.0), c, 2.0);
}

/// Resize: a double-headed diagonal arrow (top-left ↔ bottom-right)
/// — the universal "scale this" mark.
fn g_resize(p: &egui::Painter, r: egui::Rect, c: Color32) {
    crate::rail::seg(p, r, (-5.0, -5.0), (5.0, 5.0), c, 2.0);
    // Arrowheads: a small V at each end, pointing outward.
    crate::rail::seg(p, r, (-5.0, -5.0), (-5.0, -1.0), c, 2.0);
    crate::rail::seg(p, r, (-5.0, -5.0), (-1.0, -5.0), c, 2.0);
    crate::rail::seg(p, r, (5.0, 5.0), (5.0, 1.0), c, 2.0);
    crate::rail::seg(p, r, (5.0, 5.0), (1.0, 5.0), c, 2.0);
}

/// Settings: a cog — center ring with four radial ticks.
fn g_settings(p: &egui::Painter, r: egui::Rect, c: Color32) {
    p.circle_stroke(crate::rail::pt(r, 0.0, 0.0), 3.0, egui::Stroke::new(2.0, c));
    for (dx, dy) in [(0.0_f32, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
        crate::rail::seg(p, r, (dx * 4.5, dy * 4.5), (dx * 6.5, dy * 6.5), c, 2.0);
    }
}

/// The breadcrumb separator: a small filled right-pointing
/// triangle (U+25B8's text glyph is tofu in the font chain —
/// the cmap scan).
fn g_chevron(p: &egui::Painter, r: egui::Rect, c: Color32) {
    p.add(egui::Shape::convex_polygon(
        vec![
            crate::rail::pt(r, -2.0, -4.0),
            crate::rail::pt(r, 3.0, 0.0),
            crate::rail::pt(r, -2.0, 4.0),
        ],
        c,
        egui::Stroke::NONE,
    ));
}

/// Four-point spark — the Ask-the-shot mark: a diamond star of four
/// tapered points. Relocated from the rail chip to the topbar's
/// glyph kit (the AI hero's icon; the rail chip retires with this
/// effort).
pub(crate) fn g_spark(p: &egui::Painter, r: egui::Rect, c: Color32) {
    // Centre cross of four lines out to the tips, plus a smaller
    // rotated square for the star's body.
    let ct = r.center();
    let long = 7.0;
    let short = 2.2;
    p.line_segment(
        [
            egui::pos2(ct.x - long, ct.y),
            egui::pos2(ct.x - short, ct.y),
        ],
        Stroke::new(1.8, c),
    );
    p.line_segment(
        [
            egui::pos2(ct.x + short, ct.y),
            egui::pos2(ct.x + long, ct.y),
        ],
        Stroke::new(1.8, c),
    );
    p.line_segment(
        [
            egui::pos2(ct.x, ct.y - long),
            egui::pos2(ct.x, ct.y - short),
        ],
        Stroke::new(1.8, c),
    );
    p.line_segment(
        [
            egui::pos2(ct.x, ct.y + short),
            egui::pos2(ct.x, ct.y + long),
        ],
        Stroke::new(1.8, c),
    );
    let body = short * std::f32::consts::FRAC_1_SQRT_2;
    p.line_segment(
        [egui::pos2(ct.x - body, ct.y), egui::pos2(ct.x, ct.y - body)],
        Stroke::new(1.8, c),
    );
    p.line_segment(
        [egui::pos2(ct.x, ct.y - body), egui::pos2(ct.x + body, ct.y)],
        Stroke::new(1.8, c),
    );
    p.line_segment(
        [egui::pos2(ct.x + body, ct.y), egui::pos2(ct.x, ct.y + body)],
        Stroke::new(1.8, c),
    );
    p.line_segment(
        [egui::pos2(ct.x, ct.y + body), egui::pos2(ct.x - body, ct.y)],
        Stroke::new(1.8, c),
    );
}

/// The AI hero's mark: the spark in gold — the retired rail chip's
/// color carried into the topbar. Disabled heroes pass their dim
/// ink through untouched.
fn g_spark_gold(p: &egui::Painter, r: egui::Rect, c: Color32) {
    let ink = if c == theme::palette::CREAM_TEXT {
        theme::palette::GOLD
    } else {
        c
    };
    g_spark(p, r, ink);
}

/// The dots overflow: an about note + route-varying items. Editor
/// surfaces get Save / Upload / Paste; every surface gets the
/// Settings entry (the routed window swaps to the settings route;
/// worker windows delegate through the shell's open-settings hook).
fn overflow(
    ui: &mut egui::Ui,
    surface: Surface,
    status: Option<&str>,
    enable: Enable,
    resize_live: bool,
    fired: &mut Option<TopbarAction>,
) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(OVERFLOW_EDGE, OVERFLOW_EDGE),
        egui::Sense::hover(),
    );
    let resp = ui
        .interact(rect, egui::Id::new("topbar-overflow"), egui::Sense::click())
        .on_hover_text("More");
    let fill = if resp.hovered() {
        theme::palette::with_alpha(theme::palette::CREAM_TEXT, 36)
    } else {
        theme::palette::with_alpha(theme::palette::CREAM_TEXT, 15)
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::POD_RADIUS),
        fill,
        egui::Stroke::new(
            1.5,
            theme::palette::with_alpha(theme::palette::CREAM_TEXT, 71),
        ),
        egui::StrokeKind::Inside,
    );
    more_dots(ui.painter(), rect, theme::palette::CREAM_TEXT);
    egui::Popup::from_toggle_button_response(&resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(212.0);
            if enable.has(Enable::EDITOR_ITEMS) {
                if om_row(
                    ui,
                    "topbar-save",
                    crate::panel::g_save,
                    "Save",
                    Some("Ctrl+S"),
                ) {
                    *fired = Some(TopbarAction::Save);
                }
                if om_row(ui, "topbar-upload", crate::panel::g_upload, "Upload", None) {
                    *fired = Some(TopbarAction::Upload);
                }
                if om_row(ui, "topbar-paste", g_paste, "Paste", Some("Ctrl+V")) {
                    *fired = Some(TopbarAction::Paste);
                }
                if om_row_enabled(ui, "topbar-resize", g_resize, "Resize", None, resize_live) {
                    *fired = Some(TopbarAction::Resize);
                }
                om_separator(ui);
            }
            // The about row: informational, hover-only.
            om_row_about(ui, g_about, "About Flintshot");
            om_note(ui, status);
            om_separator(ui);
            if om_row(ui, "topbar-settings", g_settings, "Settings", None) {
                *fired = Some(TopbarAction::Settings);
            }
            let _ = surface;
        });
}

// --- Resize popover (editor-resize-image) ---

/// Cross-frame state for the resize popover, owned by the shell
/// (mirrors the app-side `copied_visible` pattern: the menu row is
/// transient, but the field contents must survive frames).
#[derive(Debug, Default)]
pub(crate) struct ResizeMenu {
    /// Whether the popover is open.
    pub(crate) open: bool,
    /// The custom-width field's raw contents (digits only, validated
    /// at apply — `open_for` seeds it with the current width).
    pub(crate) width_text: String,
}

impl ResizeMenu {
    /// Open the popover, seeding the field with the current width so
    /// editing starts from a meaningful number.
    pub(crate) fn open_for(&mut self, w: u32) {
        self.open = true;
        self.width_text = w.to_string();
    }
}

/// The preset percents offered (spec: 25 / 50 / 75 — never upscale).
pub(crate) const RESIZE_PRESETS: [u32; 3] = [25, 50, 75];

/// Percent → target width: `(w · pct / 100)` rounded, min 1, clamped
/// to `w` (never upscale).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "percent math rounds before the u32 cast; dims ≤ 2^24"
)]
pub(crate) fn preset_width(w: u32, pct: u32) -> u32 {
    (((w as f32) * (pct as f32) / 100.0).round().max(1.0) as u32).min(w)
}

/// Aspect-locked height for `new_w`: `(h · new_w / w)` rounded,
/// min 1. The label beside the custom field — never an input.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "aspect math rounds before the u32 cast; dims ≤ 2^24"
)]
pub(crate) fn derived_height(w: u32, h: u32, new_w: u32) -> u32 {
    (((h as f32) * (new_w as f32) / (w as f32)).round().max(1.0) as u32).min(h)
}

/// Draw the resize popover: preset chips (25 / 50 / 75 %, hover shows
/// the resulting `WxH`) plus a custom-width row whose `× H` label
/// derives live. Returns `Some(target_w)` when a resize should apply
/// — a preset click or Enter on a valid shrink target — and closes
/// the menu on apply or Esc.
///
/// Mechanism note: the overflow menu row fires
/// [`TopbarAction::Resize`]; the shell opens `menu` and answers with
/// this standalone [`egui::Popup`] anchored under the overflow chip
/// (a popup hung off the menu row's response would close with the
/// menu itself).
pub(crate) fn resize_popover(
    ctx: &egui::Context,
    menu: &mut ResizeMenu,
    dims: (u32, u32),
) -> Option<u32> {
    if !menu.open {
        return None;
    }
    let (w, h) = dims;
    if w <= 1 {
        menu.open = false;
        return None;
    }
    let mut apply = None;
    let popup = egui::Popup::new(
        egui::Id::new("resize-popover"),
        ctx.clone(),
        overflow_chip_rect(ctx),
        egui::LayerId::background(),
    )
    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
    .show(|ui| {
        egui::Frame::new()
            .fill(theme::palette::PAPER_CREAM)
            .corner_radius(egui::CornerRadius::same(theme::CUTOUT_RADIUS))
            .inner_margin(egui::Margin::same(theme::POD_PAD))
            .stroke(theme::HAIRLINE_STROKE)
            .show(ui, |ui| {
                ui.set_min_width(216.0);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Resize image")
                            .color(theme::palette::INK)
                            .size(theme::TYPE_MD),
                    );
                    ui.add_space(theme::SPACE_SCALE[0]);
                    ui.horizontal(|ui| {
                        for (i, pct) in RESIZE_PRESETS.iter().enumerate() {
                            if let Some(target) = preset_chip(ui, i, *pct, w, h) {
                                apply = Some(target);
                            }
                        }
                    });
                    ui.add_space(theme::SPACE_SCALE[0]);
                    if let Some(target) = custom_width_row(ui, menu, w, h) {
                        apply = Some(target);
                    }
                });
            });
    });
    if apply.is_some()
        || ctx.input(|i| i.key_pressed(egui::Key::Escape))
        || popup.is_some_and(|p| p.response.should_close())
    {
        menu.open = false;
    }
    apply
}

/// The overflow chip's rect (top-right, `OVERFLOW_EDGE` square
/// centered in the bar) — the anchor the popover hangs below like a
/// submenu. Built from the screen rect because the menu row's
/// painted response dies with the closed menu.
fn overflow_chip_rect(ctx: &egui::Context) -> egui::Rect {
    let screen = ctx.viewport_rect();
    let pad = (TOPBAR_HEIGHT - OVERFLOW_EDGE) / 2.0;
    egui::Rect::from_min_size(
        egui::pos2(screen.right() - f32::from(BAR_MARGIN) - OVERFLOW_EDGE, pad),
        egui::vec2(OVERFLOW_EDGE, OVERFLOW_EDGE),
    )
}

/// One preset chip: painted pod whose hover shows the resulting
/// `WxH`; a click reports the computed target width.
fn preset_chip(ui: &mut egui::Ui, i: usize, pct: u32, w: u32, h: u32) -> Option<u32> {
    let target = preset_width(w, pct);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(52.0, 26.0), egui::Sense::hover());
    let resp = ui.interact(
        rect,
        egui::Id::new(("resize-preset", i)),
        egui::Sense::click(),
    );
    let fill = if resp.hovered() {
        theme::palette::PAPER_DIM
    } else {
        theme::palette::PAPER_CREAM
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::POD_RADIUS),
        fill,
        theme::HAIRLINE_STROKE,
        egui::StrokeKind::Middle,
    );
    let galley = ui.painter().layout_no_wrap(
        format!("{pct}%"),
        egui::FontId::proportional(theme::TYPE_SM),
        theme::palette::INK,
    );
    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        theme::palette::INK,
    );
    let clicked = resp.clicked();
    resp.on_hover_text(format!("→ {target} × {}", derived_height(w, h, target)));
    clicked.then_some(target)
}

/// The custom-width row: integer field + live `× H` label. Enter
/// applies a valid shrink target (1 ≤ v < w — no upscale); ≥ w never
/// applies.
fn custom_width_row(ui: &mut egui::Ui, menu: &mut ResizeMenu, w: u32, h: u32) -> Option<u32> {
    let mut target = None;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Width")
                .color(theme::palette::INK)
                .size(theme::TYPE_SM),
        );
        ui.add(
            egui::TextEdit::singleline(&mut menu.width_text)
                .hint_text("px")
                .desired_width(64.0)
                .id(egui::Id::new("resize-width-field")),
        );
        menu.width_text.retain(|ch| ch.is_ascii_digit());
        let parsed = menu
            .width_text
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|v| *v >= 1 && *v < w);
        let height_label = parsed.map_or_else(
            || "× —".to_owned(),
            |v| format!("× {}", derived_height(w, h, v)),
        );
        ui.label(
            egui::RichText::new(height_label)
                .color(theme::palette::INK_SOFT)
                .size(theme::TYPE_SM),
        );
        // The popover is modal over shortcuts (the shell's Esc gate),
        // so Enter anywhere in it applies a valid target.
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            target = parsed;
        }
    });
    target
}

/// One clickable overflow row: painted glyph + label + optional
/// kbd chip, paper-cream with a dim hover.
fn om_row(
    ui: &mut egui::Ui,
    id: &'static str,
    glyph: crate::rail::Glyph,
    label: &str,
    kbd: Option<&str>,
) -> bool {
    om_row_enabled(ui, id, glyph, label, kbd, true)
}

/// [`om_row`] with a live/dead switch: a dead row paints dim and
/// never fires (nothing to shrink), but still registers so callers
/// can position / assert it.
fn om_row_enabled(
    ui: &mut egui::Ui,
    id: &'static str,
    glyph: crate::rail::Glyph,
    label: &str,
    kbd: Option<&str>,
    enabled: bool,
) -> bool {
    use crate::theme::palette as p;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), OVERFLOW_EDGE),
        egui::Sense::hover(),
    );
    let resp = if enabled {
        ui.interact(rect, egui::Id::new(id), egui::Sense::click())
    } else {
        ui.interact(rect, egui::Id::new(id), egui::Sense::hover())
    };
    let ink = if enabled { p::INK } else { p::INK_SOFT };
    let fill = if enabled && resp.hovered() {
        p::PAPER_DIM
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(8),
        fill,
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    let glyph_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + ROW_GLYPH_AT, rect.center().y),
        egui::vec2(16.0, 16.0),
    );
    glyph(ui.painter(), glyph_rect, ink);
    ui.painter().text(
        egui::pos2(rect.left() + ROW_LABEL_AT, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(theme::TYPE_SM),
        ink,
    );
    if let Some(kbd) = kbd {
        ui.painter().text(
            egui::pos2(rect.right() - 12.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            kbd,
            egui::FontId::proportional(theme::TYPE_XS),
            if enabled { p::INK_SOFT } else { ink },
        );
    }
    enabled && resp.clicked()
}

/// The about row — informational (the About "action" is the note
/// itself), so hover-only with the same row styling.
fn om_row_about(ui: &mut egui::Ui, glyph: crate::rail::Glyph, label: &str) {
    use crate::theme::palette as p;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), OVERFLOW_EDGE),
        egui::Sense::hover(),
    );
    let glyph_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + ROW_GLYPH_AT, rect.center().y),
        egui::vec2(16.0, 16.0),
    );
    glyph(ui.painter(), glyph_rect, p::INK);
    ui.painter().text(
        egui::pos2(rect.left() + ROW_LABEL_AT, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(theme::TYPE_SM),
        p::INK,
    );
}

/// The diagnostics note under the about row: the status line when
/// one exists + the backends hint.
fn om_note(ui: &mut egui::Ui, status: Option<&str>) {
    use crate::theme::palette as p;
    let mut note = "Diagnostics: run `flintshot backends`".to_owned();
    if let Some(status) = status {
        note.push_str("\nStatus: ");
        note.push_str(status);
    }
    let galley = ui.painter().layout_no_wrap(
        note,
        egui::FontId::proportional(theme::TYPE_XS),
        p::INK_SOFT,
    );
    let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    ui.painter().galley(rect.min, galley, p::INK_SOFT);
}

/// The hairline separator between overflow groups.
fn om_separator(ui: &mut egui::Ui) {
    use crate::theme::palette as p;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        egui::Stroke::new(1.0, p::with_alpha(p::FOREST_INK, 38)),
    );
    ui.add_space(4.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(time: f64, events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            time: Some(time),
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(640.0, 480.0),
            )),
            ..egui::RawInput::default()
        }
    }

    fn click_events(at: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    /// The overflow chip's dots are painted as vector circles, never
    /// text: the embedded font chain's cmaps lack U+22EF, so the
    /// text glyph renders as tofu (ux-redesign-regressions spec's
    /// cmap scan).
    #[test]
    fn more_dots_paint_three_circles_inside_the_chip() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let rect = egui::Rect::from_min_size(egui::pos2(300.0, 7.0), egui::vec2(30.0, 30.0));
        let full = ctx.run_ui(screen(0.0, vec![]), |ui| {
            more_dots(ui.painter(), rect, theme::palette::CREAM_TEXT);
        });
        let dots: Vec<(egui::Pos2, f32, Color32)> = full
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Circle(c) => Some((c.center, c.radius, c.fill)),
                _ => None,
            })
            .collect();
        assert_eq!(dots.len(), 3, "exactly three dots, got {dots:?}");
        let mut xs: Vec<f32> = dots.iter().map(|(c, _, _)| c.x).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        for (center, radius, fill) in &dots {
            assert!(rect.contains(*center), "dot {center} inside the chip");
            assert!(*radius > 0.0, "dots are filled circles, not points");
            assert_eq!(*fill, theme::palette::CREAM_TEXT);
        }
        let spread = [xs[1] - xs[0], xs[2] - xs[1]];
        assert!(
            (spread[0] - spread[1]).abs() < 0.01 && spread[0] > 0.0,
            "dots are evenly spread on a row: {xs:?}"
        );
        for (center, _, _) in &dots {
            assert!(
                (center.y - rect.center().y).abs() < 0.01,
                "dots sit on the chip's centerline"
            );
        }
    }

    /// The glyph kit, machine-checked (task 002 acceptance): every
    /// kit glyph, painted into a 24 px rect on a test painter,
    /// produces at least one shape whose visual bounds stay inside
    /// (or stroke-adjacent to) the rect — no zero-size or runaway
    /// geometry.
    #[test]
    fn glyph_kit_paints_bounds_inside_the_glyph_rect() {
        let kit: &[(&str, crate::rail::Glyph)] = &[
            ("capture", g_capture),
            ("copy", crate::panel::g_copy),
            ("open", g_open),
            ("more", more_dots),
            ("save", crate::panel::g_save),
            ("upload", crate::panel::g_upload),
            ("paste", g_paste),
            ("resize", g_resize),
            ("about", g_about),
            ("settings", g_settings),
            ("check", crate::panel::g_check),
            ("chevron", g_chevron),
        ];
        for (name, glyph) in kit {
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            let rect = egui::Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(24.0, 24.0));
            let full = ctx.run_ui(screen(0.0, vec![]), |ui| {
                glyph(ui.painter(), rect, theme::palette::CREAM_TEXT);
            });
            let tol = 3.0_f32;
            let bounds: Vec<egui::Rect> = full
                .shapes
                .iter()
                .map(|s| s.shape.visual_bounding_rect())
                .filter(|b| b.intersects(rect.expand(tol)))
                .collect();
            assert!(
                !bounds.is_empty(),
                "{name}: the glyph painted at least one shape"
            );
            for b in &bounds {
                assert!(
                    rect.expand(tol).contains_rect(*b),
                    "{name}: shape bounds {b:?} runaway from the glyph rect {rect:?}"
                );
                assert!(
                    b.width() > 0.0 || b.height() > 0.0,
                    "{name}: zero-size shape {b:?}"
                );
            }
        }
    }

    /// Tofu ban (task 003 acceptance): the cmap-verified-absent
    /// codepoints — U+22EF, U+29C9, U+25B8, U+2913, U+21E7,
    /// U+2713 (spec §3's scan of Plus Jakarta Sans, Ubuntu-Light,
    /// `NotoEmoji`, emoji-icon-font) — must not appear anywhere in
    /// `crates/flintshot-editor-egui/src`: icons are painted
    /// vector glyphs, never font-chain text. Mirrors the
    /// `Color32` token scan in `theme/mod.rs`; the needle chars
    /// are assembled from codepoints at runtime so this test's
    /// own source cannot trip the scan. Re-derive the list from a
    /// cmap check of the actual font files if the chain changes.
    #[test]
    fn banned_tofu_codepoints_are_absent_from_the_crate_source() {
        fn scan(dir: &std::path::Path, banned: &[char], hits: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan(&path, banned, hits);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let Ok(body) = std::fs::read_to_string(&path) else {
                        continue;
                    };
                    if banned.iter().any(|c| body.contains(*c)) {
                        hits.push(path);
                    }
                }
            }
        }
        // Assembled from codepoints so this test's own source
        // cannot trip the scan. Re-derive from a cmap check of the
        // actual font files if the chain changes.
        let banned: Vec<char> = [0x22EF, 0x29C9, 0x25B8, 0x2913, 0x21E7, 0x2713]
            .iter()
            .filter_map(|&u| char::from_u32(u))
            .collect();
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut hits = Vec::new();
        scan(&src, &banned, &mut hits);
        assert!(
            hits.is_empty(),
            "tofu codepoints in source (icons must be painted glyphs): {hits:?}"
        );
    }

    /// The topbar is a top strip: the central panel starts at its
    /// 48 px edge.
    #[test]
    fn topbar_is_a_48px_top_strip() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let central_top = std::cell::Cell::new(0.0_f32);
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            let _ = show(ui, Surface::Editor, None, Enable::ALL, false, true);
            let central = egui::CentralPanel::default().show(ui, |ui| {
                ui.allocate_space(egui::vec2(1.0, 1.0));
            });
            central_top.set(central.response.rect.top());
        });
        assert!(
            (central_top.get() - TOPBAR_HEIGHT).abs() < 1.0,
            "the topbar strips {} px off the top, central top {}",
            TOPBAR_HEIGHT,
            central_top.get()
        );
    }

    /// The bar height pins the density bump (spec table: 48 px).
    #[test]
    fn topbar_height_pins_the_density_bump() {
        assert_eq!(TOPBAR_HEIGHT.to_bits(), 48.0_f32.to_bits());
    }

    /// The overflow chip pins the density bump: 34×34, centered in
    /// the 48 px bar, and the popover anchor agrees with the painted
    /// chip (both derive from the bar tokens, never literals).
    #[test]
    fn overflow_chip_pins_and_centers_in_the_bar() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            let _ = show(ui, Surface::Editor, None, Enable::ALL, false, true);
        });
        let chip = ctx
            .read_response(egui::Id::new("topbar-overflow"))
            .expect("overflow chip")
            .rect;
        for edge in [chip.width(), chip.height()] {
            assert!((edge - 34.0).abs() < 0.5, "chip edge {edge}");
        }
        assert!(
            chip.center().y - TOPBAR_HEIGHT / 2.0 < 1.0,
            "chip centers vertically: {chip:?}"
        );
        let anchor = overflow_chip_rect(&ctx);
        assert!(
            (anchor.left() - chip.left()).abs() < 1.0 && (anchor.top() - chip.top()).abs() < 1.0,
            "anchor {anchor:?} must agree with the painted chip {chip:?}"
        );
    }

    /// Overflow menu rows clear the 34 px control height at the new
    /// `TYPE_SM`.
    #[test]
    fn overflow_menu_rows_clear_the_control_height() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let draw = |events: Vec<egui::Event>| {
            let _ = ctx.run_ui(screen(0.0, events), |ui| {
                let _ = show(
                    ui,
                    Surface::Editor,
                    Some("connected"),
                    Enable::ALL,
                    false,
                    true,
                );
            });
        };
        draw(vec![]);
        let ovf = ctx
            .read_response(egui::Id::new("topbar-overflow"))
            .expect("overflow chip")
            .rect;
        // Open the menu, then settle passes while the popup
        // re-anchors (the quirk the overflow item tests document).
        draw(click_events(ovf.center()));
        draw(vec![]);
        draw(vec![]);
        let save = ctx
            .read_response(egui::Id::new("topbar-save"))
            .expect("save row")
            .rect;
        assert!(
            save.height() >= 34.0,
            "row height {} must clear the 34 px control height",
            save.height()
        );
    }

    /// The heroes + the overflow register on every surface — the AI
    /// hero included (disabled surfaces swallow input, they never
    /// reshuffle the bar).
    #[test]
    fn heroes_and_overflow_register_on_every_surface() {
        for surface in [
            Surface::Home,
            Surface::Editor,
            Surface::Loading,
            Surface::Settings,
        ] {
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
                show(ui, surface, None, Enable::ALL, false, true);
            });
            for id in [
                "topbar-capture",
                "topbar-copy",
                "topbar-ai",
                "topbar-open",
                "topbar-overflow",
            ] {
                assert!(
                    ctx.read_response(egui::Id::new(id)).is_some(),
                    "{surface:?}: {id} registers"
                );
            }
        }
    }

    /// The AI hero fires its action when enabled and swallows the
    /// click when disabled — Copy's grammar.
    #[test]
    fn ai_hero_fires_when_enabled_and_swallows_when_disabled() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            let _ = show(ui, Surface::Editor, None, Enable::ALL, false, true);
        });
        let hero = ctx
            .read_response(egui::Id::new("topbar-ai"))
            .expect("ai hero")
            .rect;
        let fired = std::cell::Cell::new(None);
        let _ = ctx.run_ui(screen(0.016, click_events(hero.center())), |ui| {
            fired.set(show(ui, Surface::Editor, None, Enable::ALL, false, true));
        });
        assert_eq!(fired.get(), Some(TopbarAction::Ai));

        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            let _ = show(
                ui,
                Surface::Home,
                None,
                enable_for(Surface::Home),
                false,
                true,
            );
        });
        let hero = ctx
            .read_response(egui::Id::new("topbar-ai"))
            .expect("ai hero")
            .rect;
        let fired = std::cell::Cell::new(None);
        let _ = ctx.run_ui(screen(0.016, click_events(hero.center())), |ui| {
            fired.set(show(
                ui,
                Surface::Home,
                None,
                enable_for(Surface::Home),
                false,
                true,
            ));
        });
        assert_eq!(fired.get(), None, "Home keeps AI dim — the click swallows");
    }

    /// AI enablement mirrors Copy: live on Editor and the worker
    /// shell, off on Home / Settings / Loading.
    #[test]
    fn ai_enablement_mirrors_copy() {
        assert!(enable_for(Surface::Editor).has(Enable::AI));
        assert!(Enable::WORKER.has(Enable::AI));
        for surface in [Surface::Home, Surface::Settings, Surface::Loading] {
            assert!(!enable_for(surface).has(Enable::AI), "{surface:?}");
        }
    }

    /// Clicking a hero reports its action; a disabled hero swallows
    /// the click (the cream-dim state).
    #[test]
    fn heroes_fire_when_enabled_and_swallow_when_disabled() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        // Locate the Capture hero.
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            show(ui, Surface::Editor, None, Enable::ALL, false, true);
        });
        let capture = ctx
            .read_response(egui::Id::new("topbar-capture"))
            .expect("capture hero")
            .rect;
        let fired = std::cell::Cell::new(None);
        let _ = ctx.run_ui(screen(0.016, click_events(capture.center())), |ui| {
            let f = show(ui, Surface::Editor, None, Enable::ALL, false, true);
            fired.set(f);
        });
        assert_eq!(fired.get(), Some(TopbarAction::Capture));

        // Worker: Open is disabled — the same click reports nothing.
        let open = ctx
            .read_response(egui::Id::new("topbar-open"))
            .expect("open hero")
            .rect;
        let _ = ctx.run_ui(screen(0.032, click_events(open.center())), |ui| {
            let f = show(ui, Surface::Editor, None, Enable::WORKER, false, true);
            fired.set(f);
        });
        assert_eq!(fired.get(), None, "a disabled hero swallows clicks");
    }

    /// The Copy hero's "Copied" state: armed, the hero paints the
    /// "Copied" galley and the gold painted check; unarmed, it paints
    /// "Copy" and no check. Shapes-only — the clipboard is untouched.
    #[test]
    fn copy_hero_shows_the_copied_state_when_armed() {
        fn hero_paints(copied: bool) -> (Vec<String>, bool) {
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            let full = ctx.run_ui(screen(0.0, vec![]), |ui| {
                let _ = show(ui, Surface::Editor, None, Enable::ALL, copied, true);
            });
            let texts = full
                .shapes
                .iter()
                .filter_map(|cs| match &cs.shape {
                    egui::Shape::Text(t) => Some(t.galley.text()),
                    _ => None,
                })
                .map(str::to_owned)
                .collect();
            // `panel::g_check` is a 2.5-px gold stroked path — the
            // only gold path stroke in the topbar.
            let gold_check = full.shapes.iter().any(|cs| {
                matches!(&cs.shape, egui::Shape::Path(p)
                    if p.stroke.color == egui::epaint::ColorMode::Solid(theme::palette::GOLD)
                        && (p.stroke.width - 2.5).abs() < 0.01)
            });
            (texts, gold_check)
        }
        let (texts, check) = hero_paints(true);
        assert!(
            texts.iter().any(|t| t == "Copied"),
            "armed: the label reads Copied, got {texts:?}"
        );
        assert!(check, "armed: the gold check glyph paints");
        let (texts, check) = hero_paints(false);
        assert!(
            texts.iter().any(|t| t == "Copy"),
            "unarmed: the label reads Copy, got {texts:?}"
        );
        assert!(!check, "unarmed: no check glyph");
    }

    /// The overflow opens, Save fires, and the Settings entry is
    /// present but callers receive it inertly until Task 10.
    #[test]
    fn overflow_save_fires_and_settings_is_present() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            show(
                ui,
                Surface::Editor,
                Some("connected"),
                Enable::ALL,
                false,
                true,
            );
        });
        let ovf = ctx
            .read_response(egui::Id::new("topbar-overflow"))
            .expect("overflow")
            .rect;
        // Open the menu.
        let _ = ctx.run_ui(screen(0.016, click_events(ovf.center())), |ui| {
            show(
                ui,
                Surface::Editor,
                Some("connected"),
                Enable::ALL,
                false,
                true,
            );
        });
        // Settle passes: the popup anchors at the screen centre on
        // its open pass and re-anchors below the chip one pass later
        // (the same quirk the `+` popup tests document).
        let _ = ctx.run_ui(screen(0.024, vec![]), |ui| {
            show(
                ui,
                Surface::Editor,
                Some("connected"),
                Enable::ALL,
                false,
                true,
            );
        });
        let _ = ctx.run_ui(screen(0.028, vec![]), |ui| {
            show(
                ui,
                Surface::Editor,
                Some("connected"),
                Enable::ALL,
                false,
                true,
            );
        });
        // The items render.
        assert!(
            ctx.read_response(egui::Id::new("topbar-save")).is_some(),
            "Save item renders"
        );
        assert!(
            ctx.read_response(egui::Id::new("topbar-upload")).is_some(),
            "Upload item renders"
        );
        assert!(
            ctx.read_response(egui::Id::new("topbar-paste")).is_some(),
            "Paste item renders"
        );
        assert!(
            ctx.read_response(egui::Id::new("topbar-settings"))
                .is_some(),
            "Settings entry renders"
        );
        // Click Save.
        let save = ctx
            .read_response(egui::Id::new("topbar-save"))
            .expect("save item")
            .rect;
        let fired = std::cell::Cell::new(None);
        let _ = ctx.run_ui(screen(0.032, click_events(save.center())), |ui| {
            let f = show(
                ui,
                Surface::Editor,
                Some("connected"),
                Enable::ALL,
                false,
                true,
            );
            fired.set(f);
        });
        assert_eq!(fired.get(), Some(TopbarAction::Save));
    }

    /// The home overflow carries only the about/note + Settings —
    /// no editor items.
    #[test]
    fn home_overflow_omits_editor_items() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            show(
                ui,
                Surface::Home,
                Some("connected"),
                Enable::NONE,
                false,
                false,
            );
        });
        let ovf = ctx
            .read_response(egui::Id::new("topbar-overflow"))
            .expect("overflow")
            .rect;
        let _ = ctx.run_ui(screen(0.016, click_events(ovf.center())), |ui| {
            show(
                ui,
                Surface::Home,
                Some("connected"),
                Enable::NONE,
                false,
                false,
            );
        });
        assert!(ctx.read_response(egui::Id::new("topbar-save")).is_none());
        assert!(ctx.read_response(egui::Id::new("topbar-paste")).is_none());
        assert!(
            ctx.read_response(egui::Id::new("topbar-resize")).is_none(),
            "no Resize entry off the editor surface"
        );
        assert!(
            ctx.read_response(egui::Id::new("topbar-settings"))
                .is_some(),
            "Settings is on every surface"
        );
    }

    /// The per-route enablement matrix (spec §Routes): exactly one
    /// answer per surface × button.
    #[test]
    fn enable_matrix_matches_the_spec() {
        let e = |s: Surface| enable_for(s);
        for (surface, matrix) in [
            (Surface::Home, (true, false, true, false)),
            (Surface::Editor, (true, true, true, true)),
            (Surface::Settings, (true, false, false, false)),
            (Surface::Loading, (false, false, false, false)),
        ] {
            let (capture, copy, open, editor_items) = matrix;
            assert_eq!(
                e(surface).has(Enable::CAPTURE),
                capture,
                "{surface:?}: capture"
            );
            assert_eq!(e(surface).has(Enable::COPY), copy, "{surface:?}: copy");
            assert_eq!(e(surface).has(Enable::OPEN), open, "{surface:?}: open");
            assert_eq!(
                e(surface).has(Enable::EDITOR_ITEMS),
                editor_items,
                "{surface:?}: editor items"
            );
        }
    }

    /// The breadcrumb's current stage paints gold; on the editor it
    /// reads `Editor`.
    #[test]
    fn breadcrumb_current_stage_paints_gold() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let out = ctx.run_ui(screen(0.0, vec![]), |ui| {
            show(ui, Surface::Editor, None, Enable::ALL, false, true);
        });
        let gold_editor = out.shapes.iter().any(|cs| {
            matches!(
                &cs.shape,
                egui::Shape::Text(t)
                    if t.galley.job.text == "Editor"
                        && t.galley
                            .job
                            .sections
                            .first()
                            .is_some_and(|s| s.format.color == theme::palette::GOLD)
            )
        });
        assert!(gold_editor, "the current crumb stage paints gold");
    }

    /// The route suffix is gone: the identity reads
    /// `Flintshot | Home > Editor` — no duplicated `| Editor` label,
    /// but the `|` separator between title and breadcrumb remains.
    #[test]
    fn route_suffix_is_gone_and_separator_remains() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let out = ctx.run_ui(screen(0.0, vec![]), |ui| {
            show(ui, Surface::Editor, None, Enable::ALL, false, true);
        });
        let texts = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.job.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            !texts.iter().any(|t| t == " | Editor"),
            "the duplicated route suffix must be gone (texts: {texts:?})"
        );
        assert!(
            texts.iter().any(|t| t == "|"),
            "the title/breadcrumb separator remains (texts: {texts:?})"
        );
        assert!(
            texts.iter().any(|t| t == "Flintshot"),
            "the title remains (texts: {texts:?})"
        );
    }

    /// The breadcrumb's Home crumb is a real link on the editor
    /// surface — one click reports [`TopbarAction::NavigateHome`].
    /// The current stage is static (no click response at all).
    #[test]
    fn home_crumb_is_a_link_that_fires_navigate_home() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            show(ui, Surface::Editor, None, Enable::ALL, false, true);
        });
        assert!(
            ctx.read_response(egui::Id::new(("topbar-crumb", 0)))
                .is_some(),
            "the Home crumb registers as a link"
        );
        assert!(
            ctx.read_response(egui::Id::new(("topbar-crumb", 1)))
                .is_none(),
            "the current stage registers no interaction"
        );
        let crumb = ctx
            .read_response(egui::Id::new(("topbar-crumb", 0)))
            .expect("home crumb")
            .rect;
        let fired = std::cell::Cell::new(None);
        let _ = ctx.run_ui(screen(0.016, click_events(crumb.center())), |ui| {
            fired.set(show(ui, Surface::Editor, None, Enable::ALL, false, true));
        });
        assert_eq!(fired.get(), Some(TopbarAction::NavigateHome));
    }

    /// Loading is mid-grab: its Home crumb paints but takes no
    /// clicks (no navigation away from a delivery in flight).
    #[test]
    fn loading_crumb_stays_inert() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let out = ctx.run_ui(screen(0.0, vec![]), |ui| {
            show(ui, Surface::Loading, None, Enable::NONE, false, false);
        });
        assert!(
            ctx.read_response(egui::Id::new(("topbar-crumb", 0)))
                .is_none(),
            "the Loading surface registers no crumb link"
        );
        let has_loading = out
            .shapes
            .iter()
            .any(|cs| matches!(&cs.shape, egui::Shape::Text(t) if t.galley.job.text == "Loading"));
        assert!(has_loading, "the stage still paints");
    }

    // --- Resize entry + popover (editor-resize-image) ---

    /// Percent → target width: round, min 1, never above the source.
    #[test]
    fn preset_width_rounds_and_clamps() {
        assert_eq!(preset_width(100, 25), 25);
        assert_eq!(preset_width(100, 50), 50);
        assert_eq!(preset_width(100, 75), 75);
        assert_eq!(preset_width(101, 50), 51, "101 @ 50% rounds to 51");
        assert_eq!(preset_width(101, 25), 25, "101 @ 25% rounds to 25");
        assert_eq!(preset_width(3, 25), 1, "sub-px targets clamp to 1");
        assert_eq!(preset_width(1, 75), 1, "nothing above the source");
    }

    /// Aspect-derived height: round, min 1.
    #[test]
    fn derived_height_rounds_min_one() {
        assert_eq!(derived_height(100, 80, 50), 40);
        assert_eq!(derived_height(101, 51, 51), 26);
        assert_eq!(derived_height(100, 80, 1), 1, "sub-px heights clamp to 1");
        assert_eq!(derived_height(100, 3, 1), 1);
    }

    /// The editor overflow carries a live Resize item that fires
    /// [`TopbarAction::Resize`].
    #[test]
    fn overflow_shows_and_fires_resize_on_editor_surface() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            show(ui, Surface::Editor, None, Enable::ALL, false, true);
        });
        let ovf = ctx
            .read_response(egui::Id::new("topbar-overflow"))
            .expect("overflow")
            .rect;
        let _ = ctx.run_ui(screen(0.016, click_events(ovf.center())), |ui| {
            show(ui, Surface::Editor, None, Enable::ALL, false, true);
        });
        let _ = ctx.run_ui(screen(0.024, vec![]), |ui| {
            show(ui, Surface::Editor, None, Enable::ALL, false, true);
        });
        let _ = ctx.run_ui(screen(0.028, vec![]), |ui| {
            show(ui, Surface::Editor, None, Enable::ALL, false, true);
        });
        let item = ctx
            .read_response(egui::Id::new("topbar-resize"))
            .expect("Resize item renders under EDITOR_ITEMS");
        let fired = std::cell::Cell::new(None);
        let _ = ctx.run_ui(screen(0.032, click_events(item.rect.center())), |ui| {
            fired.set(show(ui, Surface::Editor, None, Enable::ALL, false, true));
        });
        assert_eq!(fired.get(), Some(TopbarAction::Resize));
    }

    /// The Resize item paints dim and fires nothing when the image
    /// can't shrink (≤ 1 px wide).
    #[test]
    fn overflow_resize_item_inert_when_image_too_small() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            show(ui, Surface::Editor, None, Enable::ALL, false, false);
        });
        let ovf = ctx
            .read_response(egui::Id::new("topbar-overflow"))
            .expect("overflow")
            .rect;
        let _ = ctx.run_ui(screen(0.016, click_events(ovf.center())), |ui| {
            show(ui, Surface::Editor, None, Enable::ALL, false, false);
        });
        let _ = ctx.run_ui(screen(0.024, vec![]), |ui| {
            show(ui, Surface::Editor, None, Enable::ALL, false, false);
        });
        let _ = ctx.run_ui(screen(0.028, vec![]), |ui| {
            show(ui, Surface::Editor, None, Enable::ALL, false, false);
        });
        let item = ctx
            .read_response(egui::Id::new("topbar-resize"))
            .expect("the row still paints (dim)");
        let fired = std::cell::Cell::new(None);
        let _ = ctx.run_ui(screen(0.032, click_events(item.rect.center())), |ui| {
            fired.set(show(ui, Surface::Editor, None, Enable::ALL, false, false));
        });
        assert!(fired.get().is_none(), "a dead entry never fires");
    }

    /// The open popover offers the three preset chips plus the custom
    /// width field with its derived-height label.
    #[test]
    fn resize_popover_offers_presets_and_custom_field() {
        let (ctx, _menu) = opened_resize_popover((100, 80));
        for i in 0..RESIZE_PRESETS.len() {
            assert!(
                ctx.read_response(egui::Id::new(("resize-preset", i)))
                    .is_some(),
                "preset chip {i} renders"
            );
        }
        assert!(
            ctx.read_response(egui::Id::new("resize-width-field"))
                .is_some(),
            "the custom width field renders"
        );
    }

    /// A preset click reports its computed width and closes.
    #[test]
    fn resize_popover_preset_click_applies_and_closes() {
        let (ctx, mut menu) = settled_resize_popover((100, 80));
        let chip = ctx
            .read_response(egui::Id::new(("resize-preset", 1)))
            .expect("50% chip")
            .rect
            .center();
        let fired = std::cell::Cell::new(None);
        let _ = ctx.run_ui(screen(0.032, click_events(chip)), |ui| {
            fired.set(resize_popover(ui.ctx(), &mut menu, (100, 80)));
        });
        assert_eq!(fired.get(), Some(50));
        assert!(!menu.open, "apply closes the popover");
    }

    /// Enter in the custom field applies the typed width.
    #[test]
    fn resize_popover_custom_enter_applies() {
        let (ctx, mut menu) = settled_resize_popover((100, 80));
        menu.width_text = "80".into();
        let fired = std::cell::Cell::new(None);
        let _ = ctx.run_ui(screen(0.032, vec![enter_key()]), |ui| {
            fired.set(resize_popover(ui.ctx(), &mut menu, (100, 80)));
        });
        assert_eq!(fired.get(), Some(80));
        assert!(!menu.open, "apply closes the popover");
    }

    /// Enter at or above the current width applies nothing — the
    /// no-upscale guarantee is enforced at the UI layer too.
    #[test]
    fn resize_popover_refuses_upscale_targets() {
        let (ctx, mut menu) = settled_resize_popover((100, 80));
        menu.width_text = "100".into();
        let fired = std::cell::Cell::new(None);
        let _ = ctx.run_ui(screen(0.032, vec![enter_key()]), |ui| {
            fired.set(resize_popover(ui.ctx(), &mut menu, (100, 80)));
        });
        assert!(fired.get().is_none(), "current width never applies");
        assert!(menu.open, "the popover stays for a valid target");
    }

    /// Esc closes the popover without applying.
    #[test]
    fn resize_popover_escape_closes() {
        let (ctx, mut menu) = settled_resize_popover((100, 80));
        let esc = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let _ = ctx.run_ui(screen(0.032, vec![esc]), |ui| {
            let _ = resize_popover(ui.ctx(), &mut menu, (100, 80));
        });
        assert!(!menu.open, "Esc closes");
    }

    fn enter_key() -> egui::Event {
        egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Lay the popover out once (open), mirroring the popover.rs
    /// harness: interactions drive the next pass.
    fn opened_resize_popover(dims: (u32, u32)) -> (egui::Context, ResizeMenu) {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut menu = ResizeMenu::default();
        menu.open_for(dims.0);
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            let _ = resize_popover(ui.ctx(), &mut menu, dims);
        });
        (ctx, menu)
    }

    /// [`opened_resize_popover`] plus two settle passes so the
    /// popover's widgets are real: pass 1 sizes invisibly and
    /// `read_response` lags one pass — the same quirk the overflow
    /// menu tests document.
    fn settled_resize_popover(dims: (u32, u32)) -> (egui::Context, ResizeMenu) {
        let (ctx, mut menu) = opened_resize_popover(dims);
        for t in [0.016, 0.024] {
            let _ = ctx.run_ui(screen(t, vec![]), |ui| {
                let _ = resize_popover(ui.ctx(), &mut menu, dims);
            });
        }
        (ctx, menu)
    }
}
