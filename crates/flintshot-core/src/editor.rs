//! The annotation data model and editor state machine.
//!
//! All types use [`crate::geom`] primitives, never GUI-toolkit types.
//! Phase 0 ships the type definitions; Phase 1 implements hit-testing,
//! undo/redo, and tool-specific state transitions.

use serde::{Deserialize, Serialize};

use crate::geom::{Color, Point, Rect, Stroke};

/// Available annotation tools.
///
/// `Default` is [`Tool::Select`] — the editor opens on the
/// object-selection tool; `flintshot-app` overrides this from
/// config. Tools are sticky across commits by default
/// (see [`EditorState::flip_to_select`]); flipping to Select after a
/// commit is opt-in via `[ui] flip-to-select`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Tool {
    /// Object selection: click / marquee / drag-move annotations.
    #[default]
    Select,
    /// Region crop: drag a region, then Enter or the Crop button
    /// narrows the image to it.
    Crop,
    /// Pan the view (the photo-editor hand). Presses are shell-owned
    /// and never dispatched; core no-ops them defensively.
    Hand,
    /// Rectangle outline.
    Rect,
    /// Ellipse outline.
    Ellipse,
    /// Straight line.
    Line,
    /// Arrow with barbs.
    Arrow,
    /// Marker (semi-transparent thick stroke).
    Marker,
    /// Freehand pencil.
    Pencil,
    /// Text label.
    Text,
    /// Blur / pixelate region.
    Blur,
    /// Auto-incrementing numbered bubble.
    Number,
}

/// Distinguishes Gaussian blur from pixelate. Computed in core, emitted
/// to the [`crate::Canvas`] as an image blit so output is identical
/// across toolkits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BlurKind {
    /// Gaussian blur.
    #[default]
    Gaussian,
    /// Blocky pixelate.
    Pixelate,
}

/// One annotation. Variants mirror the [`Tool`] enum. Stored in
/// [`EditorState::anns`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Annotation {
    /// Outlined / filled rectangle.
    Rect {
        /// Geometry.
        rect: Rect,
        /// Outline style.
        stroke: Stroke,
        /// Optional fill.
        fill: Option<Color>,
    },
    /// Outlined / filled ellipse.
    Ellipse {
        /// Geometry (ellipse is inscribed in `rect`).
        rect: Rect,
        /// Outline style.
        stroke: Stroke,
        /// Optional fill.
        fill: Option<Color>,
    },
    /// Straight line.
    Line {
        /// Endpoint A.
        a: Point,
        /// Endpoint B.
        b: Point,
        /// Stroke style.
        stroke: Stroke,
    },
    /// Arrow with barbed head.
    Arrow {
        /// Tail endpoint.
        a: Point,
        /// Head endpoint.
        b: Point,
        /// Stroke style.
        stroke: Stroke,
        /// barb length in physical px.
        head_size: f32,
    },
    /// Semi-transparent thick stroke.
    Marker {
        /// Polyline points.
        points: Vec<Point>,
        /// Marker color.
        color: Color,
        /// Stroke width.
        width: f32,
    },
    /// Opaque freehand stroke.
    Pencil {
        /// Polyline points.
        points: Vec<Point>,
        /// Stroke color.
        color: Color,
        /// Stroke width.
        width: f32,
    },
    /// Left-to-right text label.
    Text {
        /// Anchor (top-left of text).
        pos: Point,
        /// Text content. May contain `\n` (multiline, via Shift+Enter).
        text: String,
        /// Font size in physical px.
        font_size: f32,
        /// Text color.
        color: Color,
        /// Font family.
        #[serde(default)]
        family: crate::text::FontFamily,
        /// Font weight.
        #[serde(default)]
        weight: crate::text::FontWeight,
        /// Font style.
        #[serde(default)]
        style: crate::text::FontStyle,
        /// Horizontal alignment.
        #[serde(default)]
        align: crate::text::TextAlign,
        /// Line-spacing multiplier (line height = `font_size * line_spacing`).
        #[serde(default = "crate::text::default_line_spacing")]
        line_spacing: f32,
        /// Optional glyph outline for readability.
        #[serde(default)]
        outline: Option<crate::text::TextOutline>,
        /// Optional filled background behind the text block.
        #[serde(default)]
        background: Option<Color>,
    },
    /// Blur / pixelate region.
    Blur {
        /// Region to obscure.
        rect: Rect,
        /// Blur variant.
        kind: BlurKind,
        /// Strength (`0..=255` for pixelate block size, or sigma for Gaussian).
        strength: u8,
    },
    /// Auto-incrementing numbered bubble.
    Number {
        /// Bubble center.
        pos: Point,
        /// Number to display.
        n: u32,
        /// Bubble color.
        color: Color,
        /// Bubble diameter.
        diameter: f32,
    },
}

/// A captured editor state used for undo/redo history.
///
/// Captures the fields a mutating action can change: always `anns`
/// (every mutation touches the annotation list) and `next_number`
/// (so undoing a Number placement decrements the counter — otherwise
/// the bubble count would ratchet upward across undos, which is
/// unintuitive), and `image` only when the mutation replaced the
/// source screenshot (crop, Task 14). Annotation-only commits set
/// `image` to `None` so the undo path leaves `state.image` untouched;
/// crop sets it to `Some(pre_crop_image)`.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Annotation list at capture time (pre-mutation).
    pub anns: Vec<Annotation>,
    /// Source image at capture time. `None` when the action did not
    /// touch the image (annotation commit); `Some` when it did (crop).
    pub image: Option<image::RgbaImage>,
    /// `next_number` at capture time. Captured so undoing a Number
    /// placement decrements the counter back to the pre-placement
    /// value — without this the next click after an undo would skip a
    /// number. Annotation commits and crops both snapshot it (every
    /// mutation could be followed by a Number placement).
    pub next_number: u32,
}

impl Snapshot {
    /// Construct an annotation-only snapshot (no image change).
    /// Used by [`EditorState::commit_annotation`] and
    /// [`EditorState::place_number`]. Captures `next_number` so undo
    /// restores the pre-commit counter.
    #[must_use]
    pub fn anns_only(anns: Vec<Annotation>, next_number: u32) -> Self {
        Self {
            anns,
            image: None,
            next_number,
        }
    }

    /// Construct a full snapshot capturing image, anns, and counter.
    /// Used by [`EditorState::crop`].
    #[must_use]
    pub fn full(anns: Vec<Annotation>, image: image::RgbaImage, next_number: u32) -> Self {
        Self {
            anns,
            image: Some(image),
            next_number,
        }
    }
}

/// An in-flight annotation move started by a Hand-tool press on a
/// committed annotation (see [`EditorState::moving`]).
///
/// The drag translates the whole selection set by the incremental
/// pointer delta — each `update_move` applies `at - last_pos` to
/// every member — so the pressed annotation's grab point tracks the
/// cursor without snapping (for the single-member set this is
/// byte-identical to the old absolute `bbox.min = at - grab_offset`
/// positioning) and every other member rides along by the same
/// delta.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnnotationDrag {
    /// Index into [`EditorState::anns`] of the *pressed* annotation.
    /// Chrome-only today (the grab outline keys off it); the
    /// translation itself iterates the selection set. Stable for the
    /// whole drag — moving never reorders `anns`.
    pub index: usize,
    /// Pointer position at the previous event (press or last drag),
    /// physical px. Each drag event translates the set by
    /// `at - last_pos`, then records `at` here.
    pub last_pos: Point,
    /// Whether the drag has produced actual movement yet. The undo
    /// snapshot is pushed lazily on the first movement, so a
    /// click-without-drag leaves history untouched.
    pub moved: bool,
}

/// A lift-out re-edit session over a committed text (text-select-edit
/// spec).
///
/// A double-clicked `Annotation::Text` moves out of
/// [`EditorState::anns`] into [`EditorState::pending`], reusing the
/// entire fresh-draft modal path (cursor ops, key suppression,
/// commit / cancel). This record is what makes the session exact-once
/// undoable: commit pushes a single snapshot containing `original`,
/// and Esc reverts to it with no history touch.
#[derive(Debug, Clone, PartialEq)]
pub struct TextReeditSession {
    /// Index the annotation occupied in `anns` before lift-out. The
    /// slot is vacant for the session's duration; commit / revert
    /// splice back here.
    pub index: usize,
    /// The annotation as it was before lift-out — the revert target
    /// and the undo snapshot's payload.
    pub original: Annotation,
}

/// Full editor state: source image + committed annotations + history +
/// active tool settings.
///
/// `image` is the source screenshot, never mutated. Annotations are
/// appended to `anns`; undo/redo is tracked by snapshot. The
/// toolkit-specific texture handle does *not* live here — the shell
/// owns it on its side.
///
/// `pending` holds the annotation currently being drawn (e.g. during a
/// drag). The shared `render()` path draws it on top of `anns` so the
/// user sees live feedback.
#[derive(Debug, Clone)]
pub struct EditorState {
    /// Source screenshot, never mutated.
    pub image: image::RgbaImage,
    /// Committed annotations, in draw order.
    pub anns: Vec<Annotation>,
    /// Undo history (snapshots of pre-mutation state).
    pub undo: Vec<Snapshot>,
    /// Redo history (snapshots of post-mutation state).
    pub redo: Vec<Snapshot>,
    /// In-flight annotation being drawn (live feedback during drag).
    /// `None` when no tool interaction is in progress.
    pub pending: Option<Annotation>,
    /// In-flight crop region for `Tool::Crop`.
    ///
    /// GOTCHA: formerly `selection`; **not** the annotation selection
    /// set ([`selected`](Self::selected)) — that is the selection
    /// tools' multi-select state. This rect is the region drag:
    /// `None` when no drag is active. Set by the dispatcher on
    /// PointerDown/PointerDrag with `Tool::Crop`; consumed by
    /// [`confirm_crop`](Self::confirm_crop) on Enter or the shell's
    /// Crop button. Rendered on top of annotations so the user sees
    /// live feedback during the drag.
    pub crop_region: Option<Rect>,
    /// In-flight marquee for `Tool::Select` (the rubber band drawn
    /// over empty space).
    ///
    /// GOTCHA: transient by design — release resolves it into
    /// [`selected`](Self::selected) and clears it; never part of
    /// saves/history. Not to be confused with
    /// [`crop_region`](Self::crop_region).
    pub marquee: Option<Rect>,
    /// Whether the in-flight marquee merges into the selection set
    /// (Shift/Ctrl) or replaces it (plain). Captured at press time
    /// because `PointerUp` carries no modifiers.
    pub marquee_additive: bool,
    /// Currently-active tool.
    pub tool: Tool,
    /// Policy: committing an eligible annotation flips `tool` to
    /// [`Tool::Select`] so the next press can reposition what was
    /// just drawn. Defaults to `false` — tools are sticky (ADR-0058,
    /// superseding ADR-0040's default-on); plumbed from
    /// `[ui] flip-to-select` (legacy files: `flip-to-move`) at
    /// editor-session start (read-once, like the magnifier config).
    /// The flip itself never creates an undo entry — history stays
    /// purely about annotations.
    pub flip_to_select: bool,
    /// Active annotation color.
    pub color: Color,
    /// Active stroke width.
    pub stroke_width: f32,
    /// Active blur kind for `Tool::Blur` (Gaussian / Pixelate). Set by
    /// the toolbar's `BlurKind` toggle; read at `begin_annotation` time
    /// so the dragged annotation uses the currently-selected variant.
    pub blur_kind: BlurKind,
    /// When `true`, `Tool::Rect` / `Tool::Ellipse` drafts are created
    /// filled with the annotation color (in addition to the stroke);
    /// `false` (default) keeps them outlined-only, matching the
    /// pre-toggle behavior. Read at `begin_annotation` time.
    pub fill_shapes: bool,
    /// Active font size for `Tool::Text`.
    pub font_size: f32,
    /// Active font family for `Tool::Text`.
    pub font_family: crate::text::FontFamily,
    /// Active font weight for `Tool::Text`.
    pub font_weight: crate::text::FontWeight,
    /// Active font style for `Tool::Text`.
    pub font_style: crate::text::FontStyle,
    /// Active text alignment for `Tool::Text`.
    pub text_align: crate::text::TextAlign,
    /// Dedicated text colour for `Tool::Text`, decoupled from the
    /// drawing-tools [`color`](Self::color). Snapshot into
    /// [`Annotation::Text`] at `begin_annotation` time.
    pub text_color: Color,
    /// Line-spacing multiplier for `Tool::Text`.
    pub line_spacing: f32,
    /// Optional outline for `Tool::Text`.
    pub text_outline: Option<crate::text::TextOutline>,
    /// Optional background fill for `Tool::Text`.
    pub text_background: Option<Color>,
    /// Next number to assign for `Tool::Number`.
    pub next_number: u32,
    /// Active bubble diameter for `Tool::Number`, in physical px.
    /// Session tool state (like [`color`](Self::color)): the panel's
    /// Number section edits it, `place_number` stamps it into each
    /// committed annotation, and committed bubbles never track later
    /// changes. Bounded by [`NUMBER_MIN_DIAMETER`] /
    /// [`NUMBER_MAX_DIAMETER`].
    pub number_diameter: f32,
    /// In-flight annotation move for `Tool::Hand` (press on a
    /// committed annotation). `None` when no move drag is active.
    /// While set, Hand-tool `PointerDrag`s translate the grabbed
    /// annotation.
    pub moving: Option<AnnotationDrag>,
    /// Selected annotation indices (the Hand / Text-tool selection
    /// set).
    ///
    /// **Invariant: sorted ascending, no duplicates** — maintained
    /// by the private routing helpers; consumers may rely on stable
    /// ascending order. Under Hand, built by the dispatcher's press
    /// routing: plain click replaces the set with the hit
    /// annotation, Shift+click toggles membership, a miss clears.
    /// Under Text, a press on a committed Text selects it — the set
    /// is always a singleton there (`select_only`); a miss clears
    /// and starts a fresh caret. **Kept after release** (persistent
    /// selection — the outline the shells render). Cleared by a
    /// miss press, [`set_tool`](Self::set_tool) away from the
    /// selection-owning tool (Hand / Text), `undo` / `redo` /
    /// `crop`, and [`delete_selected`](Self::delete_selected).
    /// Chrome-only: never composed into Save / Copy / Upload output,
    /// never restored by undo (the set is chrome, not content).
    ///
    /// Not to be confused with [`crop_region`](Self::crop_region) —
    /// the `Tool::Crop` region rectangle, a different concept.
    pub selected: Vec<usize>,
    /// Insertion-point position within the in-flight `Tool::Text`
    /// draft, as a **char index** (not byte index) into
    /// `pending`'s `text`. Reset to 0 when a draft begins; advanced by
    /// typing / newline, retreated by backspace, and moved by the arrow
    /// / Home / End keys. The renderer reads this to place the block
    /// caret on the correct line and column.
    pub text_cursor: usize,
    /// Char index of the text-selection anchor; `None` = collapsed
    /// (no selection). The selection is
    /// `min(anchor, cursor)..max(anchor, cursor)`, char-index based
    /// like [`text_cursor`](Self::text_cursor) and only meaningful
    /// while a Text draft is pending. Plain caret moves and every
    /// edit stroke clear it; only selection gestures (Shift chords,
    /// mouse drags) set it. Session boundaries (commit / cancel /
    /// re-edit open) leave it `None`.
    pub text_anchor: Option<usize>,
    /// Char index the last in-text press landed on, staged as the
    /// anchor seed for the first
    /// [`text_drag_to_char`](Self::text_drag_to_char) of the gesture —
    /// `None` disarms (only a press-then-drag can start a mouse
    /// selection). Consumed by the seeding drag; reset at session
    /// boundaries.
    text_drag_seed: Option<usize>,
    /// Kind of the most recent inspector property edit, for undo
    /// coalescing — see
    /// [`end_property_edit`](Self::end_property_edit). `None` when no
    /// burst is open; the property mutators (in
    /// [`inspector`](crate::inspector)) own every write. Crate-private
    /// so shells can't forge a burst, only close one.
    pub(crate) last_edit_kind: Option<crate::inspector::PropertyEditKind>,
    /// Active lift-out re-edit session, if any. Set when a Text-tool
    /// `DoubleClick` lifts a committed text into `pending`; cleared by
    /// commit / revert / the defensive guards in `crop` / `undo` /
    /// `redo`. While set, `pending` holds the text being re-edited and
    /// its slot in `anns` is vacant.
    pub text_reedit: Option<TextReeditSession>,
    /// Staged pre-gesture snapshot for the restyle API
    /// ([`begin_style_gesture`](Self::begin_style_gesture)). Set by
    /// `begin`, consumed by
    /// [`end_style_gesture`](Self::end_style_gesture) (which pushes it
    /// onto `undo`). Dropped defensively by `crop` / `undo` / `redo`.
    pub style_gesture_snapshot: Option<Snapshot>,
}

impl EditorState {
    /// Construct a fresh editor state for a captured screenshot.
    #[must_use]
    pub fn new(image: image::RgbaImage) -> Self {
        Self {
            image,
            anns: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            pending: None,
            crop_region: None,
            marquee: None,
            marquee_additive: false,
            tool: Tool::default(),
            flip_to_select: false,
            fill_shapes: false,
            color: Color::RED,
            stroke_width: 3.0,
            blur_kind: BlurKind::default(),
            font_size: 16.0,
            font_family: crate::text::FontFamily::default(),
            font_weight: crate::text::FontWeight::default(),
            font_style: crate::text::FontStyle::default(),
            text_align: crate::text::TextAlign::default(),
            text_color: Color::RED,
            line_spacing: crate::text::DEFAULT_LINE_SPACING,
            text_outline: None,
            text_background: None,
            next_number: 1,
            number_diameter: NUMBER_DEFAULT_DIAMETER,
            moving: None,
            selected: Vec::new(),
            text_cursor: 0,
            text_anchor: None,
            text_drag_seed: None,
            last_edit_kind: None,
            text_reedit: None,
            style_gesture_snapshot: None,
        }
    }

    /// Switch the active tool, dropping a persistent selection when
    /// leaving [`Tool::Select`] or [`Tool::Text`].
    ///
    /// Tool switches bypass the dispatcher (shells write `tool`
    /// directly from rail chips and hotkeys), so this method owns
    /// the "the selection is tool chrome and must not outlive the
    /// tool that produced it" invariant: route every switch through
    /// it and the selection lifecycle stays correct no matter where
    /// the switch originated. Staying on (or switching to) the
    /// selection-owning tool never clears. Hand is not an owner —
    /// it is pan-only and produces no selection.
    pub fn set_tool(&mut self, tool: Tool) {
        if matches!(self.tool, Tool::Select | Tool::Text) && tool != self.tool {
            self.selected.clear();
        }
        self.tool = tool;
    }

    /// Begin a drag-based annotation at `at`.
    ///
    /// Constructs an initial `Annotation` from the active `tool` /
    /// `color` / `stroke_width`, with zero or trivial extent, and
    /// stores it in `pending`. Subsequent pointer events should call
    /// [`update_annotation`](Self::update_annotation); release should
    /// call [`commit_annotation`](Self::commit_annotation).
    ///
    /// `Tool::Text` uses this same entry point: a click places a caret
    /// (an empty `Annotation::Text` in `pending`), and subsequent
    /// [`InputEvent::TextInput`] events append to the draft until
    /// `Enter` / `Esc` (see [`dispatch_input`](Self::dispatch_input)'s
    /// text-editing branch).
    ///
    /// Tools with a different input model (`Number`, `Select`,
    /// `Crop`, `Hand`) are no-ops here; they have dedicated entry
    /// points.
    ///
    /// If a `pending` annotation already exists it is replaced — the
    /// shell should `commit_annotation()` or `cancel_annotation()`
    /// first if it wants to preserve the in-flight one.
    pub fn begin_annotation(&mut self, at: Point) {
        let stroke = Stroke::new(self.color, self.stroke_width);
        // The fill toggle applies to the closed shapes alone; every
        // other tool has no interior to fill.
        let fill = self.fill_shapes.then_some(self.color);
        let ann = match self.tool {
            Tool::Rect => Annotation::Rect {
                rect: Rect::new(at, at),
                stroke,
                fill,
            },
            Tool::Ellipse => Annotation::Ellipse {
                rect: Rect::new(at, at),
                stroke,
                fill,
            },
            Tool::Line => Annotation::Line {
                a: at,
                b: at,
                stroke,
            },
            Tool::Arrow => Annotation::Arrow {
                a: at,
                b: at,
                stroke,
                head_size: ARROW_DEFAULT_HEAD_SIZE,
            },
            Tool::Marker => Annotation::Marker {
                points: vec![at],
                // Drop alpha at construction so every backend renders a
                // translucent highlight with no render-path special-casing
                // (ADR-0022). Pencil below keeps the user's full-opacity
                // color — the only freehand-tool divergence.
                color: self.color.with_alpha(crate::freehand::MARKER_ALPHA),
                width: self.stroke_width,
            },
            Tool::Pencil => Annotation::Pencil {
                points: vec![at],
                color: self.color,
                width: self.stroke_width,
            },
            Tool::Blur => Annotation::Blur {
                rect: Rect::new(at, at),
                kind: self.blur_kind,
                strength: BLUR_DEFAULT_STRENGTH,
            },
            Tool::Text => {
                let style = crate::text::TextStyle {
                    size: self.font_size,
                    color: self.text_color,
                    family: self.font_family,
                    weight: self.font_weight,
                    style: self.font_style,
                    align: self.text_align,
                    line_spacing: self.line_spacing,
                    outline: self.text_outline,
                    background: self.text_background,
                };
                Annotation::text(at, "", style)
            }
            // Number / Select / Crop / Hand have non-drag input
            // models; their entry points are tool-specific (see
            // method doc).
            Tool::Number | Tool::Select | Tool::Crop | Tool::Hand => return,
        };
        if self.tool == Tool::Text {
            self.text_cursor = 0;
            self.text_anchor = None;
            self.text_drag_seed = None;
        }
        self.pending = Some(ann);
    }

    /// Update the in-flight annotation's geometry to follow `at`.
    ///
    /// No-op when `pending` is `None`. For rect-shaped annotations
    /// (`Rect` / `Ellipse` / `Blur`) `at` becomes the corner opposite
    /// the begin anchor — storage is intentionally *not* normalized
    /// during the drag so the anchor stays fixed; [`Rect::normalized`]
    /// is applied at commit / render / hit-test time. For `Line` /
    /// `Arrow` `at` becomes the head endpoint. For `Marker` / `Pencil`
    /// `at` is appended to the polyline.
    pub fn update_annotation(&mut self, at: Point) {
        let Some(ann) = self.pending.as_mut() else {
            return;
        };
        match ann {
            Annotation::Rect { rect, .. }
            | Annotation::Ellipse { rect, .. }
            | Annotation::Blur { rect, .. } => rect.max = at,
            Annotation::Line { b, .. } | Annotation::Arrow { b, .. } => *b = at,
            Annotation::Marker { points, .. } | Annotation::Pencil { points, .. } => {
                points.push(at);
            }
            // Text / Number pending annotations are not produced by
            // `begin_annotation`; their input flow owns its own state.
            Annotation::Text { .. } | Annotation::Number { .. } => {}
        }
    }

    /// Commit the in-flight annotation to `anns`.
    ///
    /// Pushes a snapshot of the current `anns` onto `undo`, clears
    /// `redo`, normalizes the pending annotation's rect (if any),
    /// then appends it to `anns`. When the [`flip_to_select`](Self::flip_to_select)
    /// policy is on and the committing tool owns the pending
    /// annotation, the active tool switches to [`Tool::Hand`] (no
    /// undo entry). No-op when `pending` is `None`.
    pub fn commit_annotation(&mut self) {
        let Some(mut ann) = self.pending.take() else {
            return;
        };
        self.text_anchor = None;
        normalize_rects(&mut ann);
        let owner = owning_tool(&ann);
        // Clone-then-push: snapshot must capture pre-commit `anns`
        // (without the new annotation). `mem::take` here would wipe
        // `self.anns` and lose prior annotations.
        self.undo
            .push(Snapshot::anns_only(self.anns.clone(), self.next_number));
        self.redo.clear();
        self.last_edit_kind = None;
        self.anns.push(ann);
        // Ownership guard: flip only when the committing tool still
        // owns the pending annotation. A Text draft flushed because
        // the user already switched tools (the shell's
        // toolbar-switch flush runs after `tool` changed) must not
        // flip — the explicit tool choice wins.
        if self.flip_to_select && is_flip_eligible(owner) && owner == self.tool {
            self.tool = Tool::Select;
        }
    }

    /// Drop the in-flight annotation without committing.
    pub fn cancel_annotation(&mut self) {
        self.pending = None;
        self.text_anchor = None;
    }

    /// Place a numbered bubble at `pos` and increment `next_number`.
    ///
    /// Entry point for `Tool::Number`'s click-to-place model: a single
    /// `PointerDown` commits an `Annotation::Number` immediately (no
    /// drag, no `pending`), snapshots the pre-placement `anns` +
    /// `next_number` onto `undo` (so undo decrements the counter), and
    /// clears `redo`. The bubble's colour comes from the active
    /// [`color`](Self::color) (same picker the drawing tools use); its
    /// diameter comes from [`number_diameter`](Self::number_diameter)
    /// — the panel's Size control — and is stamped into the annotation
    /// at commit, so later panel changes never resize placed bubbles.
    ///
    /// Public so a future keyboard shortcut ("place next number at
    /// cursor") can call it without going through `dispatch_input`.
    pub fn place_number(&mut self, pos: Point) {
        let ann = Annotation::Number {
            pos,
            n: self.next_number,
            color: self.color,
            diameter: self.number_diameter,
        };
        // Snapshot pre-placement state so undo restores next_number
        // to the pre-increment value. `mem::take` would wipe self.anns.
        self.undo
            .push(Snapshot::anns_only(self.anns.clone(), self.next_number));
        self.redo.clear();
        self.last_edit_kind = None;
        self.anns.push(ann);
        self.next_number = self.next_number.saturating_add(1);
    }

    // --- Crop region (Tool::Crop) ---

    /// Begin a crop-region drag at `anchor`. Replaces any prior region.
    fn begin_crop_region(&mut self, anchor: Point) {
        self.crop_region = Some(Rect::new(anchor, anchor));
    }

    /// Extend the in-flight crop region's far corner to `at`. No-op
    /// when no region is active (e.g. a stray drag before
    /// `PointerDown`).
    fn update_crop_region(&mut self, at: Point) {
        if let Some(sel) = &mut self.crop_region {
            sel.max = at;
        }
    }

    /// Normalize the crop region on pointer release so `min` is
    /// top-left. Matches [`commit_annotation`]'s normalization contract.
    fn finish_crop_region(&mut self) {
        if let Some(sel) = &mut self.crop_region {
            *sel = sel.normalized();
        }
    }

    // --- Object selection + move (Tool::Select) ---

    /// Route a Select-tool press: the set model first (Shift/Ctrl
    /// toggles, plain replaces, miss clears), then the drag grab or
    /// a marquee start on empty space.
    ///
    /// - hit + Shift/Ctrl on a non-member: toggle in, grab
    ///   (press-and-hold after adding drags the set).
    /// - hit + Shift/Ctrl on a member: toggle out, start no drag.
    /// - plain hit on a non-member: replace the set with the hit
    ///   annotation, grab it (last click wins).
    /// - plain hit on a member: **keep the set** and grab — this is
    ///   the group-drag entry (spec exit criterion: drag any selected
    ///   member → all translate together).
    /// - miss, plain: clear the set, start a marquee.
    /// - miss, Shift/Ctrl: keep the set, start a marquee (release
    ///   merges the marquee hits into it).
    ///
    /// Shared by the normal dispatcher and the text-draft
    /// commit-then-route path so both stay modifier-aware in
    /// lockstep.
    fn route_select_press(&mut self, pos: Point, modifiers: crate::input::Modifiers) {
        let toggle = modifiers.shift || modifiers.ctrl;
        let Some(index) = self.hit_test(pos) else {
            if !toggle {
                self.selected.clear();
            }
            self.begin_marquee(pos, toggle);
            return;
        };
        let is_member = self.selected.binary_search(&index).is_ok();
        if toggle {
            if self.toggle_selected(index) {
                self.begin_move(index, pos);
            }
        } else if is_member {
            self.begin_move(index, pos);
        } else {
            self.select_only(index);
            self.begin_move(index, pos);
        }
    }

    /// Begin a marquee drag at `anchor`, capturing whether release
    /// merges (`additive`) or replaces.
    fn begin_marquee(&mut self, anchor: Point, additive: bool) {
        self.marquee = Some(Rect::new(anchor, anchor));
        self.marquee_additive = additive;
    }

    /// Extend the in-flight marquee's far corner to `at`. No-op when
    /// no marquee is active.
    fn update_marquee(&mut self, at: Point) {
        if let Some(m) = &mut self.marquee {
            m.max = at;
        }
    }

    /// Resolve the marquee on release: every annotation whose
    /// bounding box intersects the normalized marquee enters the
    /// selection set — replacing it (plain) or merging (Shift/Ctrl).
    /// A degenerate marquee (< 1 px, a click-without-drag) selects
    /// nothing; the press already handled plain-miss clearing.
    /// The set's sorted-deduped invariant holds either way.
    fn finish_marquee(&mut self) {
        let Some(m) = self.marquee.take() else {
            return;
        };
        let n = m.normalized();
        if n.width().abs() < 1.0 || n.height().abs() < 1.0 {
            return;
        }
        let hits: Vec<usize> = self
            .anns
            .iter()
            .enumerate()
            .filter(|(_, ann)| ann.bounding_box().is_some_and(|b| b.intersect(n).is_some()))
            .map(|(index, _)| index)
            .collect();
        if self.marquee_additive {
            for index in hits {
                self.add_selected(index);
            }
        } else {
            self.selected = hits;
        }
    }

    /// Insert `index` into the selection set if absent (union),
    /// preserving the sorted-deduped invariant.
    fn add_selected(&mut self, index: usize) {
        if let Err(slot) = self.selected.binary_search(&index) {
            self.selected.insert(slot, index);
        }
    }

    /// Replace the selection set with a single member.
    fn select_only(&mut self, index: usize) {
        self.selected.clear();
        self.selected.push(index);
    }

    /// Toggle `index`'s membership in the selection set, preserving
    /// the sorted-deduped invariant. Returns `true` when the index
    /// is a member after the call (a toggle-in).
    fn toggle_selected(&mut self, index: usize) -> bool {
        match self.selected.binary_search(&index) {
            Ok(pos) => {
                self.selected.remove(pos);
                false
            }
            Err(pos) => {
                self.selected.insert(pos, index);
                true
            }
        }
    }

    /// Start moving the annotation at `index` from a press at `at`.
    ///
    /// Records the grab offset (press position minus bounding-box
    /// `min`) so subsequent [`update_move`](Self::update_move) calls
    /// keep the grab point under the cursor. Selection is *not*
    /// touched here — the press routing owns the set. Returns
    /// `false` — the drag is a no-op — when the annotation has no
    /// bounding box (e.g. an empty polyline) and thus nothing to
    /// grab or move by.
    fn begin_move(&mut self, index: usize, at: Point) -> bool {
        if self
            .anns
            .get(index)
            .and_then(Annotation::bounding_box)
            .is_none()
        {
            return false;
        }
        self.moving = Some(AnnotationDrag {
            index,
            last_pos: at,
            moved: false,
        });
        true
    }

    /// Translate every selected annotation by the incremental
    /// pointer delta (`at` minus the previous pointer position).
    ///
    /// GOTCHA: the delta is incremental, not absolute. The old
    /// single-annotation drag positioned `bbox.min = at -
    /// grab_offset` each event; that formula has no plural form (each
    /// member has its own bbox), while per-event deltas compose to
    /// the identical result for the single case and generalize to
    /// the set for free. Incremental also survives `anns` mutations
    /// that preserve indices without re-deriving any anchor.
    ///
    /// Pushes the undo snapshot lazily: the first drag event with
    /// actual movement snapshots the pre-move `anns` and clears
    /// `redo`, so one move gesture — any set size, any event count —
    /// is exactly one undo entry and a click-without-drag records
    /// nothing. Members without a bounding box (degenerate
    /// polylines) are skipped, matching `begin_move`'s grab rule;
    /// stale indices are skipped rather than fatal.
    fn update_move(&mut self, at: Point) {
        let Some(drag) = self.moving else {
            return;
        };
        let dx = at.x - drag.last_pos.x;
        let dy = at.y - drag.last_pos.y;
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        if !drag.moved {
            self.undo
                .push(Snapshot::anns_only(self.anns.clone(), self.next_number));
            self.redo.clear();
            self.last_edit_kind = None;
        }
        self.moving = Some(AnnotationDrag {
            last_pos: at,
            moved: true,
            ..drag
        });
        let members: Vec<usize> = self.selected.clone();
        for index in members {
            if let Some(ann) = self.anns.get_mut(index) {
                if ann.bounding_box().is_some() {
                    ann.translate(dx, dy);
                }
            }
        }
    }

    /// Drop the in-flight move state (`moving` + the selection set)
    /// without touching `anns` — the translation already applied
    /// stays applied.
    fn end_move(&mut self) {
        self.moving = None;
        self.selected.clear();
    }

    /// Remove every selected annotation in one pass.
    ///
    /// Pushes a single `anns`-only snapshot onto `undo` (pre-delete
    /// state), clears `redo`, and drops the selection — one `Del`,
    /// one undo entry that restores all members. Any in-flight move
    /// drag ends first (its index dies with the removal). No-op when
    /// the selection set is empty.
    pub fn delete_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        // Capture the doomed indices before `end_move` clears the set.
        let doomed: Vec<usize> = self.selected.clone();
        self.end_move();
        self.undo
            .push(Snapshot::anns_only(self.anns.clone(), self.next_number));
        self.redo.clear();
        self.last_edit_kind = None;
        // Remove back-to-front so live indices below each removal
        // stay valid (the set is sorted ascending). The bounds guard
        // makes a stale index a skip, not a panic.
        for index in doomed.into_iter().rev() {
            if index < self.anns.len() {
                self.anns.remove(index);
            }
        }
    }

    /// Confirm the active crop region: crop the editor to it, then
    /// clear the region. No-op when no region is active or the region
    /// is degenerate (see [`crop`]).
    ///
    /// This is the entry point the dispatcher's Enter handler and the
    /// shell's "Crop" button both call.
    pub fn confirm_crop(&mut self) {
        let Some(sel) = self.crop_region.take() else {
            return;
        };
        self.crop(sel);
    }

    /// Crop the editor to `rect`, replacing `image` with the cropped
    /// sub-image and translating every annotation by subtracting the
    /// crop origin.
    ///
    /// Annotations fully outside `rect` are dropped. Rect-shaped
    /// annotations (`Rect` / `Ellipse` / `Blur`) that partially
    /// overlap `rect` are geometrically clipped to the overlap;
    /// other annotation types keep their geometry (the rendered edge
    /// is clipped at the new image boundary by the canvas).
    ///
    /// Pushes a [`Snapshot::full`] (pre-crop image + anns) onto `undo`
    /// and clears `redo`. Clears `pending` (an in-flight annotation
    /// in the old coordinate space would be misleading after crop)
    /// and any in-flight move (its index refers to the pre-crop
    /// list). No-op when `rect` is degenerate (either normalized
    /// dimension < 1 px) or the crop falls entirely outside the image.
    pub fn crop(&mut self, rect: Rect) {
        let n = rect.normalized();
        if n.width().abs() < 1.0 || n.height().abs() < 1.0 {
            return;
        }
        let new_image = crate::image_ops::crop_region(&self.image, n);
        let (cw, ch) = new_image.dimensions();
        if cw == 0 || ch == 0 {
            return;
        }
        // Shell-direct entry (the Crop button): close any live re-edit
        // session first so the crop sees the pre-session anns.
        self.restore_active_reedit();
        let snapshot = Snapshot::full(self.anns.clone(), self.image.clone(), self.next_number);
        self.image = new_image;
        self.anns.retain_mut(|ann| ann.crop_and_translate(n));
        self.pending = None;
        self.text_anchor = None;
        self.end_move();
        self.undo.push(snapshot);
        self.redo.clear();
        self.last_edit_kind = None;
    }

    /// Pop the last committed state from `undo` and swap it into `anns`.
    ///
    /// Also clears `pending` (Ctrl+Z mid-drag cancels the in-flight
    /// annotation, matching typical editor behaviour) and any
    /// in-flight move (Ctrl+Z mid-move restores the pre-move
    /// position the snapshot carries and the drag's index would
    /// point into the swapped-out list). If the snapshot carries an
    /// image (crop), the pre-crop image is restored too.
    /// `next_number` is always restored from the snapshot so undoing a
    /// Number placement decrements the counter. The current state is
    /// pushed onto `redo` before the swap. No-op when `undo` is empty.
    pub fn undo(&mut self) {
        self.pending = None;
        self.text_anchor = None;
        // Shell-direct undo mid-session must not splice against the
        // session's vacant slot — restore the original first.
        self.restore_active_reedit();
        self.end_move();
        let Some(prev) = self.undo.pop() else {
            return;
        };
        // Build the "current state" snapshot for redo. If the undo
        // entry captured an image (crop), capture the current image
        // too so redo can restore it; otherwise image is None and
        // redo leaves image untouched (matching commit's snapshot).
        let current = Snapshot {
            anns: std::mem::take(&mut self.anns),
            image: prev.image.as_ref().map(|_| self.image.clone()),
            next_number: self.next_number,
        };
        self.redo.push(current);
        self.last_edit_kind = None;
        self.anns = prev.anns;
        self.next_number = prev.next_number;
        if let Some(img) = prev.image {
            self.image = img;
        }
    }

    /// Pop the last undone state from `redo` and swap it into `anns`.
    ///
    /// Mirrors [`undo`](Self::undo): any in-flight move is dropped
    /// for the same index-stability reason, and if the redo
    /// snapshot carries an image, it is restored; `next_number` is
    /// always restored. The current state is pushed onto `undo`
    /// before the swap. No-op when `redo` is empty (which is always
    /// the case immediately after a commit or crop, since both clear
    /// `redo`).
    pub fn redo(&mut self) {
        // Same mid-session guard as `undo`: restore before swapping.
        self.restore_active_reedit();
        self.end_move();
        self.text_anchor = None;
        let Some(next) = self.redo.pop() else {
            return;
        };
        let current = Snapshot {
            anns: std::mem::take(&mut self.anns),
            image: next.image.as_ref().map(|_| self.image.clone()),
            next_number: self.next_number,
        };
        self.undo.push(current);
        self.last_edit_kind = None;
        self.anns = next.anns;
        self.next_number = next.next_number;
        if let Some(img) = next.image {
            self.image = img;
        }
    }

    /// Return the index of the topmost annotation containing `point`,
    /// or `None` if no annotation is hit.
    ///
    /// "Topmost" = highest index in `anns` (drawn last). Per-variant
    /// geometry: rect-shaped annotations use point-in-rect; `Line` /
    /// `Arrow` use distance-to-segment within `stroke.width / 2 +
    /// HIT_TEST_TOLERANCE_PX`; `Marker` / `Pencil` test each segment;
    /// `Number` uses circle distance; `Text` uses a coarse bounding-box
    /// approximation (real shaping lands in Task 12).
    #[must_use]
    pub fn hit_test(&self, point: Point) -> Option<usize> {
        self.anns
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, ann)| ann.contains(point).then_some(i))
    }

    /// Index of the topmost annotation containing `point`, when that
    /// annotation is a `Text`.
    ///
    /// The Text tool's select-before-draft gate (shared by the
    /// dispatcher and the shells' hover affordances): the hit is
    /// resolved against *every* annotation kind first — a non-Text
    /// annotation on top of a Text is a miss, exactly like a click on
    /// empty canvas (the fresh-caret behaviour stays byte-for-byte
    /// what it was before selection existed; spec assumption: only
    /// Text is selectable under `Tool::Text`).
    #[must_use]
    pub fn text_hit_test(&self, point: Point) -> Option<usize> {
        self.hit_test(point)
            .filter(|&i| matches!(self.anns.get(i), Some(Annotation::Text { .. })))
    }

    /// Text-tool `PointerDown` routing: a press on a committed Text
    /// selects it (chrome + panel reflection); anything else — empty
    /// canvas or a non-Text annotation — clears the selection and
    /// starts a fresh caret. Shared by the normal and text-modal
    /// dispatch branches so both shells see identical semantics.
    fn text_pointer_down(&mut self, pos: Point) {
        if let Some(index) = self.text_hit_test(pos) {
            self.select_only(index);
        } else {
            self.selected.clear();
            self.begin_annotation(pos);
        }
    }

    /// Dispatch a toolkit-agnostic [`InputEvent`] and return the
    /// side-effect the shell should perform.
    ///
    /// This is the primary input boundary: shells translate their
    /// native events into [`InputEvent`]s at the toolkit edge (egui →
    /// `InputEvent`, future iced / slint shells likewise) and feed
    /// them through this method. The dispatcher handles every
    /// keyboard shortcut and every pointer-driven tool transition in
    /// a single pure function — no `egui::Context` or window-system
    /// types reach this code.
    ///
    /// The shell retains responsibility for:
    /// - filtering repeat events (key held, pointer moved without a
    ///   button change),
    /// - translating screen-logical to canvas-physical coordinates,
    /// - actually performing the side-effect when the outcome is
    ///   `Cancel` / `Save` / `Copy` (those need a window / file
    ///   dialog / clipboard, which core can't touch).
    ///
    /// # Pointer-up semantics
    ///
    /// [`InputEvent::PointerUp`] carries no position. The dispatcher
    /// commits with the in-flight annotation's last-drag geometry,
    /// matching the contract the user sees during the drag — what
    /// was rendered is what gets committed. If the shell wants to
    /// capture the release position precisely, it can emit a final
    /// `PointerDrag { pos: release_pos }` before the `PointerUp`.
    pub fn dispatch_input(
        &mut self,
        event: &crate::input::InputEvent,
    ) -> crate::input::DispatchOutcome {
        use crate::input::{DispatchOutcome, InputEvent};

        // Text-editing modal mode: once a PointerDown with Tool::Text
        // has placed a caret (`pending` is an empty/partial Text
        // annotation), keyboard shortcuts are suppressed so the user
        // can type freely without Ctrl+S / Ctrl+Z stealing focus.
        // Only text-affecting events (TextInput, Backspace, Enter, Esc)
        // and pointer events that would start a new annotation are
        // routed differently — see `dispatch_text_event`.
        if self.is_editing_text() {
            return self.dispatch_text_event(event);
        }

        match event {
            InputEvent::Key { key, modifiers } => {
                // Enter confirms a Crop-tool region before falling
                // through to the normal shortcut path.
                if *key == crate::geom::Key::Enter
                    && self.tool == Tool::Crop
                    && self.crop_region.is_some()
                {
                    self.confirm_crop();
                    DispatchOutcome::Continue
                } else {
                    self.dispatch_key(*key, *modifiers)
                }
            }
            InputEvent::PointerDown { pos, modifiers } => {
                if self.tool == Tool::Crop {
                    // Crop owns the region drag — a press on a
                    // committed annotation still starts a fresh region
                    // over it (annotation moves belong to the
                    // selection tools).
                    self.begin_crop_region(*pos);
                } else if self.tool == Tool::Select {
                    // A press routes through the selection set first
                    // (Shift/Ctrl toggles, plain replaces, miss clears
                    // or merges), then grabs the hit annotation for a
                    // drag or starts a marquee. See
                    // `route_select_press` for the arm details and the
                    // degenerate-bbox no-grab path.
                    self.route_select_press(*pos, *modifiers);
                } else if self.tool == Tool::Number {
                    // Number is click-to-place: a single PointerDown
                    // commits the bubble immediately (no pending, no
                    // drag). The dedicated entry point keeps the
                    // counter + snapshot logic in one place.
                    self.place_number(*pos);
                } else if self.tool == Tool::Text {
                    // Text selects before it drafts: a hit on a
                    // committed Text picks it; a miss (or a non-Text
                    // annotation) clears the selection and starts a
                    // fresh caret exactly as before.
                    self.text_pointer_down(*pos);
                } else {
                    self.begin_annotation(*pos);
                }
                DispatchOutcome::Continue
            }
            InputEvent::PointerDrag { pos } => {
                if self.tool == Tool::Crop {
                    self.update_crop_region(*pos);
                } else if self.tool == Tool::Select {
                    // Exactly one of these is live: a grab drags the
                    // set, otherwise the marquee extends. Both
                    // self-guard on their Option state.
                    self.update_move(*pos);
                    self.update_marquee(*pos);
                } else if self.tool == Tool::Hand {
                    // Pan-only contract: the shell owns panning and
                    // never dispatches Hand drags; core no-ops them
                    // defensively so synthetic state can't mutate
                    // annotations.
                } else {
                    self.update_annotation(*pos);
                }
                DispatchOutcome::Continue
            }
            InputEvent::PointerUp => {
                if self.tool == Tool::Crop {
                    self.finish_crop_region();
                } else if self.tool == Tool::Select {
                    // Release ends the drag but keeps the selection —
                    // the press selected the annotation (persistent
                    // selection); Del / a miss press / tool switch own
                    // deselecting. A live marquee resolves into the
                    // set.
                    self.moving = None;
                    self.finish_marquee();
                } else if self.tool == Tool::Hand {
                    // Pan-only contract (see PointerDrag).
                } else {
                    self.finish_pointer_up();
                }
                DispatchOutcome::Continue
            }
            // Stray text input outside text-editing mode: drop it.
            // (The shell shouldn't send these, but the dispatcher must
            // be robust against a TextInput arriving when no draft is
            // active — e.g. a race between placement and the next
            // frame.) Triple-click outside editing is likewise a
            // documented no-op — line selection is a text-editing
            // affordance only (see `dispatch_text_event`).
            InputEvent::TextInput(_) | InputEvent::TripleClick { .. } => DispatchOutcome::Continue,
            // Double-click owns exactly one behavior: re-edit. Only
            // the Text tool reacts, and only when the accompanying
            // second PointerDown didn't just start / continue a draft
            // (mid-edit arrivals no-op in `dispatch_text_event`) and
            // something text-shaped is actually under the pointer.
            InputEvent::DoubleClick { pos } => {
                if self.tool == Tool::Text && self.pending.is_none() {
                    if let Some(index) = self.text_hit_test(*pos) {
                        self.begin_text_reedit(index);
                    }
                }
                DispatchOutcome::Continue
            }
        }
    }

    /// Whether a text draft is currently being edited.
    ///
    /// True when `pending` holds an in-progress `Annotation::Text`.
    /// The dispatcher uses this to switch into text-editing modal
    /// mode (see [`dispatch_input`](Self::dispatch_input)).
    #[must_use]
    pub fn is_editing_text(&self) -> bool {
        matches!(self.pending, Some(Annotation::Text { .. }))
    }

    /// Dispatch an event while in text-editing mode. See
    /// [`dispatch_input`](Self::dispatch_input).
    ///
    /// A `PointerDown` commits the current draft and immediately starts
    /// the next action at the clicked position — Flameshot-like
    /// behaviour where clicking elsewhere places a new text caret (if
    /// still on Text tool) or begins the newly-selected tool's drag.
    fn dispatch_text_event(
        &mut self,
        event: &crate::input::InputEvent,
    ) -> crate::input::DispatchOutcome {
        use crate::geom::Key;
        use crate::input::{DispatchOutcome, InputEvent};
        match event {
            InputEvent::Key {
                key: Key::Escape, ..
            } => {
                // Mid-session Esc reverts the whole edit (the re-edit
                // analog of cancel-draft); a fresh draft just cancels.
                if let Some(session) = self.text_reedit.take() {
                    self.revert_text_reedit(session);
                } else {
                    self.cancel_annotation();
                }
                DispatchOutcome::Continue
            }
            InputEvent::Key {
                key: Key::Enter,
                modifiers,
            } => {
                // Shift+Enter inserts a line break (multiline text);
                // plain Enter commits the draft. Multi-line support was
                // pulled forward from Phase 2 (ADR-0017) once the save
                // path learned to rasterise multiple baselines.
                if modifiers.shift {
                    self.insert_newline();
                } else {
                    self.commit_text_draft();
                }
                DispatchOutcome::Continue
            }
            InputEvent::Key {
                key: Key::Backspace,
                ..
            } => {
                self.backspace_text();
                DispatchOutcome::Continue
            }
            // Delete replaces the selection, else forward-deletes the
            // char after the caret.
            InputEvent::Key {
                key: Key::Delete, ..
            } => {
                self.delete_text();
                DispatchOutcome::Continue
            }
            // Ctrl+A selects the whole draft. Plain 'a' presses never
            // arrive as Key events mid-edit (typed text comes in as
            // TextInput), so the ctrl_only guard is the only gate
            // needed.
            InputEvent::Key {
                key: Key::Char('a' | 'A'),
                modifiers,
            } if modifiers.ctrl_only() => {
                self.select_all_text();
                DispatchOutcome::Continue
            }
            // Every other key routes through the motion table: Shift
            // chords extend the selection, plain presses move and
            // collapse, unhandled modifier combos no-op (word-wise
            // chords are a later task).
            InputEvent::Key { key, modifiers } => {
                self.dispatch_text_motion(*key, *modifiers);
                DispatchOutcome::Continue
            }
            InputEvent::TextInput(ch) => {
                self.type_char(*ch);
                DispatchOutcome::Continue
            }
            // Clicking elsewhere while editing commits the current
            // draft and starts a fresh action at the new position
            // (Flameshot behaviour). If the tool is still Text, a new
            // caret is placed; if the user switched tools via the
            // toolbar, the new tool's drag begins.
            InputEvent::PointerDown { pos, modifiers } => {
                self.commit_text_draft();
                if self.tool == Tool::Crop {
                    self.begin_crop_region(*pos);
                } else if self.tool == Tool::Select {
                    self.route_select_press(*pos, *modifiers);
                } else if self.tool == Tool::Number {
                    self.place_number(*pos);
                } else if self.tool == Tool::Text {
                    self.text_pointer_down(*pos);
                } else {
                    self.begin_annotation(*pos);
                }
                DispatchOutcome::Continue
            }
            // Click-count gestures while editing: double-click selects
            // the word, triple-click the line, both around the cursor
            // (the intercepted press collapsed it onto the click
            // point). Empty drafts no-op — there is nothing to select.
            InputEvent::DoubleClick { .. } => {
                self.text_select_word();
                DispatchOutcome::Continue
            }
            InputEvent::TripleClick { .. } => {
                self.text_select_line();
                DispatchOutcome::Continue
            }
            // Any other key / pointer event is ignored while editing
            // text. PointerDrag / PointerUp from the click that placed
            // the caret are harmless no-ops here.
            _ => DispatchOutcome::Continue,
        }
    }

    /// The selected char range of the pending Text draft, normalized
    /// so `start <= end`. `None` when no Text draft is pending, the
    /// anchor is unset, or the selection is collapsed
    /// (`anchor == cursor`).
    #[must_use]
    pub fn selection_range(&self) -> Option<std::ops::Range<usize>> {
        if !matches!(self.pending, Some(Annotation::Text { .. })) {
            return None;
        }
        let anchor = self.text_anchor?;
        let (a, b) = (anchor.min(self.text_cursor), anchor.max(self.text_cursor));
        (a != b).then_some(a..b)
    }

    /// Click-to-position the caret inside the draft at a shell-resolved
    /// char index. The pump's press-inside-pending interception calls
    /// this *instead of* dispatching `PointerDown` (which would run
    /// the commit-and-move-on path), so the draft stays open with the
    /// caret repositioned. Collapses any selection and arms
    /// drag-seeding for a following [`text_drag_to_char`]. No-op when
    /// not editing text; `idx` clamps to `0..=len`.
    pub fn text_press_at_char(&mut self, idx: usize) {
        let Some(Annotation::Text { text, .. }) = &self.pending else {
            return;
        };
        let len = text.chars().count();
        let clamped = idx.min(len);
        self.text_cursor = clamped;
        self.text_anchor = None;
        self.text_drag_seed = Some(clamped);
    }

    /// Drag-extend the selection to a shell-resolved char index. The
    /// first drag after a press seeds the anchor at the pressed
    /// position; later drags move only the cursor (anchor sticky, so
    /// sweeping back and forth keeps one edge pinned). A drag without
    /// a preceding in-text press is a full no-op — only
    /// [`text_press_at_char`] arms seeding, so a stray drag can never
    /// conjure a selection. No-op when not editing text; `idx` clamps
    /// to `0..=len`.
    pub fn text_drag_to_char(&mut self, idx: usize) {
        let Some(Annotation::Text { text, .. }) = &self.pending else {
            return;
        };
        // GOTCHA: only the first drag of a gesture seeds — the staged
        // press index is consumed here, so a later stray drag (anchor
        // still None because nothing was selected) no-ops.
        if self.text_anchor.is_none() {
            let Some(pressed) = self.text_drag_seed.take() else {
                return;
            };
            self.text_anchor = Some(pressed);
        }
        let len = text.chars().count();
        self.text_cursor = idx.min(len);
    }

    /// Select the word around the cursor — the double-click payload.
    /// The pump collapses the cursor onto the click point first (the
    /// press interception), so this resolves around the cursor with no
    /// geometry. No-op when not editing text or when no word exists
    /// (empty / whitespace-only draft).
    pub fn text_select_word(&mut self) {
        if let Some(Annotation::Text { text, .. }) = &self.pending {
            let (a, b) = word_bounds_around(text, self.text_cursor);
            if a == b {
                return;
            }
            self.text_anchor = Some(a);
            self.text_cursor = b;
        }
    }

    /// Select the line around the cursor — the triple-click payload
    /// (degenerates to whole-text for the common single-line
    /// annotation). No-op when not editing text or on an empty line.
    pub fn text_select_line(&mut self) {
        if let Some(Annotation::Text { text, .. }) = &self.pending {
            let a = cursor_line_start(text, self.text_cursor);
            let b = cursor_line_end(text, self.text_cursor);
            if a == b {
                return;
            }
            self.text_anchor = Some(a);
            self.text_cursor = b;
        }
    }

    /// Delete the selected range (if any), park the cursor at the
    /// range start, and clear the anchor — the shared "edit ops
    /// consume the selection" step. A collapsed or absent selection
    /// leaves state untouched, so the plain-caret path is
    /// byte-for-byte what it was before selections existed.
    fn delete_selection(&mut self) {
        let Some(range) = self.selection_range() else {
            return;
        };
        if let Some(Annotation::Text { text, .. }) = &mut self.pending {
            let start = char_to_byte(text, range.start);
            let end = char_to_byte(text, range.end);
            text.replace_range(start..end, "");
        }
        self.text_cursor = range.start;
        self.text_anchor = None;
    }

    /// Shift-chord engine: seed the selection anchor at the current
    /// cursor on the first press (later presses keep the original
    /// anchor — standard anchor semantics), then move the cursor with
    /// `mv`. No-op when no text draft is active.
    fn extend_by(&mut self, mv: fn(&mut Self)) {
        if self.is_editing_text() {
            self.text_anchor = Some(self.text_anchor.unwrap_or(self.text_cursor));
            mv(self);
        }
    }

    /// Select the entire draft (Ctrl+A). An empty draft is a no-op —
    /// there is nothing to highlight and a `0..0` "selection" would
    /// be indistinguishable from collapsed.
    fn select_all_text(&mut self) {
        if let Some(Annotation::Text { text, .. }) = &self.pending {
            let len = text.chars().count();
            if len > 0 {
                self.text_anchor = Some(0);
                self.text_cursor = len;
            }
        }
    }

    /// Delete the character immediately after the cursor (forward
    /// delete), or the whole selection when one exists. No-op at the
    /// end of the draft or when no draft is active.
    fn delete_text(&mut self) {
        if self.selection_range().is_some() {
            self.delete_selection();
            return;
        }
        if let Some(Annotation::Text { text, .. }) = &mut self.pending {
            if self.text_cursor < text.chars().count() {
                let cur = char_to_byte(text, self.text_cursor);
                let next = char_to_byte(text, self.text_cursor + 1);
                text.replace_range(cur..next, "");
            }
        }
    }

    /// Arrow / Home / End motion while editing text, as a table:
    /// Shift chords extend the selection (anchor seeded on the first
    /// press via [`extend_by`](Self::extend_by)), modifier-free
    /// presses move and collapse the selection, and the horizontal
    /// arrows additionally take Ctrl word-wise chords (Ctrl+arrow
    /// moves word-by-word, Ctrl+Shift+arrow extends word-wise). Other
    /// modifier combos and non-motion keys no-op.
    fn dispatch_text_motion(&mut self, key: crate::geom::Key, modifiers: crate::input::Modifiers) {
        use crate::geom::Key;
        type Motion = fn(&mut EditorState);
        let motions: Option<(Motion, Option<Motion>)> = match key {
            Key::ArrowLeft => Some((Self::cursor_left, Some(Self::cursor_word_left))),
            Key::ArrowRight => Some((Self::cursor_right, Some(Self::cursor_word_right))),
            Key::ArrowUp => Some((Self::cursor_up, None)),
            Key::ArrowDown => Some((Self::cursor_down, None)),
            Key::Home => Some((Self::cursor_home, None)),
            Key::End => Some((Self::cursor_end, None)),
            _ => None,
        };
        let Some((plain, word)) = motions else {
            return;
        };
        // Chord resolution, most-specific first: Ctrl+Shift extends
        // word-wise, Ctrl moves word-wise and collapses, Shift extends
        // char-wise, plain moves char-wise and collapses. Anything
        // else (Alt / Super combos) no-ops.
        let extend: Option<Motion> = if modifiers.ctrl_shift() {
            word
        } else if modifiers.ctrl_only() {
            let mv = word.unwrap_or(plain);
            mv(self);
            self.text_anchor = None;
            return;
        } else if modifiers.shift {
            Some(plain)
        } else if modifiers == crate::input::Modifiers::default() {
            plain(self);
            self.text_anchor = None;
            return;
        } else {
            None
        };
        if let Some(mv) = extend {
            self.extend_by(mv);
        }
    }

    /// Append a typed character to the in-flight text draft **at the
    /// cursor**, then advance the cursor past it. A non-empty
    /// selection is replaced by the stroke.
    ///
    /// No-op when no text draft is active (`is_editing_text()` is
    /// false). Called by the dispatcher's text-editing branch.
    fn type_char(&mut self, ch: char) {
        self.delete_selection();
        if let Some(Annotation::Text { text, .. }) = &mut self.pending {
            let byte = char_to_byte(text, self.text_cursor);
            text.insert(byte, ch);
            self.text_cursor += 1;
        }
    }

    /// Insert a line break at the cursor (Shift+Enter), then advance
    /// the cursor onto the new line. A non-empty selection is replaced
    /// by the stroke. The `\n` is stored verbatim; both renderers
    /// split on it to lay out subsequent lines. No-op when no text
    /// draft is active.
    fn insert_newline(&mut self) {
        self.delete_selection();
        if let Some(Annotation::Text { text, .. }) = &mut self.pending {
            let byte = char_to_byte(text, self.text_cursor);
            text.insert(byte, '\n');
            self.text_cursor += 1;
        }
    }

    /// Delete the character immediately before the cursor and retreat
    /// the cursor; a non-empty selection deletes the whole range
    /// instead. No-op when the selection is collapsed and the cursor
    /// is at the start of the draft (column 0 of line 0) or when no
    /// draft is active.
    fn backspace_text(&mut self) {
        if self.selection_range().is_some() {
            self.delete_selection();
            return;
        }
        if let Some(Annotation::Text { text, .. }) = &mut self.pending {
            if self.text_cursor > 0 {
                let prev = char_to_byte(text, self.text_cursor - 1);
                let cur = char_to_byte(text, self.text_cursor);
                text.replace_range(prev..cur, "");
                self.text_cursor -= 1;
            }
        }
    }

    /// Move the cursor left one char (wrapping across line breaks to
    /// the end of the previous line). Clamps at 0. Pure mover —
    /// anchor policy (collapse / extend) lives at the dispatch arms.
    fn cursor_left(&mut self) {
        self.text_cursor = self.text_cursor.saturating_sub(1);
    }

    /// Move the cursor right one char (wrapping across line breaks to
    /// the start of the next line). Clamps at the end of the draft.
    fn cursor_right(&mut self) {
        if let Some(Annotation::Text { text, .. }) = &self.pending {
            let len = text.chars().count();
            self.text_cursor = (self.text_cursor + 1).min(len);
        }
    }

    /// Move the cursor up one line, preserving the column. Clamps to
    /// column 0 of the first line when already there.
    fn cursor_up(&mut self) {
        if let Some(Annotation::Text { text, .. }) = &self.pending {
            self.text_cursor = cursor_up(text, self.text_cursor);
        }
    }

    /// Move the cursor down one line, preserving the column. Clamps to
    /// the end of the draft when already on the last line.
    fn cursor_down(&mut self) {
        if let Some(Annotation::Text { text, .. }) = &self.pending {
            self.text_cursor = cursor_down(text, self.text_cursor);
        }
    }

    /// Move the cursor to the start of the current/previous word
    /// (Ctrl+Left). Pure mover, like its char-wise siblings.
    fn cursor_word_left(&mut self) {
        if let Some(Annotation::Text { text, .. }) = &self.pending {
            self.text_cursor = word_left(text, self.text_cursor);
        }
    }

    /// Move the cursor to the start of the next word (Ctrl+Right).
    fn cursor_word_right(&mut self) {
        if let Some(Annotation::Text { text, .. }) = &self.pending {
            self.text_cursor = word_right(text, self.text_cursor);
        }
    }

    /// Move the cursor to the start of the current line.
    fn cursor_home(&mut self) {
        if let Some(Annotation::Text { text, .. }) = &self.pending {
            self.text_cursor = cursor_line_start(text, self.text_cursor);
        }
    }

    /// Move the cursor to the end of the current line.
    fn cursor_end(&mut self) {
        if let Some(Annotation::Text { text, .. }) = &self.pending {
            self.text_cursor = cursor_line_end(text, self.text_cursor);
        }
    }

    /// Commit the in-flight text draft to `anns`, or cancel it if empty.
    ///
    /// An empty draft (no characters typed) never commits — committing
    /// a zero-content `Text` would litter `anns` with invisible
    /// annotations and skew the undo history.
    ///
    /// When a lift-out re-edit session is active, the session branch
    /// runs instead: unchanged text splices back silently, an edit (of
    /// content or style) splices back with exactly one undo entry
    /// carrying the original, and an edited-to-empty text is removed
    /// (same single entry) — fresh drafts never commit empty, and
    /// neither do re-edited ones.
    ///
    /// Public so the shell can flush a pending draft when the user
    /// switches tools via the toolbar (the tool change bypasses the
    /// dispatcher, so the draft would otherwise stay modal forever).
    pub fn commit_text_draft(&mut self) {
        if let Some(session) = self.text_reedit.take() {
            self.finish_text_reedit(session);
            return;
        }
        let nonempty = matches!(
            &self.pending,
            Some(Annotation::Text { text, .. }) if !text.is_empty()
        );
        if nonempty {
            self.commit_annotation();
        } else {
            self.cancel_annotation();
        }
    }

    /// Lift `anns[index]` (a committed Text) out into `pending` and
    /// open a re-edit session over it.
    ///
    /// The caret lands at the end of the text (v1; caret-from-click
    /// needs shell-side glyph metrics). The selection is vacated for
    /// the session's duration — its slot in `anns` is gone, so a
    /// lingering selection would point `delete_selected` at
    /// the wrong annotation — and restored on commit / revert.
    fn begin_text_reedit(&mut self, index: usize) {
        let char_count = match self.anns.get(index) {
            Some(Annotation::Text { text, .. }) => text.chars().count(),
            _ => return,
        };
        let original = self.anns.remove(index);
        self.text_reedit = Some(TextReeditSession {
            index,
            original: original.clone(),
        });
        self.pending = Some(original);
        self.text_cursor = char_count;
        self.text_anchor = None;
        self.text_drag_seed = None;
        self.selected.clear();
    }

    /// Close a re-edit session by committing `pending`.
    ///
    /// Three outcomes, all history-exact:
    /// - pending missing or unchanged → splice the original back
    ///   silently (a no-op edit never records history);
    /// - changed → snapshot the pre-edit state (current `anns` with
    ///   `original` spliced at `index`) onto `undo`, clear `redo`,
    ///   then splice the edited text back at `index`;
    /// - edited to empty → the annotation is removed outright (same
    ///   single snapshot; nothing spliced back).
    fn finish_text_reedit(&mut self, session: TextReeditSession) {
        let edited = self.pending.take();
        self.text_anchor = None;
        let unchanged = edited.as_ref().is_none_or(|ann| *ann == session.original);
        let survives = matches!(&edited, Some(Annotation::Text { text, .. }) if !text.is_empty());
        let index = session.index;
        if unchanged {
            let at = index.min(self.anns.len());
            self.anns.insert(at, session.original);
            self.select_only(at);
            return;
        }
        // Snapshot-before-mutate: pre-edit state is `anns` with the
        // original back in its slot — one entry covers content and
        // any style changes made mid-session (lift-out invariant).
        let mut pre = self.anns.clone();
        pre.insert(index.min(pre.len()), session.original);
        self.undo.push(Snapshot::anns_only(pre, self.next_number));
        self.redo.clear();
        if survives {
            if let Some(edited) = edited {
                let at = index.min(self.anns.len());
                self.anns.insert(at, edited);
                self.select_only(at);
            }
        } else {
            self.selected.clear();
        }
    }

    /// Close a re-edit session by reverting: splice the original back
    /// at its index, drop `pending`, restore the selection. No
    /// history touch — revert is a return to the pre-session state,
    /// not a mutation of it.
    fn revert_text_reedit(&mut self, session: TextReeditSession) {
        self.pending = None;
        self.text_anchor = None;
        let index = session.index.min(self.anns.len());
        self.anns.insert(index, session.original);
        self.select_only(index);
    }

    /// Defensive guard for shell-direct mutations (`crop` / `undo` /
    /// `redo` bypass the modal dispatcher): restore the session's
    /// original into `anns` and drop the session + pending before the
    /// mutation runs, so it operates on the pre-session state instead
    /// of splicing against a vacant slot. Selection and any staged
    /// restyle gesture are dropped too — their indices and snapshots
    /// refer to pre-mutation state the mutation is about to replace.
    fn restore_active_reedit(&mut self) {
        if let Some(session) = self.text_reedit.take() {
            self.revert_text_reedit(session);
            self.selected.clear();
        }
        self.style_gesture_snapshot = None;
    }

    /// Stage the pre-gesture snapshot for a restyle gesture.
    ///
    /// Pairing contract: the shell calls `begin` when a style control
    /// interaction starts (a discrete click, or the press half of a
    /// slider / `DragValue` drag), any number of
    /// [`restyle_selected_text`](Self::restyle_selected_text) calls
    /// follow, and [`end_style_gesture`](Self::end_style_gesture)
    /// closes the gesture with exactly one undo entry.
    ///
    /// First-wins: a second `begin` inside one gesture is a no-op
    /// (each `end` consumes at most one staging). No-op entirely while
    /// a re-edit session is active — the lifted-out text is absent
    /// from `anns`, so an independent snapshot here would undo-delete
    /// it; mid-session restyles absorb into the session's single
    /// commit entry instead (spec Decisions).
    pub fn begin_style_gesture(&mut self) {
        if self.text_reedit.is_some() || self.style_gesture_snapshot.is_some() {
            return;
        }
        // Stage only when a restyle target exists: exactly one
        // selected annotation, and it is a `Text`. (No selection, a
        // multi-selection, or a non-Text selection makes the whole
        // gesture a no-op — nothing to snapshot for.)
        let text_selected = match self.selected.as_slice() {
            [i] => matches!(self.anns.get(*i), Some(Annotation::Text { .. })),
            _ => false,
        };
        if !text_selected {
            return;
        }
        self.style_gesture_snapshot =
            Some(Snapshot::anns_only(self.anns.clone(), self.next_number));
    }

    /// Write `style` onto the restyle target and sync the session
    /// defaults.
    ///
    /// Target resolution: the re-edit session's `pending` draft when a
    /// session is active, else the single selected annotation — which
    /// must be a `Text` (the Text tool only selects Text; a
    /// multi-selection, a non-Text selection, or nothing selected at
    /// all is a full no-op, defaults included).
    /// Never pushes history itself — bracket with
    /// [`begin_style_gesture`](Self::begin_style_gesture) /
    /// [`end_style_gesture`](Self::end_style_gesture). The anchor
    /// `pos` is never touched; the bbox grows with the font size.
    /// Session defaults (`font_size`, `text_color`, `font_family`,
    /// `font_weight`, `font_style`, `text_align`, `line_spacing`,
    /// `text_outline`, `text_background`) always follow the write so
    /// the next placed text inherits the selection's look.
    pub fn restyle_selected_text(&mut self, style: crate::text::TextStyle) {
        let wrote = if self.text_reedit.is_some() {
            self.pending
                .as_mut()
                .is_some_and(|ann| apply_text_style(ann, &style))
        } else {
            let [index] = self.selected.as_slice() else {
                return;
            };
            self.anns
                .get_mut(*index)
                .is_some_and(|ann| apply_text_style(ann, &style))
        };
        if !wrote {
            return;
        }
        self.font_size = style.size;
        self.text_color = style.color;
        self.font_family = style.family;
        self.font_weight = style.weight;
        self.font_style = style.style;
        self.text_align = style.align;
        self.line_spacing = style.line_spacing;
        self.text_outline = style.outline;
        self.text_background = style.background;
    }

    /// Close a restyle gesture: push the staged snapshot onto `undo`,
    /// clear `redo`, drop the staging. No-op when nothing is staged
    /// (including the whole mid-session case, where `begin` never
    /// staged).
    pub fn end_style_gesture(&mut self) {
        if let Some(snapshot) = self.style_gesture_snapshot.take() {
            self.undo.push(snapshot);
            self.redo.clear();
        }
    }

    /// Keyboard dispatch helper. See [`dispatch_input`](Self::dispatch_input).
    fn dispatch_key(
        &mut self,
        key: crate::geom::Key,
        modifiers: crate::input::Modifiers,
    ) -> crate::input::DispatchOutcome {
        use crate::geom::Key;
        use crate::input::DispatchOutcome;

        // macOS users typically press Cmd where Linux / Windows users
        // press Ctrl. The shell is responsible for mapping its native
        // "primary chord modifier" onto `Modifiers::ctrl` so this
        // dispatcher stays platform-agnostic.
        match key {
            Key::Escape => DispatchOutcome::Cancel,
            // Plain Del / Backspace delete the persistent selection.
            // Any modifier spoils the chord (Ctrl combos reserved);
            // the text-editing modal mode owns Backspace during a
            // draft before this handler runs.
            Key::Delete | Key::Backspace if modifiers == crate::input::Modifiers::default() => {
                self.delete_selected();
                DispatchOutcome::Continue
            }
            Key::Char('z' | 'Z') if modifiers.ctrl_only() => {
                self.undo();
                DispatchOutcome::Continue
            }
            Key::Char('z' | 'Z') if modifiers.ctrl_shift() => {
                self.redo();
                DispatchOutcome::Continue
            }
            Key::Char('y' | 'Y') if modifiers.ctrl_only() => {
                self.redo();
                DispatchOutcome::Continue
            }
            Key::Char('s' | 'S') if modifiers.ctrl_only() => DispatchOutcome::Save,
            Key::Char('c' | 'C') if modifiers.ctrl_only() => DispatchOutcome::Copy,
            _ => DispatchOutcome::Continue,
        }
    }

    /// Pointer-up dispatch helper: filter degenerate annotations, then
    /// commit. See [`dispatch_input`](Self::dispatch_input).
    ///
    /// Two filters, matching the Task 8 close-out's "deviation 3"
    /// zero-area contract (now in core so all shells inherit it):
    /// - Rect-shaped annotations (`Rect` / `Ellipse` / `Blur`) cancel
    ///   when either normalized dimension is < 1 px (a click without a
    ///   drag would otherwise commit a 0×0 rect).
    /// - `Arrow` cancels when the shaft is < 1 px long: a degenerate
    ///   arrow has no defined head direction, so the barbs would
    ///   render as nothing and the user would see a silent no-op
    ///   instead of the cancel they expect from the Rect tool's
    ///   click-without-drag behaviour (Task 11).
    /// - `Line` cancels under the same < 1 px length test: it is also
    ///   a two-point drag tool, so a click-without-drag would otherwise
    ///   commit an invisible zero-length segment. Grouped with Arrow
    ///   since both share the `a`/`b` endpoint shape (Task 002).
    fn finish_pointer_up(&mut self) {
        let degenerate = match &self.pending {
            Some(
                Annotation::Rect { rect, .. }
                | Annotation::Ellipse { rect, .. }
                | Annotation::Blur { rect, .. },
            ) => {
                let n = rect.normalized();
                n.width().abs() < 1.0 || n.height().abs() < 1.0
            }
            // Line and Arrow are both two-point tools; a sub-1px span is
            // a click-without-drag and commits nothing visible.
            Some(Annotation::Line { a, b, .. } | Annotation::Arrow { a, b, .. }) => {
                let dx = b.x - a.x;
                let dy = b.y - a.y;
                (dx * dx + dy * dy).sqrt() < 1.0
            }
            _ => false,
        };
        if degenerate {
            self.cancel_annotation();
            return;
        }
        self.commit_annotation();
    }
}

/// Default arrow-head barb length in physical pixels.
const ARROW_DEFAULT_HEAD_SIZE: f32 = 10.0;

/// Default blur strength (sigma for Gaussian, block size for Pixelate).
const BLUR_DEFAULT_STRENGTH: u8 = 8;

/// Default Number-tool bubble diameter in physical pixels. Big enough
/// to fit two-digit numbers (1–99) at `diameter * 0.6` font size,
/// matching the visual scale of numbered bubbles in flameshot /
/// sharex-style annotation tools.
const NUMBER_DEFAULT_DIAMETER: f32 = 28.0;

/// Minimum Number-tool bubble diameter in physical pixels. Below this
/// a two-digit label (font `diameter * 0.6`) drops under 10 px and
/// stops being legible on the annotation's own bubble. Public so the
/// properties panel's Size stepper clamps to the same bound core
/// documents, instead of duplicating a literal.
pub const NUMBER_MIN_DIAMETER: f32 = 16.0;

/// Maximum Number-tool bubble diameter in physical pixels. A practical
/// ceiling for labeling even 4K captures, well past the point where
/// the bubble stops reading as a marker. Public for the same reason
/// as [`NUMBER_MIN_DIAMETER`] — the panel's clamp lives in core.
pub const NUMBER_MAX_DIAMETER: f32 = 96.0;

/// Hit-test tolerance in physical pixels, added to half the stroke
/// width so thin lines are still grabbable.
const HIT_TEST_TOLERANCE_PX: f32 = 3.0;

/// Write a [`crate::text::TextStyle`] onto a `Text` annotation's
/// per-field style storage. Returns `false` (touching nothing) for
/// non-`Text` variants — the restyle API is Text-only this effort.
fn apply_text_style(ann: &mut Annotation, style: &crate::text::TextStyle) -> bool {
    let Annotation::Text {
        font_size,
        color,
        family,
        weight,
        style: font_style,
        align,
        line_spacing,
        outline,
        background,
        ..
    } = ann
    else {
        return false;
    };
    *font_size = style.size;
    *color = style.color;
    *family = style.family;
    *weight = style.weight;
    *font_style = style.style;
    *align = style.align;
    *line_spacing = style.line_spacing;
    *outline = style.outline;
    *background = style.background;
    true
}

/// Normalize any `Rect` fields in `ann` so `min` is top-left.
///
/// Called at commit time so committed storage always has normalized
/// rects; pending annotations may have un-normalized rects during the
/// drag (the anchor stays at `rect.min` until release).
fn normalize_rects(ann: &mut Annotation) {
    match ann {
        Annotation::Rect { rect, .. }
        | Annotation::Ellipse { rect, .. }
        | Annotation::Blur { rect, .. } => *rect = rect.normalized(),
        _ => {}
    }
}

/// Tool that owns (would produce) `ann`. Feeds the flip-to-select
/// ownership guard in [`EditorState::commit_annotation`].
fn owning_tool(ann: &Annotation) -> Tool {
    match ann {
        Annotation::Rect { .. } => Tool::Rect,
        Annotation::Ellipse { .. } => Tool::Ellipse,
        Annotation::Line { .. } => Tool::Line,
        Annotation::Arrow { .. } => Tool::Arrow,
        Annotation::Marker { .. } => Tool::Marker,
        Annotation::Pencil { .. } => Tool::Pencil,
        Annotation::Text { .. } => Tool::Text,
        Annotation::Blur { .. } => Tool::Blur,
        Annotation::Number { .. } => Tool::Number,
    }
}

/// Whether committing `tool`'s annotation flips the active tool to
/// Hand. Number is exempt — its click-to-place-the-next-number input
/// model exists for rapid sequential labeling; Select / Hand produce
/// no pending annotations.
fn is_flip_eligible(tool: Tool) -> bool {
    matches!(
        tool,
        Tool::Rect
            | Tool::Ellipse
            | Tool::Line
            | Tool::Arrow
            | Tool::Marker
            | Tool::Pencil
            | Tool::Blur
            | Tool::Text
    )
}

impl Annotation {
    /// Build the [`crate::text::TextStyle`] describing how a `Text`
    /// variant should be rendered.
    ///
    /// Panics for non-`Text` variants (callers gate on the variant).
    /// Bundling every text property into one [`crate::text::TextStyle`]
    /// keeps the [`crate::render::Canvas`] signature stable as new
    /// text properties are added.
    #[must_use]
    pub fn text_style(&self) -> crate::text::TextStyle {
        let Annotation::Text {
            font_size,
            color,
            family,
            weight,
            style,
            align,
            line_spacing,
            outline,
            background,
            ..
        } = self
        else {
            unreachable!("text_style only valid for Annotation::Text");
        };
        crate::text::TextStyle {
            size: *font_size,
            color: *color,
            family: *family,
            weight: *weight,
            style: *style,
            align: *align,
            line_spacing: *line_spacing,
            outline: *outline,
            background: *background,
        }
    }

    /// Construct a [`Annotation::Text`] from a [`crate::text::TextStyle`]
    /// bundle, expanding the style into the variant's individual fields.
    /// The inverse of [`text_style`](Self::text_style). Used by
    /// `begin_annotation` and by tests so neither has to enumerate
    /// every text field by hand.
    #[must_use]
    pub fn text(pos: Point, text: impl Into<String>, style: crate::text::TextStyle) -> Self {
        Annotation::Text {
            pos,
            text: text.into(),
            font_size: style.size,
            color: style.color,
            family: style.family,
            weight: style.weight,
            style: style.style,
            align: style.align,
            line_spacing: style.line_spacing,
            outline: style.outline,
            background: style.background,
        }
    }

    /// Clip this annotation to `bounds`, translate all coordinates by
    /// `-bounds.min`, and return whether the annotation should be kept.
    ///
    /// Returns `false` when the annotation is entirely outside
    /// `bounds` (drop it); `true` when any part survives. For
    /// rect-shaped variants the geometry is clipped to the overlap;
    /// for point-based variants (Line / Arrow / Marker / Pencil) the
    /// endpoints are translated verbatim and render-time clipping
    /// handles the visible edge.
    fn crop_and_translate(&mut self, bounds: Rect) -> bool {
        let n = bounds.normalized();
        let (dx, dy) = (n.min.x, n.min.y);
        match self {
            Annotation::Rect { rect, .. }
            | Annotation::Ellipse { rect, .. }
            | Annotation::Blur { rect, .. } => match rect.normalized().intersect(n) {
                Some(clip) => {
                    *rect = Rect::new(
                        Point::new(clip.min.x - dx, clip.min.y - dy),
                        Point::new(clip.max.x - dx, clip.max.y - dy),
                    );
                    true
                }
                None => false,
            },
            Annotation::Line { a, b, .. } | Annotation::Arrow { a, b, .. } => {
                let bbox = Rect::new(*a, *b).normalized();
                if bbox.intersect(n).is_none() {
                    return false;
                }
                a.x -= dx;
                a.y -= dy;
                b.x -= dx;
                b.y -= dy;
                true
            }
            Annotation::Marker { points, .. } | Annotation::Pencil { points, .. } => {
                if !points.iter().any(|&p| n.contains(p)) {
                    return false;
                }
                for p in points {
                    p.x -= dx;
                    p.y -= dy;
                }
                true
            }
            Annotation::Text { pos, .. } | Annotation::Number { pos, .. } => {
                if !n.contains(*pos) {
                    return false;
                }
                pos.x -= dx;
                pos.y -= dy;
                true
            }
        }
    }

    /// Translate every geometry field by `(dx, dy)`, verbatim — no
    /// clipping, no normalization change.
    ///
    /// The move-drag counterpart of
    /// [`crop_and_translate`](Self::crop_and_translate)'s field
    /// mapping (per-variant points / rects / polylines). Overflow
    /// past a region or image edge is owned downstream: paint-time
    /// canvas clip in the shells, and geometric clip / drop in
    /// [`EditorState::crop`].
    pub fn translate(&mut self, dx: f32, dy: f32) {
        match self {
            Annotation::Rect { rect, .. }
            | Annotation::Ellipse { rect, .. }
            | Annotation::Blur { rect, .. } => {
                rect.min.x += dx;
                rect.min.y += dy;
                rect.max.x += dx;
                rect.max.y += dy;
            }
            Annotation::Line { a, b, .. } | Annotation::Arrow { a, b, .. } => {
                a.x += dx;
                a.y += dy;
                b.x += dx;
                b.y += dy;
            }
            Annotation::Marker { points, .. } | Annotation::Pencil { points, .. } => {
                for p in points {
                    p.x += dx;
                    p.y += dy;
                }
            }
            Annotation::Text { pos, .. } | Annotation::Number { pos, .. } => {
                pos.x += dx;
                pos.y += dy;
            }
        }
    }

    /// Axis-aligned bounding box of the annotation, normalized so
    /// `min` is top-left. Returns `None` for annotations with no
    /// extent (empty polyline, zero-size rect).
    ///
    /// Used by hit-testing and by future select-tool bounding-box
    /// rendering. Per variant:
    /// - Rect / Ellipse / Blur: the rect itself (normalized).
    /// - Line / Arrow: the rect spanning both endpoints.
    /// - Marker / Pencil: the rect spanning all polyline points.
    /// - Text: a coarse approximation (`font_size` × estimated
    ///   advance width); real metrics land with Task 12.
    /// - Number: a square centred on `pos` with side `diameter`.
    #[must_use]
    pub fn bounding_box(&self) -> Option<Rect> {
        let rect = match self {
            Annotation::Rect { rect, .. }
            | Annotation::Ellipse { rect, .. }
            | Annotation::Blur { rect, .. } => *rect,
            Annotation::Line { a, b, .. } | Annotation::Arrow { a, b, .. } => Rect::new(*a, *b),
            Annotation::Marker { points, .. } | Annotation::Pencil { points, .. } => {
                if points.is_empty() {
                    return None;
                }
                let mut min = points[0];
                let mut max = points[0];
                for &p in &points[1..] {
                    min.x = min.x.min(p.x);
                    min.y = min.y.min(p.y);
                    max.x = max.x.max(p.x);
                    max.y = max.y.max(p.y);
                }
                Rect::new(min, max)
            }
            Annotation::Text {
                pos,
                text,
                font_size,
                line_spacing,
                ..
            } => {
                // Coarse approximation: ~half the font size per glyph of
                // advance. Width uses the longest line; height spans
                // every line at `font_size * line_spacing` leading.
                // Real glyph metrics live in the render backends; this
                // estimate is for hit-testing / select-tool bounds only.
                let (max_chars, line_count) =
                    text.split('\n')
                        .fold((0_usize, 0_usize), |(mut mx, mut n), line| {
                            mx = mx.max(line.chars().count());
                            n += 1;
                            (mx, n)
                        });
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "per-line char counts fit in f32's 24-bit mantissa"
                )]
                let approx_w = font_size * 0.5 * max_chars as f32;
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "line count fits in f32's 24-bit mantissa"
                )]
                let h = line_count as f32 * font_size * line_spacing;
                Rect::from_xywh(pos.x, pos.y, approx_w, h)
            }
            Annotation::Number { pos, diameter, .. } => {
                let half = diameter / 2.0;
                Rect::from_xywh(pos.x - half, pos.y - half, *diameter, *diameter)
            }
        };
        Some(rect.normalized())
    }

    /// Whether `point` lies on or inside this annotation.
    ///
    /// Used by [`EditorState::hit_test`]. Geometry per variant — see
    /// [`bounding_box`](Self::bounding_box) for the coarse bbox;
    /// fine-grained tests below use stroke-width-aware tolerances so
    /// thin lines remain selectable.
    #[must_use]
    pub fn contains(&self, point: Point) -> bool {
        match self {
            Annotation::Rect { rect, .. }
            | Annotation::Blur { rect, .. }
            | Annotation::Ellipse { rect, .. } => {
                let n = rect.normalized();
                point_in_rect(point, n)
            }
            Annotation::Line { a, b, stroke } | Annotation::Arrow { a, b, stroke, .. } => {
                distance_to_segment(point, *a, *b) <= stroke.width / 2.0 + HIT_TEST_TOLERANCE_PX
            }
            Annotation::Marker { points, width, .. } | Annotation::Pencil { points, width, .. } => {
                let tol = width / 2.0 + HIT_TEST_TOLERANCE_PX;
                points
                    .windows(2)
                    .any(|w| distance_to_segment(point, w[0], w[1]) <= tol)
            }
            Annotation::Text { .. } => self
                .bounding_box()
                .is_some_and(|bbox| point_in_rect(point, bbox)),
            Annotation::Number { pos, diameter, .. } => {
                let r = *diameter / 2.0 + HIT_TEST_TOLERANCE_PX;
                let dx = point.x - pos.x;
                let dy = point.y - pos.y;
                (dx * dx + dy * dy) <= r * r
            }
        }
    }
}

/// Per-pixel distance from `point` to the segment `a -> b`.
fn distance_to_segment(point: Point, a: Point, b: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq == 0.0 {
        0.0
    } else {
        (((point.x - a.x) * dx + (point.y - a.y) * dy) / len_sq).clamp(0.0, 1.0)
    };
    let proj = Point::new(a.x + t * dx, a.y + t * dy);
    let ex = point.x - proj.x;
    let ey = point.y - proj.y;
    (ex * ex + ey * ey).sqrt()
}

/// Whether `point` lies inside `rect` (boundaries inclusive).
fn point_in_rect(point: Point, rect: Rect) -> bool {
    point.x >= rect.min.x && point.x <= rect.max.x && point.y >= rect.min.y && point.y <= rect.max.y
}

// --- Text-cursor navigation helpers (char-index based) ---
//
// All operate on a `&str` draft plus a `char_idx` cursor (the count of
// chars before the insertion point). `\n` is a single byte in UTF-8 and
// never part of a multi-byte sequence, so scanning for it over char
// indices is sound.

/// Convert a char index into `s` to its matching byte index. A char
/// index at the end maps to `s.len()`.
fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map_or(s.len(), |(byte_idx, _)| byte_idx)
}

/// Char index of the first char on the line containing `char_idx`
/// (i.e. just past the last `\n` before it, or 0).
fn cursor_line_start(text: &str, char_idx: usize) -> usize {
    let mut start = 0;
    for (i, c) in text.chars().enumerate() {
        if i >= char_idx {
            break;
        }
        if c == '\n' {
            start = i + 1;
        }
    }
    start
}

/// Char index one past the last char on the line containing
/// `char_idx` (i.e. at the next `\n` at/after it, or the char count if
/// the line runs to the end).
fn cursor_line_end(text: &str, char_idx: usize) -> usize {
    for (i, c) in text.chars().enumerate() {
        if i < char_idx {
            continue;
        }
        if c == '\n' {
            return i;
        }
    }
    text.chars().count()
}

// --- Word-motion helpers (char-index based) ---
//
// The classic two-class word model: every non-whitespace char is
// either alphanumeric (word class) or not (punctuation class), and a
// "word" is a maximal run of one class. This is the boundary rule
// every desktop editor uses — Ctrl+Left stops at `foo|,` because the
// comma is a one-char word of the other class — and it keeps
// punctuation-adjacent double-clicks (Task 007) intuitive.

/// Word class of `c`: `true` = alphanumeric, `false` = punctuation.
/// Whitespace is neither — callers guard it out before classing.
fn word_class(c: char) -> bool {
    c.is_alphanumeric()
}

/// Char index of the start of the word at / before `char_idx`
/// (Ctrl+Left): skip any whitespace leftward, then the maximal
/// same-class run. Clamps at 0.
fn word_left(text: &str, char_idx: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut i = char_idx.min(chars.len());
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    if i > 0 {
        let class = word_class(chars[i - 1]);
        while i > 0 && !chars[i - 1].is_whitespace() && word_class(chars[i - 1]) == class {
            i -= 1;
        }
    }
    i
}

/// Char index of the start of the next word at / after `char_idx`
/// (Ctrl+Right): skip the same-class run under the cursor (if any),
/// then any trailing whitespace — i.e. land on the next word's start
/// or the text end. Mirrors [`word_left`]'s clamps.
fn word_right(text: &str, char_idx: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut i = char_idx.min(chars.len());
    if i < chars.len() && !chars[i].is_whitespace() {
        let class = word_class(chars[i]);
        while i < chars.len() && !chars[i].is_whitespace() && word_class(chars[i]) == class {
            i += 1;
        }
    }
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    i
}

/// Maximal word span `(start, end)` (char indices, `end` exclusive)
/// containing or adjacent to `idx` — the double-click payload. A
/// boundary directly after a word resolves to that word; a boundary
/// in a whitespace run resolves to the nearest word (backward
/// preference). Empty text yields `(0, 0)`.
fn word_bounds_around(text: &str, idx: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return (0, 0);
    }
    let idx = idx.min(chars.len());
    let at = |i: usize| chars.get(i).copied();
    let anchor = if at(idx).is_some_and(|c| !c.is_whitespace()) {
        idx
    } else if idx > 0 && at(idx - 1).is_some_and(|c| !c.is_whitespace()) {
        idx - 1
    } else {
        // Inside a whitespace run: nearest non-whitespace, backward
        // preferred (a click in the gap after a word reads as it).
        let mut j = idx;
        while j > 0 && at(j - 1).is_some_and(char::is_whitespace) {
            j -= 1;
        }
        if j > 0 {
            j - 1
        } else {
            let mut k = idx;
            while at(k).is_some_and(char::is_whitespace) {
                k += 1;
            }
            return (k, k);
        }
    };
    let class = word_class(chars[anchor]);
    let mut a = anchor;
    while a > 0 && !chars[a - 1].is_whitespace() && word_class(chars[a - 1]) == class {
        a -= 1;
    }
    let mut b = anchor + 1;
    while b < chars.len() && !chars[b].is_whitespace() && word_class(chars[b]) == class {
        b += 1;
    }
    (a, b)
}

/// Move `char_idx` up one line, preserving the column. Clamps to 0
/// when already on the first line.
fn cursor_up(text: &str, char_idx: usize) -> usize {
    let line_start = cursor_line_start(text, char_idx);
    if line_start == 0 {
        return 0;
    }
    let column = char_idx - line_start;
    let prev_end = line_start - 1;
    let prev_start = cursor_line_start(text, prev_end);
    let prev_len = prev_end - prev_start;
    prev_start + column.min(prev_len)
}

/// Move `char_idx` down one line, preserving the column. Clamps to the
/// end of the draft when already on the last line.
fn cursor_down(text: &str, char_idx: usize) -> usize {
    let total = text.chars().count();
    let line_end = cursor_line_end(text, char_idx);
    if line_end >= total {
        return total;
    }
    let column = char_idx - cursor_line_start(text, char_idx);
    let next_start = line_end + 1;
    let next_end = cursor_line_end(text, next_start);
    let next_len = next_end - next_start;
    next_start + column.min(next_len)
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::geom::Key;
    use crate::input::{DispatchOutcome, InputEvent, Modifiers};
    use crate::MARKER_ALPHA;

    fn blank_image() -> image::RgbaImage {
        image::RgbaImage::new(2, 2)
    }

    fn rect_ann(x1: f32, y1: f32, x2: f32, y2: f32) -> Annotation {
        Annotation::Rect {
            rect: Rect::new(Point::new(x1, y1), Point::new(x2, y2)),
            stroke: Stroke::new(Color::RED, 3.0),
            fill: None,
        }
    }

    #[test]
    fn new_state_has_sensible_defaults() {
        let s = EditorState::new(blank_image());
        assert_eq!(s.image.dimensions(), (2, 2));
        assert!(s.anns.is_empty());
        assert!(s.undo.is_empty());
        assert!(s.redo.is_empty());
        assert!(s.pending.is_none());
        assert_eq!(s.color, Color::RED);
        assert_eq!(s.stroke_width, 3.0);
        assert_eq!(s.blur_kind, BlurKind::Gaussian);
        assert_eq!(s.font_size, 16.0);
        assert_eq!(s.next_number, 1);
        assert!(s.selected.is_empty());
        assert!(s.crop_region.is_none(), "crop region starts None");
        assert!(s.marquee.is_none(), "marquee starts None");
        assert!(!s.marquee_additive, "marquee additivity starts plain");
        assert_eq!(s.tool, Tool::Select, "Select is the default tool");
        assert!(!s.flip_to_select, "flip policy defaults off (sticky tools)");
    }

    #[test]
    fn pending_is_none_in_new_state() {
        let s = EditorState::new(blank_image());
        assert!(s.pending.is_none());
    }

    #[test]
    fn tool_default_is_move() {
        assert_eq!(Tool::default(), Tool::Select);
    }

    #[test]
    fn blurkind_default_is_gaussian() {
        assert_eq!(BlurKind::default(), BlurKind::Gaussian);
    }

    #[test]
    fn annotation_serializes_roundtrip() {
        let ann = Annotation::Arrow {
            a: Point::new(1.0, 2.0),
            b: Point::new(3.0, 4.0),
            stroke: Stroke::new(Color::RED, 2.5),
            head_size: 10.0,
        };
        let json = serde_json::to_string(&ann).expect("serialize");
        let back: Annotation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ann, back);
    }

    /// `begin_annotation` then `update_annotation` should grow the
    /// pending annotation's geometry without committing.
    #[test]
    fn begin_then_update_extends_pending() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        s.begin_annotation(Point::new(10.0, 10.0));
        assert!(s.pending.is_some(), "begin produces a pending ann");
        s.update_annotation(Point::new(20.0, 30.0));
        match &s.pending {
            Some(Annotation::Rect { rect, .. }) => {
                assert_eq!(rect.min, Point::new(10.0, 10.0), "anchor preserved");
                assert_eq!(rect.max, Point::new(20.0, 30.0), "max follows pointer");
            }
            other => panic!("expected pending Rect, got {other:?}"),
        }
        assert!(s.anns.is_empty(), "update does not commit");
    }

    /// The marker tool stores a translucent color at construction time
    /// (alpha reduced to `MARKER_ALPHA`) so every backend renders it as
    /// a highlight without render-path special-casing. Pencil keeps the
    /// user's full-opacity color — the only freehand-tool divergence
    /// (ADR-0022).
    #[test]
    fn begin_marker_stores_translucent_color_pencil_stays_opaque() {
        let mut s = EditorState::new(blank_image());
        s.color = Color::rgb(255, 0, 0);
        s.stroke_width = 8.0;

        s.tool = Tool::Marker;
        s.begin_annotation(Point::new(5.0, 5.0));
        match &s.pending {
            Some(Annotation::Marker { color, width, .. }) => {
                assert_eq!(*color, Color::rgba(255, 0, 0, MARKER_ALPHA));
                assert!(*width >= 8.0, "marker keeps the user's stroke width");
            }
            other => panic!("expected pending Marker, got {other:?}"),
        }

        s.pending = None;
        s.tool = Tool::Pencil;
        s.begin_annotation(Point::new(6.0, 6.0));
        match &s.pending {
            Some(Annotation::Pencil { color, .. }) => {
                assert_eq!(*color, Color::rgb(255, 0, 0), "pencil stays fully opaque");
            }
            other => panic!("expected pending Pencil, got {other:?}"),
        }
    }

    /// Drag up-left from the anchor: anchor stays at `min`, current
    /// pointer at `max`. Normalization happens at commit, not during
    /// the drag.
    #[test]
    fn update_preserves_anchor_across_drag_direction_changes() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        s.begin_annotation(Point::new(10.0, 10.0));
        s.update_annotation(Point::new(5.0, 20.0));
        s.update_annotation(Point::new(15.0, 25.0));
        match &s.pending {
            Some(Annotation::Rect { rect, .. }) => {
                assert_eq!(rect.min, Point::new(10.0, 10.0));
                assert_eq!(rect.max, Point::new(15.0, 25.0));
            }
            other => panic!("unexpected pending {other:?}"),
        }
    }

    #[test]
    fn commit_normalizes_rect() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        s.begin_annotation(Point::new(40.0, 40.0));
        s.update_annotation(Point::new(10.0, 10.0));
        s.commit_annotation();
        assert!(s.pending.is_none(), "commit takes pending");
        assert_eq!(s.anns.len(), 1);
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => {
                assert_eq!(rect.min, Point::new(10.0, 10.0));
                assert_eq!(rect.max, Point::new(40.0, 40.0));
            }
            other => panic!("unexpected committed {other:?}"),
        }
    }

    /// TODO Task 7 acceptance: push 3 anns, undo twice, redo once,
    /// verify exact `anns` contents at each step.
    #[test]
    fn undo_redo_snapshot_roundtrip() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;

        // Fixtures identified by an index so each call produces a
        // fresh clone — `Annotation` is not `Copy`.
        let ann = |n: f32| rect_ann(0.0, 0.0, n, n);
        for n in [1.0_f32, 2.0, 3.0] {
            s.pending = Some(ann(n));
            s.commit_annotation();
        }
        let (a1, a2, a3) = (ann(1.0), ann(2.0), ann(3.0));
        assert_eq!(s.anns.len(), 3);
        assert_eq!(s.undo.len(), 3);

        // Undo once: anns = [a1, a2], redo = [[a1,a2,a3]]
        s.undo();
        assert_eq!(s.anns, vec![a1.clone(), a2.clone()]);
        assert_eq!(s.redo.len(), 1);
        assert_eq!(s.redo[0].anns, vec![a1.clone(), a2.clone(), a3.clone()]);
        assert!(s.redo[0].image.is_none(), "commit snapshots carry no image");

        // Undo twice: anns = [a1], redo = [[a1,a2,a3], [a1,a2]]
        s.undo();
        assert_eq!(s.anns, vec![a1.clone()]);
        assert_eq!(s.redo.len(), 2);
        assert_eq!(s.redo[1].anns, vec![a1.clone(), a2.clone()]);

        // Redo once: anns = [a1, a2], redo back to length 1
        s.redo();
        assert_eq!(s.anns, vec![a1, a2]);
        assert_eq!(s.redo.len(), 1);
        assert_eq!(s.undo.len(), 2);
    }

    /// TODO Task 7 acceptance: committing after an undo wipes redo.
    #[test]
    fn commit_clears_redo() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        for n in [1.0_f32, 2.0] {
            s.pending = Some(rect_ann(0.0, 0.0, n, n));
            s.commit_annotation();
        }
        s.undo();
        assert!(!s.redo.is_empty(), "undo populated redo");
        // Fresh commit should wipe redo.
        s.pending = Some(rect_ann(0.0, 0.0, 5.0, 5.0));
        s.commit_annotation();
        assert!(s.redo.is_empty(), "commit clears redo");
        assert_eq!(s.anns.len(), 2);
    }

    // --- flip-to-select policy (prominent-move-tool) ---

    /// Full pointer drag through the dispatcher with the active tool,
    /// committing a non-degenerate annotation on release.
    fn drag_commit(s: &mut EditorState, from: Point, to: Point) {
        s.dispatch_input(&down(from));
        s.dispatch_input(&InputEvent::PointerDrag { pos: to });
        s.dispatch_input(&InputEvent::PointerUp);
    }

    #[test]
    fn commit_flips_each_drag_tool_to_select() {
        for tool in [
            Tool::Rect,
            Tool::Ellipse,
            Tool::Line,
            Tool::Arrow,
            Tool::Marker,
            Tool::Pencil,
            Tool::Blur,
        ] {
            let mut s = EditorState::new(blank_image());
            s.tool = tool;
            s.flip_to_select = true;
            drag_commit(&mut s, Point::new(10.0, 10.0), Point::new(50.0, 40.0));
            assert_eq!(s.anns.len(), 1, "{tool:?} drag must commit");
            assert_eq!(s.tool, Tool::Select, "{tool:?} commit must flip to Select");
        }
    }

    #[test]
    fn text_enter_commit_flips_to_select() {
        let mut s = text_state();
        s.flip_to_select = true;
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::TextInput('h'));
        s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.anns.len(), 1);
        assert_eq!(s.tool, Tool::Select, "Enter commit must flip to Select");
    }

    #[test]
    fn commit_flip_off_keeps_tool_sticky() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        s.flip_to_select = false;
        drag_commit(&mut s, Point::new(10.0, 10.0), Point::new(50.0, 40.0));
        assert_eq!(s.anns.len(), 1);
        assert_eq!(s.tool, Tool::Rect, "policy off → tool stays");
    }

    #[test]
    fn commit_flip_requires_owning_tool_active() {
        // The shell's toolbar-switch flush changes `tool` before
        // calling `commit_text_draft`; the explicit tool choice must
        // win over the flip.
        let mut s = text_state();
        s.flip_to_select = true;
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::TextInput('h'));
        s.tool = Tool::Rect;
        s.commit_text_draft();
        assert_eq!(s.anns.len(), 1, "draft still commits");
        assert_eq!(s.tool, Tool::Rect, "stale-tool flush must not flip");
    }

    #[test]
    fn number_placement_does_not_flip() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Number;
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        assert_eq!(s.anns.len(), 1);
        assert_eq!(s.tool, Tool::Number, "Number never flips");
    }

    #[test]
    fn degenerate_cancel_does_not_flip() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        // Click without drag → zero-extent → cancel path, no commit.
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert!(s.anns.is_empty(), "degenerate rect cancels");
        assert_eq!(s.tool, Tool::Rect, "cancel must not flip");
    }

    #[test]
    fn flip_adds_no_undo_entry_and_undo_keeps_select() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        s.flip_to_select = true;
        drag_commit(&mut s, Point::new(10.0, 10.0), Point::new(50.0, 40.0));
        assert_eq!(
            s.undo.len(),
            1,
            "exactly the commit snapshot, no flip entry"
        );
        s.undo();
        assert!(s.anns.is_empty(), "undo removed the committed annotation");
        assert_eq!(s.tool, Tool::Select, "undo does not touch the flipped tool");
    }

    #[test]
    fn commit_with_move_already_active_never_flips_other_tool() {
        // A hand-injected pending whose owning tool differs from the
        // active tool (unreachable via normal input, defensive) must
        // not flip — the ownership guard blocks it.
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Hand;
        s.flip_to_select = true;
        s.pending = Some(rect_ann(0.0, 0.0, 10.0, 10.0));
        s.commit_annotation();
        assert_eq!(s.anns.len(), 1);
        assert_eq!(s.tool, Tool::Hand, "guard blocks non-owning commit");
    }

    /// TODO Task 7 acceptance: pending is None after cancel.
    #[test]
    fn cancel_drops_pending() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        s.begin_annotation(Point::new(1.0, 1.0));
        assert!(s.pending.is_some());
        s.cancel_annotation();
        assert!(s.pending.is_none());
        assert!(s.anns.is_empty(), "cancel does not commit");
    }

    /// TODO Task 7 acceptance: undo with empty history is a no-op.
    #[test]
    fn undo_with_empty_history_is_noop() {
        let mut s = EditorState::new(blank_image());
        s.anns = vec![rect_ann(0.0, 0.0, 1.0, 1.0)];
        s.undo();
        assert!(s.undo.is_empty());
        assert!(s.redo.is_empty());
        assert_eq!(s.anns.len(), 1, "anns unchanged");
    }

    /// TODO Task 7 acceptance: redo after a new commit is a no-op
    /// (because commit clears redo).
    #[test]
    fn redo_after_new_commit_is_noop() {
        let mut s = EditorState::new(blank_image());
        s.pending = Some(rect_ann(0.0, 0.0, 1.0, 1.0));
        s.commit_annotation();
        s.undo();
        s.pending = Some(rect_ann(0.0, 0.0, 2.0, 2.0));
        s.commit_annotation();
        s.redo();
        assert!(s.redo.is_empty());
        assert_eq!(s.anns.len(), 1);
    }

    /// Per the user decision: `undo()` mid-drag clears `pending`.
    #[test]
    fn undo_clears_pending() {
        let mut s = EditorState::new(blank_image());
        s.pending = Some(rect_ann(0.0, 0.0, 1.0, 1.0));
        s.anns = vec![rect_ann(0.0, 0.0, 5.0, 5.0)];
        s.undo();
        assert!(s.pending.is_none(), "undo clears pending");
    }

    /// `hit_test` returns the topmost annotation (highest index)
    /// whose geometry contains the point.
    #[test]
    fn hit_test_returns_topmost() {
        let mut s = EditorState::new(blank_image());
        // Two overlapping rects: the later one is "topmost".
        s.anns = vec![rect_ann(0.0, 0.0, 10.0, 10.0), rect_ann(0.0, 0.0, 5.0, 5.0)];
        assert_eq!(s.hit_test(Point::new(2.0, 2.0)), Some(1));
        // Only the outer rect covers this corner.
        assert_eq!(s.hit_test(Point::new(8.0, 8.0)), Some(0));
    }

    #[test]
    fn hit_test_returns_none_when_empty() {
        let s = EditorState::new(blank_image());
        assert_eq!(s.hit_test(Point::new(0.0, 0.0)), None);
    }

    #[test]
    fn hit_test_returns_none_when_miss() {
        let mut s = EditorState::new(blank_image());
        s.anns = vec![rect_ann(0.0, 0.0, 5.0, 5.0)];
        assert_eq!(s.hit_test(Point::new(50.0, 50.0)), None);
    }

    /// `bounding_box` returns the rect spanned by the annotation.
    /// For drag-from-bottom-right the stored rect is un-normalized
    /// during drag; bbox normalizes.
    #[test]
    fn bounding_box_normalizes_unnormalized_rect() {
        let ann = rect_ann(40.0, 40.0, 10.0, 10.0);
        let bbox = ann.bounding_box().expect("some bbox");
        assert_eq!(bbox.min, Point::new(10.0, 10.0));
        assert_eq!(bbox.max, Point::new(40.0, 40.0));
    }

    /// Line hit-test uses stroke-width-aware tolerance.
    #[test]
    fn hit_test_line_uses_stroke_tolerance() {
        let ann = Annotation::Line {
            a: Point::new(0.0, 0.0),
            b: Point::new(100.0, 0.0),
            stroke: Stroke::new(Color::RED, 4.0),
        };
        // 2 px above the line — within stroke half-width + tolerance.
        assert!(ann.contains(Point::new(50.0, -3.0)));
        // Far away.
        assert!(!ann.contains(Point::new(50.0, -50.0)));
    }

    /// `begin_annotation` for `Tool::Number` / `Select` / `Hand` is a
    /// no-op (different input model — those tools have dedicated entry
    /// points).
    /// `Tool::Text` is NOT in this list as of Task 12: a click places
    /// a caret (see `dispatch_text_click_places_caret`).
    #[test]
    fn begin_annotation_skips_non_drag_tools() {
        let mut s = EditorState::new(blank_image());
        for tool in [Tool::Number, Tool::Select, Tool::Crop, Tool::Hand] {
            s.tool = tool;
            s.begin_annotation(Point::new(1.0, 1.0));
            assert!(s.pending.is_none(), "{tool:?} should be a no-op");
        }
    }

    /// The fill toggle seeds closed-shape drafts with the
    /// annotation color; the default stays outlined-only.
    #[test]
    fn fill_toggle_controls_closed_shape_drafts() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        s.begin_annotation(Point::new(1.0, 1.0));
        assert!(
            matches!(&s.pending, Some(Annotation::Rect { fill: None, .. })),
            "default stays outlined-only"
        );

        s.fill_shapes = true;
        s.begin_annotation(Point::new(1.0, 1.0));
        match &s.pending {
            Some(Annotation::Rect {
                fill: Some(f),
                stroke,
                ..
            }) => {
                assert_eq!(*f, stroke.color, "fill uses the annotation color");
            }
            other => panic!("expected a filled rect, got {other:?}"),
        }

        // Line never gains a fill regardless of the toggle.
        s.tool = Tool::Line;
        s.begin_annotation(Point::new(1.0, 1.0));
        assert!(matches!(&s.pending, Some(Annotation::Line { .. })));
    }

    // --- Task 10: dispatch_input tests ---
    //
    // The shell's translation layer is too thin to unit-test
    // meaningfully without a synthetic `egui::Context`; the
    // dispatcher is where every keyboard + pointer contract lives.
    // These tests are the "synthetic Key events through the shell's
    // dispatcher" verification the TODO spec asks for, just running
    // directly against `EditorState::dispatch_input` rather than a
    // shell wrapper.

    fn ctrl(ch: char) -> InputEvent {
        InputEvent::Key {
            key: Key::Char(ch),
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        }
    }

    fn ctrl_shift(ch: char) -> InputEvent {
        InputEvent::Key {
            key: Key::Char(ch),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::default()
            },
        }
    }

    fn key(key: Key) -> InputEvent {
        InputEvent::Key {
            key,
            modifiers: Modifiers::default(),
        }
    }

    fn down(pos: Point) -> InputEvent {
        InputEvent::PointerDown {
            pos,
            modifiers: Modifiers::default(),
        }
    }

    fn shift_down(pos: Point) -> InputEvent {
        InputEvent::PointerDown {
            pos,
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        }
    }

    fn ctrl_down(pos: Point) -> InputEvent {
        InputEvent::PointerDown {
            pos,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        }
    }

    /// Drag a marquee from `from` to `to` under Select and release.
    fn marquee(s: &mut EditorState, from: Point, to: Point) {
        s.dispatch_input(&down(from));
        s.dispatch_input(&InputEvent::PointerDrag { pos: to });
        s.dispatch_input(&InputEvent::PointerUp);
    }

    #[test]
    fn marquee_release_replaces_set_with_intersections() {
        let mut s = three_ann_move_state();
        // Pre-select ann 2 (outside the band the marquee will cover).
        s.dispatch_input(&down(Point::new(85.0, 60.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![2], "fixture: ann 2 selected");
        // Marquee over the left band: intersects ann 0 only.
        marquee(&mut s, Point::new(5.0, 45.0), Point::new(30.0, 75.0));
        assert_eq!(s.selected, vec![0], "plain marquee replaces the set");
        // Wide marquee: every bbox intersects.
        marquee(&mut s, Point::new(0.0, 40.0), Point::new(100.0, 80.0));
        assert_eq!(s.selected, vec![0, 1, 2], "wide marquee selects all");
    }

    #[test]
    fn marquee_release_merges_with_shift_or_ctrl() {
        let mut s = three_ann_move_state();
        // Shift marquee over the left band: hits ann 0.
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(5.0, 45.0),
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        });
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(30.0, 75.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0], "shift marquee selects ann 0");
        // Ctrl marquee over the right band: merges anns 1 + 2 in.
        s.dispatch_input(&ctrl_down(Point::new(55.0, 45.0)));
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(100.0, 75.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(
            s.selected,
            vec![0, 1, 2],
            "ctrl marquee merges into the existing set (union, not toggle)"
        );
        // Re-marqueeing the same band under Ctrl keeps the set
        // stable — merge is idempotent per member.
        s.dispatch_input(&ctrl_down(Point::new(55.0, 45.0)));
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(100.0, 75.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0, 1, 2], "merge never toggles members out");
    }

    #[test]
    fn degenerate_marquee_selects_nothing() {
        let mut s = two_ann_move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0], "fixture: one member");
        // A sub-pixel drag over empty space: the press cleared the
        // set (plain), the degenerate release adds nothing.
        marquee(&mut s, Point::new(70.0, 70.0), Point::new(70.5, 70.0));
        assert!(s.selected.is_empty(), "degenerate marquee selects nothing");
        assert!(s.marquee.is_none(), "the marquee never survives release");
    }

    /// Ctrl+click toggles membership exactly like Shift+click:
    /// toggle-in grabs for a drag, toggle-out starts nothing.
    #[test]
    fn ctrl_click_toggles_membership_in_and_out() {
        let mut s = two_ann_move_state();
        s.dispatch_input(&ctrl_down(Point::new(30.0, 25.0)));
        assert!(s.moving.is_some(), "ctrl toggle-in grabs the annotation");
        s.dispatch_input(&InputEvent::PointerUp);
        s.dispatch_input(&ctrl_down(Point::new(75.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0, 1], "ctrl-click adds to the set");
        s.dispatch_input(&ctrl_down(Point::new(75.0, 25.0)));
        assert!(s.moving.is_none(), "ctrl toggle-out starts no drag");
        assert_eq!(s.selected, vec![0], "ctrl-click on a member removes it");
    }

    /// Hand is pan-only at the core too: presses, drags, and releases
    /// under Hand never mutate annotation or selection state — the
    /// shell owns panning and must be free to (defensively) route
    /// stray synthetic events here without side effects.
    #[test]
    fn hand_events_never_touch_annotations_or_selection() {
        let mut s = move_state();
        s.tool = Tool::Hand;
        s.selected = vec![0];
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(60.0, 45.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        assert!(s.moving.is_none(), "Hand press starts no drag");
        assert!(s.marquee.is_none(), "Hand press starts no marquee");
        assert_eq!(s.selected, vec![0], "Hand press keeps the set (no clear)");
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => assert_eq!(rect.min, Point::new(10.0, 10.0)),
            other => panic!("expected Rect, got {other:?}"),
        }
        assert!(s.undo.is_empty(), "Hand gesture records no history");
    }

    #[test]
    fn dispatch_esc_returns_cancel() {
        let mut s = EditorState::new(blank_image());
        assert_eq!(s.dispatch_input(&key(Key::Escape)), DispatchOutcome::Cancel);
        // Cancel is purely a side-effect request — state itself is
        // untouched. The shell is responsible for any state rollback
        // it wants before closing the window.
        assert_eq!(s.anns.len(), 0);
        assert!(s.pending.is_none());
    }

    #[test]
    fn dispatch_ctrl_z_undoes_last_commit() {
        let mut s = EditorState::new(blank_image());
        s.anns.push(rect_ann(0.0, 0.0, 10.0, 10.0));
        s.anns.push(rect_ann(20.0, 20.0, 30.0, 30.0));
        // Take a snapshot manually so undo has something to pop (the
        // state machine's `commit_annotation` normally does this; we
        // bypass it here to isolate dispatcher behaviour).
        s.undo
            .push(Snapshot::anns_only(vec![rect_ann(0.0, 0.0, 10.0, 10.0)], 1));

        assert_eq!(s.dispatch_input(&ctrl('z')), DispatchOutcome::Continue);
        assert_eq!(s.anns.len(), 1);
        assert_eq!(s.anns[0], rect_ann(0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn dispatch_ctrl_z_clears_pending() {
        // Ctrl+Z mid-drag cancels the in-flight annotation — matches
        // the `undo` doc comment (Task 7 deviation 2).
        let mut s = EditorState::new(blank_image());
        s.pending = Some(rect_ann(0.0, 0.0, 5.0, 5.0));
        assert_eq!(s.dispatch_input(&ctrl('z')), DispatchOutcome::Continue);
        assert!(s.pending.is_none());
    }

    #[test]
    fn dispatch_ctrl_y_redoes() {
        let mut s = EditorState::new(blank_image());
        s.anns.push(rect_ann(0.0, 0.0, 10.0, 10.0));
        s.undo.push(Snapshot::anns_only(Vec::new(), 1));
        s.redo
            .push(Snapshot::anns_only(vec![rect_ann(0.0, 0.0, 10.0, 10.0)], 1));
        s.anns.clear();

        assert_eq!(s.dispatch_input(&ctrl('y')), DispatchOutcome::Continue);
        assert_eq!(s.anns.len(), 1);
        assert_eq!(s.anns[0], rect_ann(0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn dispatch_ctrl_shift_z_redoes() {
        let mut s = EditorState::new(blank_image());
        s.anns.push(rect_ann(0.0, 0.0, 10.0, 10.0));
        s.undo.push(Snapshot::anns_only(Vec::new(), 1));
        s.redo
            .push(Snapshot::anns_only(vec![rect_ann(0.0, 0.0, 10.0, 10.0)], 1));
        s.anns.clear();

        assert_eq!(
            s.dispatch_input(&ctrl_shift('z')),
            DispatchOutcome::Continue
        );
        assert_eq!(s.anns.len(), 1);
    }

    #[test]
    fn dispatch_ctrl_s_returns_save() {
        let mut s = EditorState::new(blank_image());
        assert_eq!(s.dispatch_input(&ctrl('s')), DispatchOutcome::Save);
        // The dispatcher does not touch state for Save — the shell
        // performs the save pipeline after seeing the outcome.
        assert_eq!(s.anns.len(), 0);
    }

    #[test]
    fn dispatch_ctrl_c_returns_copy() {
        let mut s = EditorState::new(blank_image());
        assert_eq!(s.dispatch_input(&ctrl('c')), DispatchOutcome::Copy);
    }

    #[test]
    fn dispatch_unknown_key_continues() {
        let mut s = EditorState::new(blank_image());
        assert_eq!(
            s.dispatch_input(&key(Key::Char('q'))),
            DispatchOutcome::Continue
        );
        assert_eq!(s.dispatch_input(&key(Key::Tab)), DispatchOutcome::Continue);
        assert_eq!(
            s.dispatch_input(&key(Key::Other)),
            DispatchOutcome::Continue
        );
    }

    #[test]
    fn dispatch_ignores_shortcut_when_modifiers_dont_match() {
        let mut s = EditorState::new(blank_image());
        // Shift+S alone (no Ctrl) — not a shortcut.
        assert_eq!(
            s.dispatch_input(&InputEvent::Key {
                key: Key::Char('s'),
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                }
            }),
            DispatchOutcome::Continue
        );
        // Ctrl+Alt+S — Alt spoils the chord.
        assert_eq!(
            s.dispatch_input(&InputEvent::Key {
                key: Key::Char('s'),
                modifiers: Modifiers {
                    ctrl: true,
                    alt: true,
                    ..Modifiers::default()
                }
            }),
            DispatchOutcome::Continue
        );
        // Plain S with no modifiers — not a shortcut.
        assert_eq!(
            s.dispatch_input(&key(Key::Char('s'))),
            DispatchOutcome::Continue
        );
    }

    #[test]
    fn dispatch_pointer_down_begins_annotation() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        assert_eq!(
            s.dispatch_input(&down(Point::new(5.0, 5.0))),
            DispatchOutcome::Continue
        );
        assert!(s.pending.is_some());
    }

    #[test]
    fn dispatch_pointer_down_drag_up_commits_rect() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(50.0, 60.0),
        });
        assert_eq!(
            s.dispatch_input(&InputEvent::PointerUp),
            DispatchOutcome::Continue
        );
        assert!(s.pending.is_none(), "pending cleared on commit");
        assert_eq!(s.anns.len(), 1);
        // The committed rect should be normalized to (10,10)-(50,60).
        if let Annotation::Rect { rect, .. } = &s.anns[0] {
            assert_eq!(rect.min, Point::new(10.0, 10.0));
            assert_eq!(rect.max, Point::new(50.0, 60.0));
        } else {
            panic!("expected Rect, got {:?}", s.anns[0]);
        }
    }

    #[test]
    fn dispatch_pointer_down_immediate_up_cancels_zero_area() {
        // Task 8 deviation 3 contract: a click without a drag does not
        // commit a 0×0 rect. The zero-area filter moved into the
        // dispatcher in Task 10 so all shells inherit it.
        let mut s = EditorState::new(blank_image());
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert!(s.pending.is_none(), "pending cancelled, not committed");
        assert_eq!(s.anns.len(), 0, "no annotation committed");
    }

    #[test]
    fn dispatch_pointer_drag_without_down_is_noop() {
        // Stray drag event (e.g. the shell forwards one before the
        // first PointerDown due to a focus change) doesn't crash and
        // doesn't conjure an annotation.
        let mut s = EditorState::new(blank_image());
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(5.0, 5.0),
        });
        assert!(s.pending.is_none());
        assert_eq!(s.anns.len(), 0);
    }

    /// TODO Task 11 acceptance: drag with Arrow tool active commits an
    /// `Annotation::Arrow` with tail at the down point and head at the
    /// up point. Unlike Rect, arrow endpoints are **not** normalized —
    /// direction matters (tail = where the press went down, head = where
    /// the release went up). `head_size` defaults to 10 physical px;
    /// the shaft inherits `state.stroke_width`.
    #[test]
    fn dispatch_pointer_down_drag_up_commits_arrow() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Arrow;
        s.stroke_width = 4.0;
        s.color = Color::rgb(0, 0, 255);

        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(50.0, 60.0),
        });
        assert_eq!(
            s.dispatch_input(&InputEvent::PointerUp),
            DispatchOutcome::Continue
        );

        assert!(s.pending.is_none(), "pending cleared on commit");
        assert_eq!(s.anns.len(), 1);
        match &s.anns[0] {
            Annotation::Arrow {
                a,
                b,
                stroke,
                head_size,
            } => {
                assert_eq!(*a, Point::new(10.0, 10.0), "tail = down point");
                assert_eq!(*b, Point::new(50.0, 60.0), "head = up point");
                assert_eq!(stroke.color, Color::rgb(0, 0, 255));
                assert_eq!(stroke.width, 4.0, "shaft inherits stroke_width");
                assert_eq!(*head_size, 10.0, "head_size defaults to 10 px");
            }
            other => panic!("expected Arrow, got {other:?}"),
        }
    }

    /// TODO Task 11 acceptance: arrow drag is not subject to the
    /// zero-area rect filter — even a tiny drag commits a real arrow
    /// (the head still has barbs, unlike a 0×0 rect which is
    /// meaningless). A click-without-drag still cancels because the
    /// shaft is degenerate, matching the Rect tool's "click cancels"
    /// contract for consistency.
    #[test]
    fn dispatch_arrow_click_without_drag_cancels() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Arrow;
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert!(s.pending.is_none(), "degenerate arrow cancelled");
        assert_eq!(s.anns.len(), 0, "no annotation committed");
    }

    /// Mirrors the Arrow degenerate-cancel contract above for the other
    /// two-point drag tool: a Line click-without-drag (`a == b`) is
    /// sub-1px long and must cancel rather than commit an invisible
    /// zero-length segment.
    #[test]
    fn dispatch_line_click_without_drag_cancels() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Line;
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert!(s.pending.is_none(), "degenerate line cancelled");
        assert_eq!(s.anns.len(), 0, "no annotation committed");
    }

    // --- Task 12: text tool state machine tests ---
    //
    // Text editing is a click-to-place-then-type model (not a drag).
    // The dispatcher enters "text editing mode" once a PointerDown
    // with `Tool::Text` creates a `pending: Annotation::Text`; while
    // editing, keyboard shortcuts (Ctrl+S/Z/etc.) are suppressed and
    // only text-affecting events (TextInput, Backspace, Enter, Esc)
    // are honoured. Enter commits; Esc cancels; an empty draft never
    // commits.

    fn text_state() -> EditorState {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Text;
        s
    }

    #[test]
    fn dispatch_text_click_places_caret() {
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(30.0, 40.0)));
        match &s.pending {
            Some(Annotation::Text { pos, text, .. }) => {
                assert_eq!(*pos, Point::new(30.0, 40.0), "caret at click position");
                assert!(text.is_empty(), "draft starts empty");
            }
            other => panic!("expected pending Text, got {other:?}"),
        }
    }

    #[test]
    fn text_style_round_trips_through_annotation_text() {
        // `Annotation::text` expands a TextStyle into the variant's
        // fields; `text_style` folds them back. The pair must be a
        // round-trip so the two render backends always see the same
        // style bundle the editor captured.
        let style = crate::text::TextStyle {
            size: 24.0,
            color: Color::rgb(1, 2, 3),
            family: crate::text::FontFamily::Monospace,
            weight: crate::text::FontWeight::Bold,
            style: crate::text::FontStyle::Italic,
            align: crate::text::TextAlign::Center,
            line_spacing: 1.5,
            outline: Some(crate::text::TextOutline {
                color: Color::rgb(0, 0, 0),
                width: 3.0,
            }),
            background: Some(Color::rgb(255, 235, 0)),
        };
        let ann = Annotation::text(Point::new(1.0, 2.0), "body", style);
        assert_eq!(ann.text_style(), style, "text_style() must invert text()");
    }

    #[test]
    fn begin_text_annotation_snapshots_text_color_not_tool_color() {
        // The Text tool's colour is decoupled from the drawing-tools
        // `color`; `begin_annotation` must snapshot `text_color` so a
        // red arrow and a white label can coexist.
        let mut s = text_state();
        s.color = Color::rgb(255, 0, 0);
        s.text_color = Color::rgb(255, 255, 255);
        s.dispatch_input(&down(Point::new(5.0, 5.0)));
        match &s.pending {
            Some(Annotation::Text { color, .. }) => {
                assert_eq!(*color, Color::rgb(255, 255, 255), "uses text_color");
            }
            other => panic!("expected pending Text, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_text_typing_appends_chars() {
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::TextInput('h'));
        s.dispatch_input(&InputEvent::TextInput('i'));
        match &s.pending {
            Some(Annotation::Text { text, .. }) => assert_eq!(text, "hi"),
            other => panic!("expected pending Text, got {other:?}"),
        }
        assert_eq!(s.anns.len(), 0, "typing does not commit");
    }

    #[test]
    fn dispatch_text_backspace_deletes_last_char() {
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        for ch in "hi!".chars() {
            s.dispatch_input(&InputEvent::TextInput(ch));
        }
        s.dispatch_input(&InputEvent::Key {
            key: Key::Backspace,
            modifiers: Modifiers::default(),
        });
        match &s.pending {
            Some(Annotation::Text { text, .. }) => assert_eq!(text, "hi"),
            other => panic!("expected pending Text, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_text_enter_commits_nonempty_draft() {
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        for ch in "hello".chars() {
            s.dispatch_input(&InputEvent::TextInput(ch));
        }
        assert_eq!(
            s.dispatch_input(&InputEvent::Key {
                key: Key::Enter,
                modifiers: Modifiers::default(),
            }),
            DispatchOutcome::Continue
        );
        assert!(s.pending.is_none(), "Enter clears pending");
        assert_eq!(s.anns.len(), 1);
        match &s.anns[0] {
            Annotation::Text { text, pos, .. } => {
                assert_eq!(text, "hello");
                assert_eq!(*pos, Point::new(10.0, 10.0));
            }
            other => panic!("expected committed Text, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_text_shift_enter_inserts_newline() {
        // Shift+Enter breaks the line instead of committing; a
        // subsequent plain Enter commits the multi-line draft whole.
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        for ch in "ab".chars() {
            s.dispatch_input(&InputEvent::TextInput(ch));
        }
        s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        });
        for ch in "cd".chars() {
            s.dispatch_input(&InputEvent::TextInput(ch));
        }
        // Still editing — Shift+Enter must not commit.
        assert!(s.is_editing_text(), "Shift+Enter keeps the draft open");
        match &s.pending {
            Some(Annotation::Text { text, .. }) => assert_eq!(text, "ab\ncd"),
            other => panic!("expected pending Text, got {other:?}"),
        }
        // Plain Enter commits the two-line draft.
        s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.anns.len(), 1);
        match &s.anns[0] {
            Annotation::Text { text, .. } => assert_eq!(text, "ab\ncd"),
            other => panic!("expected committed multi-line Text, got {other:?}"),
        }
    }

    #[test]
    fn shift_enter_advances_cursor_to_new_line() {
        // Regression: after Shift+Enter the caret must sit on the new
        // (second) line, not stay pinned to the first. The cursor
        // (char index) should point just past the '\n'.
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        for ch in "ab".chars() {
            s.dispatch_input(&InputEvent::TextInput(ch));
        }
        s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        });
        // "ab\n" = 3 chars; cursor at 3 means the next typed char lands
        // on line 1 (the second line), column 0.
        assert_eq!(s.text_cursor, 3, "cursor advanced past the newline");
        s.dispatch_input(&InputEvent::TextInput('x'));
        match &s.pending {
            Some(Annotation::Text { text, .. }) => assert_eq!(text, "ab\nx"),
            other => panic!("expected pending Text, got {other:?}"),
        }
    }

    #[test]
    fn arrow_left_then_type_inserts_mid_string() {
        // Arrow Left retreats the cursor so the next typed char
        // inserts before the last char rather than appending.
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        for ch in "abc".chars() {
            s.dispatch_input(&InputEvent::TextInput(ch));
        }
        assert_eq!(s.text_cursor, 3);
        s.dispatch_input(&InputEvent::Key {
            key: Key::ArrowLeft,
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.text_cursor, 2, "Left retreated cursor");
        s.dispatch_input(&InputEvent::TextInput('X'));
        match &s.pending {
            Some(Annotation::Text { text, .. }) => assert_eq!(text, "abXc"),
            other => panic!("expected pending Text, got {other:?}"),
        }
        // Cursor now sits between X and c.
        assert_eq!(s.text_cursor, 3);
    }

    #[test]
    fn backspace_deletes_at_cursor_mid_string() {
        // Backspace removes the char immediately before the cursor,
        // not always the last char of the draft.
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        for ch in "abc".chars() {
            s.dispatch_input(&InputEvent::TextInput(ch));
        }
        // Move to between 'a' and 'b' (cursor 1).
        s.dispatch_input(&InputEvent::Key {
            key: Key::ArrowLeft,
            modifiers: Modifiers::default(),
        });
        s.dispatch_input(&InputEvent::Key {
            key: Key::ArrowLeft,
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.text_cursor, 1);
        s.dispatch_input(&InputEvent::Key {
            key: Key::Backspace,
            modifiers: Modifiers::default(),
        });
        match &s.pending {
            Some(Annotation::Text { text, .. }) => assert_eq!(text, "bc"),
            other => panic!("expected pending Text, got {other:?}"),
        }
        assert_eq!(s.text_cursor, 0);
    }

    #[test]
    fn arrow_keys_navigate_multiline() {
        // Up/Down preserve the column across lines; Left/Right wrap
        // across '\n'; Home/End jump to the line edges.
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        // Build "abc\ndefgh" (line 0 width 3, line 1 width 5).
        for ch in "abc\ndefgh".chars() {
            s.dispatch_input(&InputEvent::TextInput(ch));
        }
        assert_eq!(s.text_cursor, 9, "cursor at end");
        // Home → start of line 1 (char 4).
        s.dispatch_input(&InputEvent::Key {
            key: Key::Home,
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.text_cursor, 4, "Home → line start");
        // Up → column 0 of line 0 (char 0).
        s.dispatch_input(&InputEvent::Key {
            key: Key::ArrowUp,
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.text_cursor, 0, "Up to first line, col 0");
        // End → end of line 0 (char 3, before '\n').
        s.dispatch_input(&InputEvent::Key {
            key: Key::End,
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.text_cursor, 3, "End → line end");
        // Down → same column (3) on line 1 (char 7).
        s.dispatch_input(&InputEvent::Key {
            key: Key::ArrowDown,
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.text_cursor, 7, "Down preserves column onto line 1");
        // Down again → already last line, clamp to end (9).
        s.dispatch_input(&InputEvent::Key {
            key: Key::ArrowDown,
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.text_cursor, 9, "Down on last line clamps to end");
    }

    #[test]
    fn cursor_helpers_unit_checks() {
        // Direct checks on the free navigation helpers.
        // char indices: a0 b1 \n2 c3 d4 e5 \n6 f7  (count = 8)
        let text = "ab\ncde\nf";
        assert_eq!(cursor_line_start(text, 4), 3, "line start of 'd'");
        assert_eq!(cursor_line_end(text, 4), 6, "line end of 'd' is the \\n");
        assert_eq!(cursor_up(text, 4), 1, "up from 'd' col1 -> 'b' col1");
        assert_eq!(cursor_down(text, 1), 4, "down from 'b' col1 -> 'd' col1");
        // Up from first line clamps to 0.
        assert_eq!(cursor_up(text, 1), 0);
        // Down on the last line clamps to the end of the text (8).
        assert_eq!(cursor_down(text, 7), 8, "down on last line -> end");
        // Column clamps to a shorter target line: col 2 of "cde" down
        // to "f" (len 1) lands past 'f' at 8.
        assert_eq!(cursor_down(text, 5), 8, "column clamped to last line len");
        // Separate text for a clean mid-line column clamp:
        // "abcd\nef" -> a0 b1 c2 d3 \n4 e5 f6 (count 7). Down from
        // col 3 of line 0 to line 1 (len 2) clamps to col 2 -> pos 7.
        let t2 = "abcd\nef";
        assert_eq!(cursor_down(t2, 3), 7, "col 3 clamps to line-1 len 2");
    }

    #[test]
    fn dispatch_text_enter_empty_cancels() {
        // Placing the caret and immediately pressing Enter must NOT
        // commit an empty Text annotation — that would litter the
        // annotation list with invisible empties.
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        assert!(s.pending.is_none(), "empty draft cancelled");
        assert_eq!(s.anns.len(), 0, "nothing committed");
    }

    #[test]
    fn dispatch_text_escape_cancels_draft() {
        // Esc while editing cancels the DRAFT (stays in the editor),
        // unlike Esc outside editing which cancels the EDITOR (closes
        // the window). The dispatcher distinguishes via the outcome:
        // Continue here vs Cancel for editor-close.
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        for ch in "draft".chars() {
            s.dispatch_input(&InputEvent::TextInput(ch));
        }
        assert_eq!(
            s.dispatch_input(&InputEvent::Key {
                key: Key::Escape,
                modifiers: Modifiers::default(),
            }),
            DispatchOutcome::Continue,
            "Esc while editing returns Continue (stays in editor)"
        );
        assert!(s.pending.is_none(), "draft cancelled");
        assert_eq!(s.anns.len(), 0);
    }

    #[test]
    fn dispatch_text_editing_bypasses_ctrl_s() {
        // While editing text, Ctrl+S must not trigger Save (the user
        // is mid-edit; committing the draft first is the user's job).
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::TextInput('a'));
        assert_eq!(
            s.dispatch_input(&InputEvent::Key {
                key: Key::Char('s'),
                modifiers: Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
            }),
            DispatchOutcome::Continue,
            "Ctrl+S suppressed while editing text"
        );
    }

    #[test]
    fn dispatch_text_editing_bypasses_ctrl_z() {
        // Ctrl+Z while editing text does NOT undo committed annotations
        // (it would be jarring to lose prior work while typing). The
        // user escapes or commits the draft first to regain undo.
        let mut s = EditorState::new(blank_image());
        s.anns.push(Annotation::Rect {
            rect: Rect::from_xywh(0.0, 0.0, 5.0, 5.0),
            stroke: Stroke::new(Color::RED, 1.0),
            fill: None,
        });
        s.tool = Tool::Text;
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        assert_eq!(
            s.dispatch_input(&InputEvent::Key {
                key: Key::Char('z'),
                modifiers: Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
            }),
            DispatchOutcome::Continue
        );
        assert_eq!(s.anns.len(), 1, "prior annotation untouched");
        assert!(s.pending.is_some(), "draft still active");
    }

    #[test]
    fn dispatch_text_pointer_commits_and_flips_to_select() {
        // Clicking elsewhere while a draft is active commits the
        // current draft. With the flip policy on (opt-in), the tool
        // switches to Select — the click's own follow-up action then
        // belongs to Select (here: a miss on the freshly committed
        // text, so nothing begins). The old behavior placed a new
        // caret; that now requires staying on Text via a re-press.
        let mut s = text_state();
        s.flip_to_select = true;
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::TextInput('h'));
        // Click somewhere else while the draft is active.
        s.dispatch_input(&down(Point::new(50.0, 50.0)));
        assert_eq!(s.anns.len(), 1, "previous draft committed");
        assert!(s.pending.is_none(), "no new caret after the flip");
        assert_eq!(s.tool, Tool::Select, "commit flipped the tool to Select");
        match &s.anns[0] {
            Annotation::Text { pos, text, .. } => {
                assert_eq!(*pos, Point::new(10.0, 10.0));
                assert_eq!(text, "h");
            }
            other => panic!("expected committed Text, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_text_pointer_empty_draft_cancels_then_repositions() {
        // Clicking elsewhere with an empty draft cancels it (no empty
        // annotation committed) and places a new caret.
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        // No text typed — draft is empty.
        s.dispatch_input(&down(Point::new(50.0, 50.0)));
        assert_eq!(s.anns.len(), 0, "empty draft was cancelled, not committed");
        match &s.pending {
            Some(Annotation::Text { pos, text, .. }) => {
                assert_eq!(*pos, Point::new(50.0, 50.0), "new caret at click position");
                assert!(text.is_empty());
            }
            other => panic!("expected pending Text, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_text_pointer_switches_to_other_tool() {
        // User was editing text, switched to Rectangle via toolbar,
        // then clicks: the draft is committed and a Rect drag begins.
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::TextInput('x'));
        // Simulate toolbar tool change.
        s.tool = Tool::Rect;
        // Click to start drawing — this should commit the text and
        // begin a rect annotation.
        s.dispatch_input(&down(Point::new(20.0, 20.0)));
        assert_eq!(s.anns.len(), 1, "text draft committed");
        match &s.pending {
            Some(Annotation::Rect { rect, .. }) => {
                assert_eq!(rect.min, Point::new(20.0, 20.0));
            }
            other => panic!("expected pending Rect, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_text_undo_after_commit_restores_empty() {
        // Undo after committing a text annotation pops the snapshot
        // (pre-commit anns) — works via the existing snapshot path
        // (Task 7), no extra plumbing.
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::TextInput('x'));
        s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.anns.len(), 1);
        s.dispatch_input(&InputEvent::Key {
            key: Key::Char('z'),
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        });
        assert_eq!(s.anns.len(), 0, "undo removed committed text");
    }

    #[test]
    fn dispatch_text_input_when_not_editing_is_noop() {
        // A TextInput event with no active draft does nothing (the
        // shell should not send these, but the dispatcher must be
        // robust against stray events).
        let mut s = text_state();
        s.dispatch_input(&InputEvent::TextInput('h'));
        assert!(s.pending.is_none());
        assert_eq!(s.anns.len(), 0);
    }

    // --- Text-tool selection over committed text (text-select-edit) ---

    /// Helper: Text-tool state over 100×100 with one committed text
    /// "hello" anchored at (10,10) (default 16 px style → coarse
    /// bbox ≈ (10,10)–(50,29)).
    fn text_select_state() -> EditorState {
        let mut s = hundred_state();
        s.tool = Tool::Text;
        s.anns.push(Annotation::text(
            Point::new(10.0, 10.0),
            "hello",
            crate::text::TextStyle::simple(16.0, Color::RED),
        ));
        s
    }

    /// A Text-tool press on a committed Text selects it instead of
    /// starting a new caret on top of it.
    #[test]
    fn text_press_on_committed_text_selects_it() {
        let mut s = text_select_state();
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(20.0, 15.0),
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.selected, vec![0], "press on text selects it");
        assert!(s.pending.is_none(), "no fresh draft on top of the text");
        assert!(s.anns.len() == 1, "no new annotation committed");
        // A stray release must not commit anything phantom.
        s.dispatch_input(&InputEvent::PointerUp);
        assert!(s.pending.is_none());
        assert_eq!(s.selected, vec![0], "release keeps the selection");
    }

    /// A miss (empty canvas) clears a Text-tool selection and starts a
    /// fresh caret exactly as before selection existed.
    #[test]
    fn text_press_on_miss_clears_selection_and_starts_draft() {
        let mut s = text_select_state();
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(20.0, 15.0),
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.selected, vec![0], "fixture: selected");
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(80.0, 80.0),
            modifiers: Modifiers::default(),
        });
        assert!(s.selected.is_empty(), "miss clears the selection");
        match &s.pending {
            Some(Annotation::Text { pos, text, .. }) => {
                assert_eq!(*pos, Point::new(80.0, 80.0), "fresh caret at the miss");
                assert!(text.is_empty());
            }
            other => panic!("expected pending Text, got {other:?}"),
        }
    }

    /// A hit on a non-Text annotation is a miss to the Text tool: the
    /// selection clears and a fresh caret starts (spec assumption 4).
    #[test]
    fn text_press_on_non_text_annotation_is_a_miss() {
        let mut s = text_select_state();
        s.anns.push(rect_ann(60.0, 60.0, 90.0, 90.0));
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(20.0, 15.0),
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.selected, vec![0], "fixture: selected");
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(75.0, 75.0),
            modifiers: Modifiers::default(),
        });
        assert!(
            s.selected.is_empty(),
            "press on the rect does not select it"
        );
        match &s.pending {
            Some(Annotation::Text { pos, .. }) => {
                assert_eq!(*pos, Point::new(75.0, 75.0), "caret starts on the rect");
            }
            other => panic!("expected pending Text, got {other:?}"),
        }
    }

    /// Topmost-wins: when a Text overlaps another Text, the press
    /// selects the later (drawn-on-top) one.
    #[test]
    fn text_press_selects_topmost_text() {
        let mut s = text_select_state();
        s.anns.push(Annotation::text(
            Point::new(12.0, 12.0),
            "overlap",
            crate::text::TextStyle::simple(16.0, Color::RED),
        ));
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(20.0, 15.0),
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.selected, vec![1], "topmost text wins");
        assert!(s.pending.is_none());
    }

    /// Leaving the Text tool clears a Text-tool selection; staying on
    /// Text never clears. Mirrors the Hand-tool rule.
    #[test]
    fn set_tool_away_from_text_clears_selection() {
        let mut s = text_select_state();
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(20.0, 15.0),
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.selected, vec![0], "fixture: selected");
        s.set_tool(Tool::Text);
        assert_eq!(s.selected, vec![0], "Text→Text switch keeps the selection");
        s.set_tool(Tool::Hand);
        assert!(s.selected.is_empty(), "Text→Hand deselects");
        s.set_tool(Tool::Text);
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(20.0, 15.0),
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.selected, vec![0], "fixture: reselected under Text");
        s.set_tool(Tool::Rect);
        assert!(s.selected.is_empty(), "Text→Rect deselects");
    }

    /// `undo` / `redo` / `crop` clear a Text-tool selection through the
    /// existing generic paths (pinned).
    #[test]
    fn undo_redo_crop_clear_text_tool_selection() {
        let mut s = text_select_state();
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(20.0, 15.0),
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.selected, vec![0], "fixture: selected");
        s.dispatch_input(&ctrl('z'));
        assert!(s.selected.is_empty(), "undo drops the selection");
        s.dispatch_input(&ctrl('y'));
        assert!(s.selected.is_empty(), "redo keeps it dropped");
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(20.0, 15.0),
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.selected, vec![0], "fixture: reselected");
        s.crop(Rect::from_xywh(0.0, 0.0, 80.0, 80.0));
        assert!(s.selected.is_empty(), "crop drops the selection");
    }

    /// `Del` with a Text-tool selection removes exactly that annotation
    /// with one undo entry; `Ctrl+Z` restores it.
    #[test]
    fn del_with_text_tool_selection_deletes_one_undo_entry() {
        let mut s = text_select_state();
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(20.0, 15.0),
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.selected, vec![0], "fixture: selected");
        s.dispatch_input(&InputEvent::Key {
            key: Key::Delete,
            modifiers: Modifiers::default(),
        });
        assert!(s.anns.is_empty(), "selected text deleted");
        assert!(s.selected.is_empty(), "delete drops the selection");
        assert_eq!(s.undo.len(), 1, "exactly one undo entry");
        s.dispatch_input(&ctrl('z'));
        assert_eq!(s.anns.len(), 1, "Ctrl+Z restores the deleted text");
        match &s.anns[0] {
            Annotation::Text { text, .. } => assert_eq!(text, "hello"),
            other => panic!("expected restored Text, got {other:?}"),
        }
    }

    /// While editing a draft, a `PointerDown` on a *different* committed
    /// text commits the draft and selects that text (the modal branch
    /// mirrors the normal one's select-before-draft gate).
    #[test]
    fn text_flush_click_on_committed_text_selects_it() {
        let mut s = text_select_state();
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(60.0, 60.0),
            modifiers: Modifiers::default(),
        });
        s.dispatch_input(&InputEvent::TextInput('x'));
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(20.0, 15.0),
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.anns.len(), 2, "the draft was committed");
        assert!(s.pending.is_none(), "no new caret on top of the text");
        assert_eq!(s.selected, vec![0], "the committed text is selected");
    }

    // --- Double-click lift-out re-edit session (text-select-edit) ---

    /// Helper: like `text_select_state` but with a distinctive style so
    /// undo-exactness tests can tell content + style restoration apart.
    fn reedit_state() -> EditorState {
        let mut s = hundred_state();
        s.tool = Tool::Text;
        s.anns.push(Annotation::text(
            Point::new(10.0, 10.0),
            "hello",
            crate::text::TextStyle {
                size: 24.0,
                color: Color::rgb(1, 2, 3),
                ..crate::text::TextStyle::simple(24.0, Color::RED)
            },
        ));
        s
    }

    fn dblclick(s: &mut EditorState, pos: Point) {
        s.dispatch_input(&InputEvent::DoubleClick { pos });
    }

    /// Double-click on a committed Text lifts it out of `anns` into
    /// `pending` with the caret at the end and the selection vacated
    /// (its slot in `anns` is gone for the session's duration).
    #[test]
    fn doubleclick_enters_reedit_caret_at_end() {
        let mut s = reedit_state();
        s.selected = vec![0];
        dblclick(&mut s, Point::new(20.0, 15.0));
        assert!(s.anns.is_empty(), "the text left anns for the session");
        match &s.pending {
            Some(Annotation::Text { text, .. }) => assert_eq!(text, "hello"),
            other => panic!("expected pending Text, got {other:?}"),
        }
        assert_eq!(s.text_cursor, 5, "caret lands at the end of the text");
        assert!(s.selected.is_empty(), "selection vacated mid-session");
        assert!(s.is_editing_text(), "the modal path is live");
    }

    /// Edit + Enter commits the session: edited text back at its index,
    /// exactly one undo entry, `Ctrl+Z` restoring content *and* style.
    /// No flip-to-select — the annotation is not new.
    #[test]
    fn reedit_enter_commits_one_undo_entry() {
        let mut s = reedit_state();
        s.flip_to_select = true;
        dblclick(&mut s, Point::new(20.0, 15.0));
        for ch in " world".chars() {
            s.dispatch_input(&InputEvent::TextInput(ch));
        }
        s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.anns.len(), 1, "spliced back, not appended");
        match &s.anns[0] {
            Annotation::Text {
                text, font_size, ..
            } => {
                assert_eq!(text, "hello world");
                assert_eq!(*font_size, 24.0, "style carried through the session");
            }
            other => panic!("expected Text, got {other:?}"),
        }
        assert_eq!(s.undo.len(), 1, "exactly one undo entry");
        assert!(s.redo.is_empty());
        assert_eq!(s.tool, Tool::Text, "re-edit commit never flips tools");
        assert_eq!(s.selected, vec![0], "selection restored on commit");
        s.dispatch_input(&ctrl('z'));
        match &s.anns[0] {
            Annotation::Text {
                text,
                font_size,
                color,
                ..
            } => {
                assert_eq!(text, "hello", "content restored");
                assert_eq!(*font_size, 24.0, "style restored");
                assert_eq!(*color, Color::rgb(1, 2, 3), "colour restored");
            }
            other => panic!("expected restored Text, got {other:?}"),
        }
    }

    /// Esc mid-session reverts to the original with zero history
    /// entries and restores the selection.
    #[test]
    fn reedit_esc_reverts_without_history() {
        let mut s = reedit_state();
        dblclick(&mut s, Point::new(20.0, 15.0));
        s.dispatch_input(&InputEvent::TextInput('!'));
        s.dispatch_input(&InputEvent::TextInput('!'));
        s.dispatch_input(&InputEvent::Key {
            key: Key::Escape,
            modifiers: Modifiers::default(),
        });
        match &s.anns[0] {
            Annotation::Text { text, .. } => assert_eq!(text, "hello", "original restored"),
            other => panic!("expected Text, got {other:?}"),
        }
        assert!(s.undo.is_empty(), "revert touches no history");
        assert!(s.pending.is_none());
        assert_eq!(s.selected, vec![0], "selection restored on revert");
    }

    /// Opening and immediately committing a session without changes
    /// leaves history untouched (no-op edit ⇒ no entry).
    #[test]
    fn reedit_noop_edit_pushes_no_history() {
        let mut s = reedit_state();
        dblclick(&mut s, Point::new(20.0, 15.0));
        s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        match &s.anns[0] {
            Annotation::Text { text, .. } => assert_eq!(text, "hello"),
            other => panic!("expected Text, got {other:?}"),
        }
        assert!(s.undo.is_empty(), "no-op edit ⇒ no undo entry");
        assert!(s.pending.is_none());
    }

    /// Editing down to the empty string removes the annotation (fresh
    /// drafts never commit empty) with exactly one undo entry; undo
    /// restores it at its original index.
    #[test]
    fn reedit_edited_to_empty_removes_with_one_entry() {
        let mut s = reedit_state();
        dblclick(&mut s, Point::new(20.0, 15.0));
        for _ in 0..5 {
            s.dispatch_input(&InputEvent::Key {
                key: Key::Backspace,
                modifiers: Modifiers::default(),
            });
        }
        s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        assert!(s.anns.is_empty(), "edited-to-empty text is removed");
        assert!(s.selected.is_empty(), "nothing left to select");
        assert_eq!(s.undo.len(), 1, "the removal is one undo entry");
        s.dispatch_input(&ctrl('z'));
        match &s.anns[0] {
            Annotation::Text { text, .. } => assert_eq!(text, "hello", "undo restores it"),
            other => panic!("expected restored Text, got {other:?}"),
        }
    }

    /// `DoubleClick` is inert everywhere except a Text-tool hit on a
    /// committed Text: misses, non-Text tools, and mid-edit arrivals
    /// (the accompanying second `PointerDown` already did the work).
    #[test]
    fn doubleclick_noops_miss_other_tools_and_mid_edit() {
        // Miss under Text.
        let mut s = reedit_state();
        dblclick(&mut s, Point::new(80.0, 80.0));
        assert_eq!(s.anns.len(), 1);
        assert!(s.pending.is_none());

        // Non-Text tool over a text hit.
        let mut s = reedit_state();
        s.tool = Tool::Hand;
        dblclick(&mut s, Point::new(20.0, 15.0));
        assert_eq!(s.anns.len(), 1, "Hand treats DoubleClick as noise");
        assert!(s.pending.is_none());

        // Mid-edit (fresh draft active): modal path swallows it.
        let mut s = reedit_state();
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(60.0, 60.0),
            modifiers: Modifiers::default(),
        });
        s.dispatch_input(&InputEvent::TextInput('x'));
        dblclick(&mut s, Point::new(20.0, 15.0));
        match &s.pending {
            Some(Annotation::Text { text, .. }) => {
                assert_eq!(text, "x", "the fresh draft is untouched");
            }
            other => panic!("expected pending draft, got {other:?}"),
        }
        assert_eq!(s.anns.len(), 1, "lift-out did not fire mid-edit");
        assert!(s.text_reedit.is_none());
    }

    /// Clicking away mid-session commits the edit and starts the new
    /// action at the click point — the Flameshot flow preserved.
    #[test]
    fn reedit_clickaway_commits_then_starts_new_action() {
        let mut s = reedit_state();
        dblclick(&mut s, Point::new(20.0, 15.0));
        s.dispatch_input(&InputEvent::TextInput('!'));
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(70.0, 70.0),
            modifiers: Modifiers::default(),
        });
        match &s.anns[0] {
            Annotation::Text { text, .. } => assert_eq!(text, "hello!", "session committed"),
            other => panic!("expected Text, got {other:?}"),
        }
        match &s.pending {
            Some(Annotation::Text { pos, text, .. }) => {
                assert_eq!(*pos, Point::new(70.0, 70.0), "fresh caret at the click");
                assert!(text.is_empty());
            }
            other => panic!("expected fresh pending Text, got {other:?}"),
        }
        assert_eq!(s.undo.len(), 1, "one entry for the session");
        assert!(s.text_reedit.is_none(), "session closed");
    }

    /// The splice-back keeps the annotation at its original index —
    /// neighbours on both sides stay put.
    #[test]
    fn reedit_splice_back_index_is_stable() {
        let mut s = hundred_state();
        s.tool = Tool::Text;
        s.anns.push(rect_ann(0.0, 0.0, 5.0, 5.0));
        s.anns.push(Annotation::text(
            Point::new(10.0, 10.0),
            "mid",
            crate::text::TextStyle::simple(16.0, Color::RED),
        ));
        s.anns.push(rect_ann(60.0, 60.0, 90.0, 90.0));
        dblclick(&mut s, Point::new(20.0, 15.0));
        assert_eq!(s.anns.len(), 2, "fixture: the text is lifted out");
        s.dispatch_input(&InputEvent::TextInput('!'));
        s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.anns.len(), 3);
        assert!(matches!(s.anns[0], Annotation::Rect { .. }));
        match &s.anns[1] {
            Annotation::Text { text, .. } => assert_eq!(text, "mid!", "back at index 1"),
            other => panic!("expected Text at index 1, got {other:?}"),
        }
        assert!(matches!(s.anns[2], Annotation::Rect { .. }));
    }

    /// Shell-direct `crop` / `undo` / `redo` mid-session restore the
    /// original safely: no lost text, no wrong-index splice, no
    /// zombie session.
    #[test]
    fn direct_crop_undo_redo_mid_session_restore_original() {
        // Crop: the toolbar's Crop button bypasses the dispatcher.
        let mut s = reedit_state();
        dblclick(&mut s, Point::new(20.0, 15.0));
        s.crop(Rect::from_xywh(0.0, 0.0, 80.0, 80.0));
        assert!(s.pending.is_none(), "guard closed the session");
        assert!(s.text_reedit.is_none());
        match &s.anns[0] {
            Annotation::Text { text, pos, .. } => {
                assert_eq!(text, "hello", "original restored, not the edited pending");
                assert_eq!(*pos, Point::new(10.0, 10.0));
            }
            other => panic!("expected restored Text, got {other:?}"),
        }

        // Undo / redo with live history to swap.
        let mut s = reedit_state();
        s.anns.clear();
        s.anns.push(Annotation::text(
            Point::new(10.0, 10.0),
            "first",
            crate::text::TextStyle::simple(16.0, Color::RED),
        ));
        s.undo.push(Snapshot::anns_only(Vec::new(), s.next_number));
        dblclick(&mut s, Point::new(20.0, 15.0));
        s.dispatch_input(&InputEvent::TextInput('!'));
        s.undo();
        assert!(s.anns.is_empty(), "undo swapped in the pre-commit state");
        assert!(s.text_reedit.is_none(), "guard dropped the session");
        s.redo();
        assert_eq!(s.anns.len(), 1, "redo swapped the committed state back");
        match &s.anns[0] {
            Annotation::Text { text, .. } => assert_eq!(text, "first"),
            other => panic!("expected Text, got {other:?}"),
        }
    }

    // --- Text selection model (text-selection-highlight spec) ---

    /// Open a fresh text draft at (10, 10) and type `text` into it,
    /// leaving the cursor at the end.
    fn type_draft(s: &mut EditorState, text: &str) {
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        for ch in text.chars() {
            s.dispatch_input(&InputEvent::TextInput(ch));
        }
    }

    fn shift_enter() -> InputEvent {
        InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        }
    }

    fn shift_key(k: Key) -> InputEvent {
        InputEvent::Key {
            key: k,
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        }
    }

    fn ctrl_key(k: Key) -> InputEvent {
        InputEvent::Key {
            key: k,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        }
    }

    fn ctrl_shift_key(k: Key) -> InputEvent {
        InputEvent::Key {
            key: k,
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::default()
            },
        }
    }

    fn pending_text(s: &EditorState) -> &str {
        match &s.pending {
            Some(Annotation::Text { text, .. }) => text,
            other => panic!("expected pending Text, got {other:?}"),
        }
    }

    #[test]
    fn text_anchor_defaults_none_and_session_boundaries_clear_it() {
        let mut s = text_state();
        assert_eq!(s.text_anchor, None, "anchor starts None");
        type_draft(&mut s, "hi");
        assert_eq!(s.text_anchor, None, "plain typing keeps the anchor None");

        s.text_anchor = Some(0);
        s.dispatch_input(&key(Key::Enter));
        assert_eq!(s.text_anchor, None, "fresh-draft commit clears the anchor");

        let mut s = text_state();
        type_draft(&mut s, "hi");
        s.text_anchor = Some(0);
        s.dispatch_input(&key(Key::Escape));
        assert_eq!(s.text_anchor, None, "cancel clears the anchor");

        let mut s = reedit_state();
        dblclick(&mut s, Point::new(20.0, 15.0));
        assert_eq!(s.text_anchor, None, "re-edit opens collapsed");
        s.text_anchor = Some(0);
        s.dispatch_input(&key(Key::Enter));
        assert_eq!(s.text_anchor, None, "re-edit commit clears the anchor");

        let mut s = reedit_state();
        dblclick(&mut s, Point::new(20.0, 15.0));
        s.text_anchor = Some(0);
        s.dispatch_input(&key(Key::Escape));
        assert_eq!(s.text_anchor, None, "re-edit revert clears the anchor");
    }

    #[test]
    fn selection_range_only_when_nonempty_text_selection_exists() {
        let mut s = text_state();
        assert_eq!(s.selection_range(), None, "no draft -> no range");
        type_draft(&mut s, "hello");
        assert_eq!(s.selection_range(), None, "anchor None -> collapsed");
        s.text_anchor = Some(5);
        assert_eq!(s.selection_range(), None, "anchor == cursor -> collapsed");
        s.text_anchor = Some(1);
        assert_eq!(s.selection_range(), Some(1..5));
        s.text_cursor = 0;
        s.text_anchor = Some(5);
        assert_eq!(
            s.selection_range(),
            Some(0..5),
            "cursor before anchor normalizes"
        );
        // A forged anchor over a non-Text pending is meaningless.
        s.pending = Some(rect_ann(0.0, 0.0, 5.0, 5.0));
        s.text_anchor = Some(0);
        assert_eq!(s.selection_range(), None, "range requires a Text draft");
    }

    #[test]
    fn plain_caret_moves_collapse_the_selection() {
        let mut s = text_state();
        type_draft(&mut s, "ab\ncd");
        for k in [
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::ArrowUp,
            Key::ArrowDown,
            Key::Home,
            Key::End,
        ] {
            s.text_anchor = Some(0);
            s.dispatch_input(&key(k));
            assert_eq!(s.text_anchor, None, "{k:?} collapses the selection");
        }
    }

    #[test]
    fn typing_replaces_the_selection_in_one_stroke() {
        let mut s = text_state();
        type_draft(&mut s, "hello");
        s.text_anchor = Some(0);
        s.dispatch_input(&InputEvent::TextInput('X'));
        assert_eq!(pending_text(&s), "X");
        assert_eq!(s.text_cursor, 1, "cursor lands just past the insertion");
        assert_eq!(s.text_anchor, None, "the stroke consumed the selection");

        let mut s = text_state();
        type_draft(&mut s, "hello world");
        s.text_anchor = Some(6); // "world" selected, cursor 11
        s.dispatch_input(&InputEvent::TextInput('!'));
        assert_eq!(pending_text(&s), "hello !");
        assert_eq!(s.text_cursor, 7);
    }

    #[test]
    fn newline_replaces_the_selection() {
        let mut s = text_state();
        type_draft(&mut s, "hello");
        s.text_anchor = Some(0);
        s.dispatch_input(&shift_enter());
        assert_eq!(pending_text(&s), "\n");
        assert_eq!(s.text_cursor, 1);
        assert_eq!(s.text_anchor, None);
    }

    #[test]
    fn backspace_deletes_the_selection_range() {
        let mut s = text_state();
        type_draft(&mut s, "hello");
        s.text_anchor = Some(0);
        s.dispatch_input(&key(Key::Backspace));
        assert_eq!(pending_text(&s), "");
        assert_eq!(s.text_cursor, 0);
        assert_eq!(s.text_anchor, None);

        // Partial range, cursor after anchor.
        let mut s = text_state();
        type_draft(&mut s, "hello");
        s.text_cursor = 4;
        s.text_anchor = Some(1); // "ell" selected
        s.dispatch_input(&key(Key::Backspace));
        assert_eq!(pending_text(&s), "ho");
        assert_eq!(s.text_cursor, 1, "cursor parks at the range start");

        // Drag direction reversed: anchor after cursor.
        let mut s = text_state();
        type_draft(&mut s, "hello");
        s.text_anchor = Some(4);
        s.text_cursor = 1; // "ell" selected, anchored from the right
        s.dispatch_input(&key(Key::Backspace));
        assert_eq!(pending_text(&s), "ho");
        assert_eq!(s.text_cursor, 1);
        assert_eq!(s.text_anchor, None);
    }

    #[test]
    fn backspace_collapsed_anchor_behaves_exactly_as_today() {
        let mut s = text_state();
        type_draft(&mut s, "hello");
        s.text_cursor = 2;
        s.text_anchor = Some(2); // == cursor: collapsed
        s.dispatch_input(&key(Key::Backspace));
        assert_eq!(
            pending_text(&s),
            "hllo",
            "deletes the char before the caret"
        );

        // Column 0 stays a no-op even with a (collapsed) anchor set.
        let mut s = text_state();
        type_draft(&mut s, "hello");
        s.text_cursor = 0;
        s.text_anchor = Some(0);
        s.dispatch_input(&key(Key::Backspace));
        assert_eq!(pending_text(&s), "hello");
        assert_eq!(s.text_cursor, 0);
    }

    #[test]
    fn selection_ops_on_multibyte_chars_never_split() {
        let mut s = text_state();
        type_draft(&mut s, "🙂🙂");
        s.text_cursor = 1; // select the first emoji
        s.text_anchor = Some(0);
        s.dispatch_input(&key(Key::Backspace));
        assert_eq!(pending_text(&s), "🙂");
        assert_eq!(s.text_cursor, 0);

        let mut s = text_state();
        type_draft(&mut s, "a🙂b");
        s.text_anchor = Some(1);
        s.text_cursor = 2; // just the emoji
        s.dispatch_input(&InputEvent::TextInput('x'));
        assert_eq!(pending_text(&s), "axb");
        assert_eq!(s.text_cursor, 2);

        let mut s = text_state();
        type_draft(&mut s, "a🙂b");
        s.text_anchor = Some(1);
        s.text_cursor = 3; // "🙂b"
        s.dispatch_input(&shift_enter());
        assert_eq!(pending_text(&s), "a\n");
        assert_eq!(s.text_cursor, 2);
    }

    // --- Keyboard selection chords (text-selection-highlight Task 002) ---

    #[test]
    fn shift_arrows_seed_anchor_then_extend_by_char() {
        let mut s = text_state();
        type_draft(&mut s, "hello");
        for _ in 0..3 {
            s.dispatch_input(&shift_key(Key::ArrowLeft));
        }
        assert_eq!(s.text_anchor, Some(5), "first press seeds the anchor");
        assert_eq!(s.text_cursor, 2);
        assert_eq!(s.selection_range(), Some(2..5));
        s.dispatch_input(&shift_key(Key::ArrowRight));
        assert_eq!(s.text_cursor, 3, "cursor moves, anchor stays");
        assert_eq!(s.text_anchor, Some(5));

        // Multi-byte: one Shift+Left over an emoji moves one char.
        let mut s = text_state();
        type_draft(&mut s, "🙂🙂");
        s.dispatch_input(&shift_key(Key::ArrowLeft));
        assert_eq!(s.text_cursor, 1, "emoji is one char wide");
        assert_eq!(s.text_anchor, Some(2));
    }

    #[test]
    fn plain_arrow_with_live_selection_collapses_and_moves() {
        let mut s = text_state();
        type_draft(&mut s, "hello");
        s.dispatch_input(&shift_key(Key::ArrowLeft));
        s.dispatch_input(&shift_key(Key::ArrowLeft));
        assert_eq!(s.selection_range(), Some(3..5));
        s.dispatch_input(&key(Key::ArrowLeft));
        assert_eq!(s.text_anchor, None, "plain move collapses");
        assert_eq!(s.text_cursor, 2, "and still moves");
    }

    #[test]
    fn shift_up_down_extend_line_wise() {
        let mut s = text_state();
        type_draft(&mut s, "ab\ncd");
        assert_eq!(s.text_cursor, 5, "fixture: cursor at draft end");
        s.dispatch_input(&shift_key(Key::ArrowUp));
        assert_eq!(s.text_anchor, Some(5));
        assert_eq!(s.text_cursor, 2, "column clamps to line 0's length");
        s.dispatch_input(&shift_key(Key::ArrowDown));
        assert_eq!(s.text_cursor, 5, "column preserved back");
        assert_eq!(s.text_anchor, Some(5), "anchor never reseeds mid-chord");
    }

    #[test]
    fn shift_home_end_extend_to_line_edges() {
        let mut s = text_state();
        type_draft(&mut s, "ab\ncd");
        s.dispatch_input(&key(Key::ArrowUp));
        s.dispatch_input(&key(Key::ArrowLeft));
        assert_eq!(s.text_cursor, 1, "fixture: mid line 0");
        s.dispatch_input(&shift_key(Key::End));
        assert_eq!(s.text_anchor, Some(1));
        assert_eq!(s.text_cursor, 2, "end of line 0, not past the \\n");
        s.dispatch_input(&shift_key(Key::Home));
        assert_eq!(s.text_cursor, 0, "start of line 0");
        assert_eq!(s.text_anchor, Some(1));

        let mut s = text_state();
        type_draft(&mut s, "ab\ncd");
        s.dispatch_input(&shift_key(Key::Home));
        assert_eq!(s.text_anchor, Some(5), "fixture end of line 1 anchors");
        assert_eq!(s.text_cursor, 3, "line 1 start");
    }

    #[test]
    fn ctrl_a_selects_all_and_is_noop_on_empty() {
        let mut s = text_state();
        type_draft(&mut s, "hello");
        s.dispatch_input(&ctrl('a'));
        assert_eq!(s.text_anchor, Some(0));
        assert_eq!(s.text_cursor, 5);
        assert_eq!(s.selection_range(), Some(0..5));

        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&ctrl('a'));
        assert_eq!(s.text_anchor, None, "empty draft: no-op");
        assert_eq!(s.text_cursor, 0);
    }

    #[test]
    fn delete_key_deletes_selection_or_forward_char() {
        let mut s = text_state();
        type_draft(&mut s, "hello");
        s.text_anchor = Some(0);
        s.dispatch_input(&key(Key::Delete));
        assert_eq!(pending_text(&s), "");
        assert_eq!(s.text_cursor, 0);
        assert_eq!(s.text_anchor, None);

        let mut s = text_state();
        type_draft(&mut s, "hello");
        s.dispatch_input(&key(Key::ArrowLeft));
        s.dispatch_input(&key(Key::ArrowLeft));
        s.dispatch_input(&key(Key::ArrowLeft));
        s.dispatch_input(&key(Key::ArrowLeft));
        assert_eq!(s.text_cursor, 1, "fixture: on 'e'");
        s.dispatch_input(&key(Key::Delete));
        assert_eq!(pending_text(&s), "hllo", "forward delete at cursor");
        assert_eq!(s.text_cursor, 1, "cursor unmoved");

        // At the end of the draft, Delete is a no-op.
        let mut s = text_state();
        type_draft(&mut s, "hi");
        s.dispatch_input(&key(Key::Delete));
        assert_eq!(pending_text(&s), "hi");

        // Multi-byte forward delete removes a whole emoji.
        let mut s = text_state();
        type_draft(&mut s, "🙂b");
        s.dispatch_input(&key(Key::ArrowLeft));
        s.dispatch_input(&key(Key::ArrowLeft));
        s.dispatch_input(&key(Key::Delete));
        assert_eq!(pending_text(&s), "b");
    }

    #[test]
    fn commit_chords_behave_exactly_as_before() {
        // Enter with stray modifiers (no shift) still commits — the
        // pre-existing arm shape is untouched.
        let mut s = text_state();
        type_draft(&mut s, "hi");
        s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        });
        assert!(s.pending.is_none(), "Ctrl+Enter commits like plain Enter");
        assert_eq!(s.anns.len(), 1);
    }

    // --- Word motion (text-selection-highlight Task 005) ---

    #[test]
    fn word_left_right_boundary_table() {
        // "hello world": h0 e1 l2 l3 o4 ' '5 w6 o7 r8 l9 d10.
        assert_eq!(word_left("hello world", 8), 6);
        assert_eq!(word_right("hello world", 8), 11);
        // Cursor at the space: left = start of "hello", right = start
        // of "world" (whitespace consumed, the word not).
        assert_eq!(word_left("hello world", 5), 0);
        assert_eq!(word_right("hello world", 5), 6);
        // Word start falls back to the previous word.
        assert_eq!(word_left("hello world", 6), 0);
        // Punctuation is its own word class: "foo, bar" f0 o1 o2 ,3.
        assert_eq!(word_left("foo, bar", 4), 3, "comma is a one-char word");
        assert_eq!(word_right("foo, bar", 2), 3, "stops at the class boundary");
        assert_eq!(word_right("foo bar", 2), 4, "run then trailing space");
        assert_eq!(word_right("foo, bar", 3), 5, "punct run then space");
        assert_eq!(word_left("foo, bar", 7), 5);
        // Clamps.
        assert_eq!(word_left("hello", 0), 0);
        assert_eq!(word_right("hello", 5), 5);
        assert_eq!(word_left("", 0), 0);
        assert_eq!(word_right("", 0), 0);
        // Multi-byte: an emoji is one char, never split.
        assert_eq!(word_right("🙂 b", 0), 2);
        assert_eq!(word_left("🙂 b", 3), 2);
    }

    #[test]
    fn word_bounds_around_table() {
        assert_eq!(word_bounds_around("hello world", 7), (6, 11));
        // At a space: the word ending at the boundary wins.
        assert_eq!(word_bounds_around("hello world", 5), (0, 5));
        assert_eq!(word_bounds_around("hello world", 6), (6, 11));
        assert_eq!(word_bounds_around("", 0), (0, 0));
        // Punctuation forms its own word span.
        assert_eq!(word_bounds_around("foo, bar", 3), (3, 4));
    }

    #[test]
    fn ctrl_shift_arrows_extend_word_wise() {
        let mut s = text_state();
        type_draft(&mut s, "foo bar baz");
        s.dispatch_input(&key(Key::Home));
        assert_eq!(s.text_cursor, 0, "fixture: cursor at start");
        // Ctrl+Shift+Right x2: word+space, then word+space — cursor
        // lands at the start of "baz".
        s.dispatch_input(&ctrl_shift_key(Key::ArrowRight));
        assert_eq!(s.text_cursor, 4, "first press: word + space");
        assert_eq!(s.text_anchor, Some(0), "anchor seeded on first press");
        s.dispatch_input(&ctrl_shift_key(Key::ArrowRight));
        assert_eq!(s.text_cursor, 8, "second press: start of the next word");
        assert_eq!(s.text_anchor, Some(0), "anchor never reseeds");

        // Backwards: from the end, Ctrl+Shift+Left x2 walks to word
        // starts ("baz" -> "bar").
        let mut s = text_state();
        type_draft(&mut s, "foo bar baz");
        s.dispatch_input(&ctrl_shift_key(Key::ArrowLeft));
        assert_eq!(s.text_cursor, 8, "start of 'baz'");
        s.dispatch_input(&ctrl_shift_key(Key::ArrowLeft));
        assert_eq!(s.text_cursor, 4, "start of 'bar'");
        assert_eq!(s.text_anchor, Some(11));
        assert_eq!(s.selection_range(), Some(4..11));
    }

    #[test]
    fn ctrl_arrows_move_word_wise_and_collapse() {
        let mut s = text_state();
        type_draft(&mut s, "hello world");
        s.dispatch_input(&key(Key::ArrowLeft));
        s.dispatch_input(&key(Key::ArrowLeft));
        s.dispatch_input(&key(Key::ArrowLeft));
        assert_eq!(s.text_cursor, 8, "fixture: mid 'world'");
        s.dispatch_input(&ctrl('r')); // any ctrl_only key: no-op
        s.dispatch_input(&ctrl_key(Key::ArrowRight));
        assert_eq!(s.text_cursor, 11, "Ctrl+Right jumps to text end");
        s.text_anchor = Some(0);
        s.dispatch_input(&ctrl_key(Key::ArrowLeft));
        assert_eq!(s.text_cursor, 6, "Ctrl+Left to word start");
        assert_eq!(s.text_anchor, None, "plain word move collapses");
    }

    // --- Mouse selection APIs (text-selection-highlight Task 006) ---

    #[test]
    fn text_press_at_char_collapses_and_clamps() {
        let mut s = text_state();
        type_draft(&mut s, "hello");
        s.text_press_at_char(3);
        assert_eq!(s.text_cursor, 3);
        assert_eq!(s.text_anchor, None, "press collapses");
        s.text_press_at_char(10);
        assert_eq!(s.text_cursor, 5, "idx clamps to len");
        s.text_press_at_char(usize::MAX);
        assert_eq!(s.text_cursor, 5);
        s.text_press_at_char(0);
        assert_eq!(s.text_cursor, 0);
    }

    #[test]
    fn text_drag_to_char_selects_from_press_point() {
        let mut s = text_state();
        type_draft(&mut s, "hello");
        s.text_press_at_char(1);
        s.text_drag_to_char(4);
        assert_eq!(s.text_anchor, Some(1), "first drag seeds from the press");
        assert_eq!(s.text_cursor, 4);
        s.text_drag_to_char(1);
        assert_eq!(s.text_anchor, Some(1), "anchor sticky across drags");
        assert_eq!(s.text_cursor, 1, "dragged back to collapsed");
        s.text_drag_to_char(2);
        assert_eq!(s.text_anchor, Some(1));
        assert_eq!(s.text_cursor, 2);
    }

    #[test]
    fn text_drag_without_press_never_selects() {
        let mut s = text_state();
        type_draft(&mut s, "hello");
        // No press: cursor sits at 5 from typing, anchor None.
        s.text_drag_to_char(2);
        assert_eq!(s.text_cursor, 5, "stray drag is a full no-op");
        assert_eq!(s.text_anchor, None);
    }

    #[test]
    fn text_select_word_and_line_around_cursor() {
        let mut s = text_state();
        type_draft(&mut s, "hello world");
        s.text_press_at_char(7);
        s.text_select_word();
        assert_eq!(s.text_anchor, Some(6));
        assert_eq!(s.text_cursor, 11);
        assert_eq!(s.selection_range(), Some(6..11));

        let mut s = text_state();
        type_draft(&mut s, "ab\ncd");
        s.text_press_at_char(4); // end of line 1
        s.text_select_line();
        assert_eq!(s.text_anchor, Some(3));
        assert_eq!(s.text_cursor, 5);
    }

    #[test]
    fn mouse_selection_apis_noop_outside_text_draft() {
        // No pending at all.
        let mut s = text_state();
        s.text_press_at_char(3);
        s.text_drag_to_char(1);
        s.text_select_word();
        s.text_select_line();
        assert!(s.pending.is_none());
        assert_eq!(s.text_cursor, 0);

        // Non-Text pending.
        let mut s = text_state();
        s.pending = Some(rect_ann(0.0, 0.0, 5.0, 5.0));
        s.text_press_at_char(3);
        s.text_drag_to_char(1);
        s.text_select_word();
        s.text_select_line();
        assert_eq!(s.text_cursor, 0);

        // Empty draft: no panics, no phantom selection.
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.text_press_at_char(2);
        assert_eq!(s.text_cursor, 0, "clamps to 0 on empty text");
        s.text_select_word();
        s.text_select_line();
        assert_eq!(s.text_anchor, None, "nothing to select in an empty draft");
    }

    #[test]
    fn mouse_selection_apis_touch_no_history_or_selection_set() {
        let mut s = reedit_state();
        dblclick(&mut s, Point::new(20.0, 15.0));
        s.text_press_at_char(2);
        s.text_drag_to_char(4);
        s.text_select_word();
        s.text_select_line();
        assert!(s.undo.is_empty(), "no history entries from selection");
        assert!(s.redo.is_empty());
        assert!(s.selected.is_empty(), "ann selection untouched");
        assert_eq!(s.anns.len(), 0, "committed anns untouched (lifted)");
    }

    // --- Click-count gesture arms (text-selection-highlight Task 007) ---

    #[test]
    fn doubleclick_while_editing_selects_word_around_cursor() {
        let mut s = text_state();
        type_draft(&mut s, "hello world");
        // The pump's intercepted press collapses the cursor onto the
        // click point before the DoubleClick arrives.
        s.text_press_at_char(7);
        s.dispatch_input(&InputEvent::DoubleClick {
            pos: Point::new(11.0, 11.0),
        });
        assert_eq!(s.text_anchor, Some(6));
        assert_eq!(s.text_cursor, 11);
        assert_eq!(s.selection_range(), Some(6..11));
    }

    #[test]
    fn doubleclick_on_empty_draft_is_a_noop() {
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::DoubleClick {
            pos: Point::new(10.0, 10.0),
        });
        assert_eq!(s.text_anchor, None, "nothing to select");
        assert_eq!(s.text_cursor, 0);
        assert!(s.is_editing_text(), "draft stays open");
    }

    #[test]
    fn tripleclick_while_editing_selects_line() {
        let mut s = text_state();
        type_draft(&mut s, "ab\ncd");
        s.text_press_at_char(4);
        s.dispatch_input(&InputEvent::TripleClick {
            pos: Point::new(11.0, 11.0),
        });
        assert_eq!(s.text_anchor, Some(3));
        assert_eq!(s.text_cursor, 5);
        // Single-line text: the line *is* the whole draft.
        let mut s = text_state();
        type_draft(&mut s, "hello");
        s.text_press_at_char(2);
        s.dispatch_input(&InputEvent::TripleClick {
            pos: Point::new(11.0, 11.0),
        });
        assert_eq!(s.selection_range(), Some(0..5));
    }

    #[test]
    fn tripleclick_outside_editing_is_a_documented_noop() {
        let mut s = reedit_state();
        assert_eq!(
            s.dispatch_input(&InputEvent::TripleClick {
                pos: Point::new(20.0, 15.0)
            }),
            DispatchOutcome::Continue
        );
        assert!(s.pending.is_none(), "no session opened");
        assert_eq!(s.anns.len(), 1, "annotations untouched");
        // Re-edit via double-click still works right after.
        dblclick(&mut s, Point::new(20.0, 15.0));
        assert!(s.is_editing_text(), "re-edit flow intact");
    }

    // --- Gesture-bracketed restyle (text-select-edit Task 005) ---

    /// A distinctive style that differs from `text_select_state`'s
    /// 16 px / RED in every field the panel can edit.
    fn loud_style() -> crate::text::TextStyle {
        crate::text::TextStyle {
            size: 32.0,
            color: Color::rgb(9, 8, 7),
            family: crate::text::FontFamily::Monospace,
            weight: crate::text::FontWeight::Bold,
            style: crate::text::FontStyle::Italic,
            align: crate::text::TextAlign::Center,
            line_spacing: 1.5,
            outline: Some(crate::text::TextOutline {
                color: Color::rgb(0, 0, 0),
                width: 3.0,
            }),
            background: Some(Color::rgb(255, 235, 0)),
        }
    }

    /// One gesture (many restyle calls between begin / end) restyles
    /// the selected text, syncs the session defaults so the next
    /// placed text inherits them, and pushes exactly one undo entry
    /// that Ctrl+Z restores style from. The anchor never moves.
    #[test]
    fn restyle_gesture_one_entry_syncs_defaults_and_restores() {
        let mut s = text_select_state();
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(20.0, 15.0),
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.selected, vec![0], "fixture: selected");
        s.begin_style_gesture();
        // Re-begin inside the same gesture is first-wins: one entry.
        s.begin_style_gesture();
        let style = loud_style();
        s.restyle_selected_text(style);
        s.restyle_selected_text(style);
        s.end_style_gesture();
        // Second end with nothing staged is a no-op.
        s.end_style_gesture();

        match &s.anns[0] {
            Annotation::Text {
                pos,
                font_size,
                color,
                family,
                weight,
                style: fs,
                align,
                line_spacing,
                outline,
                background,
                ..
            } => {
                assert_eq!(*pos, Point::new(10.0, 10.0), "anchor never moves");
                assert_eq!(*font_size, 32.0);
                assert_eq!(*color, Color::rgb(9, 8, 7));
                assert_eq!(*family, crate::text::FontFamily::Monospace);
                assert_eq!(*weight, crate::text::FontWeight::Bold);
                assert_eq!(*fs, crate::text::FontStyle::Italic);
                assert_eq!(*align, crate::text::TextAlign::Center);
                assert_eq!(*line_spacing, 1.5);
                assert_eq!(*outline, style.outline);
                assert_eq!(*background, style.background);
            }
            other => panic!("expected Text, got {other:?}"),
        }
        // Session defaults synced: the next placed draft inherits.
        assert_eq!(s.font_size, 32.0);
        assert_eq!(s.text_color, Color::rgb(9, 8, 7));
        assert_eq!(s.font_family, crate::text::FontFamily::Monospace);
        assert_eq!(s.font_weight, crate::text::FontWeight::Bold);
        assert_eq!(s.font_style, crate::text::FontStyle::Italic);
        assert_eq!(s.text_align, crate::text::TextAlign::Center);
        assert_eq!(s.line_spacing, 1.5);
        assert_eq!(s.text_outline, style.outline);
        assert_eq!(s.text_background, style.background);
        assert_eq!(s.undo.len(), 1, "exactly one entry per gesture");

        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(70.0, 70.0),
            modifiers: Modifiers::default(),
        });
        match &s.pending {
            Some(Annotation::Text {
                font_size, color, ..
            }) => {
                assert_eq!(*font_size, 32.0, "next draft inherits the size");
                assert_eq!(
                    *color,
                    Color::rgb(9, 8, 7),
                    "next draft inherits the colour"
                );
            }
            other => panic!("expected fresh draft, got {other:?}"),
        }

        s.pending = None;
        s.dispatch_input(&ctrl('z'));
        match &s.anns[0] {
            Annotation::Text {
                font_size, color, ..
            } => {
                assert_eq!(*font_size, 16.0, "Ctrl+Z restores the prior style");
                assert_eq!(*color, Color::RED);
            }
            other => panic!("expected restored Text, got {other:?}"),
        }
    }

    /// Mid-session restyles target the lifted-out draft and stage
    /// nothing: the session's commit still pushes exactly one entry
    /// covering content and style together.
    #[test]
    fn restyle_mid_session_targets_draft_and_absorbs() {
        let mut s = reedit_state();
        dblclick(&mut s, Point::new(20.0, 15.0));
        s.begin_style_gesture();
        assert!(
            s.style_gesture_snapshot.is_none(),
            "begin no-ops while a session is active"
        );
        s.restyle_selected_text(loud_style());
        match &s.pending {
            Some(Annotation::Text {
                text,
                font_size,
                color,
                ..
            }) => {
                assert_eq!(text, "hello", "content untouched by restyle");
                assert_eq!(*font_size, 32.0, "the draft carries the new style");
                assert_eq!(*color, Color::rgb(9, 8, 7));
            }
            other => panic!("expected pending draft, got {other:?}"),
        }
        assert_eq!(s.font_size, 32.0, "defaults still sync mid-session");
        s.end_style_gesture();
        assert!(s.undo.is_empty(), "end with nothing staged pushes nothing");

        s.dispatch_input(&InputEvent::TextInput('!'));
        s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.undo.len(), 1, "session commit is still one entry");
        match &s.anns[0] {
            Annotation::Text {
                text,
                font_size,
                color,
                ..
            } => {
                assert_eq!(text, "hello!");
                assert_eq!(
                    *font_size, 32.0,
                    "style change covered by the session entry"
                );
                assert_eq!(*color, Color::rgb(9, 8, 7));
            }
            other => panic!("expected Text, got {other:?}"),
        }
        s.dispatch_input(&ctrl('z'));
        match &s.anns[0] {
            Annotation::Text {
                text,
                font_size,
                color,
                ..
            } => {
                assert_eq!(text, "hello", "content restored");
                assert_eq!(*font_size, 24.0, "style restored");
                assert_eq!(*color, Color::rgb(1, 2, 3));
            }
            other => panic!("expected restored Text, got {other:?}"),
        }
    }

    /// With no Text target (nothing selected, or a non-Text
    /// annotation selected), all three calls are full no-ops —
    /// not even the session defaults move.
    #[test]
    fn restyle_noops_without_text_target() {
        let mut s = text_select_state();
        let before = s.clone();
        s.begin_style_gesture();
        s.restyle_selected_text(loud_style());
        s.end_style_gesture();
        assert_eq!(s.anns, before.anns, "annotation untouched");
        assert_eq!(s.font_size, before.font_size, "defaults untouched");
        assert_eq!(s.text_color, before.text_color);
        assert!(s.undo.is_empty(), "no history from a no-op gesture");

        // Non-Text selection: restyle still a no-op.
        let mut s = move_state();
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(30.0, 25.0),
            modifiers: Modifiers::default(),
        });
        s.begin_style_gesture();
        s.restyle_selected_text(loud_style());
        s.end_style_gesture();
        assert_eq!(s.font_size, 16.0, "a rect selection restyles nothing");
        assert!(matches!(s.anns[0], Annotation::Rect { .. }));
    }

    // --- Task 14: Select / crop region tool tests ---

    /// Helper: a state with a 100×100 image so crops have room.
    fn hundred_state() -> EditorState {
        EditorState::new(image::RgbaImage::new(100, 100))
    }

    /// `crop(rect)` replaces `image` with the cropped sub-image.
    #[test]
    fn crop_replaces_image_with_cropped_subimage() {
        let mut img = image::RgbaImage::new(100, 100);
        for y in 0..100 {
            for x in 0..100 {
                #[allow(clippy::cast_possible_truncation, reason = "x, y ≤ 99")]
                let (x8, y8) = (x as u8, y as u8);
                img.put_pixel(x, y, image::Rgba([x8, y8, 0, 255]));
            }
        }
        let mut s = EditorState::new(img);
        s.crop(Rect::from_xywh(10.0, 20.0, 30.0, 40.0));
        assert_eq!(s.image.dimensions(), (30, 40));
        assert_eq!(
            s.image.get_pixel(0, 0).0,
            [10, 20, 0, 255],
            "origin = crop min"
        );
        assert_eq!(s.image.get_pixel(29, 39).0, [39, 59, 0, 255]);
    }

    /// `crop` translates rect annotations by subtracting crop origin.
    #[test]
    fn crop_translates_rect_annotations() {
        let mut s = hundred_state();
        s.anns.push(rect_ann(20.0, 20.0, 50.0, 50.0));
        s.crop(Rect::from_xywh(10.0, 10.0, 80.0, 80.0));
        assert_eq!(s.anns.len(), 1);
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => {
                assert_eq!(
                    rect.min,
                    Point::new(10.0, 10.0),
                    "translated by -crop_origin"
                );
                assert_eq!(rect.max, Point::new(40.0, 40.0));
            }
            other => panic!("expected Rect, got {other:?}"),
        }
    }

    /// Annotations fully outside the crop are dropped.
    #[test]
    fn crop_drops_annotations_outside_bounds() {
        let mut s = hundred_state();
        s.anns.push(rect_ann(0.0, 0.0, 5.0, 5.0)); // top-left, outside crop
        s.anns.push(rect_ann(50.0, 50.0, 80.0, 80.0)); // inside crop
        s.crop(Rect::from_xywh(40.0, 40.0, 50.0, 50.0));
        assert_eq!(s.anns.len(), 1, "outside annotation dropped");
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => {
                assert_eq!(rect.min, Point::new(10.0, 10.0));
                assert_eq!(rect.max, Point::new(40.0, 40.0));
            }
            other => panic!("expected surviving Rect, got {other:?}"),
        }
    }

    /// Rect annotations partially overlapping the crop are geometrically
    /// clipped to the overlap (not just translated).
    #[test]
    fn crop_clips_partial_rect_overlap() {
        let mut s = hundred_state();
        // Rect straddles the crop's left edge: x from 5 to 30, crop
        // starts at x=10. Clipped to x 10..30, then translated to 0..20.
        s.anns.push(rect_ann(5.0, 0.0, 30.0, 40.0));
        s.crop(Rect::from_xywh(10.0, 0.0, 80.0, 100.0));
        assert_eq!(s.anns.len(), 1);
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => {
                assert_eq!(rect.min, Point::new(0.0, 0.0), "clipped min.x = crop.min.x");
                assert_eq!(rect.max, Point::new(20.0, 40.0));
            }
            other => panic!("expected clipped Rect, got {other:?}"),
        }
    }

    /// Arrow annotations translate endpoints verbatim; render-time
    /// clipping handles the visible edge.
    #[test]
    fn crop_translates_arrow_endpoints() {
        let mut s = hundred_state();
        s.anns.push(Annotation::Arrow {
            a: Point::new(20.0, 30.0),
            b: Point::new(60.0, 70.0),
            stroke: Stroke::new(Color::RED, 3.0),
            head_size: 10.0,
        });
        s.crop(Rect::from_xywh(10.0, 10.0, 80.0, 80.0));
        assert_eq!(s.anns.len(), 1);
        match &s.anns[0] {
            Annotation::Arrow { a, b, .. } => {
                assert_eq!(*a, Point::new(10.0, 20.0));
                assert_eq!(*b, Point::new(50.0, 60.0));
            }
            other => panic!("expected Arrow, got {other:?}"),
        }
    }

    /// Crop pushes a full snapshot (image + anns) to undo; undo
    /// restores both.
    #[test]
    fn crop_is_undoable() {
        let mut s = hundred_state();
        let original_dims = s.image.dimensions();
        s.anns.push(rect_ann(20.0, 20.0, 50.0, 50.0));
        s.crop(Rect::from_xywh(10.0, 10.0, 80.0, 80.0));
        assert_eq!(s.image.dimensions(), (80, 80), "image cropped");
        assert_eq!(s.anns.len(), 1, "annotation survived crop");
        assert_eq!(s.undo.len(), 1);
        assert!(s.undo[0].image.is_some(), "crop snapshot has image");

        s.undo();
        assert_eq!(s.image.dimensions(), original_dims, "image restored");
        assert_eq!(s.anns.len(), 1, "anns restored");
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => {
                assert_eq!(rect.min, Point::new(20.0, 20.0), "pre-crop geometry");
                assert_eq!(rect.max, Point::new(50.0, 50.0));
            }
            other => panic!("{other:?}"),
        }
    }

    /// Redo after undoing a crop restores the cropped state.
    #[test]
    fn crop_redo_after_undo() {
        let mut s = hundred_state();
        s.crop(Rect::from_xywh(10.0, 10.0, 80.0, 80.0));
        let cropped_dims = s.image.dimensions();
        s.undo();
        assert_eq!(s.image.dimensions(), (100, 100), "undo restores full image");
        s.redo();
        assert_eq!(s.image.dimensions(), cropped_dims, "redo restores crop");
    }

    /// Crop clears redo (new mutation invalidates the redo stack).
    #[test]
    fn crop_clears_redo() {
        let mut s = hundred_state();
        // Populate redo via an undo of a commit.
        s.pending = Some(rect_ann(0.0, 0.0, 10.0, 10.0));
        s.commit_annotation();
        s.undo();
        assert!(!s.redo.is_empty(), "undo populated redo");
        s.crop(Rect::from_xywh(10.0, 10.0, 50.0, 50.0));
        assert!(s.redo.is_empty(), "crop cleared redo");
    }

    /// A degenerate crop rect (< 1 px in either dimension) is a no-op.
    #[test]
    fn crop_degenerate_is_noop() {
        let mut s = hundred_state();
        s.anns.push(rect_ann(10.0, 10.0, 50.0, 50.0));
        let dims_before = s.image.dimensions();
        s.crop(Rect::from_xywh(50.0, 50.0, 0.5, 50.0));
        assert_eq!(s.image.dimensions(), dims_before, "image untouched");
        assert_eq!(s.anns.len(), 1, "anns untouched");
        assert!(s.undo.is_empty(), "no snapshot pushed");
    }

    /// Crop tool: `PointerDown` starts a region; `PointerDrag` extends it.
    #[test]
    fn dispatch_crop_drag_creates_region() {
        let mut s = hundred_state();
        s.tool = Tool::Crop;
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        assert!(
            s.crop_region.is_some(),
            "PointerDown starts the crop region"
        );
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(50.0, 60.0),
        });
        match s.crop_region {
            Some(r) => {
                assert_eq!(r.min, Point::new(10.0, 10.0));
                assert_eq!(r.max, Point::new(50.0, 60.0));
            }
            None => panic!("crop region should be active"),
        }
    }

    /// Crop tool: `PointerUp` normalizes the region rect.
    #[test]
    fn dispatch_crop_up_normalizes_region() {
        let mut s = hundred_state();
        s.tool = Tool::Crop;
        s.dispatch_input(&down(Point::new(50.0, 60.0)));
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(10.0, 10.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        match s.crop_region {
            Some(r) => {
                assert_eq!(r.min, Point::new(10.0, 10.0), "normalized: min is top-left");
                assert_eq!(r.max, Point::new(50.0, 60.0));
            }
            None => panic!("crop region retained after PointerUp"),
        }
    }

    /// Crop tool: Enter confirms the region (crops) and clears it.
    #[test]
    fn dispatch_enter_confirms_crop() {
        let mut s = hundred_state();
        s.tool = Tool::Crop;
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(50.0, 50.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        assert!(s.crop_region.is_none(), "region cleared on confirm");
        assert_eq!(s.image.dimensions(), (40, 40), "cropped to region");
        assert_eq!(s.undo.len(), 1, "crop pushed undo");
    }

    /// Crop tool: Enter without an active region is a no-op
    /// (falls through to the normal key path).
    #[test]
    fn dispatch_enter_without_region_is_noop() {
        let mut s = hundred_state();
        s.tool = Tool::Crop;
        s.dispatch_input(&InputEvent::Key {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        assert_eq!(s.image.dimensions(), (100, 100), "image unchanged");
        assert!(s.undo.is_empty());
    }

    /// `confirm_crop` is a no-op when no region is active.
    #[test]
    fn confirm_crop_noop_without_region() {
        let mut s = hundred_state();
        s.confirm_crop();
        assert_eq!(s.image.dimensions(), (100, 100));
        assert!(s.undo.is_empty());
    }

    /// Crop tool does not interfere with keyboard shortcuts (Ctrl+Z
    /// still undoes while Crop is active).
    #[test]
    fn dispatch_crop_does_not_suppress_undo() {
        let mut s = hundred_state();
        s.tool = Tool::Crop;
        s.pending = Some(rect_ann(0.0, 0.0, 10.0, 10.0));
        s.anns.push(rect_ann(20.0, 20.0, 30.0, 30.0));
        s.commit_annotation();
        assert_eq!(s.anns.len(), 2);
        assert_eq!(s.dispatch_input(&ctrl('z')), DispatchOutcome::Continue);
        assert_eq!(s.anns.len(), 1, "Ctrl+Z undid last commit");
    }

    // --- Task 004: Number tool state machine tests ---
    //
    // Number is click-to-place: a PointerDown with `Tool::Number`
    // commits `Annotation::Number { pos, n: next_number, color,
    // diameter }` immediately (no `pending`, no drag), increments
    // `next_number`, and pushes a `Snapshot` so undo decrements the
    // counter. PointerDrag / PointerUp are no-ops for Number (pending
    // is always None).

    fn number_state() -> EditorState {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Number;
        s.color = Color::rgb(255, 0, 0);
        s
    }

    /// `PointerDown` with `Tool::Number` commits the bubble immediately
    /// — no `pending` annotation is created (Number is click-to-place,
    /// not drag). The first bubble is numbered `1` and `next_number`
    /// advances to `2`.
    #[test]
    fn dispatch_number_click_places_bubble_immediately() {
        let mut s = number_state();
        s.dispatch_input(&down(Point::new(20.0, 30.0)));
        assert!(s.pending.is_none(), "Number is click-to-place, no pending");
        assert_eq!(s.anns.len(), 1, "exactly one bubble committed");
        assert_eq!(s.next_number, 2, "counter advanced");
        assert_eq!(s.undo.len(), 1, "snapshot pushed for undo");
        assert!(s.redo.is_empty(), "fresh commit clears redo");
        match &s.anns[0] {
            Annotation::Number {
                pos,
                n,
                color,
                diameter,
            } => {
                assert_eq!(*pos, Point::new(20.0, 30.0), "bubble at click position");
                assert_eq!(*n, 1, "first bubble is 1");
                assert_eq!(*color, Color::rgb(255, 0, 0), "uses active drawing color");
                assert!(*diameter > 0.0, "diameter defaults to a positive constant");
            }
            other => panic!("expected Number, got {other:?}"),
        }
    }

    /// Three successive clicks place bubbles labelled 1, 2, 3; a
    /// fourth click would place 4 (counter ready).
    #[test]
    fn dispatch_number_increments_label_across_clicks() {
        let mut s = number_state();
        // Zip positions with their expected bubble numbers so the
        // loop body never has to cast `enumerate`'s usize to u32.
        let positions = [
            Point::new(10.0, 10.0),
            Point::new(20.0, 10.0),
            Point::new(30.0, 10.0),
        ];
        for (expected_n, pos) in [1_u32, 2, 3].into_iter().zip(positions) {
            s.dispatch_input(&down(pos));
            let idx = usize::try_from(expected_n - 1).expect("non-zero");
            match &s.anns[idx] {
                Annotation::Number { n, pos: p, .. } => {
                    assert_eq!(
                        *n, expected_n,
                        "bubble {expected_n} is numbered {expected_n}"
                    );
                    assert_eq!(*p, pos);
                }
                other => panic!("expected Number at index {idx}, got {other:?}"),
            }
        }
        assert_eq!(s.next_number, 4, "counter ready for the fourth bubble");
    }

    /// Undo after placing `3` decrements `next_number` from `4` back to
    /// `3`, so the next placement re-places `3` (counter monotonic only
    /// across forward placements, not across undo).
    #[test]
    fn dispatch_number_undo_decrements_counter() {
        let mut s = number_state();
        for pos in [
            Point::new(10.0, 10.0),
            Point::new(20.0, 10.0),
            Point::new(30.0, 10.0),
        ] {
            s.dispatch_input(&down(pos));
        }
        assert_eq!(s.next_number, 4);
        assert_eq!(s.anns.len(), 3);
        // Undo removes bubble 3 AND decrements the counter to 3.
        s.dispatch_input(&ctrl('z'));
        assert_eq!(s.anns.len(), 2, "undo removed the third bubble");
        assert_eq!(
            s.next_number, 3,
            "counter decremented so the next bubble is 3 again"
        );
        // Re-place: the new bubble is numbered 3, not 4.
        s.dispatch_input(&down(Point::new(40.0, 10.0)));
        match &s.anns[2] {
            Annotation::Number { n, .. } => assert_eq!(*n, 3, "counter reused the undone number"),
            other => panic!("expected Number, got {other:?}"),
        }
    }

    /// Redo after undoing a Number placement re-applies the bubble and
    /// restores the counter to its post-placement value. Locks the
    /// `next_number` round-trip through both stacks (not just undo).
    #[test]
    fn dispatch_number_redo_reincrements_counter() {
        let mut s = number_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        assert_eq!(s.next_number, 2);
        s.dispatch_input(&ctrl('z'));
        assert_eq!(s.next_number, 1, "undo decremented");
        s.dispatch_input(&ctrl('y'));
        assert_eq!(s.anns.len(), 1, "redo re-applied the bubble");
        assert_eq!(s.next_number, 2, "redo re-incremented the counter");
    }

    /// A fresh editor places bubbles at the default diameter (28 px)
    /// — `number_diameter` starts at `NUMBER_DEFAULT_DIAMETER`.
    #[test]
    fn dispatch_number_default_diameter_is_28() {
        let mut s = number_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        match &s.anns[0] {
            Annotation::Number { diameter, .. } => assert_eq!(
                *diameter, NUMBER_DEFAULT_DIAMETER,
                "fresh state places at the default diameter"
            ),
            other => panic!("expected Number, got {other:?}"),
        }
    }

    /// `number_diameter` is session tool state: set it to 64 and every
    /// subsequent placement commits 64 — the setting persists across
    /// placements (it is not consumed like the counter).
    #[test]
    fn dispatch_number_uses_number_diameter_for_new_placements() {
        let mut s = number_state();
        s.number_diameter = 64.0;
        for pos in [Point::new(10.0, 10.0), Point::new(50.0, 10.0)] {
            s.dispatch_input(&down(pos));
        }
        for (i, ann) in s.anns.iter().enumerate() {
            match ann {
                Annotation::Number { diameter, .. } => assert_eq!(
                    *diameter,
                    64.0,
                    "bubble {} committed at the panel-set diameter",
                    i + 1
                ),
                other => panic!("expected Number at index {i}, got {other:?}"),
            }
        }
    }

    /// Committed bubbles are immutable snapshots: changing
    /// `number_diameter` after a placement never resizes the bubble
    /// already on canvas — only the next placement picks it up.
    #[test]
    fn number_diameter_change_leaves_committed_bubbles_immutable() {
        let mut s = number_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.number_diameter = 96.0;
        match &s.anns[0] {
            Annotation::Number { diameter, .. } => assert_eq!(
                *diameter, NUMBER_DEFAULT_DIAMETER,
                "committed bubble keeps its placement-time diameter"
            ),
            other => panic!("expected Number, got {other:?}"),
        }
    }

    /// The snapshot discipline is unchanged by a custom diameter:
    /// undo removes the bubble and decrements the counter, redo
    /// re-applies both — the existing round-trip, pinned at 64 px so
    /// a regression that couples diameter to history fails loudly.
    #[test]
    fn dispatch_number_custom_diameter_undo_redo_restores_counter() {
        let mut s = number_state();
        s.number_diameter = 64.0;
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&ctrl('z'));
        assert_eq!(s.anns.len(), 0, "undo removed the custom-diameter bubble");
        assert_eq!(s.next_number, 1, "counter decremented");
        s.dispatch_input(&ctrl('y'));
        assert_eq!(s.anns.len(), 1, "redo re-applied the bubble");
        match &s.anns[0] {
            Annotation::Number { diameter, n, .. } => {
                assert_eq!(*diameter, 64.0, "redo restored the custom diameter");
                assert_eq!(*n, 1, "redo re-incremented the counter");
            }
            other => panic!("expected Number, got {other:?}"),
        }
        assert_eq!(s.next_number, 2);
    }

    /// Number's `PointerDrag` and `PointerUp` are no-ops: there's no
    /// `pending` annotation to update or commit. A click is the entire
    /// interaction.
    #[test]
    fn dispatch_number_drag_and_up_are_noops() {
        let mut s = number_state();
        s.dispatch_input(&down(Point::new(20.0, 30.0)));
        // A stray drag (e.g. the user wiggled the mouse before
        // releasing) must not crash or conjure extra annotations.
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(50.0, 60.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.anns.len(), 1, "still exactly one bubble");
        assert!(s.pending.is_none());
    }

    /// Clicking with `Tool::Number` while editing a text draft commits
    /// the draft and places a number bubble at the click position —
    /// matches the Flameshot-like "click commits and starts the next
    /// action" contract already in place for Select / drag tools.
    #[test]
    fn dispatch_text_pointer_switches_to_number_tool() {
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::TextInput('x'));
        // Simulate a toolbar tool change while the draft is open.
        s.tool = Tool::Number;
        s.dispatch_input(&down(Point::new(50.0, 50.0)));
        // Draft committed + number bubble placed.
        assert_eq!(s.anns.len(), 2);
        assert!(matches!(s.anns[0], Annotation::Text { .. }));
        match &s.anns[1] {
            Annotation::Number { pos, n, .. } => {
                assert_eq!(*pos, Point::new(50.0, 50.0));
                assert_eq!(*n, 1);
            }
            other => panic!("expected Number at index 1, got {other:?}"),
        }
    }

    // --- Object selection + move (Tool::Select) tests ---

    /// Helper: Select-tool state over 100×100 with one committed
    /// rect annotation at (10,10)–(50,40).
    fn move_state() -> EditorState {
        let mut s = hundred_state();
        s.tool = Tool::Select;
        s.anns.push(rect_ann(10.0, 10.0, 50.0, 40.0));
        s
    }

    /// Helper: Select-tool state with two non-overlapping committed
    /// rects — ann 0 at (10,10)–(50,40), ann 1 at (60,10)–(90,40).
    fn two_ann_move_state() -> EditorState {
        let mut s = hundred_state();
        s.tool = Tool::Select;
        s.anns.push(rect_ann(10.0, 10.0, 50.0, 40.0));
        s.anns.push(rect_ann(60.0, 10.0, 90.0, 40.0));
        s
    }

    /// Helper: Select-tool state with three non-overlapping committed
    /// rects, in a lower band so fixtures compose: ann 0 at
    /// (0,50)–(20,70), ann 1 at (60,50)–(80,70), ann 2 at
    /// (80,50)–(95,70).
    fn three_ann_move_state() -> EditorState {
        let mut s = hundred_state();
        s.tool = Tool::Select;
        s.anns.push(rect_ann(0.0, 50.0, 20.0, 70.0));
        s.anns.push(rect_ann(60.0, 50.0, 80.0, 70.0));
        s.anns.push(rect_ann(80.0, 50.0, 95.0, 70.0));
        s
    }

    /// Select press on a committed annotation grabs it for a move;
    /// the drag translates it with the grab point tracking the
    /// cursor; release clears the drag state.
    #[test]
    fn move_press_on_annotation_moves_it() {
        let mut s = move_state();
        // Press at the annotation's centre: grab offset = (20, 15).
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        assert!(s.moving.is_some(), "press on annotation starts a move");
        assert_eq!(s.selected, vec![0]);
        assert!(s.crop_region.is_none(), "move does not start a crop region");
        // Drag so the cursor lands at (60, 45): bbox.min follows at
        // cursor − grab offset = (40, 30).
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(60.0, 45.0),
        });
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => {
                assert_eq!(rect.min, Point::new(40.0, 30.0));
                assert_eq!(rect.max, Point::new(80.0, 60.0), "size preserved");
            }
            other => panic!("expected Rect, got {other:?}"),
        }
        s.dispatch_input(&InputEvent::PointerUp);
        assert!(s.moving.is_none(), "release ends the move");
        assert_eq!(
            s.selected,
            vec![0],
            "release keeps the selection (persistent selection)"
        );
    }

    /// One move gesture is exactly one undo entry, no matter how
    /// many pointer events it took; undo restores the pre-move
    /// position and redo re-applies it.
    #[test]
    fn one_move_gesture_is_one_undo_entry() {
        let mut s = move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        for x in [35.0, 45.0, 55.0] {
            s.dispatch_input(&InputEvent::PointerDrag {
                pos: Point::new(x, 25.0),
            });
        }
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.undo.len(), 1, "one gesture = one snapshot");
        s.undo();
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => {
                assert_eq!(rect.min, Point::new(10.0, 10.0), "undo restores position");
            }
            other => panic!("expected Rect, got {other:?}"),
        }
        s.redo();
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => {
                assert_eq!(rect.min, Point::new(35.0, 10.0), "redo re-applies the move");
            }
            other => panic!("expected Rect, got {other:?}"),
        }
    }

    /// A click-without-drag on an annotation records no history and
    /// leaves it untouched.
    #[test]
    fn click_without_move_records_no_history() {
        let mut s = move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        // Same-position drag: zero movement, no snapshot.
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(30.0, 25.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        assert!(s.undo.is_empty(), "no movement = no snapshot");
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => assert_eq!(rect.min, Point::new(10.0, 10.0)),
            other => panic!("expected Rect, got {other:?}"),
        }
    }

    /// Ctrl+Z mid-move cancels the drag and restores the pre-move
    /// geometry; a subsequent drag does not resurrect the move.
    #[test]
    fn undo_mid_move_cancels_and_restores() {
        let mut s = move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(60.0, 45.0),
        });
        assert!(s.moving.is_some());
        s.dispatch_input(&InputEvent::Key {
            key: Key::Char('z'),
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        });
        assert!(s.moving.is_none(), "undo cancels the in-flight move");
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => {
                assert_eq!(rect.min, Point::new(10.0, 10.0), "undo restores position");
            }
            other => panic!("expected Rect, got {other:?}"),
        }
        // A stray drag after the cancel must not move anything.
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(90.0, 90.0),
        });
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => assert_eq!(rect.min, Point::new(10.0, 10.0)),
            other => panic!("expected Rect, got {other:?}"),
        }
    }

    /// Crop press (on or off an annotation) always starts a fresh
    /// region selection — annotation moves are `Tool::Hand`'s job, so
    /// Crop never grabs.
    #[test]
    fn crop_press_off_annotation_starts_region() {
        let mut s = move_state();
        s.tool = Tool::Crop;
        // Off the annotation.
        s.dispatch_input(&down(Point::new(70.0, 70.0)));
        assert!(s.moving.is_none());
        assert!(
            s.crop_region.is_some(),
            "empty-space press starts the region"
        );
        // The drag extends the region, not the annotation.
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(90.0, 80.0),
        });
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => assert_eq!(rect.min, Point::new(10.0, 10.0)),
            other => panic!("expected Rect, got {other:?}"),
        }
    }

    /// Crop press directly on an annotation also starts a region
    /// — the separation contract: Crop owns the region, Hand moves
    #[test]
    fn crop_press_on_annotation_starts_region() {
        let mut s = move_state();
        s.tool = Tool::Crop;
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        assert!(s.moving.is_none(), "Crop never grabs an annotation");
        assert!(
            s.crop_region.is_some(),
            "press on annotation starts the region"
        );
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => assert_eq!(rect.min, Point::new(10.0, 10.0)),
            other => panic!("expected Rect, got {other:?}"),
        }
    }

    /// Line mid-segment grab (within the stroke + tolerance radius)
    /// moves both endpoints by the drag delta.
    #[test]
    fn move_press_on_line_moves_endpoints() {
        let mut s = hundred_state();
        s.tool = Tool::Select;
        s.anns.push(Annotation::Line {
            a: Point::new(10.0, 10.0),
            b: Point::new(90.0, 10.0),
            stroke: Stroke::new(Color::RED, 3.0),
        });
        // (50, 12) is 2 px off the segment — inside the 1.5 + 3.0
        // hit radius.
        s.dispatch_input(&down(Point::new(50.0, 12.0)));
        assert!(s.moving.is_some(), "near-segment press grabs the line");
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(60.0, 32.0),
        });
        match &s.anns[0] {
            Annotation::Line { a, b, .. } => {
                assert_eq!(*a, Point::new(20.0, 30.0));
                assert_eq!(*b, Point::new(100.0, 30.0), "both endpoints shift equally");
            }
            other => panic!("expected Line, got {other:?}"),
        }
    }

    /// A Select-tool press on a degenerate annotation (empty
    /// polyline) is a no-op — no move, no grab, nothing starts.
    #[test]
    fn move_press_on_degenerate_annotation_is_noop() {
        let mut s = hundred_state();
        s.tool = Tool::Select;
        s.anns.push(Annotation::Pencil {
            points: Vec::new(),
            color: Color::RED,
            width: 3.0,
        });
        assert!(!s.begin_move(0, Point::new(50.0, 50.0)));
        s.dispatch_input(&down(Point::new(50.0, 50.0)));
        assert!(s.moving.is_none());
        assert!(s.crop_region.is_none(), "Hand never starts a crop region");
    }

    /// A Select-tool press that misses every annotation starts a
    /// marquee (resolved by its own test below); the press itself
    /// grabs nothing and records no history.
    #[test]
    fn move_press_on_empty_space_grabs_nothing() {
        let mut s = move_state();
        s.dispatch_input(&down(Point::new(70.0, 70.0)));
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(90.0, 80.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        assert!(s.moving.is_none());
        assert!(s.crop_region.is_none());
        assert!(s.undo.is_empty());
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => assert_eq!(rect.min, Point::new(10.0, 10.0)),
            other => panic!("expected Rect, got {other:?}"),
        }
    }

    // --- Annotation delete (annotation-delete Task 001) ---
    //
    // Persistent selection: a Hand-tool click selects an annotation
    // and the selection survives release; Del / Backspace (or
    // `delete_selected` directly) remove exactly it as a fully
    // undoable anns-only snapshot.

    #[test]
    fn move_click_keeps_selection_after_release() {
        let mut s = move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert!(s.moving.is_none());
        assert_eq!(
            s.selected,
            vec![0],
            "click-without-drag leaves the annotation selected"
        );
    }

    #[test]
    fn move_drag_release_keeps_selection() {
        let mut s = move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(60.0, 45.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        assert!(s.moving.is_none());
        assert_eq!(s.selected, vec![0], "post-move release stays selected");
    }

    #[test]
    fn move_miss_press_clears_selection() {
        let mut s = move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0], "fixture: selected");
        // A press on empty canvas deselects — and starts nothing.
        s.dispatch_input(&down(Point::new(70.0, 70.0)));
        assert!(s.selected.is_empty(), "miss press clears selection");
        assert!(s.moving.is_none());
        assert!(s.crop_region.is_none());
    }

    #[test]
    fn move_regrab_replaces_selection_last_click_wins() {
        let mut s = two_ann_move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0]);
        s.dispatch_input(&down(Point::new(75.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![1], "last click wins");
    }

    #[test]
    fn delete_selected_removes_exactly_it_with_undo_symmetry() {
        let mut s = two_ann_move_state();
        // Pre-populate redo to prove delete clears it (new mutation).
        s.redo.push(Snapshot::anns_only(Vec::new(), 1));
        s.dispatch_input(&down(Point::new(75.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![1], "fixture: ann 1 selected");
        s.delete_selected();
        assert_eq!(
            s.anns,
            vec![rect_ann(10.0, 10.0, 50.0, 40.0)],
            "exactly ann 1 removed"
        );
        assert!(s.selected.is_empty());
        assert_eq!(s.undo.len(), 1);
        assert_eq!(s.undo[0].anns.len(), 2, "pre-delete snapshot carries both");
        assert!(s.redo.is_empty(), "delete clears redo");
        // Undo restores exactly it.
        s.undo();
        assert_eq!(s.anns.len(), 2);
        assert_eq!(s.anns[1], rect_ann(60.0, 10.0, 90.0, 40.0));
        // Redo re-deletes.
        s.redo();
        assert_eq!(s.anns, vec![rect_ann(10.0, 10.0, 50.0, 40.0)]);
    }

    #[test]
    fn delete_selected_noop_without_selection() {
        let mut s = hundred_state();
        s.anns.push(rect_ann(0.0, 0.0, 10.0, 10.0));
        s.delete_selected();
        assert_eq!(s.anns.len(), 1, "nothing removed");
        assert!(s.undo.is_empty(), "no snapshot pushed");
        assert!(s.redo.is_empty());
        // Also fine on a fresh, empty state.
        let mut fresh = EditorState::new(blank_image());
        fresh.delete_selected();
        assert!(fresh.anns.is_empty());
    }

    #[test]
    fn delete_selected_preserves_next_number() {
        let mut s = number_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&down(Point::new(30.0, 10.0)));
        assert_eq!(s.next_number, 3);
        s.set_tool(Tool::Select);
        // Click bubble 1's centre to select it, then delete.
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0]);
        s.delete_selected();
        assert_eq!(s.anns.len(), 1);
        assert_eq!(
            s.next_number, 3,
            "the delete itself must not touch the counter"
        );
        s.undo();
        assert_eq!(s.anns.len(), 2);
        assert_eq!(
            s.next_number, 3,
            "undo of a delete restores the same counter (no ratchet)"
        );
    }

    #[test]
    fn dispatch_del_and_backspace_delete_selected() {
        let mut s = two_ann_move_state();
        s.dispatch_input(&down(Point::new(75.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(
            s.dispatch_input(&key(Key::Delete)),
            DispatchOutcome::Continue
        );
        assert_eq!(s.anns.len(), 1, "Del removed the selection");
        // Backspace aliases Del outside text mode.
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(
            s.dispatch_input(&key(Key::Backspace)),
            DispatchOutcome::Continue
        );
        assert!(s.anns.is_empty(), "Backspace removed the selection");
    }

    #[test]
    fn dispatch_delete_without_selection_is_noop() {
        let mut s = move_state();
        assert_eq!(
            s.dispatch_input(&key(Key::Delete)),
            DispatchOutcome::Continue
        );
        assert_eq!(s.anns.len(), 1);
        assert!(s.undo.is_empty());
        assert_eq!(
            s.dispatch_input(&key(Key::Backspace)),
            DispatchOutcome::Continue
        );
        assert_eq!(s.anns.len(), 1);
    }

    #[test]
    fn delete_keys_with_modifiers_are_reserved() {
        let mut s = move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        for key in [Key::Delete, Key::Backspace] {
            assert_eq!(
                s.dispatch_input(&InputEvent::Key {
                    key,
                    modifiers: Modifiers {
                        ctrl: true,
                        ..Modifiers::default()
                    },
                }),
                DispatchOutcome::Continue
            );
        }
        assert_eq!(s.anns.len(), 1, "modified chords must not delete");
        assert!(s.undo.is_empty());
        assert_eq!(
            s.selected,
            vec![0],
            "reserved chords must not even deselect"
        );
    }

    #[test]
    fn backspace_while_drafting_edits_draft_not_committed_anns() {
        let mut s = text_state();
        s.anns.push(rect_ann(0.0, 0.0, 5.0, 5.0));
        // Caret away from the committed rect; type two chars.
        s.dispatch_input(&down(Point::new(50.0, 50.0)));
        s.dispatch_input(&InputEvent::TextInput('a'));
        s.dispatch_input(&InputEvent::TextInput('b'));
        s.dispatch_input(&key(Key::Backspace));
        match &s.pending {
            Some(Annotation::Text { text, .. }) => {
                assert_eq!(text, "a", "Backspace edits the draft");
            }
            other => panic!("expected pending Text, got {other:?}"),
        }
        assert_eq!(s.anns.len(), 1, "committed annotation untouched");
        // Del is swallowed by the modal branch: draft and anns intact.
        s.dispatch_input(&key(Key::Delete));
        assert!(s.pending.is_some(), "draft survives Del");
        assert_eq!(s.anns.len(), 1);
    }

    #[test]
    fn undo_and_redo_clear_selection() {
        let mut s = move_state();
        // A real move gesture so undo has something to pop.
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(60.0, 45.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0], "fixture: selected after release");
        s.dispatch_input(&ctrl('z'));
        assert!(s.selected.is_empty(), "undo drops the selection");
        s.dispatch_input(&ctrl('y'));
        assert!(s.selected.is_empty(), "redo keeps it dropped");
        assert_eq!(s.anns.len(), 1);
    }

    #[test]
    fn crop_clears_selection() {
        let mut s = move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0], "fixture: selected");
        s.crop(Rect::from_xywh(0.0, 0.0, 80.0, 80.0));
        assert!(s.selected.is_empty(), "crop drops the selection");
        assert_eq!(s.anns.len(), 1, "fixture: the annotation survived");
    }

    #[test]
    fn set_tool_away_from_move_clears_selection() {
        let mut s = move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0], "fixture: selected");
        s.set_tool(Tool::Rect);
        assert_eq!(s.tool, Tool::Rect);
        assert!(s.selected.is_empty(), "leaving Select deselects");
        // Switching back, then Select→Select, must never clear.
        s.set_tool(Tool::Select);
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0], "fixture: reselected");
        s.set_tool(Tool::Select);
        assert_eq!(
            s.selected,
            vec![0],
            "Select→Select switch keeps the selection"
        );
    }

    #[test]
    fn delete_of_dragged_annotation_ends_drag_cleanly() {
        let mut s = move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        assert!(s.moving.is_some(), "fixture: drag in flight");
        s.delete_selected();
        assert!(s.moving.is_none(), "the drag died with the annotation");
        assert!(s.selected.is_empty());
        assert!(s.anns.is_empty());
        assert_eq!(s.undo.len(), 1, "pre-delete snapshot pushed");
        // A stray drag after the delete must not panic or conjure
        // geometry.
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(40.0, 40.0),
        });
        assert!(s.anns.is_empty());
    }

    #[test]
    fn text_flush_move_miss_press_clears_selection() {
        // Toolbar tool switch while a draft is open: the draft
        // commits, and the follow-up Select press that misses every
        // annotation must still clear a stale selection (the
        // text-mode press branch mirrors the normal one).
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::TextInput('x'));
        s.tool = Tool::Select;
        s.selected = vec![0];
        s.dispatch_input(&down(Point::new(70.0, 70.0)));
        assert_eq!(s.anns.len(), 1, "the draft committed");
        assert!(
            s.selected.is_empty(),
            "miss press cleared the stale selection"
        );
    }

    // --- Multi-select routing (multi-select-properties Task 001) ---

    /// Shift+click toggles membership: a press on an unselected
    /// annotation adds it to the set; a press on a member removes it.
    #[test]
    fn shift_click_toggles_membership_in_and_out() {
        let mut s = two_ann_move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        s.dispatch_input(&shift_down(Point::new(75.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0, 1], "shift-click adds to the set");
        s.dispatch_input(&shift_down(Point::new(75.0, 25.0)));
        assert_eq!(s.selected, vec![0], "shift-click on a member removes it");
    }

    /// Toggling a member out deselects it and starts no drag — the
    /// pointer-up of that press and any stray drag are no-ops.
    #[test]
    fn shift_click_toggle_out_starts_no_drag() {
        let mut s = move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        s.dispatch_input(&shift_down(Point::new(30.0, 25.0)));
        assert!(s.selected.is_empty(), "member toggled out");
        assert!(s.moving.is_none(), "toggle-out starts no drag");
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(60.0, 45.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => {
                assert_eq!(rect.min, Point::new(10.0, 10.0), "stray drag moved nothing");
            }
            other => panic!("expected Rect, got {other:?}"),
        }
        assert!(s.undo.is_empty(), "no gesture, no snapshot");
    }

    /// Plain-click routing splits on membership: a press on a
    /// non-member replaces the set (last click wins); a press on a
    /// member keeps the set — that press is the group-drag entry.
    #[test]
    fn plain_click_replaces_set_with_non_member_only() {
        let mut s = two_ann_move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        s.dispatch_input(&shift_down(Point::new(75.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0, 1], "fixture: two members");
        // Plain press on a member: set survives, drag grabs the group.
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        assert_eq!(s.selected, vec![0, 1], "member press keeps the set");
        assert!(s.moving.is_some(), "member press still grabs");
        s.dispatch_input(&InputEvent::PointerUp);
        // Plain press on a non-member replaces the set.
        let mut s3 = three_ann_move_state();
        s3.dispatch_input(&shift_down(Point::new(5.0, 50.0)));
        s3.dispatch_input(&InputEvent::PointerUp);
        s3.dispatch_input(&shift_down(Point::new(65.0, 50.0)));
        s3.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s3.selected, vec![0, 1], "fixture: two of three");
        s3.dispatch_input(&down(Point::new(85.0, 50.0)));
        s3.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s3.selected, vec![2], "non-member press collapses the set");
    }

    /// A plain miss press clears the whole set; a Shift miss press
    /// keeps it (the additive marquee contract — release may merge
    /// hits back in; a degenerate release selects nothing).
    #[test]
    fn miss_press_clears_whole_set_plain_only() {
        let mut s = two_ann_move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        s.dispatch_input(&shift_down(Point::new(75.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        s.dispatch_input(&down(Point::new(70.0, 70.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert!(s.selected.is_empty(), "plain miss press clears the set");
        s.dispatch_input(&shift_down(Point::new(30.0, 25.0)));
        assert_eq!(s.selected, vec![0], "fixture: rebuilt one member");
        s.dispatch_input(&shift_down(Point::new(70.0, 70.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(
            s.selected,
            vec![0],
            "shift miss press keeps the set; degenerate marquee adds nothing"
        );
    }

    /// The set is maintained sorted ascending regardless of the order
    /// members were toggled in (documented invariant consumers rely
    /// on for stable iteration).
    #[test]
    fn selected_set_stays_sorted() {
        let mut s = two_ann_move_state();
        s.dispatch_input(&shift_down(Point::new(75.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        s.dispatch_input(&shift_down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0, 1], "sorted ascending, not toggle order");
    }

    /// `Del` removes every selected annotation in one pass with one
    /// `anns`-only snapshot; a single undo restores all of them and
    /// leaves the selection empty (undo never restores the set).
    #[test]
    fn delete_selected_removes_all_with_one_snapshot() {
        let mut s = two_ann_move_state();
        s.dispatch_input(&shift_down(Point::new(75.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        s.dispatch_input(&shift_down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0, 1], "fixture: both selected");
        s.delete_selected();
        assert!(s.anns.is_empty(), "both removed in one pass");
        assert!(s.selected.is_empty(), "delete drops the set");
        assert_eq!(s.undo.len(), 1, "one delete gesture = one snapshot");
        assert_eq!(s.undo[0].anns.len(), 2, "pre-delete snapshot carries both");
        s.undo();
        assert_eq!(
            s.anns,
            vec![
                rect_ann(10.0, 10.0, 50.0, 40.0),
                rect_ann(60.0, 10.0, 90.0, 40.0)
            ],
            "one undo restores all members, in draw order"
        );
        assert!(s.selected.is_empty(), "undo does not resurrect the set");
    }

    /// The text-draft commit-then-route press mirrors the normal
    /// Select routing, Shift included: a shift hit toggles, a shift
    /// miss clears.
    #[test]
    fn text_flush_shift_press_routes_selection() {
        let mut s = text_state();
        s.dispatch_input(&down(Point::new(10.0, 10.0)));
        s.dispatch_input(&InputEvent::TextInput('x'));
        s.tool = Tool::Select;
        // Text "x" at (10,10), size 16: coarse bbox (10,10)-(18,26).
        s.dispatch_input(&shift_down(Point::new(12.0, 18.0)));
        assert_eq!(s.anns.len(), 1, "the draft committed");
        assert_eq!(
            s.selected,
            vec![0],
            "shift hit toggles the committed ann in"
        );
        s.dispatch_input(&InputEvent::PointerUp);
        s.dispatch_input(&shift_down(Point::new(70.0, 70.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(
            s.selected,
            vec![0],
            "shift miss keeps the set; degenerate marquee adds nothing"
        );
    }

    // --- Multi-annotation drag (multi-select-properties Task 002) ---

    /// A drag on one selected member translates the whole set by the
    /// same incremental pointer delta — group move, sizes preserved.
    #[test]
    fn multi_drag_translates_whole_set_together() {
        let mut s = two_ann_move_state();
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        s.dispatch_input(&shift_down(Point::new(75.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.selected, vec![0, 1], "fixture: both selected");
        // Press on ann 0 and drag by (+20, +10) across two events.
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(40.0, 25.0),
        });
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(50.0, 35.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        for (i, expected_min) in [(0, Point::new(30.0, 20.0)), (1, Point::new(80.0, 20.0))] {
            match &s.anns[i] {
                Annotation::Rect { rect, .. } => {
                    assert_eq!(
                        rect.min, expected_min,
                        "ann {i} translated by the group delta"
                    );
                }
                other => panic!("expected Rect, got {other:?}"),
            }
        }
        assert_eq!(
            match &s.anns[0] {
                Annotation::Rect { rect, .. } => rect.max,
                other => panic!("expected Rect, got {other:?}"),
            },
            Point::new(70.0, 50.0),
            "size preserved through the group move"
        );
    }

    /// One multi-drag gesture is exactly one undo entry no matter
    /// the set size or event count; undo restores every member.
    #[test]
    fn multi_drag_is_one_undo_entry_regardless_of_set_size() {
        let mut s = two_ann_move_state();
        s.dispatch_input(&shift_down(Point::new(75.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        s.dispatch_input(&shift_down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerUp);
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        for x in [35.0, 45.0, 55.0] {
            s.dispatch_input(&InputEvent::PointerDrag {
                pos: Point::new(x, 25.0),
            });
        }
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.undo.len(), 1, "one group gesture = one snapshot");
        s.undo();
        assert_eq!(
            s.anns,
            vec![
                rect_ann(10.0, 10.0, 50.0, 40.0),
                rect_ann(60.0, 10.0, 90.0, 40.0)
            ],
            "one undo restores every member's pre-drag position"
        );
    }

    /// Members without a bounding box are skipped by the drag, not
    /// fatal: the healthy members still move.
    #[test]
    fn multi_drag_skips_degenerate_bbox_members() {
        let mut s = two_ann_move_state();
        s.anns.push(Annotation::Pencil {
            points: vec![],
            color: Color::RED,
            width: 3.0,
        });
        // A miss can't select the empty polyline (nothing to hit);
        // seed it into the set directly to exercise the skip.
        s.selected = vec![0, 1, 2];
        s.dispatch_input(&down(Point::new(30.0, 25.0)));
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(50.0, 35.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        assert_eq!(s.anns.len(), 3, "degenerate member survived untouched");
        for (i, expected_min) in [(0, Point::new(30.0, 20.0)), (1, Point::new(80.0, 20.0))] {
            match &s.anns[i] {
                Annotation::Rect { rect, .. } => {
                    assert_eq!(rect.min, expected_min, "healthy member {i} still moved");
                }
                other => panic!("expected Rect, got {other:?}"),
            }
        }
        assert!(
            matches!(&s.anns[2], Annotation::Pencil { points, .. } if points.is_empty()),
            "the degenerate member is untouched"
        );
    }

    /// `translate` shifts every variant's geometry verbatim.
    #[test]
    fn translate_shifts_every_variant() {
        let cases: Vec<(Annotation, Annotation)> = vec![
            (
                Annotation::Ellipse {
                    rect: Rect::new(Point::new(1.0, 2.0), Point::new(3.0, 4.0)),
                    stroke: Stroke::new(Color::RED, 1.0),
                    fill: None,
                },
                Annotation::Ellipse {
                    rect: Rect::new(Point::new(11.0, 7.0), Point::new(13.0, 9.0)),
                    stroke: Stroke::new(Color::RED, 1.0),
                    fill: None,
                },
            ),
            (
                Annotation::Arrow {
                    a: Point::new(1.0, 2.0),
                    b: Point::new(3.0, 4.0),
                    stroke: Stroke::new(Color::RED, 1.0),
                    head_size: 10.0,
                },
                Annotation::Arrow {
                    a: Point::new(11.0, 7.0),
                    b: Point::new(13.0, 9.0),
                    stroke: Stroke::new(Color::RED, 1.0),
                    head_size: 10.0,
                },
            ),
            (
                Annotation::Marker {
                    points: vec![Point::new(1.0, 2.0), Point::new(3.0, 4.0)],
                    color: Color::RED,
                    width: 8.0,
                },
                Annotation::Marker {
                    points: vec![Point::new(11.0, 7.0), Point::new(13.0, 9.0)],
                    color: Color::RED,
                    width: 8.0,
                },
            ),
            (
                Annotation::Text {
                    pos: Point::new(1.0, 2.0),
                    text: "hi".into(),
                    font_size: 16.0,
                    color: Color::RED,
                    family: crate::text::FontFamily::default(),
                    weight: crate::text::FontWeight::default(),
                    style: crate::text::FontStyle::default(),
                    align: crate::text::TextAlign::default(),
                    line_spacing: crate::text::DEFAULT_LINE_SPACING,
                    outline: None,
                    background: None,
                },
                Annotation::Text {
                    pos: Point::new(11.0, 7.0),
                    text: "hi".into(),
                    font_size: 16.0,
                    color: Color::RED,
                    family: crate::text::FontFamily::default(),
                    weight: crate::text::FontWeight::default(),
                    style: crate::text::FontStyle::default(),
                    align: crate::text::TextAlign::default(),
                    line_spacing: crate::text::DEFAULT_LINE_SPACING,
                    outline: None,
                    background: None,
                },
            ),
            (
                Annotation::Blur {
                    rect: Rect::new(Point::new(1.0, 2.0), Point::new(30.0, 40.0)),
                    kind: BlurKind::Pixelate,
                    strength: 8,
                },
                Annotation::Blur {
                    rect: Rect::new(Point::new(11.0, 7.0), Point::new(40.0, 45.0)),
                    kind: BlurKind::Pixelate,
                    strength: 8,
                },
            ),
            (
                Annotation::Number {
                    pos: Point::new(1.0, 2.0),
                    n: 1,
                    color: Color::RED,
                    diameter: 28.0,
                },
                Annotation::Number {
                    pos: Point::new(11.0, 7.0),
                    n: 1,
                    color: Color::RED,
                    diameter: 28.0,
                },
            ),
        ];
        for (mut before, after) in cases {
            before.translate(10.0, 5.0);
            assert_eq!(before, after);
        }
    }
}
