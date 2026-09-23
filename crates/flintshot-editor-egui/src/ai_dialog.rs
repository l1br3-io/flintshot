//! The Ask-the-shot transform dialog — a modal over the canvas.
//!
//! Opened by the rail's spark chip (`crate::rail`) or the `I` key.
//! Pod language: cream paper on a paper shadow, `POD_RADIUS`,
//! `POD_PAD`. Body: the saved-transform pill list (radio-select
//! from the store snapshot), a hairline, the custom-prompt field
//! (typing deselects; selecting clears the prompt — mutual
//! exclusivity), and the gold Run pill. With no provider/model/key
//! configured the Run row is replaced by a **Configure AI…** link
//! (routed by Task 009) — never a dead button.
//!
//! The dialog is an *action*, not editor state: nothing about it
//! sticks (`state.tool` is untouched), and it emits a typed
//! [`AiDialogAction`] the page routes.

use flintshot_core::AiTransform;

use crate::theme;
use egui::{Align2, RichText, Sense};

/// What the dialog asked the page for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AiDialogAction {
    /// Run the transform: flatten the canvas and send the prompt.
    Run {
        /// The effective prompt (custom text wins over the selected
        /// transform's template).
        prompt: String,
    },
    /// Route to the AI config page (unconfigured mode's link).
    Configure,
}

/// The dialog's cross-frame state. Kept over opens? No — every
/// [`AiDialog::open`] resets the selection and the draft (spec Open
/// Question 3: no remembered transform in v1).
#[derive(Default)]
pub(crate) struct AiDialog {
    /// Whether the dialog shows.
    open: bool,
    /// The selected saved transform's id (radio-select).
    selected: Option<String>,
    /// The custom prompt draft.
    prompt: String,
    /// Whether the opening frame should focus the prompt field.
    focus_prompt: bool,
}

impl AiDialog {
    /// Open with fresh state (no remembered transform — spec Open
    /// Question 3's default).
    pub(crate) fn open(&mut self) {
        self.open = true;
        self.selected = None;
        self.prompt.clear();
        self.focus_prompt = true;
    }

    /// Close without side effects (Esc / outside the Run path).
    pub(crate) fn close(&mut self) {
        self.open = false;
    }

    /// Whether the dialog shows.
    #[must_use]
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    /// The id of the selected saved transform, if any (assertion
    /// accessor for tests).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn selected_id(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// The custom prompt draft (assertion accessor for tests).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn prompt(&self) -> &str {
        &self.prompt
    }

    /// The prompt Run would send: custom text when non-empty, else
    /// the selected transform's template. `None` = nothing to run.
    #[must_use]
    pub(crate) fn effective_prompt(&self, transforms: &[AiTransform]) -> Option<String> {
        if !self.prompt.trim().is_empty() {
            return Some(self.prompt.trim().to_string());
        }
        self.selected
            .as_deref()
            .and_then(|id| transforms.iter().find(|t| t.id == id))
            .map(|t| t.prompt.clone())
    }

    /// Selecting a saved transform clears the custom prompt (mutual
    /// exclusivity). Test seam for the pill-click path.
    #[cfg(test)]
    pub(crate) fn select(&mut self, id: &str) {
        self.selected = Some(id.to_string());
        self.prompt.clear();
    }

    /// Render the dialog (no-op while closed). Returns the fired
    /// action, if any; both actions close the dialog.
    pub(crate) fn show(
        &mut self,
        ctx: &egui::Context,
        transforms: &[AiTransform],
        configured: bool,
    ) -> Option<AiDialogAction> {
        if !self.open {
            return None;
        }
        let mut action = None;
        let mut prompt_edit_response = None;
        egui::Area::new(egui::Id::new("ai-dialog"))
            .order(egui::Order::Middle)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .interactable(true)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(theme::palette::PAPER_CREAM)
                    .corner_radius(egui::CornerRadius::same(theme::POD_RADIUS))
                    .inner_margin(egui::Margin::same(theme::POD_PAD))
                    .shadow(theme::SHADOW_PAPER)
                    .show(ui, |ui| {
                        // Pin the width from the first frame: the
                        // pills and hairline size off available_width,
                        // and a center-anchored Area wobbles while
                        // its size estimate changes.
                        ui.set_width(340.0);
                        ui.heading(
                            RichText::new("Ask the shot")
                                .size(theme::TYPE_LG)
                                .color(theme::palette::INK),
                        );
                        ui.add_space(theme::SPACE_SCALE[1]);

                        field_label(ui, "SAVED TRANSFORMS");
                        saved_list(ui, transforms, &mut self.selected);
                        ui.add_space(theme::SPACE_SCALE[1]);
                        hairline(ui);
                        ui.add_space(theme::SPACE_SCALE[1]);
                        field_label(ui, "CUSTOM PROMPT");

                        let mut prompt = std::mem::take(&mut self.prompt);
                        let edit = ui.add(
                            egui::TextEdit::singleline(&mut prompt)
                                .id(egui::Id::new("ai-dialog-prompt"))
                                .hint_text("Type a custom prompt…")
                                .desired_width(ui.available_width()),
                        );
                        if edit.changed() && !prompt.trim().is_empty() {
                            // Typing deselects (mutual exclusivity).
                            self.selected = None;
                        }
                        if self.focus_prompt {
                            edit.request_focus();
                            self.focus_prompt = false;
                        }
                        prompt_edit_response = Some(edit);
                        self.prompt = prompt;

                        ui.add_space(theme::SPACE_SCALE[2]);
                        if configured {
                            let can_run = self.effective_prompt(transforms).is_some();
                            if run_pill(ui, can_run) {
                                if let Some(prompt) = self.effective_prompt(transforms) {
                                    action = Some(AiDialogAction::Run { prompt });
                                }
                            }
                        } else if configure_link(ui) {
                            action = Some(AiDialogAction::Configure);
                        }
                    });
            });

        // Esc closes without side effects (the modal swallows editor
        // shortcuts while open — see `dispatch_shortcut_key`).
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close();
            return None;
        }
        // Enter runs when the prompt field holds the focus (focus
        // lands there on open) and Run is armed — egui's singleline
        // edit blurs on Enter, so the lost_focus + Enter pair is the
        // submit signal (the follow-up pill's pattern).
        if configured
            && prompt_edit_response
                .as_ref()
                .is_some_and(egui::Response::lost_focus)
            && ctx.input(|i| i.key_pressed(egui::Key::Enter))
        {
            if let Some(prompt) = self.effective_prompt(transforms) {
                action = Some(AiDialogAction::Run { prompt });
            }
        }
        if action.is_some() {
            self.close();
        }
        action
    }
}

/// A quiet uppercase field label (the dialog's section grammar).
fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(theme::TYPE_XS)
            .color(theme::palette::INK_SOFT),
    );
    ui.add_space(theme::SPACE_SCALE[0]);
}

/// The saved-transform list: vertical radio pills, or the invitation
/// copy when the store is empty.
fn saved_list(ui: &mut egui::Ui, transforms: &[AiTransform], selected: &mut Option<String>) {
    if transforms.is_empty() {
        ui.label(
            RichText::new(
                "No saved transforms yet — type a prompt below, or create one in AI settings",
            )
            .size(theme::TYPE_SM)
            .color(theme::palette::INK_SOFT),
        );
        return;
    }
    for transform in transforms {
        let is_selected = selected.as_deref() == Some(transform.id.as_str());
        let id = egui::Id::new(("ai-transform", transform.id.clone()));
        let clicked = transform_pill(ui, id, &transform.name, is_selected);
        if clicked {
            *selected = Some(transform.id.clone());
        }
    }
}

/// One saved-transform pill (radio-select).
fn transform_pill(ui: &mut egui::Ui, id: egui::Id, name: &str, selected: bool) -> bool {
    let galley = ui.painter().layout_no_wrap(
        name.to_owned(),
        egui::FontId::proportional(theme::TYPE_SM),
        if selected {
            theme::palette::CREAM_TEXT
        } else {
            theme::palette::INK
        },
    );
    let w = ui.available_width();
    let h = (galley.size().y + 10.0).max(24.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::CUTOUT_RADIUS),
        if selected {
            theme::palette::FOREST_DEEP
        } else {
            theme::palette::PAPER_DIM
        },
        theme::HAIRLINE_STROKE,
        egui::StrokeKind::Middle,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0),
        galley,
        if selected {
            theme::palette::CREAM_TEXT
        } else {
            theme::palette::INK
        },
    );
    resp.clicked()
}

/// The dialog's mid hairline.
fn hairline(ui: &mut egui::Ui) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        theme::HAIRLINE_STROKE,
    );
}

/// The Configure AI… link (unconfigured mode's Run row): a
/// forest-mid text chip, underlined on hover. Returns whether it
/// fired.
fn configure_link(ui: &mut egui::Ui) -> bool {
    let label = "Configure AI…";
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(theme::TYPE_MD),
        theme::palette::FOREST_MID,
    );
    let w = 8.0 + galley.size().x + 8.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 24.0), egui::Sense::hover());
    let resp = ui.interact(
        rect,
        egui::Id::new("ai-configure-link"),
        egui::Sense::click(),
    );
    let pos = egui::pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0);
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

/// The gold Run pill. Returns whether it fired (only when armed).
fn run_pill(ui: &mut egui::Ui, enabled: bool) -> bool {
    let galley = ui.painter().layout_no_wrap(
        "Run".to_owned(),
        egui::FontId::proportional(theme::TYPE_MD),
        if enabled {
            theme::palette::FOREST_INK
        } else {
            theme::palette::LOCK_GREY
        },
    );
    let w = 16.0 + galley.size().x + 16.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 28.0), Sense::hover());
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(rect, egui::Id::new("ai-dialog-run"), sense);
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::POD_RADIUS),
        if enabled {
            theme::palette::GOLD
        } else {
            theme::palette::PAPER_DIM
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
        if enabled {
            theme::palette::FOREST_INK
        } else {
            theme::palette::LOCK_GREY
        },
    );
    resp.clicked() && enabled
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transforms() -> Vec<AiTransform> {
        vec![
            AiTransform {
                id: "t1".into(),
                name: "Extract text".into(),
                prompt: "Extract the text inside this image".into(),
            },
            AiTransform {
                id: "t2".into(),
                name: "Summarize".into(),
                prompt: "Summarize this screenshot".into(),
            },
        ]
    }

    fn frame(
        ctx: &egui::Context,
        dialog: &mut AiDialog,
        transforms: &[AiTransform],
        configured: bool,
        events: Vec<egui::Event>,
    ) -> Option<AiDialogAction> {
        let raw = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1024.0, 768.0),
            )),
            ..egui::RawInput::default()
        };
        let mut action = None;
        let _ = ctx.run_ui(raw, |ui| {
            let ctx = ui.ctx().clone();
            action = dialog.show(&ctx, transforms, configured);
        });
        action
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

    fn center(ctx: &egui::Context, id: egui::Id) -> egui::Pos2 {
        ctx.read_response(id)
            .unwrap_or_else(|| panic!("{id:?} registers"))
            .rect
            .center()
    }

    /// A center-anchored Area settles over its first three frames
    /// (egui positions from the previous frame's measured size, and
    /// there is none yet). Click tests read widget rects only after
    /// clean settle passes.
    fn settle(
        ctx: &egui::Context,
        dialog: &mut AiDialog,
        transforms: &[AiTransform],
        configured: bool,
    ) {
        for _ in 0..3 {
            let _ = frame(ctx, dialog, transforms, configured, vec![]);
        }
    }

    fn esc_events() -> Vec<egui::Event> {
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]
    }

    #[test]
    fn open_resets_state_and_close_is_side_effect_free() {
        let mut dialog = AiDialog::default();
        assert!(!dialog.is_open());
        dialog.open();
        dialog.select("t1");
        dialog.prompt = "draft".into();
        dialog.close();
        assert!(!dialog.is_open());
        dialog.open();
        assert!(dialog.is_open());
        assert_eq!(dialog.selected_id(), None, "open resets the selection");
        assert!(dialog.prompt().is_empty(), "open resets the draft");
    }

    #[test]
    fn effective_prompt_prefers_custom_then_selected_template() {
        let mut dialog = AiDialog::default();
        dialog.open();
        assert_eq!(dialog.effective_prompt(&transforms()), None);

        dialog.select("t1");
        assert_eq!(
            dialog.effective_prompt(&transforms()),
            Some("Extract the text inside this image".into())
        );

        dialog.prompt = "  custom  ".into();
        assert_eq!(
            dialog.effective_prompt(&transforms()),
            Some("custom".into()),
            "custom text wins and trims"
        );
    }

    #[test]
    fn select_clears_the_prompt_mutual_exclusivity() {
        let mut dialog = AiDialog::default();
        dialog.open();
        dialog.prompt = "draft".into();
        dialog.select("t1");
        assert!(dialog.prompt().is_empty());
    }

    #[test]
    fn closed_dialog_renders_nothing_and_emits_nothing() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut dialog = AiDialog::default();
        let action = frame(&ctx, &mut dialog, &transforms(), true, vec![]);
        assert!(action.is_none());
        assert!(ctx.read_response(egui::Id::new("ai-dialog-run")).is_none());
    }

    #[test]
    fn pill_click_selects_run_fires_with_the_template() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut dialog = AiDialog::default();
        dialog.open();
        let transforms = transforms();
        settle(&ctx, &mut dialog, &transforms, true);
        let pill = center(&ctx, egui::Id::new(("ai-transform", "t1")));
        let _ = frame(&ctx, &mut dialog, &transforms, true, click_events(pill));
        assert_eq!(dialog.selected_id(), Some("t1"));

        let run = center(&ctx, egui::Id::new("ai-dialog-run"));
        let action = frame(&ctx, &mut dialog, &transforms, true, click_events(run));
        assert_eq!(
            action,
            Some(AiDialogAction::Run {
                prompt: "Extract the text inside this image".into()
            })
        );
        assert!(!dialog.is_open(), "Run closes the dialog");
    }

    #[test]
    fn disabled_run_when_nothing_active() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut dialog = AiDialog::default();
        dialog.open();
        let transforms = transforms();
        settle(&ctx, &mut dialog, &transforms, true);
        let run = center(&ctx, egui::Id::new("ai-dialog-run"));
        let action = frame(&ctx, &mut dialog, &transforms, true, click_events(run));
        assert!(action.is_none(), "nothing selected/prompted — no Run");
        assert!(dialog.is_open(), "the dialog stays open");
    }

    #[test]
    fn unconfigured_mode_shows_configure_link_instead_of_run() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut dialog = AiDialog::default();
        dialog.open();
        let transforms = transforms();
        let _ = frame(&ctx, &mut dialog, &transforms, false, vec![]);
        assert!(
            ctx.read_response(egui::Id::new("ai-dialog-run")).is_none(),
            "no dead Run in unconfigured mode"
        );
        assert!(
            ctx.read_response(egui::Id::new("ai-dialog-prompt"))
                .is_some(),
            "the prompt field still renders"
        );
    }

    #[test]
    fn configure_link_click_fires_configure() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut dialog = AiDialog::default();
        dialog.open();
        let transforms = transforms();
        settle(&ctx, &mut dialog, &transforms, false);
        let link = ctx
            .read_response(egui::Id::new("ai-configure-link"))
            .expect("Configure link registers under its pinned id");
        let action = frame(
            &ctx,
            &mut dialog,
            &transforms,
            false,
            click_events(link.rect.center()),
        );
        assert_eq!(action, Some(AiDialogAction::Configure));
        assert!(!dialog.is_open());
    }

    #[test]
    fn empty_store_shows_the_invitation_copy() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut dialog = AiDialog::default();
        dialog.open();
        let empty = Vec::new();
        let _ = frame(&ctx, &mut dialog, &empty, true, vec![]);
        assert!(
            ctx.read_response(egui::Id::new(("ai-transform", "t1")))
                .is_none(),
            "no transform pills with an empty store"
        );
        assert!(
            ctx.read_response(egui::Id::new("ai-dialog-prompt"))
                .is_some(),
            "the prompt field renders"
        );
    }

    #[test]
    fn esc_closes_without_side_effects() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut dialog = AiDialog::default();
        dialog.open();
        let transforms = transforms();
        let _ = frame(&ctx, &mut dialog, &transforms, true, vec![]);
        let pill = center(&ctx, egui::Id::new(("ai-transform", "t1")));
        let _ = frame(&ctx, &mut dialog, &transforms, true, click_events(pill));
        let action = frame(&ctx, &mut dialog, &transforms, true, esc_events());
        assert!(action.is_none(), "Esc never fires an action");
        assert!(!dialog.is_open());
    }
}
