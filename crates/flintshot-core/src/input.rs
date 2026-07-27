//! Toolkit-agnostic input model.
//!
//! The shell (e.g. `flintshot-editor-egui`) translates its toolkit's
//! native input events into [`InputEvent`]s and feeds them to
//! [`EditorState::dispatch_input`](crate::EditorState::dispatch_input).
//! The dispatcher mutates state and returns a [`DispatchOutcome`]
//! telling the shell whether to perform a side effect (save, copy,
//! cancel). This keeps the input model pure and unit-testable: no
//! `egui::Context` or `eframe` types reach this module.
//!
//! ## Why this lives in core
//!
//! The Task 5 / 7 / 8 close-outs flagged that `EditorApp::logic`
//! wasn't unit-testable because it depends on `egui::Context::input`.
//! Task 10 extracts the dispatch logic into this pure module so a
//! future `iced` / `slint` shell can reuse the same dispatcher and
//! the same test suite, and so the dispatcher's contract is enforced
//! by tests rather than by interactive verification alone.

use crate::geom::{Key, Point};

/// Modifier key state at the time of an input event.
///
/// All fields default to `false`; the `Default` implementation is the
/// "no modifiers" state. Shells construct this from their toolkit's
/// native modifier state (e.g. `egui::Modifiers` → `Modifiers`) at the
/// event boundary.
//
// `clippy::struct_excessive_bools` wants a state machine or
// two-variant enums when a struct grows past three bools. That advice
// fits option-like toggles where combinations are restricted; it does
// not fit modifier keys, which are independent bits by definition
// (Ctrl+Shift is not a third state, it's the union of Ctrl and Shift).
// The same shape is used by `egui::Modifiers`, `winit::ModifiersState`,
// and `InputModifiers` in `bevy_input`. Mirroring the ecosystem keeps
// the shell-side translation lossless and predictable.
#[allow(
    clippy::struct_excessive_bools,
    reason = "modifier keys are independent bits, not option-like states"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Control key (Cmd on macOS — macOS Cmd/Ctrl translation is the
    /// shell's responsibility; this field is whatever the shell maps
    /// to "the primary chord modifier").
    pub ctrl: bool,
    /// Shift key.
    pub shift: bool,
    /// Alt / Option key.
    pub alt: bool,
    /// Super / Cmd / Windows key.
    pub super_key: bool,
}

impl Modifiers {
    /// Only Ctrl held (no Shift / Alt / Super).
    ///
    /// Convenience predicate for the common shortcut shape
    /// (`Ctrl+Z`, `Ctrl+S`, etc.).
    #[must_use]
    pub fn ctrl_only(self) -> bool {
        self.ctrl && !self.shift && !self.alt && !self.super_key
    }

    /// Ctrl + Shift held (no Alt / Super).
    ///
    /// Covers the `Ctrl+Shift+Z` (redo) chord.
    #[must_use]
    pub fn ctrl_shift(self) -> bool {
        self.ctrl && self.shift && !self.alt && !self.super_key
    }
}

/// One toolkit-agnostic input event.
///
/// Shells translate their native pointer / keyboard events into this
/// enum at the event boundary. Positions are in **canvas-physical
/// pixels** — the shell performs any `pixels_per_point` scaling before
/// constructing the event.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum InputEvent {
    /// A key was pressed this frame. Repeat events (key held) are the
    /// shell's responsibility to filter out — the dispatcher treats
    /// every `Key` event as a fresh press.
    Key {
        /// Which key.
        key: Key,
        /// Modifier state at press time.
        modifiers: Modifiers,
    },
    /// Pointer (mouse / touch) primary button went down at `pos`.
    PointerDown {
        /// Position in canvas-physical px.
        pos: Point,
    },
    /// Pointer moved while the primary button is held.
    PointerDrag {
        /// Position in canvas-physical px.
        pos: Point,
    },
    /// Pointer primary button was released.
    ///
    /// Carries no position: the dispatcher commits with the
    /// annotation's last-drag geometry, matching the contract the
    /// user sees during the drag ("what got rendered is what gets
    /// committed"). The release position itself is ignored — see
    /// `EditorState::dispatch_input` for details.
    PointerUp,
    /// A printable character was typed (text input).
    ///
    /// Only meaningful while a text draft is active (see
    /// [`EditorState::is_editing_text`]); the dispatcher drops it
    /// outside text editing. Shells translate their toolkit's text
    /// input events (egui `Event::Text`) into this variant at the
    /// event boundary — `Key::Char(c)` is reserved for key-press
    /// detection (shortcut matching), not text input, so the two
    /// paths don't collide when the user types an `s` mid-edit.
    TextInput(char),
}

/// Side-effect the shell should perform after dispatching an event.
///
/// The dispatcher in core can mutate `EditorState` freely but cannot
/// perform toolkit-specific work (open a file dialog, write to the
/// clipboard, close the window). Instead it returns one of these
/// outcomes; the shell matches on it and runs the corresponding
/// pipeline.
///
/// Order of precedence when multiple events arrive in one frame is
/// the shell's responsibility — typically "first non-`Continue`
/// wins", since e.g. a `Ctrl+S` mid-drag should pre-empt a subsequent
/// pointer-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum DispatchOutcome {
    /// Input consumed; no further action required from the shell.
    #[default]
    Continue,
    /// User requested cancel (`Esc`) — publish
    /// `EditorOutcome::Cancelled` and close the window.
    Cancel,
    /// User requested save (`Ctrl+S`) — invoke the save pipeline
    /// (`save_to_disk`) and close on success.
    Save,
    /// User requested copy (`Ctrl+C`) — invoke the copy pipeline
    /// (`copy_to_clipboard`) and close on success.
    Copy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_default_is_none() {
        let m = Modifiers::default();
        assert!(!m.ctrl);
        assert!(!m.shift);
        assert!(!m.alt);
        assert!(!m.super_key);
        assert!(!m.ctrl_only());
        assert!(!m.ctrl_shift());
    }

    #[test]
    fn modifiers_ctrl_only_distinguishes_pure_ctrl() {
        assert!(Modifiers {
            ctrl: true,
            ..Modifiers::default()
        }
        .ctrl_only());
        assert!(!Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::default()
        }
        .ctrl_only());
        assert!(!Modifiers {
            ctrl: true,
            alt: true,
            ..Modifiers::default()
        }
        .ctrl_only());
        assert!(!Modifiers {
            ctrl: true,
            super_key: true,
            ..Modifiers::default()
        }
        .ctrl_only());
    }

    #[test]
    fn modifiers_ctrl_shift_requires_both() {
        assert!(Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::default()
        }
        .ctrl_shift());
        assert!(!Modifiers {
            ctrl: true,
            ..Modifiers::default()
        }
        .ctrl_shift());
        assert!(!Modifiers {
            shift: true,
            ..Modifiers::default()
        }
        .ctrl_shift());
        // Alt or Super spoils the chord.
        assert!(!Modifiers {
            ctrl: true,
            shift: true,
            alt: true,
            super_key: false
        }
        .ctrl_shift());
    }

    #[test]
    fn dispatch_outcome_default_is_continue() {
        assert_eq!(DispatchOutcome::default(), DispatchOutcome::Continue);
    }
}
