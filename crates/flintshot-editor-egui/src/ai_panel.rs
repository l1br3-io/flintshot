//! The right panel's AI tab — the Ask-the-shot conversation view.
//!
//! A darkroom contact sheet: the initiating shot as a paper-shadowed
//! thumbnail at top, then a vertical thread of turns. User turns are
//! quiet cream chips right-aligned; assistant responses are
//! full-width cream paper cards (markdown once Task 008 lands —
//! plain wrapped text until then); failures are berry-tinted error
//! cards. A minimal one-line follow-up pill sits at the bottom,
//! deliberately understated until the chatbox spec.
//!
//! [`AiConversation`] is pure page-owned state (the unit-test seam);
//! rendering consumes theme tokens only.

use crate::theme;
use egui::{CornerRadius, RichText, Sense};

/// Render assistant markdown. One-branch switch to the vendored
/// minimal renderer if the `egui_commonmark` pin is ever rejected
/// (the spec's fallback path — see the dep comment in the workspace
/// `Cargo.toml`).
fn assistant_markdown(ui: &mut egui::Ui, cache: &mut egui_commonmark::CommonMarkCache, text: &str) {
    // Theme mapping: ink text on the cream card, code blocks on a
    // forest-tinted fill, forest links. The crate sizes headings
    // itself from the type ladder in `ui.style`.
    ui.style_mut().visuals.override_text_color = Some(theme::palette::INK);
    ui.style_mut().visuals.code_bg_color = theme::palette::with_alpha(theme::palette::FOREST, 24);
    ui.style_mut().visuals.hyperlink_color = theme::palette::FOREST_MID;
    // The shared cache carries image/loading state (none without
    // `load-images`); parsing is per-frame by crate design —
    // panel-sized responses parse in microseconds and only while
    // the AI tab is visible.
    egui_commonmark::CommonMarkViewer::new().show(ui, cache, text);
}

/// One rendered turn of the conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AiTurn {
    /// The user's instruction (the initiating transform prompt or a
    /// follow-up), rendered as a right-aligned cream chip.
    User {
        /// The prompt text.
        prompt: String,
    },
    /// An assistant response, rendered as a full-width paper card.
    Assistant {
        /// The response markdown (rendered as text until Task 008).
        text: String,
    },
    /// A failed request, rendered as a berry-tinted card.
    Error {
        /// The surfaced error message.
        message: String,
        /// Whether the card offers the Configure AI… deep-link
        /// (vision-unsupported rejections — the fix is a model
        /// change, not a retry).
        configure: bool,
    },
}

/// The AI tab's page-owned conversation state.
///
/// In-memory only — conversation history never persists to disk
/// (spec boundary). The thumbnail handle uploads when a transform
/// run starts (Task 007); `None` before the first run.
// The bools are one-shot render/handoff flags, not domain states —
// a state machine here would obscure the page-poll choreography.
#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct AiConversation {
    /// The conversation so far, oldest first.
    turns: Vec<AiTurn>,
    /// Whether a request is in flight (the quiet spinner row).
    pending: bool,
    /// The in-flight stream's partial text, appended per delta and
    /// rendered as the growing card. Replaced by the worker's
    /// authoritative full text on finalize.
    streaming: Option<String>,
    /// The follow-up input's draft text.
    draft: String,
    /// The initiating shot's thumbnail texture.
    thumb: Option<egui::TextureHandle>,
    /// The shared markdown cache for assistant cards (image/loading
    /// state — unused without `load-images`, kept so the viewer has
    /// its per-conversation home).
    md_cache: egui_commonmark::CommonMarkCache,
    /// A submitted follow-up awaiting the page's send wiring
    /// (submitted inside `panel::show`'s borrow; consumed right
    /// after via [`AiConversation::take_outbox`]).
    outbox: Option<String>,
    /// The Stop pill's one-shot click flag (consumed by the page
    /// right after the render, like the outbox).
    stop_requested: bool,
    /// The error card's Configure AI… link fired (the page's one-shot
    /// handoff to the bridge's config route, like the stop request).
    configure_requested: bool,
    /// Whether an assistant response just landed (the arrival
    /// hairline's one-shot trigger; the first render of the fresh
    /// card consumes it).
    fresh_arrival: bool,
}

impl AiConversation {
    /// Whether no turn has landed yet (the invitation-copy state).
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    /// Append a user turn (the run's prompt).
    pub(crate) fn push_user(&mut self, prompt: impl Into<String>) {
        self.turns.push(AiTurn::User {
            prompt: prompt.into(),
        });
    }

    /// Append an assistant response and clear the pending flag —
    /// response arrival always ends the wait (and arms the arrival
    /// hairline).
    pub(crate) fn push_assistant(&mut self, text: impl Into<String>) {
        self.turns.push(AiTurn::Assistant { text: text.into() });
        self.pending = false;
        self.fresh_arrival = true;
    }

    /// Take the one-shot arrival flag (the fresh card's first render).
    fn take_fresh_arrival(&mut self) -> bool {
        std::mem::take(&mut self.fresh_arrival)
    }

    /// Append a streamed delta to the in-progress assistant card
    /// (starts the card on the first delta). The run stays pending —
    /// only a terminal ([`AiConversation::finish_stream`] or
    /// [`AiConversation::fail_stream`]) ends the wait.
    pub(crate) fn push_delta(&mut self, text: impl Into<String>) {
        let text = text.into();
        match &mut self.streaming {
            Some(partial) => partial.push_str(&text),
            None => self.streaming = Some(text),
        }
    }

    /// Whether a stream is in flight (the growing card + caret).
    #[must_use]
    pub(crate) fn is_streaming(&self) -> bool {
        self.streaming.is_some()
    }

    /// The in-flight partial text, if any (assertion accessor for
    /// tests).
    pub(crate) fn streaming_text(&self) -> Option<&str> {
        self.streaming.as_deref()
    }

    /// Finalize the stream: the worker's authoritative full text
    /// replaces the locally accumulated partial and lands as the
    /// (markdown) assistant card. A `Response` with no prior delta —
    /// the fallback path — lands identically.
    pub(crate) fn finish_stream(&mut self, full: impl Into<String>) {
        self.streaming = None;
        self.push_assistant(full);
    }

    /// A failure mid-stream keeps the partial: it lands as a
    /// finished assistant card and the error card appends after it.
    /// With no partial, only the error card lands (today's shape).
    pub(crate) fn fail_stream(&mut self, message: impl Into<String>) {
        self.fail_with(message.into(), false);
    }

    /// A vision-unsupported failure lands identically to
    /// [`Self::fail_stream`] but the error card carries the
    /// Configure AI… deep-link — the fix is a model change, so the
    /// card routes to settings instead of suggesting a retry.
    pub(crate) fn fail_vision_unsupported(&mut self, message: impl Into<String>) {
        self.fail_with(message.into(), true);
    }

    /// The shared failure path: keep any streamed partial as a
    /// finished card, append the error card, end the wait.
    fn fail_with(&mut self, message: String, configure: bool) {
        if let Some(partial) = self.streaming.take() {
            if !partial.is_empty() {
                self.turns.push(AiTurn::Assistant { text: partial });
            }
        }
        self.turns.push(AiTurn::Error { message, configure });
        self.pending = false;
    }

    /// Record the Stop pill's click: the page consumes it right
    /// after the panel render and fires the bridge's stop wiring.
    pub(crate) fn request_stop(&mut self) {
        self.stop_requested = true;
    }

    /// Take the pending stop request, if any (one-shot handoff to
    /// the bridge's `ai_stop`).
    pub(crate) fn take_stop_request(&mut self) -> Option<bool> {
        self.stop_requested.then(|| {
            self.stop_requested = false;
            true
        })
    }

    /// Record the error card's Configure AI… link click: the page
    /// consumes it right after the panel render and fires the
    /// bridge's config route.
    pub(crate) fn request_configure(&mut self) {
        self.configure_requested = true;
    }

    /// Take the pending configure request, if any (one-shot handoff
    /// to the bridge's `request_ai_config_route`).
    pub(crate) fn take_configure_request(&mut self) -> Option<bool> {
        self.configure_requested.then(|| {
            self.configure_requested = false;
            true
        })
    }

    /// Mark a request in flight (the spinner row shows).
    pub(crate) fn begin_pending(&mut self) {
        self.pending = true;
    }

    /// Whether a request is in flight.
    #[must_use]
    pub(crate) fn is_pending(&self) -> bool {
        self.pending
    }

    /// Start a fresh conversation: turns, draft, pending, and
    /// thumbnail all reset (running a new transform replaces the
    /// thread — spec).
    pub(crate) fn reset(&mut self) {
        self.turns.clear();
        self.draft.clear();
        self.pending = false;
        self.streaming = None;
        self.thumb = None;
        self.outbox = None;
        self.stop_requested = false;
        self.configure_requested = false;
        self.fresh_arrival = false;
    }

    /// Take the submitted follow-up, if any (the send wiring's
    /// handoff).
    pub(crate) fn take_outbox(&mut self) -> Option<String> {
        self.outbox.take()
    }

    /// Set (or clear) the initiating thumbnail.
    pub(crate) fn set_thumb(&mut self, thumb: Option<egui::TextureHandle>) {
        self.thumb = thumb;
    }

    /// The turns, oldest first (assertion accessor for tests).
    pub(crate) fn turns(&self) -> &[AiTurn] {
        &self.turns
    }
}

/// The AI tab's empty-state copy. Pure (the `panel::tool_hint`
/// pattern) so a test can pin it: the entry points it names must
/// exist — the rail spark chip is retired, the topbar AI hero and
/// `I` are the ways in.
fn empty_hint() -> &'static str {
    "Nothing asked yet — press AI in the topbar or hit I"
}

/// Render the AI tab body. Submitted follow-ups land in the
/// conversation's outbox ([`AiConversation::take_outbox`]) for the
/// page's send wiring (Task 007) — the request itself is not this
/// function's business.
pub(crate) fn show(ui: &mut egui::Ui, conversation: &mut AiConversation) {
    let thumb = conversation.thumb.clone();
    let turns = conversation.turns().to_vec();
    let streaming = conversation.streaming_text().map(str::to_owned);
    let fresh_arrival = conversation.take_fresh_arrival();
    let empty = conversation.is_empty() && !conversation.is_pending();
    let pending = conversation.is_pending() && !conversation.is_streaming();

    // The footer draws FIRST as a bottom panel so it reserves its
    // exact height — the scroll area above can never push it (or
    // the Stop pill) below the fold, which the old trailing layout
    // allowed (a full-height auto_shrink scroll area starves later
    // siblings).
    egui::Panel::bottom(egui::Id::new("ai-footer"))
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            ui.add_space(theme::SPACE_SCALE[1]);
            // While a stream is live the footer leads with the Stop
            // pill (spec default: footer row): the gold hero grammar
            // compressed, firing the page's stop wiring on click.
            if conversation.is_streaming() && stop_pill(ui, egui::Id::new("ai-stop")) {
                conversation.request_stop();
            }
            // The follow-up pill always renders (the tab's
            // furniture); it's inert until a conversation exists and
            // no request is pending — a follow-up with no thread has
            // nowhere to go.
            let inert = conversation.is_pending() || conversation.is_empty();
            let mut draft = std::mem::take(&mut conversation.draft);
            let (edit_response, send_clicked) =
                follow_up_row(ui, egui::Id::new("ai-follow-up"), &mut draft, inert);
            let enter_pressed =
                edit_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (send_clicked || enter_pressed) && !draft.trim().is_empty() && !inert {
                conversation.outbox = Some(draft.trim().to_string());
                draft.clear();
            }
            if !edit_response.has_focus() && enter_pressed {
                edit_response.request_focus();
            }
            conversation.draft = draft;
        });

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if empty {
                crate::panel::hint(ui, empty_hint());
                return;
            }
            if let Some(thumb) = thumb.as_ref() {
                thumbnail(ui, thumb);
            }
            for (i, turn) in turns.iter().enumerate() {
                match turn {
                    AiTurn::User { prompt } => user_chip(ui, egui::Id::new(("ai-turn", i)), prompt),
                    AiTurn::Assistant { text } => {
                        // The arrival hairline: only the card a
                        // response just landed into animates.
                        let fresh = fresh_arrival && i + 1 == turns.len();
                        assistant_card(
                            ui,
                            &mut conversation.md_cache,
                            egui::Id::new(("ai-arrival", i)),
                            text,
                            fresh,
                        );
                    }
                    AiTurn::Error { message, configure } => {
                        let link_fired =
                            error_card(ui, egui::Id::new(("ai-turn", i)), message, *configure);
                        if *configure && link_fired {
                            conversation.request_configure();
                        }
                    }
                }
            }
            // The growing card renders where the response card lands;
            // while it grows it replaces the spinner row (the caret
            // is the activity signal).
            if let Some(partial) = &streaming {
                streaming_card(ui, partial);
            } else if pending {
                pending_row(ui);
            }
        });
}

/// The initiating shot as a small paper-shadowed thumbnail.
fn thumbnail(ui: &mut egui::Ui, thumb: &egui::TextureHandle) {
    let size = thumb.size_vec2();
    let max_w = ui.available_width() - theme::SPACE_SCALE[2];
    let scale = (max_w / size.x).min(1.0);
    let shown = size * scale;
    let (rect, _) = ui.allocate_exact_size(shown + egui::vec2(8.0, 8.0), egui::Sense::hover());
    let radius = CornerRadius::same(theme::POD_RADIUS);
    ui.painter().add(theme::SHADOW_PAPER.as_shape(rect, radius));
    ui.painter().rect(
        rect,
        radius,
        theme::palette::PAPER_CREAM,
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    let image_rect = egui::Rect::from_center_size(rect.center(), shown);
    ui.put(image_rect, egui::Image::new((thumb.id(), shown)));
    ui.add_space(theme::SPACE_SCALE[1]);
}

/// One user turn: a quiet cream chip, right-aligned.
fn user_chip(ui: &mut egui::Ui, _id: egui::Id, prompt: &str) {
    let galley = ui.painter().layout(
        prompt.to_owned(),
        egui::FontId::proportional(theme::TYPE_MD),
        theme::palette::INK,
        ui.available_width() - theme::SPACE_SCALE[3],
    );
    let size = galley.size() + egui::vec2(18.0, 12.0);
    let row = ui.available_rect_before_wrap();
    let rect = egui::Rect::from_min_size(egui::pos2(row.right() - size.x, row.top()), size);
    let resp = ui.allocate_rect(rect, Sense::hover());
    ui.painter().rect(
        resp.rect,
        CornerRadius::same(theme::POD_RADIUS),
        theme::palette::PAPER_CREAM,
        theme::HAIRLINE_STROKE,
        egui::StrokeKind::Middle,
    );
    ui.painter().galley(
        egui::pos2(
            resp.rect.left() + 9.0,
            resp.rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        theme::palette::INK,
    );
    ui.add_space(theme::SPACE_SCALE[1]);
}

/// The arrival hairline's thickening window (~300 ms through the
/// animation gate; the gate-off path jumps to the rest width).
const ARRIVAL_SECS: f32 = 0.3;

/// The card's left-edge gold hairline width, 0→1 progress eased.
/// Fresh arrivals start at 0 (a 1 px hairline thickening to 3 px);
/// every other card renders at rest. The animation gate jumps
/// straight to rest.
fn arrival_progress(ctx: &egui::Context, id: egui::Id, fresh: bool) -> f32 {
    if !crate::widgets::animate(ctx) {
        return 1.0;
    }
    let dt = ctx.input(|i| i.stable_dt);
    let t = ctx.data_mut(|d| {
        let start = if fresh { 0.0 } else { 1.0 };
        let current = d.get_temp::<f32>(id).unwrap_or(start);
        let next = theme::ease::approach(current, 1.0, dt, ARRIVAL_SECS);
        d.insert_temp(id, next);
        next
    });
    theme::ease::spring(t)
}

/// One assistant turn: a full-width cream paper card with the
/// markdown-rendered response. A response's arrival paints a gold
/// left-edge hairline that thickens over [`ARRIVAL_SECS`] — the one
/// motion moment; everything else stays still.
fn assistant_card(
    ui: &mut egui::Ui,
    cache: &mut egui_commonmark::CommonMarkCache,
    arrival_id: egui::Id,
    text: &str,
    fresh: bool,
) {
    let resp = egui::Frame::new()
        .fill(theme::palette::PAPER_CREAM)
        .corner_radius(CornerRadius::same(theme::POD_RADIUS))
        .inner_margin(egui::Margin::same(10))
        .stroke(theme::HAIRLINE_STROKE)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            assistant_markdown(ui, cache, text);
        })
        .response;
    let progress = arrival_progress(ui.ctx(), arrival_id, fresh);
    let width = 1.0 + 2.0 * progress;
    let left = resp.rect.left_center();
    ui.painter().line_segment(
        [
            egui::pos2(left.x + width / 2.0, resp.rect.top()),
            egui::pos2(left.x + width / 2.0, resp.rect.bottom()),
        ],
        egui::Stroke::new(width, theme::palette::GOLD),
    );
    ui.add_space(theme::SPACE_SCALE[1]);
}

/// One failed request: a berry-tinted card with the surfaced
/// message. Vision-unsupported rejections additionally offer the
/// **Configure AI…** deep-link (the 09-26 error-card pattern the
/// dialog's Run row uses). Returns whether the link fired (always
/// `false` when `configure` is unset).
fn error_card(ui: &mut egui::Ui, id: egui::Id, message: &str, configure: bool) -> bool {
    let mut link_fired = false;
    egui::Frame::new()
        .fill(theme::palette::PAPER_DIM)
        .corner_radius(CornerRadius::same(theme::POD_RADIUS))
        .inner_margin(egui::Margin::same(10))
        .stroke(egui::Stroke::new(
            1.0,
            theme::palette::with_alpha(theme::palette::BERRY_RED, 160),
        ))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(message)
                    .size(theme::TYPE_SM)
                    .color(theme::palette::INK),
            );
            if configure && configure_link(ui, id.with("configure")) {
                link_fired = true;
            }
        });
    ui.add_space(theme::SPACE_SCALE[1]);
    link_fired
}

/// The error card's **Configure AI…** deep-link: a forest-mid text
/// chip, underlined on hover (the dialog's `configure_link` grammar;
/// per-turn id, since a thread can hold several flagged cards).
fn configure_link(ui: &mut egui::Ui, id: egui::Id) -> bool {
    let label = "Configure AI…";
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(theme::TYPE_SM),
        theme::palette::FOREST_MID,
    );
    let w = 4.0 + galley.size().x + 4.0;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(w, galley.size().y + 6.0), egui::Sense::hover());
    let resp = ui.interact(rect, id, egui::Sense::click());
    let pos = egui::pos2(rect.left() + 4.0, rect.center().y - galley.size().y / 2.0);
    ui.painter()
        .galley(pos, galley.clone(), theme::palette::FOREST_MID);
    if resp.hovered() {
        ui.painter().line_segment(
            [
                egui::pos2(pos.x, pos.y + galley.size().y + 1.0),
                egui::pos2(pos.x + galley.size().x, pos.y + galley.size().y + 1.0),
            ],
            egui::Stroke::new(1.0, theme::palette::FOREST_MID),
        );
    }
    resp.clicked()
}

/// The in-flight stream's growing card: plain wrapped text (the
/// markdown parse waits for the complete text — an unclosed fence
/// mid-stream would render wrong) on the same cream paper as a
/// finished card, with a static painted caret block after the last
/// row. The caret is a painted vector, not a font codepoint (the
/// tofu ban), and never blinks — the pending repaint loop is the
/// only cadence.
fn streaming_card(ui: &mut egui::Ui, partial: &str) {
    egui::Frame::new()
        .fill(theme::palette::PAPER_CREAM)
        .corner_radius(CornerRadius::same(theme::POD_RADIUS))
        .inner_margin(egui::Margin::same(10))
        .stroke(theme::HAIRLINE_STROKE)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let galley = ui.painter().layout(
                partial.to_owned(),
                egui::FontId::proportional(theme::TYPE_MD),
                theme::palette::INK,
                ui.available_width(),
            );
            let (rect, _) =
                ui.allocate_exact_size(galley.size() + egui::vec2(8.0, 0.0), Sense::hover());
            ui.painter()
                .galley(rect.min, galley.clone(), theme::palette::INK);
            // The caret rides the last row's right edge (a trailing
            // newline leaves an empty last row, so it lands at the
            // fresh line's start — exactly where typing continues).
            if let Some(row) = galley.rows.last() {
                let row_rect = row.rect();
                let height = (row_rect.height() - 3.0).max(8.0);
                let caret = egui::Rect::from_min_size(
                    rect.min + egui::vec2(row_rect.right() + 4.0, row_rect.top() + 1.5),
                    egui::vec2(3.0, height),
                );
                ui.painter().rect(
                    caret,
                    CornerRadius::same(1),
                    theme::palette::GOLD,
                    egui::Stroke::NONE,
                    egui::StrokeKind::Middle,
                );
            }
        });
    ui.add_space(theme::SPACE_SCALE[1]);
}

/// The pending row: a quiet spinner (animation-gated — static text
/// when animations are off).
fn pending_row(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if crate::widgets::animate(ui.ctx()) {
            ui.add(egui::Spinner::new().size(16.0));
        } else {
            ui.label(
                RichText::new("···")
                    .size(theme::TYPE_MD)
                    .color(theme::palette::INK_SOFT),
            );
        }
        ui.label(
            RichText::new("Thinking…")
                .size(theme::TYPE_SM)
                .color(theme::palette::INK_SOFT),
        );
    });
}

/// The Stop pill: the gold hero grammar (ghost fill, gold key
/// border, gold-deep on hover) compressed to a compact right-aligned
/// pill with a painted stop square — a vector, never a font
/// codepoint (the tofu ban). Returns whether it fired.
fn stop_pill(ui: &mut egui::Ui, id: egui::Id) -> bool {
    use crate::theme::palette as p;
    let font = egui::FontId::proportional(theme::TYPE_SM);
    let galley = ui
        .painter()
        .layout_no_wrap("Stop".to_owned(), font, p::CREAM_TEXT);
    let glyph = 10.0;
    let pad = 10.0;
    let size = egui::vec2(pad + glyph + 6.0 + galley.size().x + pad, theme::CONTROL_H);
    let row = ui.available_rect_before_wrap();
    let rect = egui::Rect::from_min_size(egui::pos2(row.right() - size.x, row.top()), size);
    let _ = ui.allocate_rect(rect, egui::Sense::hover());
    let resp = ui
        .interact(rect, id, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let border = if resp.hovered() {
        p::GOLD_DEEP
    } else {
        p::GOLD
    };
    let fill = if resp.hovered() {
        p::with_alpha(p::CREAM_TEXT, 36)
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(theme::POD_RADIUS),
        fill,
        egui::Stroke::new(1.5, border),
        egui::StrokeKind::Inside,
    );
    let square = egui::Rect::from_center_size(
        egui::pos2(rect.left() + pad + glyph / 2.0, rect.center().y),
        egui::vec2(glyph, glyph),
    );
    ui.painter().rect(
        square,
        CornerRadius::same(2),
        p::GOLD,
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    ui.painter().galley(
        egui::pos2(
            square.right() + 6.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        p::CREAM_TEXT,
    );
    resp.clicked()
}

/// The follow-up pill: single-line input + send glyph. Disabled
/// while pending. Returns `(edit response, send clicked)`.
fn follow_up_row(
    ui: &mut egui::Ui,
    id: egui::Id,
    draft: &mut String,
    disabled: bool,
) -> (egui::Response, bool) {
    let mut send_clicked = false;
    let height = theme::CONTROL_H;
    let send_w = theme::CONTROL_H;
    let row = ui.horizontal(|ui| {
        let edit = egui::TextEdit::singleline(draft)
            .id(id)
            .desired_width(ui.available_width() - send_w - theme::SPACE_SCALE[1])
            .margin(egui::Margin::symmetric(8, 4))
            .hint_text("Ask a follow-up…")
            .desired_rows(1);
        let resp = ui.add_enabled(!disabled, edit);
        let galley = ui.painter().layout_no_wrap(
            "➤".to_owned(),
            egui::FontId::proportional(theme::TYPE_SM),
            if disabled {
                theme::palette::LOCK_GREY
            } else {
                theme::palette::CREAM_TEXT
            },
        );
        let (rect, _) = ui.allocate_exact_size(egui::vec2(send_w, height), Sense::hover());
        let btn = ui.interact(rect, id.with("send"), Sense::click());
        ui.painter().rect(
            rect,
            CornerRadius::same(theme::POD_RADIUS),
            if disabled {
                theme::palette::PAPER_DIM
            } else {
                theme::palette::FOREST_MID
            },
            egui::Stroke::NONE,
            egui::StrokeKind::Middle,
        );
        ui.painter().galley(
            egui::pos2(
                rect.center().x - galley.size().x / 2.0,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            if disabled {
                theme::palette::LOCK_GREY
            } else {
                theme::palette::CREAM_TEXT
            },
        );
        send_clicked = btn.clicked() && !disabled;
        resp
    });
    (row.inner, send_clicked)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The empty-state copy must name entry points that exist: the
    /// rail's spark chip is retired — the topbar AI hero and the
    /// `I` shortcut are the ways in.
    #[test]
    fn empty_state_copy_names_live_entry_points() {
        let copy = empty_hint();
        assert!(
            copy.contains("topbar") && copy.contains('I'),
            "names the AI hero and the shortcut: {copy}"
        );
        assert!(
            !copy.to_ascii_lowercase().contains("rail"),
            "the retired rail chip must not be named: {copy}"
        );
    }

    #[test]
    fn conversation_starts_empty_and_not_pending() {
        let conversation = AiConversation::default();
        assert!(conversation.is_empty());
        assert!(!conversation.is_pending());
        assert_eq!(conversation.turns(), &[]);
    }

    #[test]
    fn turns_land_in_order_and_clear_pending() {
        let mut conversation = AiConversation::default();
        conversation.push_user("Extract the text");
        conversation.begin_pending();
        assert!(conversation.is_pending());
        conversation.push_assistant("## Extracted");
        assert!(!conversation.is_pending(), "arrival ends the wait");
        conversation.fail_stream("boom");
        assert!(!conversation.is_pending(), "an error also ends the wait");
        assert_eq!(
            conversation.turns(),
            &[
                AiTurn::User {
                    prompt: "Extract the text".into()
                },
                AiTurn::Assistant {
                    text: "## Extracted".into()
                },
                AiTurn::Error {
                    message: "boom".into(),
                    configure: false,
                },
            ]
        );
    }

    #[test]
    fn reset_clears_turns_and_draft() {
        let mut conversation = AiConversation::default();
        conversation.push_user("p");
        conversation.begin_pending();
        conversation.draft = "leftover".into();
        conversation.reset();
        assert!(conversation.is_empty());
        assert!(!conversation.is_pending());
        assert!(conversation.draft.is_empty());
        assert!(conversation.thumb.is_none());
    }

    #[test]
    fn pending_survives_until_a_turn_lands() {
        let mut conversation = AiConversation::default();
        conversation.begin_pending();
        assert!(conversation.is_pending());
        conversation.push_user("queued prompt");
        assert!(
            conversation.is_pending(),
            "a user turn alone does not clear pending"
        );
        conversation.push_assistant("answer");
        assert!(!conversation.is_pending());
    }

    /// The arrival hairline: a fresh arrival starts thin and settles
    /// at rest over ~300 ms; the animation gate jumps straight to
    /// rest.
    #[test]
    fn arrival_hairline_thickens_then_rests_and_the_gate_jumps() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, true);
        let id = egui::Id::new("test-arrival");
        let step = |i: f64, fresh: bool| {
            let raw = egui::RawInput {
                time: Some(i * 0.016),
                predicted_dt: 1.0 / 60.0,
                ..egui::RawInput::default()
            };
            let mut out = 0.0_f32;
            let _ = ctx.run_ui(raw, |ui| {
                out = arrival_progress(ui.ctx(), id, fresh);
            });
            out
        };
        let first = step(0.0, true);
        assert!(first < 0.9, "a fresh arrival starts thin: {first}");
        let mut last = first;
        for i in 1..40 {
            last = step(f64::from(i), false);
        }
        assert!(
            (last - 1.0).abs() < 0.05,
            "the hairline settles at rest: {last}"
        );

        crate::widgets::set_animations(&ctx, false);
        assert!(
            (step(41.0, true) - 1.0).abs() < f32::EPSILON,
            "the gate jumps to rest"
        );
    }

    /// The conversation pins the density bump: user chips at
    /// `TYPE_MD`, the follow-up send chip at `CONTROL_H`.
    #[test]
    fn conversation_pins_the_density_bump() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let mut conversation = AiConversation::default();
        conversation.push_user("ocr it");
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(236.0, 600.0),
            )),
            ..egui::RawInput::default()
        };
        let out = ctx.run_ui(raw.clone(), |ui| {
            egui::Panel::right("test_ai_panel_density")
                .resizable(false)
                .exact_size(236.0)
                .show(ui, |ui| show(ui, &mut conversation));
        });
        let chip_size = out.shapes.iter().find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.job.text == "ocr it" => {
                t.galley.job.sections.first().map(|s| s.format.font_id.size)
            }
            _ => None,
        });
        assert_eq!(
            chip_size,
            Some(theme::TYPE_MD),
            "user chips paint at TYPE_MD"
        );
        if let Some(resp) = ctx.read_response(egui::Id::new("ai-follow-up").with("send")) {
            assert!(
                resp.rect.height() >= theme::CONTROL_H - 0.5,
                "send chip height {}",
                resp.rect.height()
            );
        }
    }

    /// The markdown fixture renders: a heading, a fenced code block,
    /// and a table all paint without panicking inside the panel
    /// width.
    #[test]
    fn assistant_markdown_renders_heading_code_and_table() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let mut conversation = AiConversation::default();
        conversation.push_assistant(
            "## Extracted\n\n- one\n- two\n\n```rust\nlet x = 1;\n```\n\n| Button | Action |\n|---|---|\n| Copy | copies |\n",
        );
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(236.0, 600.0),
            )),
            ..egui::RawInput::default()
        };
        let panel_width = 236.0f32;
        let mut shapes = 0;
        for _ in 0..2 {
            let out = ctx.run_ui(raw.clone(), |ui| {
                egui::Panel::right("test_ai_panel")
                    .resizable(false)
                    .exact_size(panel_width)
                    .show(ui, |ui| show(ui, &mut conversation));
            });
            shapes = out.shapes.len();
        }
        assert!(shapes > 4, "the fixture painted shapes: {shapes}");
        // Two identical render passes don't collide on the shared
        // cache (the per-turn keying criterion's smoke form).
    }

    // --- streaming (Task 006) ---

    fn render_once(ctx: &egui::Context, conversation: &mut AiConversation) -> egui::FullOutput {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(236.0, 600.0),
            )),
            ..egui::RawInput::default()
        };
        ctx.run_ui(raw, |ui| {
            egui::Panel::right("test_ai_panel_streaming")
                .resizable(false)
                .exact_size(236.0)
                .show(ui, |ui| show(ui, conversation));
        })
    }

    fn frame_texts(out: &egui::FullOutput) -> Vec<&str> {
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.job.text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// The streaming caret: a small filled gold rect (a painted
    /// vector — the tofu ban bars a font codepoint) — narrow and
    /// tall, unlike any card fill or hairline stroke.
    fn caret_present(out: &egui::FullOutput) -> bool {
        out.shapes.iter().any(|cs| match &cs.shape {
            egui::Shape::Rect(r) => {
                r.fill == theme::palette::GOLD
                    && (2.0..=5.0).contains(&r.rect.width())
                    && r.rect.height() >= 6.0
            }
            _ => false,
        })
    }

    #[test]
    fn push_delta_appends_to_the_in_progress_card() {
        let mut conversation = AiConversation::default();
        conversation.push_delta("Hel");
        assert_eq!(conversation.streaming_text(), Some("Hel"));
        conversation.push_delta("lo");
        assert_eq!(conversation.streaming_text(), Some("Hello"));
        assert!(conversation.is_streaming());
    }

    #[test]
    fn deltas_grow_the_rendered_text_with_a_static_caret_until_finalize() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let mut conversation = AiConversation::default();
        conversation.push_user("ocr it");
        conversation.begin_pending();
        conversation.push_delta("Hel");
        let first = render_once(&ctx, &mut conversation);
        assert!(
            frame_texts(&first).iter().any(|t| t.contains("Hel")),
            "the first delta renders"
        );
        assert!(caret_present(&first), "the caret paints while streaming");

        conversation.push_delta("lo");
        let second = render_once(&ctx, &mut conversation);
        assert!(
            frame_texts(&second).iter().any(|t| t.contains("Hello")),
            "the rendered text grows across frames"
        );
        assert!(caret_present(&second));

        conversation.finish_stream("Hello world");
        let third = render_once(&ctx, &mut conversation);
        assert!(
            frame_texts(&third)
                .iter()
                .any(|t| t.contains("Hello world")),
            "the finalized card carries the full text"
        );
        assert!(!caret_present(&third), "the caret is gone after finalize");
        assert!(!conversation.is_pending(), "finalize re-arms Run");
    }

    #[test]
    fn finish_stream_finalizes_with_the_authoritative_text() {
        let mut conversation = AiConversation::default();
        conversation.begin_pending();
        conversation.push_delta("local accumulation");
        conversation.finish_stream("authoritative");
        assert_eq!(
            conversation.turns(),
            &[AiTurn::Assistant {
                text: "authoritative".into()
            }],
            "the worker's full text replaces the local accumulation"
        );
        assert!(!conversation.is_streaming());
        assert!(!conversation.is_pending());
    }

    #[test]
    fn failure_mid_stream_keeps_the_partial_and_appends_the_error_card() {
        let mut conversation = AiConversation::default();
        conversation.begin_pending();
        conversation.push_delta("partial answer");
        conversation.fail_stream("The provider rejected the key — replace it in AI settings.");
        assert_eq!(
            conversation.turns(),
            &[
                AiTurn::Assistant {
                    text: "partial answer".into()
                },
                AiTurn::Error {
                    message: "The provider rejected the key — replace it in AI settings.".into(),
                    configure: false,
                },
            ],
            "the partial stays rendered and the error card appends"
        );
        assert!(!conversation.is_streaming());
        assert!(!conversation.is_pending());
    }

    #[test]
    fn response_without_a_prior_delta_lands_as_a_plain_arrival() {
        // The fallback path (non-streaming retry) answers in one
        // shot: identical to today's pre-streaming render.
        let mut conversation = AiConversation::default();
        conversation.begin_pending();
        conversation.finish_stream("whole answer");
        assert_eq!(
            conversation.turns(),
            &[AiTurn::Assistant {
                text: "whole answer".into()
            }]
        );
        assert!(!conversation.is_pending());
    }

    #[test]
    fn pending_spinner_hides_while_the_streaming_card_grows() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let mut conversation = AiConversation::default();
        conversation.push_user("ocr it");
        conversation.begin_pending();
        let waiting = render_once(&ctx, &mut conversation);
        assert!(
            frame_texts(&waiting).iter().any(|t| t.contains("Thinking")),
            "the pending row shows before the first delta"
        );
        conversation.push_delta("text");
        let streaming = render_once(&ctx, &mut conversation);
        assert!(
            !frame_texts(&streaming)
                .iter()
                .any(|t| t.contains("Thinking")),
            "the growing card replaces the spinner row"
        );
    }

    #[test]
    fn reset_clears_the_streaming_state() {
        let mut conversation = AiConversation::default();
        conversation.push_delta("half");
        conversation.reset();
        assert!(!conversation.is_streaming());
        assert_eq!(conversation.streaming_text(), None);
    }

    fn screen_with_click(at: egui::Pos2) -> egui::RawInput {
        egui::RawInput {
            time: Some(0.016),
            events: vec![
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
            ],
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(236.0, 600.0),
            )),
            ..egui::RawInput::default()
        }
    }

    #[test]
    fn stop_pill_registers_only_while_streaming_and_fires_once() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let mut conversation = AiConversation::default();
        conversation.push_user("ocr it");
        conversation.begin_pending();

        // Not streaming yet: no Stop pill registers.
        let _ = render_once(&ctx, &mut conversation);
        assert!(
            ctx.read_response(egui::Id::new("ai-stop")).is_none(),
            "no Stop pill before the first delta"
        );

        conversation.push_delta("text");
        let out = render_once(&ctx, &mut conversation);
        let pill = ctx
            .read_response(egui::Id::new("ai-stop"))
            .expect("the Stop pill registers while streaming")
            .rect;
        assert!(
            frame_texts(&out).iter().any(|t| t.contains("Stop")),
            "the pill paints its label"
        );
        assert!(
            conversation.take_stop_request().is_none(),
            "no stop before the click"
        );

        // Click the pill's center: the stop request fires exactly once.
        let _ = ctx.run_ui(screen_with_click(pill.center()), |ui| {
            egui::Panel::right("test_ai_panel_streaming")
                .resizable(false)
                .exact_size(236.0)
                .show(ui, |ui| show(ui, &mut conversation));
        });
        assert_eq!(conversation.take_stop_request(), Some(true));
        assert_eq!(conversation.take_stop_request(), None, "one-shot");
    }

    #[test]
    fn stop_pill_disappears_after_terminal_events() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);

        let mut done = AiConversation::default();
        done.push_delta("half");
        done.finish_stream("whole");
        let _ = render_once(&ctx, &mut done);
        assert!(
            ctx.read_response(egui::Id::new("ai-stop")).is_none(),
            "gone after Done"
        );

        let mut failed = AiConversation::default();
        failed.push_delta("half");
        failed.fail_stream("boom");
        let _ = render_once(&ctx, &mut failed);
        assert!(
            ctx.read_response(egui::Id::new("ai-stop")).is_none(),
            "gone after Failed"
        );
    }

    // --- vision-unsupported error cards (vision-validate T1) ---

    /// A vision-unsupported failure keeps any streamed partial and
    /// lands an error card carrying the Configure AI… deep-link: the
    /// fix is a model change, so the card routes to settings instead
    /// of suggesting a retry.
    #[test]
    fn vision_unsupported_card_offers_the_configure_deep_link() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let mut conversation = AiConversation::default();
        conversation.begin_pending();
        conversation.push_delta("partial");
        conversation.fail_vision_unsupported(
            "Model `gpt-3.5-turbo` rejected the image — it looks text-only.",
        );
        assert!(
            matches!(
                conversation.turns().last(),
                Some(AiTurn::Error {
                    configure: true,
                    ..
                })
            ),
            "the error turn carries the deep-link flag"
        );

        let out = render_once(&ctx, &mut conversation);
        assert!(
            frame_texts(&out).iter().any(|t| t.contains("text-only")),
            "the copy paints"
        );
        assert!(
            frame_texts(&out).iter().any(|t| t.contains("Configure AI")),
            "the deep-link paints"
        );
        let link = ctx
            .read_response(egui::Id::new(("ai-turn", 1)).with("configure"))
            .expect("the link registers under its per-turn id");
        assert!(
            conversation.take_configure_request().is_none(),
            "no request before the click"
        );

        let _ = ctx.run_ui(screen_with_click(link.rect.center()), |ui| {
            egui::Panel::right("test_ai_panel_streaming")
                .resizable(false)
                .exact_size(236.0)
                .show(ui, |ui| show(ui, &mut conversation));
        });
        assert_eq!(
            conversation.take_configure_request(),
            Some(true),
            "the click arms the configure handoff"
        );
        assert_eq!(conversation.take_configure_request(), None, "one-shot");
    }

    /// Plain errors carry no deep-link — only vision rejections
    /// route to settings.
    #[test]
    fn plain_error_card_has_no_configure_link() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let mut conversation = AiConversation::default();
        conversation.fail_stream("boom");
        assert!(matches!(
            conversation.turns().last(),
            Some(AiTurn::Error {
                configure: false,
                ..
            })
        ));
        let _ = render_once(&ctx, &mut conversation);
        assert!(
            ctx.read_response(egui::Id::new(("ai-turn", 0)).with("configure"))
                .is_none(),
            "no link response for a plain error"
        );
    }
}
