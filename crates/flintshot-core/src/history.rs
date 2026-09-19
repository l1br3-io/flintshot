//! Edit-kind labels for the undo history timeline.
//!
//! Every [`Snapshot`](crate::editor::Snapshot) carries the
//! [`EditKind`] of the gesture that pushed it, so the History tab
//! (and anything else surfacing the timeline) can label each entry
//! without re-deriving intent from the snapshot contents. The enum
//! is `Copy` and matched exhaustively at every push site — a new
//! variant is a compile error everywhere it must be labeled.

use std::fmt;

use crate::editor::Tool;
use crate::inspector::PropertyEditKind;

/// What kind of edit pushed a snapshot onto `undo`.
///
/// Labels render via [`fmt::Display`] (`Added arrow`, `Moved 3
/// items`, `Cropped`, …). [`EditKind::Generic`] is the fallback for
/// non-gesture pushes (`restore_to` internals, test fixtures).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditKind {
    /// An annotation was committed by `tool`.
    Add {
        /// The tool that owns (produced) the annotation.
        tool: Tool,
    },
    /// A numbered bubble was placed (`place_number`).
    AddNumber,
    /// A move gesture translated `count` selected annotations.
    Move {
        /// How many annotations the gesture moved.
        count: usize,
    },
    /// A delete removed `count` selected annotations.
    Delete {
        /// How many annotations were removed.
        count: usize,
    },
    /// The image was cropped.
    Crop,
    /// A text re-edit session committed a content change.
    EditText,
    /// A restyle gesture changed a text annotation's style.
    RestyleText,
    /// An inspector property burst of `kind`.
    Property {
        /// The property section the burst edited.
        kind: PropertyEditKind,
    },
    /// Unlabeled push (`restore_to` internals, test fixtures).
    Generic,
}

impl fmt::Display for EditKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add { tool } => write!(f, "Added {}", tool_noun(*tool)),
            Self::AddNumber => f.write_str("Added number"),
            Self::Move { count } => write!(f, "Moved {} {}", count, item_word(*count)),
            Self::Delete { count } => write!(f, "Deleted {} {}", count, item_word(*count)),
            Self::Crop => f.write_str("Cropped"),
            Self::EditText => f.write_str("Edited text"),
            Self::RestyleText => f.write_str("Text style"),
            Self::Property { kind } => f.write_str(property_name(*kind)),
            Self::Generic => f.write_str("Edit"),
        }
    }
}

/// Singular/plural "item(s)" for count-bearing labels.
fn item_word(count: usize) -> &'static str {
    if count == 1 {
        "item"
    } else {
        "items"
    }
}

/// The user-facing noun for an annotation owned by `tool`.
///
/// Select / Crop / Hand own no annotations; the defensive noun keeps
/// the match exhaustive when a future tool lands without a label.
fn tool_noun(tool: Tool) -> &'static str {
    match tool {
        Tool::Select | Tool::Crop | Tool::Hand => "annotation",
        Tool::Rect => "rect",
        Tool::Ellipse => "ellipse",
        Tool::Line => "line",
        Tool::Arrow => "arrow",
        Tool::Marker => "marker",
        Tool::Pencil => "pencil",
        Tool::Text => "text",
        Tool::Blur => "blur",
        Tool::Number => "number",
    }
}

/// The inspector section's display name.
fn property_name(kind: PropertyEditKind) -> &'static str {
    match kind {
        PropertyEditKind::Color => "Colour",
        PropertyEditKind::StrokeWidth => "Stroke width",
        PropertyEditKind::Fill => "Fill",
        PropertyEditKind::TextStyle => "Text style",
        PropertyEditKind::Blur => "Blur",
        PropertyEditKind::NumberSize => "Number size",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Display` yields the spec's exact row labels.
    #[test]
    fn display_labels_match_spec() {
        assert_eq!(
            EditKind::Add { tool: Tool::Arrow }.to_string(),
            "Added arrow"
        );
        assert_eq!(EditKind::Add { tool: Tool::Rect }.to_string(), "Added rect");
        assert_eq!(
            EditKind::Add {
                tool: Tool::Ellipse
            }
            .to_string(),
            "Added ellipse"
        );
        assert_eq!(EditKind::Add { tool: Tool::Line }.to_string(), "Added line");
        assert_eq!(
            EditKind::Add { tool: Tool::Marker }.to_string(),
            "Added marker"
        );
        assert_eq!(
            EditKind::Add { tool: Tool::Pencil }.to_string(),
            "Added pencil"
        );
        assert_eq!(EditKind::Add { tool: Tool::Text }.to_string(), "Added text");
        assert_eq!(EditKind::Add { tool: Tool::Blur }.to_string(), "Added blur");
        assert_eq!(
            EditKind::Add { tool: Tool::Number }.to_string(),
            "Added number"
        );
        assert_eq!(EditKind::AddNumber.to_string(), "Added number");
        assert_eq!(EditKind::Move { count: 3 }.to_string(), "Moved 3 items");
        assert_eq!(EditKind::Move { count: 1 }.to_string(), "Moved 1 item");
        assert_eq!(EditKind::Delete { count: 2 }.to_string(), "Deleted 2 items");
        assert_eq!(EditKind::Crop.to_string(), "Cropped");
        assert_eq!(EditKind::EditText.to_string(), "Edited text");
        assert_eq!(EditKind::RestyleText.to_string(), "Text style");
        assert_eq!(
            EditKind::Property {
                kind: PropertyEditKind::Color
            }
            .to_string(),
            "Colour"
        );
        assert_eq!(
            EditKind::Property {
                kind: PropertyEditKind::StrokeWidth
            }
            .to_string(),
            "Stroke width"
        );
        assert_eq!(
            EditKind::Property {
                kind: PropertyEditKind::Fill
            }
            .to_string(),
            "Fill"
        );
        assert_eq!(
            EditKind::Property {
                kind: PropertyEditKind::TextStyle
            }
            .to_string(),
            "Text style"
        );
        assert_eq!(
            EditKind::Property {
                kind: PropertyEditKind::Blur
            }
            .to_string(),
            "Blur"
        );
        assert_eq!(
            EditKind::Property {
                kind: PropertyEditKind::NumberSize
            }
            .to_string(),
            "Number size"
        );
        assert_eq!(EditKind::Generic.to_string(), "Edit");
    }
}
