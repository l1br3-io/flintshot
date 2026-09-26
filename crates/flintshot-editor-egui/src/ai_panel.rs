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
    },
}

/// The AI tab's page-owned conversation state.
///
/// In-memory only — conversation history never persists to disk
/// (spec boundary). The thumbnail handle uploads when a transform
/// run starts (Task 007); `None` before the first run.
#[derive(Default)]
pub(crate) struct AiConversation {
    /// The conversation so far, oldest first.
    turns: Vec<AiTurn>,
    /// Whether a request is in flight (the spinner row / growing
    /// card).
    pending: bool,
    /// The in-flight stream's text so far — appended per token,
    /// committed as the assistant turn on `Done` (or kept as partial
    /// text on failure).
    streaming: String,
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

    /// Append an error card and clear the pending flag — a surfaced
    /// failure also ends the wait.
    pub(crate) fn push_error(&mut self, message: impl Into<String>) {
        self.turns.push(AiTurn::Error {
            message: message.into(),
        });
        self.pending = false;
    }

    /// Mark a request in flight and arm a fresh streaming buffer (a
    /// stale one from an interrupted exchange never bleeds into the
    /// next card).
    pub(crate) fn begin_stream(&mut self) {
        self.pending = true;
        self.streaming.clear();
    }

    /// Whether a request is in flight.
    #[must_use]
    pub(crate) fn is_pending(&self) -> bool {
        self.pending
    }

    /// Append one streamed token to the growing card.
    pub(crate) fn push_token(&mut self, token: impl Into<String>) {
        self.streaming.push_str(&token.into());
    }

    /// The in-flight stream's text so far ("" before the first
    /// token — the spinner row's gate).
    #[must_use]
    pub(crate) fn streaming_text(&self) -> &str {
        &self.streaming
    }

    /// Commit the streamed text as the assistant turn (the `Done`
    /// event): pending clears and the arrival hairline arms — the
    /// one motion moment fires at stream end, not per token.
    pub(crate) fn finish_stream(&mut self) {
        let text = std::mem::take(&mut self.streaming);
        self.push_assistant(text);
    }

    /// Land a failure: partial text received so far commits as an
    /// assistant card first (Stop semantics — it stays), then the
    /// error card carries the actionable copy.
    pub(crate) fn fail_stream(&mut self, message: impl Into<String>) {
        let partial = std::mem::take(&mut self.streaming);
        if !partial.is_empty() {
            self.push_assistant(partial);
        }
        self.push_error(message);
    }

    /// Start a fresh conversation: turns, draft, pending, and
    /// thumbnail all reset (running a new transform replaces the
    /// thread — spec).
    pub(crate) fn reset(&mut self) {
        self.turns.clear();
        self.draft.clear();
        self.pending = false;
        self.streaming.clear();
        self.thumb = None;
        self.outbox = None;
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

/// Render the AI tab body. Submitted follow-ups land in the
/// conversation's outbox ([`AiConversation::take_outbox`]) for the
/// page's send wiring (Task 007) — the request itself is not this
/// function's business.
pub(crate) fn show(ui: &mut egui::Ui, conversation: &mut AiConversation) {
    let thumb = conversation.thumb.clone();
    let turns = conversation.turns().to_vec();
    let streaming_text = conversation.streaming_text().to_owned();
    let fresh_arrival = conversation.take_fresh_arrival();
    let empty = conversation.is_empty() && !conversation.is_pending();
    let pending = conversation.is_pending();
    let md_cache = &mut conversation.md_cache;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if empty {
                crate::panel::hint(ui, "Nothing asked yet — press the spark in the rail");
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
                        assistant_card(ui, md_cache, egui::Id::new(("ai-arrival", i)), text, fresh);
                    }
                    AiTurn::Error { message } => {
                        error_card(ui, egui::Id::new(("ai-turn", i)), message);
                    }
                }
            }
            if pending {
                if streaming_text.is_empty() {
                    pending_row(ui);
                } else {
                    // The growing card: tokens render in place at the
                    // repaint cadence; the hairline stays at rest
                    // until `Done` commits (fresh_arrival arms then).
                    assistant_card(
                        ui,
                        md_cache,
                        egui::Id::new("ai-streaming-card"),
                        &streaming_text,
                        false,
                    );
                    // Card height grows per token — keep the bottom
                    // edge visible so the newest text never shifts
                    // out of view.
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                }
            }
        });

    ui.add_space(theme::SPACE_SCALE[1]);
    // The follow-up pill always renders (the tab's furniture); it's
    // inert until a conversation exists and no request is pending —
    // a follow-up with no thread has nowhere to go.
    let inert = conversation.is_pending() || conversation.is_empty();
    let mut draft = std::mem::take(&mut conversation.draft);
    let (edit_response, send_clicked) =
        follow_up_row(ui, egui::Id::new("ai-follow-up"), &mut draft, inert);
    let enter_pressed = edit_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if (send_clicked || enter_pressed) && !draft.trim().is_empty() && !inert {
        conversation.outbox = Some(draft.trim().to_string());
        draft.clear();
    }
    if !edit_response.has_focus() && enter_pressed {
        edit_response.request_focus();
    }
    conversation.draft = draft;
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
        egui::FontId::proportional(theme::TYPE_SM),
        theme::palette::INK,
        ui.available_width() - theme::SPACE_SCALE[3],
    );
    let size = galley.size() + egui::vec2(16.0, 10.0);
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
            resp.rect.left() + 8.0,
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
/// message.
fn error_card(ui: &mut egui::Ui, id: egui::Id, message: &str) {
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
            let _ = id;
            ui.label(
                RichText::new(message)
                    .size(theme::TYPE_SM)
                    .color(theme::palette::INK),
            );
        });
    ui.add_space(theme::SPACE_SCALE[1]);
}

/// The pending row: a quiet spinner (animation-gated — static text
/// when animations are off).
fn pending_row(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if crate::widgets::animate(ui.ctx()) {
            ui.add(egui::Spinner::new().size(14.0));
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

/// The follow-up pill: single-line input + send glyph. Disabled
/// while pending. Returns `(edit response, send clicked)`.
fn follow_up_row(
    ui: &mut egui::Ui,
    id: egui::Id,
    draft: &mut String,
    disabled: bool,
) -> (egui::Response, bool) {
    let mut send_clicked = false;
    let height = 26.0;
    let send_w = 30.0;
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
        conversation.begin_stream();
        assert!(conversation.is_pending());
        conversation.push_assistant("## Extracted");
        assert!(!conversation.is_pending(), "arrival ends the wait");
        conversation.push_error("boom");
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
                    message: "boom".into()
                },
            ]
        );
    }

    #[test]
    fn reset_clears_turns_and_draft() {
        let mut conversation = AiConversation::default();
        conversation.push_user("p");
        conversation.begin_stream();
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
        conversation.begin_stream();
        assert!(conversation.is_pending());
        conversation.push_user("queued prompt");
        assert!(
            conversation.is_pending(),
            "a user turn alone does not clear pending"
        );
        conversation.push_assistant("answer");
        assert!(!conversation.is_pending());
    }

    #[test]
    fn tokens_stream_into_a_growing_card_and_done_commits() {
        let mut conversation = AiConversation::default();
        conversation.push_user("extract");
        conversation.begin_stream();
        assert!(conversation.is_pending());
        assert_eq!(conversation.streaming_text(), "");
        conversation.push_token("Hel");
        conversation.push_token("lo");
        assert_eq!(conversation.streaming_text(), "Hello");
        // No assistant turn lands before Done.
        assert!(matches!(
            conversation.turns().last(),
            Some(AiTurn::User { .. })
        ));
        conversation.finish_stream();
        assert!(!conversation.is_pending());
        assert_eq!(
            conversation.streaming_text(),
            "",
            "the buffer drains on commit"
        );
        let Some(AiTurn::Assistant { text }) = conversation.turns().last() else {
            panic!("Done commits the streaming text as the assistant turn");
        };
        assert_eq!(text, "Hello");
        assert!(
            conversation.take_fresh_arrival(),
            "the arrival hairline arms at stream end, not per token"
        );
    }

    #[test]
    fn failure_keeps_partial_text_then_lands_the_error_card() {
        let mut conversation = AiConversation::default();
        conversation.begin_stream();
        conversation.push_token("partial");
        conversation.fail_stream("the copy");
        assert!(!conversation.is_pending());
        let turns = conversation.turns();
        assert!(matches!(&turns[0], AiTurn::Assistant { text } if text == "partial"));
        assert!(matches!(&turns[1], AiTurn::Error { message } if message == "the copy"));

        // No partial text: just the error card, no empty assistant.
        let mut conversation = AiConversation::default();
        conversation.begin_stream();
        conversation.fail_stream("boom");
        assert_eq!(conversation.turns().len(), 1);
        assert!(matches!(
            conversation.turns().last(),
            Some(AiTurn::Error { .. })
        ));
    }

    #[test]
    fn begin_stream_resets_stale_streaming_text() {
        let mut conversation = AiConversation::default();
        conversation.begin_stream();
        conversation.push_token("leftover");
        conversation.finish_stream();
        conversation.begin_stream();
        assert_eq!(conversation.streaming_text(), "");
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
}
