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

// Task 007 (bridge + worker pipeline) constructs turns from real
// responses and uploads the thumbnail; until then the push_* /
// set_thumb surface is only exercised by unit tests. Allowed here so
// the CI dead-code gate stays green. Remove when Task 007 lands.
#![allow(dead_code)]

use crate::theme;
use egui::{CornerRadius, RichText, Sense};

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
    /// Whether a request is in flight (the quiet spinner row).
    pending: bool,
    /// The follow-up input's draft text.
    draft: String,
    /// The initiating shot's thumbnail texture.
    thumb: Option<egui::TextureHandle>,
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
    /// response arrival always ends the wait.
    pub(crate) fn push_assistant(&mut self, text: impl Into<String>) {
        self.turns.push(AiTurn::Assistant { text: text.into() });
        self.pending = false;
    }

    /// Append an error card and clear the pending flag — a surfaced
    /// failure also ends the wait.
    pub(crate) fn push_error(&mut self, message: impl Into<String>) {
        self.turns.push(AiTurn::Error {
            message: message.into(),
        });
        self.pending = false;
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
        self.thumb = None;
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

/// Render the AI tab body. Returns the submitted follow-up text when
/// the send affordance fires (Enter or the send glyph) with a
/// non-empty draft; send *wiring* (the request itself) is Task 007.
pub(crate) fn show(ui: &mut egui::Ui, conversation: &mut AiConversation) -> Option<String> {
    let mut submitted = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if conversation.is_empty() && !conversation.is_pending() {
                crate::panel::hint(ui, "Nothing asked yet — press the spark in the rail");
                return;
            }
            if let Some(thumb) = conversation.thumb.as_ref() {
                thumbnail(ui, thumb);
            }
            let turns = conversation.turns.clone();
            for (i, turn) in turns.iter().enumerate() {
                match turn {
                    AiTurn::User { prompt } => user_chip(ui, egui::Id::new(("ai-turn", i)), prompt),
                    AiTurn::Assistant { text } => {
                        assistant_card(ui, egui::Id::new(("ai-turn", i)), text);
                    }
                    AiTurn::Error { message } => {
                        error_card(ui, egui::Id::new(("ai-turn", i)), message);
                    }
                }
            }
            if conversation.is_pending() {
                pending_row(ui);
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
        submitted = Some(draft.trim().to_string());
        draft.clear();
    }
    if !edit_response.has_focus() && enter_pressed {
        edit_response.request_focus();
    }
    conversation.draft = draft;
    submitted
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

/// One assistant turn: a full-width cream paper card.
fn assistant_card(ui: &mut egui::Ui, id: egui::Id, text: &str) {
    egui::Frame::new()
        .fill(theme::palette::PAPER_CREAM)
        .corner_radius(CornerRadius::same(theme::POD_RADIUS))
        .inner_margin(egui::Margin::same(10))
        .stroke(theme::HAIRLINE_STROKE)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let _ = id;
            ui.label(
                RichText::new(text)
                    .size(theme::TYPE_SM)
                    .color(theme::palette::INK),
            );
        });
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
        conversation.begin_pending();
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
}
