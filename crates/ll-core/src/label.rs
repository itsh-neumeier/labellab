//! Label layout model and the `.llabel` template format (JSON).
//!
//! A label holds elements (text, QR, barcode, image). Each element either
//! has a free-layout box (`rect`, position and size in mm, set by the GUI
//! editor) or, without one, is placed by the simple flow layout: one after
//! another along the tape with `gap_mm`/`padding_mm`, filling the tape's
//! printable height (what the CLI's single-element shortcuts and version 1
//! templates use). [`render_label`] turns either kind into the one
//! [`Bitmap`] that both the preview ([`render_label_png`]) and the print
//! job (`print::print_label`) use (`AGENTS.md`: "Vorschau und Druck nutzen
//! denselben Renderpfad").
//!
//! Coordinates: `x_mm` runs along the tape from the label's start, `y_mm`
//! across it from the top edge of the printable area (the top of the PNG
//! preview). Content outside the printable area is clipped.
//!
//! Image paths in a template are stored as written; relative ones are
//! resolved against the template's directory by [`Label::load`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use ll_protocol::model::{dots_to_mm, mm_to_dots, pt_to_dots, ModelInfo, TapeGeometry};
use ll_render::{
    boxed, boxed::TextLayout, Bitmap, Face, FaceSet, ImageAdjust, ImageEdit, QrErrorCorrection,
    ShapeKind, Symbology, TextAlign, VAlign,
};
use serde::{Deserialize, Serialize};

use crate::fusebox::{self, FuseField, FuseSeparator};
use crate::CoreError;

/// Current `.llabel` format version, written by [`Label::to_json`].
/// Version 1 (no boxes, no text size/alignment) is read unchanged.
pub const LABEL_FORMAT_VERSION: u32 = 2;

/// Border thickness in print dots when [`Label::frame`] is set.
pub const BORDER_THICKNESS: u16 = 2;

pub use ll_render::{BorderSides, BorderStyle};

/// Minimum gap between a border and flow-layout content, in mm.
const BORDER_CLEARANCE_MM: f32 = 0.3;

fn default_border_width_mm() -> f32 {
    ll_protocol::model::dots_to_mm(BORDER_THICKNESS as u32)
}

fn default_border_pattern_mm() -> f32 {
    1.5
}

/// Border around the whole label, see [`Label::border`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LabelBorder {
    #[serde(default)]
    pub style: BorderStyle,
    /// Line thickness in mm.
    #[serde(default = "default_border_width_mm")]
    pub width_mm: f32,
    #[serde(default)]
    pub sides: BorderSides,
    /// Dash length (`dashed`) or stripe width (`striped`) in mm.
    #[serde(default = "default_border_pattern_mm")]
    pub pattern_mm: f32,
    /// Distance from the label edge in mm (all sides; see `insets_mm`).
    #[serde(default)]
    pub inset_mm: f32,
    /// Distance per side in mm, overriding `inset_mm`. Left/right count
    /// from the label margins (`padding_start_mm`/`padding_mm`), top/bottom
    /// from the printable area's edges.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insets_mm: Option<BorderInsets>,
}

/// Border distance per side in mm.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct BorderInsets {
    #[serde(default)]
    pub top: f32,
    #[serde(default)]
    pub bottom: f32,
    #[serde(default)]
    pub left: f32,
    #[serde(default)]
    pub right: f32,
}

impl LabelBorder {
    /// Distances per side (`insets_mm`, else `inset_mm` everywhere), never
    /// negative.
    pub fn insets(&self) -> BorderInsets {
        let i = self.insets_mm.unwrap_or(BorderInsets {
            top: self.inset_mm,
            bottom: self.inset_mm,
            left: self.inset_mm,
            right: self.inset_mm,
        });
        BorderInsets {
            top: i.top.max(0.0),
            bottom: i.bottom.max(0.0),
            left: i.left.max(0.0),
            right: i.right.max(0.0),
        }
    }
}

impl Default for LabelBorder {
    fn default() -> Self {
        Self {
            style: BorderStyle::Solid,
            width_mm: default_border_width_mm(),
            sides: BorderSides::ALL,
            pattern_mm: default_border_pattern_mm(),
            inset_mm: 0.0,
            insets_mm: None,
        }
    }
}

fn default_version() -> u32 {
    LABEL_FORMAT_VERSION
}

fn default_gap_mm() -> f32 {
    2.0
}

/// One `.llabel` template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Label {
    /// Format version, see [`LABEL_FORMAT_VERSION`].
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub elements: Vec<Item>,
    /// Flow layout: blank space between two elements without a box, in mm.
    #[serde(default = "default_gap_mm")]
    pub gap_mm: f32,
    /// Blank space after the content's end (right margin), in mm; also
    /// the start margin unless `padding_start_mm` is set.
    #[serde(default)]
    pub padding_mm: f32,
    /// Blank space before the content (left margin), in mm; `None` = same
    /// as `padding_mm`. Boxes keep their positions (the editor shows the
    /// margin and aligns/snaps to it); flow content starts after it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding_start_mm: Option<f32>,
    /// Minimum label length in mm. Flow-only labels are centered in it;
    /// with boxes it's a fixed minimum (boxes keep their positions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length_mm: Option<f32>,
    /// Draw a solid border around the whole label (older files; see
    /// `border` for styled borders, which takes precedence).
    #[serde(default)]
    pub frame: bool,
    /// With `min_length_mm`: the label is exactly that long, content
    /// beyond it is cut off (otherwise the length only grows to fit).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fixed_length: bool,
    /// Styled border: pattern, thickness, sides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<LabelBorder>,
    /// Number of tape strips stacked on top of each other (multi-tape
    /// label): the design is `strips` x the printable height tall and is
    /// printed as one strip per slice, top first.
    #[serde(default = "one_strip", skip_serializing_if = "is_one_strip")]
    pub strips: u8,
    /// How the label is designed: `landscape` (tape horizontal, the
    /// default) or `portrait` (tape vertical: boxes are in portrait
    /// coordinates, x across the tape, y along it). Printing always uses
    /// the landscape form ([`Label::to_landscape`]).
    #[serde(default, skip_serializing_if = "Orientation::is_landscape")]
    pub orientation: Orientation,
    /// Decorative segment frame `set:frame` (see `ll_render::decor`), drawn
    /// between the label margins over the full printable height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decor: Option<String>,
    /// The template this label was generated from (editable again in the
    /// wizard); no effect on rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Box<crate::layouts::Layout>>,
}

/// Editor orientation of a label, see [`Label::orientation`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    #[default]
    Landscape,
    Portrait,
}

impl Orientation {
    fn is_landscape(&self) -> bool {
        *self == Orientation::Landscape
    }
}

impl Rect {
    /// A portrait box (tape `tape_mm` wide, viewed with the label start on
    /// top) in landscape coordinates. The portrait view is the landscape
    /// view turned 90° clockwise.
    pub fn portrait_to_landscape(&self, tape_mm: f32) -> Rect {
        Rect {
            x_mm: self.y_mm,
            y_mm: tape_mm - self.x_mm - self.w_mm,
            w_mm: self.h_mm,
            h_mm: self.w_mm,
        }
    }

    /// Inverse of [`Self::portrait_to_landscape`].
    pub fn landscape_to_portrait(&self, tape_mm: f32) -> Rect {
        Rect {
            x_mm: tape_mm - self.y_mm - self.h_mm,
            y_mm: self.x_mm,
            w_mm: self.h_mm,
            h_mm: self.w_mm,
        }
    }
}

impl Label {
    /// The same label in landscape form (what is printed). Portrait boxes
    /// are mapped onto the tape and their content turned so it reads
    /// upright with the tape held vertically. `tape_mm` is the design
    /// height (printable width x strips).
    pub fn to_landscape(&self, tape_mm: f32) -> Label {
        if self.orientation == Orientation::Landscape {
            return self.clone();
        }
        let mut out = self.clone();
        out.orientation = Orientation::Landscape;
        for item in &mut out.elements {
            if let Some(r) = item.rect {
                item.rect = Some(r.portrait_to_landscape(tape_mm));
                item.rotation = (item.rotation + 270) % 360;
            }
        }
        out
    }
}

fn one_strip() -> u8 {
    1
}

fn is_one_strip(n: &u8) -> bool {
    *n <= 1
}

impl Default for Label {
    fn default() -> Self {
        Self {
            version: LABEL_FORMAT_VERSION,
            elements: Vec::new(),
            gap_mm: default_gap_mm(),
            padding_mm: 0.0,
            padding_start_mm: None,
            min_length_mm: None,
            frame: false,
            fixed_length: false,
            border: None,
            strips: 1,
            orientation: Orientation::Landscape,
            decor: None,
            source: None,
        }
    }
}

/// A free-layout box in mm, see the module docs for the axes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x_mm: f32,
    pub y_mm: f32,
    pub w_mm: f32,
    pub h_mm: f32,
}

/// One element plus its optional box. Serialized flat, e.g.
/// `{"type": "text", "text": "Hallo", "rect": {"x_mm": 0, ...}}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    #[serde(flatten)]
    pub element: Element,
    /// `None`: placed by the flow layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    /// Clockwise rotation of the content inside its box: 0, 90, 180 or
    /// 270 degrees (other values are rounded down to a quarter turn).
    /// Only applies to boxed elements.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: u16,
    /// Locked in the editor: not movable or resizable with mouse/keys.
    /// Has no effect on rendering.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    /// Horizontal position of the content inside its box (codes, images,
    /// symbols; text uses its own `align`). `None` = centered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub halign: Option<TextAlign>,
    /// Vertical position of the content inside its box (all elements).
    /// `None` = middle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valign: Option<VAlign>,
    /// Hidden in the editor's layer list: not rendered or printed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// User-given name in the layer list (no effect on rendering). Stored
    /// as `title`: the item is flattened into the element's JSON and
    /// `name` is the symbol element's icon id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

fn is_zero(v: &u16) -> bool {
    *v == 0
}

impl From<Element> for Item {
    fn from(element: Element) -> Self {
        Self {
            element,
            rect: None,
            rotation: 0,
            locked: false,
            halign: None,
            valign: None,
            hidden: false,
            title: None,
        }
    }
}

/// Element content. Serialized with a `"type"` tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Element {
    Text {
        /// May contain `\n` for multiple lines.
        text: String,
        /// Font size in points; `None` fits the text to its box (or, in the
        /// flow layout, to the tape height).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size_pt: Option<f32>,
        #[serde(default)]
        align: TextAlign,
        /// System font family; `None` = default font.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        font: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        bold: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        italic: bool,
        /// Line spacing as a multiple of the font's normal line height
        /// (0.5–3); `None` = 1.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line_spacing: Option<f32>,
    },
    Qr {
        data: String,
    },
    Barcode {
        symbology: Symbology,
        data: String,
    },
    Image {
        path: PathBuf,
        #[serde(default)]
        invert: bool,
        /// -100..100, 0 = unchanged.
        #[serde(default, skip_serializing_if = "is_zero_i8")]
        brightness: i8,
        /// -100..100, 0 = unchanged.
        #[serde(default, skip_serializing_if = "is_zero_i8")]
        contrast: i8,
        /// Crop, background removal, rotation, halftone (non-destructive).
        #[serde(default, skip_serializing_if = "ImageEdit::is_identity")]
        edit: ImageEdit,
    },
    /// A bundled symbol by name, see `ll_render::SYMBOL_NAMES`.
    Symbol {
        name: String,
        #[serde(default)]
        invert: bool,
    },
    /// A solid black box (separator lines, bars, blocks).
    Fill,
    /// Line, rectangle, rounded rectangle or ellipse filling its box.
    Shape {
        #[serde(default)]
        shape: ShapeKind,
        /// Line width in mm.
        #[serde(default = "default_stroke_mm")]
        stroke_mm: f32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        filled: bool,
    },
    /// Row of fields with separators (fuse box / distribution board),
    /// spread over its box by [`FuseField::ratio`].
    FuseBox {
        fields: Vec<FuseField>,
        /// Width of one ratio unit in mm (one module); gives the natural
        /// length, the editor sizes the box with it.
        #[serde(default = "default_pitch_mm")]
        pitch_mm: f32,
        #[serde(default)]
        separator: FuseSeparator,
        /// Text across the tape (reading bottom to top).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        vertical: bool,
        /// Fields in reverse order (last field at the label start).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        reverse: bool,
        /// All fields one module wide, no merging (patch panel); only an
        /// editor hint, rendering follows the ratios as always.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        fixed: bool,
        /// One size for all fields; `None` = largest size fitting every
        /// field.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size_pt: Option<f32>,
        #[serde(default)]
        align: TextAlign,
        /// Line spacing of multi-line field texts (0.5–3); `None` = 1.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line_spacing: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        font: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        bold: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        italic: bool,
    },
}

/// Default module width (DIN rail, 17.5 mm).
fn default_pitch_mm() -> f32 {
    17.5
}

fn is_zero_i8(v: &i8) -> bool {
    *v == 0
}

/// Default shape line width: 0.3 mm (about 2 print dots).
fn default_stroke_mm() -> f32 {
    0.3
}

impl Element {
    /// Auto-sized, centered text.
    pub fn text(text: impl Into<String>) -> Self {
        Element::Text {
            text: text.into(),
            size_pt: None,
            align: TextAlign::default(),
            font: None,
            bold: false,
            italic: false,
            line_spacing: None,
        }
    }
}

impl Label {
    /// The border to draw: `border` if set, else a default solid one when
    /// the older `frame` flag is on.
    pub fn effective_border(&self) -> Option<LabelBorder> {
        self.border
            .or_else(|| self.frame.then(LabelBorder::default))
    }

    /// A label with just `element` (flow layout) and default spacing.
    pub fn single(element: Element) -> Self {
        Self {
            elements: vec![element.into()],
            ..Self::default()
        }
    }

    /// Parses a `.llabel` JSON document. Rejects newer format versions.
    pub fn from_json(json: &str) -> Result<Self, CoreError> {
        let label: Label =
            serde_json::from_str(json).map_err(|e| CoreError::Template(e.to_string()))?;
        if label.version > LABEL_FORMAT_VERSION {
            return Err(CoreError::Template(format!(
                "format version {} is newer than supported ({LABEL_FORMAT_VERSION})",
                label.version
            )));
        }
        Ok(label)
    }

    /// Serializes as pretty-printed `.llabel` JSON (current version).
    pub fn to_json(&self) -> Result<String, CoreError> {
        let current = Label {
            version: LABEL_FORMAT_VERSION,
            ..self.clone()
        };
        serde_json::to_string_pretty(&current).map_err(|e| CoreError::Template(e.to_string()))
    }

    /// Reads a `.llabel` file and resolves relative image paths against
    /// the file's directory.
    pub fn load(path: &Path) -> Result<Self, CoreError> {
        let json = std::fs::read_to_string(path)?;
        let mut label = Self::from_json(&json)?;
        if let Some(dir) = path.parent() {
            label.resolve_paths(dir);
        }
        Ok(label)
    }

    /// Writes the label as a `.llabel` file.
    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        std::fs::write(path, self.to_json()?)?;
        Ok(())
    }

    /// Makes relative image paths absolute against `base`.
    pub fn resolve_paths(&mut self, base: &Path) {
        for item in &mut self.elements {
            if let Element::Image { path, .. } = &mut item.element {
                if path.is_relative() {
                    *path = base.join(&*path);
                }
            }
        }
    }

    fn has_text(&self) -> bool {
        self.elements
            .iter()
            .any(|i| matches!(i.element, Element::Text { .. }))
    }
}

/// Fonts loaded while rendering one label: the default font's raw bytes
/// (flow fast path, unchanged since M5) and parsed faces per
/// (family, bold, italic).
/// Font faces are parsed once per process and shared: parsing a font
/// file on every live-preview render costs noticeable time.
type FaceKey = (Option<String>, bool, bool);

fn shared_faces() -> &'static Mutex<HashMap<FaceKey, Arc<Face>>> {
    static FACES: OnceLock<Mutex<HashMap<FaceKey, Arc<Face>>>> = OnceLock::new();
    FACES.get_or_init(Default::default)
}

fn shared_default_bytes() -> Result<Arc<Vec<u8>>, CoreError> {
    static BYTES: OnceLock<Arc<Vec<u8>>> = OnceLock::new();
    if let Some(bytes) = BYTES.get() {
        return Ok(bytes.clone());
    }
    let bytes = Arc::new(ll_render::fontsrc::load_default_font()?);
    Ok(BYTES.get_or_init(|| bytes).clone())
}

/// Fonts used while composing one label (handles into the shared cache).
#[derive(Default)]
struct FontCache {
    default_bytes: Option<Arc<Vec<u8>>>,
    faces: HashMap<FaceKey, Arc<Face>>,
}

impl FontCache {
    fn default_bytes(&mut self) -> Result<&[u8], CoreError> {
        if self.default_bytes.is_none() {
            self.default_bytes = Some(shared_default_bytes()?);
        }
        Ok(self
            .default_bytes
            .as_deref()
            .map(Vec::as_slice)
            .unwrap_or_default())
    }

    fn face(
        &mut self,
        family: &Option<String>,
        bold: bool,
        italic: bool,
    ) -> Result<Arc<Face>, CoreError> {
        let key = (family.clone(), bold, italic);
        if let Some(face) = self.faces.get(&key) {
            return Ok(face.clone());
        }
        let shared = shared_faces()
            .lock()
            .ok()
            .and_then(|faces| faces.get(&key).cloned());
        let face = match shared {
            Some(face) => face,
            None => {
                let face = Arc::new(Face::load(family.as_deref(), bold, italic)?);
                if let Ok(mut faces) = shared_faces().lock() {
                    faces.insert(key.clone(), face.clone());
                }
                face
            }
        };
        self.faces.insert(key, face.clone());
        Ok(face)
    }

    /// Regular, bold, italic and bold-italic faces for a text element
    /// (on top of its base style). Without inline styles only one face is
    /// loaded.
    fn faces(
        &mut self,
        family: &Option<String>,
        bold: bool,
        italic: bool,
        text: &str,
    ) -> Result<[Arc<Face>; 4], CoreError> {
        let base = self.face(family, bold, italic)?;
        if !ll_render::richtext::has_markup(text) {
            return Ok([base.clone(), base.clone(), base.clone(), base]);
        }
        Ok([
            base,
            self.face(family, true, italic)?,
            self.face(family, bold, true)?,
            self.face(family, true, true)?,
        ])
    }
}

fn face_set(faces: &[Arc<Face>; 4]) -> FaceSet<'_> {
    FaceSet::new(&faces[0], &faces[1], &faces[2], &faces[3])
}

/// The surface a label is composed on. For a normal print it is the real
/// print head with the tape's pin range. Multi-tape labels ([`Label::strips`])
/// and the high-resolution preview use a virtual head that is exactly the
/// (stacked, scaled) printable area, offset 0.
#[derive(Debug, Clone, Copy)]
struct Canvas {
    head_pins: u16,
    offset: u16,
    pins: u16,
    /// Resolution multiplier relative to the printer's 180 dpi.
    scale: u32,
}

impl Canvas {
    fn new(label: &Label, model: &ModelInfo, geometry: &TapeGeometry, scale: u32) -> Self {
        let scale = scale.max(1);
        let strips = label.strips.max(1) as u32;
        if strips == 1 && scale == 1 {
            return Self {
                head_pins: model.head_pins,
                offset: geometry.left_offset_pins,
                pins: geometry.printable_pins,
                scale,
            };
        }
        let pins = (geometry.printable_pins as u32 * strips * scale).min(u16::MAX as u32) as u16;
        Self {
            head_pins: pins,
            offset: 0,
            pins,
            scale,
        }
    }

    /// Millimeters to dots at this canvas' resolution.
    fn mm(&self, mm: f32) -> u32 {
        mm_to_dots(mm * self.scale as f32)
    }

    fn signed_mm(&self, mm: f32) -> i32 {
        let d = self.mm(mm.abs()) as i32;
        if mm < 0.0 {
            -d
        } else {
            d
        }
    }

    fn pt(&self, pt: f32) -> f32 {
        pt_to_dots(pt) * self.scale as f32
    }

    /// A box in dots: (x, y, w, h).
    fn rect(&self, rect: &Rect) -> (i32, i32, u32, u16) {
        (
            self.signed_mm(rect.x_mm),
            self.signed_mm(rect.y_mm),
            self.mm(rect.w_mm),
            self.mm(rect.h_mm).min(u16::MAX as u32) as u16,
        )
    }
}

/// Renders one element for the flow layout: full head width, filling the
/// tape's printable height, natural length.
/// Sets `overflow` if text had to be clipped.
fn render_flow_element(
    element: &Element,
    canvas: &Canvas,
    fonts: &mut FontCache,
    overflow: &mut bool,
) -> Result<Bitmap, CoreError> {
    let (head, pins, offset) = (canvas.head_pins, canvas.pins, canvas.offset);
    Ok(match element {
        Element::Text {
            text,
            size_pt: None,
            font: None,
            bold: false,
            italic: false,
            ..
        } if !text.contains('\n') && !ll_render::richtext::has_markup(text) => {
            ll_render::render_text_with_font(text, fonts.default_bytes()?, head, pins, offset)?
        }
        Element::Text {
            text,
            size_pt,
            align,
            font,
            bold,
            italic,
            line_spacing,
        } => {
            let faces = fonts.faces(font, *bold, *italic, text)?;
            let faces = face_set(&faces);
            let size_px = size_pt.map(|pt| canvas.pt(pt));
            let spacing = line_spacing.unwrap_or(1.0);
            let width = boxed::text_natural_width(text, &faces, pins, size_px, spacing)?;
            let (local, clipped) = boxed::text_in_box_checked(
                text,
                &faces,
                width,
                pins,
                &TextLayout {
                    size_px,
                    align: *align,
                    valign: VAlign::Middle,
                    line_spacing: spacing,
                },
            )?;
            *overflow |= clipped;
            let mut out = Bitmap::new(head, width);
            out.blit(&local, offset as i32, 0, offset..offset + pins);
            out
        }
        Element::Qr { data } => {
            ll_render::render_qr(data, head, pins, offset, QrErrorCorrection::Medium)?
        }
        Element::Barcode { symbology, data } => ll_render::render_barcode_with_module(
            *symbology,
            data,
            head,
            pins,
            offset,
            ll_render::linear_barcode::MODULE_PX * canvas.scale,
        )?,
        Element::Image {
            path,
            invert,
            brightness,
            contrast,
            edit,
        } => ll_render::render_image_edited(
            path,
            head,
            pins,
            offset,
            *invert,
            ImageAdjust {
                brightness: *brightness,
                contrast: *contrast,
            },
            edit,
        )?,
        Element::Symbol { name, invert } => {
            ll_render::render_symbol(name, head, pins, offset, *invert)?
        }
        Element::Shape {
            shape,
            stroke_mm,
            filled,
        } => {
            // A flow-layout shape is a square of the tape height (a line
            // runs across that square).
            let local = ll_render::shape::shape_in_box(
                *shape,
                pins as u32,
                pins,
                canvas.mm(*stroke_mm) as f32,
                *filled,
            )?;
            let mut out = Bitmap::new(head, pins as u32);
            out.blit(&local, offset as i32, 0, offset..offset + pins);
            out
        }
        Element::FuseBox {
            fields, pitch_mm, ..
        } => {
            // Natural length: the ratios in modules of `pitch_mm`.
            let units: f32 = fields.iter().map(FuseField::ratio).sum();
            let len = canvas.mm(units * pitch_mm.max(1.0)).max(1);
            let local =
                render_fuse_box(element, len, pins, VAlign::Middle, canvas, fonts, overflow)?;
            let mut out = Bitmap::new(head, len);
            out.blit(&local, offset as i32, 0, offset..offset + pins);
            out
        }
        Element::Fill => {
            // A flow-layout fill is a 1 mm bar across the tape.
            let mut out = Bitmap::new(head, canvas.mm(1.0));
            let mut bar = Bitmap::new(pins, canvas.mm(1.0));
            bar.fill();
            out.blit(&bar, offset as i32, 0, offset..offset + pins);
            out
        }
    })
}

/// Renders one element into a box-local bitmap of `w` x `h` dots. Sets
/// `overflow` if text had to be clipped.
fn render_boxed_element(
    element: &Element,
    w: u32,
    h: u16,
    valign: VAlign,
    canvas: &Canvas,
    fonts: &mut FontCache,
    overflow: &mut bool,
) -> Result<Bitmap, CoreError> {
    Ok(match element {
        Element::Text {
            text,
            size_pt,
            align,
            font,
            bold,
            italic,
            line_spacing,
        } => {
            let faces = fonts.faces(font, *bold, *italic, text)?;
            let (bitmap, clipped) = boxed::text_in_box_checked(
                text,
                &face_set(&faces),
                w,
                h,
                &TextLayout {
                    size_px: size_pt.map(|pt| canvas.pt(pt)),
                    align: *align,
                    valign,
                    line_spacing: line_spacing.unwrap_or(1.0),
                },
            )?;
            *overflow |= clipped;
            bitmap
        }
        Element::Qr { data } => boxed::qr_in_box(data, w, h, QrErrorCorrection::Medium)?,
        Element::Barcode { symbology, data } => boxed::barcode_in_box(*symbology, data, w, h)?,
        Element::Image {
            path,
            invert,
            brightness,
            contrast,
            edit,
        } => boxed::image_in_box_edited(
            path,
            w,
            h,
            *invert,
            ImageAdjust {
                brightness: *brightness,
                contrast: *contrast,
            },
            edit,
        )?,
        Element::Shape {
            shape,
            stroke_mm,
            filled,
        } => ll_render::shape::shape_in_box(*shape, w, h, canvas.mm(*stroke_mm) as f32, *filled)?,
        Element::Symbol { name, invert } => boxed::symbol_in_box(name, w, h, *invert)?,
        Element::FuseBox { .. } => render_fuse_box(element, w, h, valign, canvas, fonts, overflow)?,
        Element::Fill => {
            let mut b = Bitmap::new(h, w);
            b.fill();
            b
        }
    })
}

/// Renders a [`Element::FuseBox`] into a `w` x `h` dot box: fields by
/// ratio, one common text size, separators on top. `valign` places the
/// text in each field. Sets `overflow` if a field's text was clipped.
fn render_fuse_box(
    element: &Element,
    w: u32,
    h: u16,
    valign: VAlign,
    canvas: &Canvas,
    fonts: &mut FontCache,
    overflow: &mut bool,
) -> Result<Bitmap, CoreError> {
    let mut out = Bitmap::new(h, w);
    let Element::FuseBox {
        fields,
        separator,
        vertical,
        reverse,
        size_pt,
        align,
        line_spacing,
        font,
        bold,
        italic,
        ..
    } = element
    else {
        return Ok(out);
    };
    let spacing = line_spacing.unwrap_or(1.0);
    let spans = fusebox::field_spans(fields, w, *reverse);
    let line = fusebox::line_dots(
        *separator,
        canvas.mm(fusebox::LINE_MM).max(1),
        canvas.mm(fusebox::BOLD_MM).max(2),
    );
    // Text keeps clear of the separators.
    let pad = line + canvas.mm(0.4);
    let frame = if *separator == FuseSeparator::Frame {
        line
    } else {
        0
    };
    let top = (frame + canvas.mm(0.3)).min(h as u32 / 2) as u16;
    let inner_h = h.saturating_sub(2 * top);
    // Per field: index, start along the box, length, text across the tape.
    let boxes: Vec<(usize, u32, u32, bool)> = spans
        .iter()
        .filter_map(|&(i, start, end)| {
            let len = (end - start).saturating_sub(2 * pad);
            (len > 0 && inner_h > 0 && !fields[i].text.trim().is_empty())
                .then(|| (i, start + pad, len, fields[i].vertical.unwrap_or(*vertical)))
        })
        .collect();
    // Text box in text coordinates (rotated for vertical fields).
    let dims = |len: u32, vertical: bool| -> (u32, u16) {
        if vertical {
            (inner_h as u32, len.min(u16::MAX as u32) as u16)
        } else {
            (len, inner_h)
        }
    };
    if !boxes.is_empty() {
        let all: Vec<&str> = fields.iter().map(|f| f.text.as_str()).collect();
        let faces = fonts.faces(font, *bold, *italic, &all.join("\n"))?;
        let faces = face_set(&faces);
        let px = match size_pt {
            Some(pt) => canvas.pt(*pt),
            None => boxes
                .iter()
                .map(|&(i, _, len, v)| {
                    let (tw, th) = dims(len, v);
                    boxed::text_fit_px(&fields[i].text, &faces, tw, th, spacing)
                })
                .fold(f32::INFINITY, f32::min),
        };
        for &(i, start, len, v) in &boxes {
            let (tw, th) = dims(len, v);
            let (text, clipped) = boxed::text_in_box_checked(
                &fields[i].text,
                &faces,
                tw,
                th,
                &TextLayout {
                    size_px: Some(px),
                    align: *align,
                    valign,
                    line_spacing: spacing,
                },
            )?;
            *overflow |= clipped;
            // Vertical text reads bottom to top, as usual on distribution boards.
            let text = if v { text.rotated(3) } else { text };
            out.blit(&text, top as i32, start as i32, 0..h);
        }
    }
    let mut edges: Vec<u32> = spans.iter().map(|s| s.1).collect();
    edges.extend(spans.last().map(|s| s.2));
    fusebox::draw_separators(
        &mut out,
        &edges,
        *separator,
        fusebox::SeparatorDots {
            line,
            dash: canvas.mm(fusebox::DASH_MM).max(1),
            gap: canvas.mm(fusebox::DASH_GAP_MM).max(1),
        },
    );
    Ok(out)
}

/// The rendered label plus, per element, its resolved box in canvas dots
/// `(x, y, w, h)` (flow elements included).
struct Composed {
    bitmap: Bitmap,
    boxes: Vec<(i32, i32, u32, u32)>,
    /// Per element: text did not fit and was clipped.
    overflow: Vec<bool>,
}

/// Space in dots the flow layout leaves free for the border per side.
#[derive(Debug, Default, Clone, Copy)]
struct Reserve {
    top: u16,
    bottom: u16,
    left: u32,
    right: u32,
}

/// Inset + line + a clearance of one line width (at least
/// [`BORDER_CLEARANCE_MM`]) on each side that has a border.
fn border_reserve(border: &LabelBorder, canvas: &Canvas) -> Reserve {
    let width = border.width_mm.max(0.0);
    let line = width + width.max(BORDER_CLEARANCE_MM);
    let insets = border.insets();
    let dots = |inset: f32| canvas.mm(inset + line);
    let pins = |inset: f32| dots(inset).min(u16::MAX as u32) as u16;
    let sides = border.sides;
    Reserve {
        top: if sides.top { pins(insets.top) } else { 0 },
        bottom: if sides.bottom { pins(insets.bottom) } else { 0 },
        left: if sides.left { dots(insets.left) } else { 0 },
        right: if sides.right { dots(insets.right) } else { 0 },
    }
}

/// Design height of `canvas` in mm (printable width x strips).
fn design_height_mm(canvas: &Canvas) -> f32 {
    dots_to_mm(canvas.pins as u32) / canvas.scale as f32
}

fn compose(label: &Label, canvas: &Canvas) -> Result<Composed, CoreError> {
    if label.orientation == Orientation::Portrait {
        return compose(&label.to_landscape(design_height_mm(canvas)), canvas);
    }
    let mut fonts = FontCache::default();
    if label.has_text() {
        fonts.default_bytes()?; // fail early with a clear "no font" error
    }
    let pins = canvas.pins;
    let gap = canvas.mm(label.gap_mm);
    let padding = canvas.mm(label.padding_mm);
    let padding_start = canvas.mm(label.padding_start_mm.unwrap_or(label.padding_mm));
    // Flow content keeps clear of the border on the sides that have one.
    let reserve = label
        .effective_border()
        .map(|b| border_reserve(&b, canvas))
        .unwrap_or_default();
    let decor = label.decor.as_deref().and_then(ll_render::decor::resolve);
    let reserve = match &decor {
        Some(frame) => {
            let (start, end) = ll_render::decor::end_widths(frame, pins)?;
            Reserve {
                left: reserve.left.max(start),
                right: reserve.right.max(end),
                ..reserve
            }
        }
        None => reserve,
    };
    let flow_canvas = Canvas {
        offset: canvas.offset + reserve.top,
        pins: pins.saturating_sub(reserve.top + reserve.bottom).max(1),
        ..*canvas
    };

    // Flow pass: elements without a box, one after another.
    let mut bitmap = Bitmap::new(canvas.head_pins, 0);
    let mut boxes: Vec<Option<(i32, i32, u32, u32)>> = vec![None; label.elements.len()];
    let mut overflow = vec![false; label.elements.len()];
    bitmap.extend_blank(padding_start + reserve.left);
    let mut first = true;
    for (i, item) in label.elements.iter().enumerate() {
        if item.rect.is_some() || item.hidden {
            continue;
        }
        if !first {
            bitmap.extend_blank(gap);
        }
        first = false;
        let start = bitmap.height_dots();
        // Every flow element renders at `canvas.head_pins` wide, so this
        // can't mismatch; checked anyway rather than dropping content.
        if !bitmap.append(&render_flow_element(
            &item.element,
            &flow_canvas,
            &mut fonts,
            &mut overflow[i],
        )?) {
            return Err(CoreError::Template("element width mismatch".into()));
        }
        boxes[i] = Some((
            start as i32,
            reserve.top as i32,
            bitmap.height_dots() - start,
            flow_canvas.pins as u32,
        ));
    }
    bitmap.extend_blank(padding + reserve.right);

    let has_boxes = label.elements.iter().any(|i| i.rect.is_some());
    let min_len = label.min_length_mm.map(|mm| canvas.mm(mm)).unwrap_or(0);
    if !has_boxes {
        // Flow only: center the content in the minimum length.
        let missing = min_len.saturating_sub(bitmap.height_dots());
        let shift = missing / 2;
        bitmap.prepend_blank(shift);
        bitmap.extend_blank(missing - shift);
        for b in boxes.iter_mut().flatten() {
            b.0 += shift as i32;
        }
    } else {
        let content_end = label
            .elements
            .iter()
            .filter_map(|i| i.rect.as_ref().map(|r| canvas.rect(r)))
            .map(|(x, _, w, _)| (x + w as i32).max(0) as u32 + padding + reserve.right)
            .max()
            .unwrap_or(0);
        let length = content_end.max(min_len);
        bitmap.extend_blank(length.saturating_sub(bitmap.height_dots()));

        let clip = canvas.offset..canvas.offset + pins;
        for (i, item) in label.elements.iter().enumerate() {
            let Some(rect) = &item.rect else { continue };
            if item.hidden {
                boxes[i] = Some(canvas.rect(rect)).map(|(x, y, w, h)| (x, y, w, h as u32));
                continue;
            }
            let (x, y, w, h) = canvas.rect(rect);
            if w > 0 && h > 0 {
                let turns = ((item.rotation / 90) % 4) as u8;
                let valign = item.valign.unwrap_or_default();
                let local = if turns % 2 == 1 {
                    // Render into the swapped box, then turn it upright.
                    let (rw, rh) = (h as u32, w.min(u16::MAX as u32) as u16);
                    render_boxed_element(
                        &item.element,
                        rw,
                        rh,
                        valign,
                        canvas,
                        &mut fonts,
                        &mut overflow[i],
                    )?
                    .rotated(turns)
                } else {
                    render_boxed_element(
                        &item.element,
                        w,
                        h,
                        valign,
                        canvas,
                        &mut fonts,
                        &mut overflow[i],
                    )?
                    .rotated(turns)
                };
                // Codes, images and symbols render centered; move them to
                // the requested side (text aligns itself in its layout).
                let local = match item.element {
                    Element::Qr { .. }
                    | Element::Barcode { .. }
                    | Element::Image { .. }
                    | Element::Symbol { .. } => {
                        boxed::align_content(&local, h, item.halign, item.valign)
                    }
                    _ => local,
                };
                bitmap.blit(&local, canvas.offset as i32 + y, x, clip.clone());
            }
            boxes[i] = Some((x, y, w, h as u32));
        }
    }

    if label.fixed_length && min_len > 0 {
        bitmap.truncate(min_len);
        bitmap.extend_blank(min_len.saturating_sub(bitmap.height_dots()));
    }

    if let Some(frame) = &decor {
        let length = bitmap.height_dots().saturating_sub(padding_start + padding);
        let local = ll_render::decor::render_frame(frame, length, pins)?;
        bitmap.blit(
            &local,
            canvas.offset as i32,
            padding_start as i32,
            canvas.offset..canvas.offset + pins,
        );
    }

    if let Some(border) = label.effective_border() {
        let insets = border.insets();
        let thickness = if border.width_mm > 0.0 {
            canvas.mm(border.width_mm).clamp(1, u16::MAX as u32) as u16
        } else {
            0
        };
        ll_render::draw_border_styled(
            &mut bitmap,
            canvas.offset,
            pins,
            &ll_render::Border {
                style: border.style,
                thickness,
                sides: border.sides,
                pattern: canvas.mm(border.pattern_mm).max(1),
                // Left/right sit inside the label margins.
                inset: ll_render::Insets {
                    top: canvas.mm(insets.top),
                    bottom: canvas.mm(insets.bottom),
                    left: padding_start + canvas.mm(insets.left),
                    right: padding + canvas.mm(insets.right),
                },
            },
        );
    }
    Ok(Composed {
        bitmap,
        boxes: boxes.into_iter().map(Option::unwrap_or_default).collect(),
        overflow,
    })
}

/// Renders `label` for `model` with the tape described by `geometry`. For
/// a single-tape label this is exactly the bitmap that gets printed; for
/// a multi-tape label it is the whole stacked design (see
/// [`render_label_pages`] for what gets printed).
pub fn render_label(
    label: &Label,
    model: &ModelInfo,
    geometry: &TapeGeometry,
) -> Result<Bitmap, CoreError> {
    Ok(compose(label, &Canvas::new(label, model, geometry, 1))?.bitmap)
}

/// The bitmaps to print for `label`, one per tape strip, top strip first.
/// A multi-tape label is rendered as one tall design and cut into
/// horizontal slices of the tape's printable height; each slice lands at
/// the tape's pin offset on the real print head.
pub fn render_label_pages(
    label: &Label,
    model: &ModelInfo,
    geometry: &TapeGeometry,
) -> Result<Vec<Bitmap>, CoreError> {
    let design = render_label(label, model, geometry)?;
    let strips = label.strips.max(1);
    if strips == 1 {
        return Ok(vec![design]);
    }
    let pins = geometry.printable_pins;
    Ok((0..strips as u16)
        .map(|k| {
            let mut page = Bitmap::new(model.head_pins, design.height_dots());
            for line in 0..design.height_dots() {
                for p in 0..pins {
                    if design.pixel(k * pins + p, line) {
                        page.set_pixel(geometry.left_offset_pins + p, line, true);
                    }
                }
            }
            page
        })
        .collect())
}

/// Every element's box in mm as rendered — flow elements get the box the
/// flow layout gave them. The GUI uses this to turn flow elements into
/// freely movable boxes without changing how the label looks.
pub fn resolved_rects(
    label: &Label,
    model: &ModelInfo,
    geometry: &TapeGeometry,
) -> Result<Vec<Rect>, CoreError> {
    let canvas = Canvas::new(label, model, geometry, 1);
    let composed = compose(label, &canvas)?;
    let tape_mm = design_height_mm(&canvas);
    let mm = |d: i32| {
        let v = dots_to_mm(d.unsigned_abs());
        if d < 0 {
            -v
        } else {
            v
        }
    };
    Ok(composed
        .boxes
        .into_iter()
        .map(|(x, y, w, h)| {
            let r = Rect {
                x_mm: mm(x),
                y_mm: mm(y),
                w_mm: dots_to_mm(w),
                h_mm: dots_to_mm(h),
            };
            match label.orientation {
                Orientation::Landscape => r,
                Orientation::Portrait => r.landscape_to_portrait(tape_mm),
            }
        })
        .collect())
}

/// Looks up `width_mm` in `model`'s tape table.
pub fn geometry_for(model: &ModelInfo, width_mm: u8) -> Result<&TapeGeometry, CoreError> {
    model
        .tape_geometries
        .iter()
        .find(|g| g.width_mm == width_mm)
        .ok_or(CoreError::Protocol(
            ll_protocol::ProtocolError::UnsupportedTapeWidth(width_mm),
        ))
}

/// Renders `label` for a `width_mm` tape and encodes the printable area
/// (all strips of a multi-tape label) as PNG (`labellab render`).
pub fn render_label_png(
    label: &Label,
    model: &ModelInfo,
    width_mm: u8,
) -> Result<Vec<u8>, CoreError> {
    let geometry = geometry_for(model, width_mm)?;
    let canvas = Canvas::new(label, model, geometry, 1);
    let bitmap = compose(label, &canvas)?.bitmap;
    Ok(ll_render::png::to_png(&bitmap, canvas.offset, canvas.pins)?)
}

/// A rendered GUI preview.
#[derive(Debug, Clone)]
pub struct Preview {
    /// Transparent PNG mask (ink opaque, background clear).
    pub png: Vec<u8>,
    /// Indices of elements whose text does not fit its box (clipped).
    pub overflowing: Vec<usize>,
}

/// GUI preview: the printable area (all strips) as a transparent PNG mask
/// at `scale` x the print resolution, plus which texts overflow.
/// `scale = 1` is the exact print raster; higher values render the same
/// layout through the same code at finer resolution for a smoother view.
pub fn render_label_preview(
    label: &Label,
    model: &ModelInfo,
    width_mm: u8,
    scale: u32,
) -> Result<Preview, CoreError> {
    let geometry = geometry_for(model, width_mm)?;
    let canvas = Canvas::new(label, model, geometry, scale);
    let composed = compose(label, &canvas)?;
    let png = if label.orientation == Orientation::Portrait {
        // Display only: the printed bitmap stays landscape.
        let mut area = Bitmap::new(canvas.pins, composed.bitmap.height_dots());
        area.blit(&composed.bitmap, -(canvas.offset as i32), 0, 0..canvas.pins);
        let turned = area.rotated(1);
        ll_render::png::to_png_mask(&turned, 0, turned.width_pins())?
    } else {
        ll_render::png::to_png_mask(&composed.bitmap, canvas.offset, canvas.pins)?
    };
    Ok(Preview {
        png,
        overflowing: composed
            .overflow
            .iter()
            .enumerate()
            .filter_map(|(i, o)| o.then_some(i))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p710() -> &'static ModelInfo {
        ll_protocol::model::find_by_name("PT-P710BT").unwrap()
    }

    fn has_font() -> bool {
        ll_render::fontsrc::load_default_font().is_ok()
    }

    fn ink_lines(bitmap: &Bitmap) -> Vec<u32> {
        (0..bitmap.height_dots())
            .filter(|&l| bitmap.row(l).iter().any(|&b| b != 0))
            .collect()
    }

    #[test]
    fn json_round_trip() {
        let label = Label {
            elements: vec![
                Item {
                    element: Element::Text {
                        text: "Server 1\nRack 3".into(),
                        size_pt: Some(12.0),
                        align: TextAlign::Left,
                        font: Some("DejaVu Sans".into()),
                        bold: true,
                        italic: false,
                        line_spacing: None,
                    },
                    rect: Some(Rect {
                        x_mm: 1.0,
                        y_mm: 0.5,
                        w_mm: 30.0,
                        h_mm: 8.0,
                    }),
                    rotation: 0,
                    locked: false,
                    halign: None,
                    valign: None,
                    hidden: false,
                    title: None,
                },
                Element::Qr {
                    data: "https://example.org".into(),
                }
                .into(),
                Element::Barcode {
                    symbology: Symbology::Ean13,
                    data: "400638133393".into(),
                }
                .into(),
                Element::Image {
                    path: "icon.svg".into(),
                    invert: false,
                    brightness: 0,
                    contrast: 0,
                    edit: Default::default(),
                }
                .into(),
            ],
            frame: true,
            min_length_mm: Some(40.0),
            ..Label::default()
        };
        let json = label.to_json().unwrap();
        assert!(json.contains(r#""type": "barcode""#));
        assert!(json.contains(r#""symbology": "ean13""#));
        assert!(json.contains(r#""x_mm": 1.0"#));
        assert_eq!(Label::from_json(&json).unwrap(), label);
    }

    #[test]
    fn symbol_keeps_its_name_next_to_the_item_title() {
        let mut item: Item = Element::Symbol {
            name: "iso7010:P002".into(),
            invert: false,
        }
        .into();
        let json = serde_json::to_string(&item).unwrap();
        assert_eq!(serde_json::from_str::<Item>(&json).unwrap(), item);
        item.title = Some("Rauchverbot".into());
        let json = serde_json::to_string(&item).unwrap();
        assert_eq!(serde_json::from_str::<Item>(&json).unwrap(), item);
    }

    #[test]
    fn reads_version_1_templates() {
        let label = Label::from_json(
            r#"{"version":1,"elements":[{"type":"text","text":"A"},{"type":"qr","data":"x"}]}"#,
        )
        .unwrap();
        assert_eq!(label.elements[0], Element::text("A").into());
        assert!(label.elements.iter().all(|i| i.rect.is_none()));
        assert!(label.to_json().unwrap().contains(r#""version": 2"#));
    }

    #[test]
    fn minimal_json_uses_defaults() {
        let label = Label::from_json(r#"{"elements":[{"type":"qr","data":"x"}]}"#).unwrap();
        assert_eq!(label.version, LABEL_FORMAT_VERSION);
        assert_eq!(label.gap_mm, 2.0);
        assert!(!label.frame);
    }

    #[test]
    fn rejects_newer_version_and_unknown_type() {
        assert!(Label::from_json(r#"{"version": 99}"#).is_err());
        assert!(Label::from_json(r#"{"elements":[{"type":"hologram"}]}"#).is_err());
    }

    #[test]
    fn resolves_relative_image_paths() {
        let mut label = Label::single(Element::Image {
            path: "a.png".into(),
            invert: false,
            brightness: 0,
            contrast: 0,
            edit: Default::default(),
        });
        label.resolve_paths(Path::new("/tmp/labels"));
        assert_eq!(
            label.elements[0].element,
            Element::Image {
                path: "/tmp/labels/a.png".into(),
                invert: false,
                brightness: 0,
                contrast: 0,
                edit: Default::default(),
            }
        );
    }

    #[test]
    fn elements_are_concatenated_with_gap() {
        let model = p710();
        let geometry = geometry_for(model, 9).unwrap();
        let qr = Element::Qr { data: "A".into() };
        let one = render_label(&Label::single(qr.clone()), model, geometry).unwrap();

        let two = Label {
            elements: vec![qr.clone().into(), qr.into()],
            gap_mm: 5.0,
            ..Label::default()
        };
        let two = render_label(&two, model, geometry).unwrap();
        assert_eq!(two.height_dots(), 2 * one.height_dots() + mm_to_dots(5.0),);
    }

    #[test]
    fn min_length_pads_and_centers() {
        let model = p710();
        let geometry = geometry_for(model, 9).unwrap();
        let label = Label {
            min_length_mm: Some(50.0),
            ..Label::single(Element::Qr { data: "A".into() })
        };
        let bitmap = render_label(&label, model, geometry).unwrap();
        assert_eq!(bitmap.height_dots(), mm_to_dots(50.0));
        // Centered: the first raster line is blank padding.
        assert!(bitmap.row(0).iter().all(|&b| b == 0));
    }

    #[test]
    fn boxed_element_lands_at_its_position() {
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let label = Label {
            elements: vec![Item {
                element: Element::Barcode {
                    symbology: Symbology::Code128,
                    data: "AB".into(),
                },
                rect: Some(Rect {
                    x_mm: 20.0,
                    y_mm: 2.0,
                    w_mm: 10.0,
                    h_mm: 4.0,
                }),
                rotation: 0,
                locked: false,
                halign: None,
                valign: None,
                hidden: false,
                title: None,
            }],
            ..Label::default()
        };
        let bitmap = render_label(&label, model, geometry).unwrap();
        let lines = ink_lines(&bitmap);
        assert!(*lines.first().unwrap() >= mm_to_dots(20.0));
        assert!(*lines.last().unwrap() < mm_to_dots(30.0));
        assert_eq!(
            bitmap.height_dots(),
            mm_to_dots(30.0),
            "auto length = box end"
        );

        // Bars span exactly the box's pins (y 2..6 mm), not the whole tape.
        let y0 = geometry.left_offset_pins as u32 + mm_to_dots(2.0);
        let line = *lines.first().unwrap();
        let inked: Vec<u32> = (0..model.head_pins as u32)
            .filter(|&p| bitmap.pixel(p as u16, line))
            .collect();
        assert_eq!(inked.first(), Some(&y0));
        assert_eq!(inked.len() as u32, mm_to_dots(4.0));
    }

    #[test]
    fn boxes_are_clipped_to_the_printable_area() {
        let model = p710();
        let geometry = geometry_for(model, 9).unwrap();
        let label = Label {
            elements: vec![Item {
                element: Element::Qr { data: "A".into() },
                rect: Some(Rect {
                    x_mm: 0.0,
                    y_mm: -5.0,
                    w_mm: 20.0,
                    h_mm: 20.0,
                }),
                rotation: 0,
                locked: false,
                halign: None,
                valign: None,
                hidden: false,
                title: None,
            }],
            ..Label::default()
        };
        let bitmap = render_label(&label, model, geometry).unwrap();
        let printable =
            geometry.left_offset_pins..geometry.left_offset_pins + geometry.printable_pins;
        for line in 0..bitmap.height_dots() {
            for pin in 0..model.head_pins {
                if bitmap.pixel(pin, line) {
                    assert!(printable.contains(&pin), "ink at pin {pin}");
                }
            }
        }
    }

    #[test]
    fn resolved_rects_match_flow_layout() {
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let label = Label {
            elements: vec![
                Element::Qr { data: "A".into() }.into(),
                Element::Qr { data: "B".into() }.into(),
            ],
            gap_mm: 3.0,
            padding_mm: 1.0,
            ..Label::default()
        };
        let rects = resolved_rects(&label, model, geometry).unwrap();
        let tape_mm = dots_to_mm(geometry.printable_pins as u32);
        assert!((rects[0].x_mm - 1.0).abs() < 0.1);
        assert!((rects[0].h_mm - tape_mm).abs() < 0.01);
        assert!((rects[1].x_mm - (rects[0].x_mm + rects[0].w_mm + 3.0)).abs() < 0.1);

        // Turning them into boxes keeps the rendering identical.
        let before = render_label(&label, model, geometry).unwrap();
        let mut boxed = label.clone();
        for (item, rect) in boxed.elements.iter_mut().zip(&rects) {
            item.rect = Some(*rect);
        }
        let after = render_label(&boxed, model, geometry).unwrap();
        assert_eq!(ink_lines(&before), ink_lines(&after));
    }

    #[test]
    fn sized_multiline_text_renders_in_flow() {
        if !has_font() {
            return;
        }
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let label = Label::single(Element::Text {
            text: "A\nB".into(),
            size_pt: Some(8.0),
            align: TextAlign::Left,
            font: None,
            bold: true,
            italic: true,
            line_spacing: None,
        });
        let bitmap = render_label(&label, model, geometry).unwrap();
        assert!(!ink_lines(&bitmap).is_empty());
    }

    #[test]
    fn multi_tape_label_is_printed_as_slices() {
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let tape_mm = dots_to_mm(geometry.printable_pins as u32);
        // A bar spanning both strips' full height.
        let label = Label {
            strips: 2,
            elements: vec![Item {
                element: Element::Barcode {
                    symbology: Symbology::Code128,
                    data: "A".into(),
                },
                rect: Some(Rect {
                    x_mm: 0.0,
                    y_mm: 0.0,
                    w_mm: 10.0,
                    h_mm: 2.0 * tape_mm,
                }),
                rotation: 0,
                locked: false,
                halign: None,
                valign: None,
                hidden: false,
                title: None,
            }],
            ..Label::default()
        };
        let design = render_label(&label, model, geometry).unwrap();
        assert_eq!(design.width_pins(), 2 * geometry.printable_pins);

        let pages = render_label_pages(&label, model, geometry).unwrap();
        assert_eq!(pages.len(), 2);
        let line = ink_lines(&pages[0])[0];
        for page in &pages {
            assert_eq!(page.width_pins(), model.head_pins);
            // Every printable pin inked, nothing outside the tape.
            for pin in 0..model.head_pins {
                let printable = (geometry.left_offset_pins
                    ..geometry.left_offset_pins + geometry.printable_pins)
                    .contains(&pin);
                assert_eq!(page.pixel(pin, line), printable, "pin {pin}");
            }
        }
        let png = render_label_png(&label, model, 12).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.height(), 2 * geometry.printable_pins as u32);
    }

    #[test]
    fn hires_preview_scales_the_same_layout() {
        let model = p710();
        let label = Label {
            elements: vec![Item {
                element: Element::Qr { data: "A".into() },
                rect: Some(Rect {
                    x_mm: 2.0,
                    y_mm: 0.0,
                    w_mm: 9.0,
                    h_mm: 9.0,
                }),
                rotation: 0,
                locked: false,
                halign: None,
                valign: None,
                hidden: false,
                title: None,
            }],
            padding_mm: 1.0,
            ..Label::default()
        };
        let normal =
            image::load_from_memory(&render_label_preview(&label, model, 12, 1).unwrap().png)
                .unwrap();
        let hires =
            image::load_from_memory(&render_label_preview(&label, model, 12, 4).unwrap().png)
                .unwrap();
        assert_eq!(hires.height(), 4 * normal.height());
        assert!((hires.width() as i64 - 4 * normal.width() as i64).abs() <= 4);
    }

    #[test]
    fn legacy_frame_draws_two_dot_solid_border() {
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let label = Label {
            frame: true,
            min_length_mm: Some(20.0),
            ..Label::default()
        };
        let canvas = Canvas::new(&label, model, geometry, 1);
        let bitmap = render_label(&label, model, geometry).unwrap();
        let mid = bitmap.height_dots() / 2;
        let top = canvas.offset;
        assert!(bitmap.pixel(top, mid) && bitmap.pixel(top + 1, mid));
        assert!(!bitmap.pixel(top + 2, mid));
    }

    #[test]
    fn styled_border_top_only_round_trips_and_renders() {
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let label = Label {
            border: Some(LabelBorder {
                style: BorderStyle::Striped,
                width_mm: 1.0,
                sides: BorderSides {
                    top: true,
                    bottom: false,
                    left: false,
                    right: false,
                },
                ..LabelBorder::default()
            }),
            min_length_mm: Some(20.0),
            ..Label::default()
        };
        let back = Label::from_json(&label.to_json().unwrap()).unwrap();
        assert_eq!(back, label);
        assert!(label.to_json().unwrap().contains(r#""style": "striped""#));

        let canvas = Canvas::new(&label, model, geometry, 1);
        let bitmap = render_label(&label, model, geometry).unwrap();
        let (top, bottom) = (canvas.offset, canvas.offset + canvas.pins - 1);
        let ink = |pin| {
            (0..bitmap.height_dots())
                .filter(|&l| bitmap.pixel(pin, l))
                .count()
        };
        assert!(ink(top) > 0, "striped top border");
        assert!(
            ink(top) < bitmap.height_dots() as usize,
            "stripes have gaps"
        );
        assert_eq!(ink(bottom), 0, "no bottom border");
    }

    #[test]
    fn fixed_length_cuts_longer_content() {
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let bar = |x_mm| Item {
            element: Element::Fill,
            rect: Some(Rect {
                x_mm,
                y_mm: 0.0,
                w_mm: 10.0,
                h_mm: 5.0,
            }),
            rotation: 0,
            locked: false,
            halign: None,
            valign: None,
            hidden: false,
            title: None,
        };
        let mut label = Label {
            elements: vec![bar(0.0), bar(40.0)],
            min_length_mm: Some(20.0),
            ..Label::default()
        };
        let grown = render_label(&label, model, geometry).unwrap();
        assert!(grown.height_dots() > mm_to_dots(45.0));
        label.fixed_length = true;
        let fixed = render_label(&label, model, geometry).unwrap();
        assert_eq!(fixed.height_dots(), mm_to_dots(20.0));
    }

    #[test]
    fn start_and_end_margins_frame_flow_content() {
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let ink = |label: &Label| {
            let b = render_label(label, model, geometry).unwrap();
            let lines = ink_lines(&b);
            (lines[0], b.height_dots() - 1 - lines[lines.len() - 1])
        };
        let mut label = Label {
            elements: vec![Element::Qr { data: "x".into() }.into()],
            padding_mm: 2.0,
            ..Label::default()
        };
        // The QR code's quiet zone adds the same blank on both runs.
        let (start2, end2) = ink(&label);
        label.padding_start_mm = Some(6.0);
        let (start6, end6) = ink(&label);
        assert!((start6 - start2).abs_diff(mm_to_dots(4.0)) <= 1);
        assert_eq!(end6, end2);
    }

    #[test]
    fn border_sits_inside_the_label_margins() {
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let label = Label {
            padding_mm: 3.0,
            padding_start_mm: Some(5.0),
            min_length_mm: Some(40.0),
            fixed_length: true,
            border: Some(LabelBorder {
                insets_mm: Some(BorderInsets {
                    left: 1.0,
                    right: 2.0,
                    ..BorderInsets::default()
                }),
                ..LabelBorder::default()
            }),
            ..Label::default()
        };
        let b = render_label(&label, model, geometry).unwrap();
        let lines = ink_lines(&b);
        let (first, last) = (lines[0], lines[lines.len() - 1]);
        assert!(first.abs_diff(mm_to_dots(6.0)) <= 1, "first {first}");
        assert!(
            (b.height_dots() - 1 - last).abs_diff(mm_to_dots(5.0)) <= 1,
            "end gap {}",
            b.height_dots() - 1 - last
        );
    }

    #[test]
    fn hidden_elements_are_not_rendered() {
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let mut bar: Item = Element::Fill.into();
        bar.rect = Some(Rect {
            x_mm: 1.0,
            y_mm: 0.0,
            w_mm: 10.0,
            h_mm: 5.0,
        });
        let mut label = Label {
            elements: vec![bar],
            ..Label::default()
        };
        assert!(!ink_lines(&render_label(&label, model, geometry).unwrap()).is_empty());
        label.elements[0].hidden = true;
        assert!(ink_lines(&render_label(&label, model, geometry).unwrap()).is_empty());
    }

    #[test]
    fn portrait_prints_like_its_landscape_form() {
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let tape = dots_to_mm(geometry.printable_pins as u32);
        let bar = Item {
            rect: Some(Rect {
                x_mm: 1.0,
                y_mm: 2.0,
                w_mm: 4.0,
                h_mm: 20.0,
            }),
            ..Element::Fill.into()
        };
        let portrait = Label {
            elements: vec![bar.clone()],
            orientation: Orientation::Portrait,
            ..Label::default()
        };
        let mut landscape = portrait.to_landscape(tape);
        assert_eq!(landscape.orientation, Orientation::Landscape);
        let r = landscape.elements[0].rect.unwrap();
        assert_eq!((r.x_mm, r.w_mm, r.h_mm), (2.0, 20.0, 4.0));
        assert!((r.y_mm - (tape - 5.0)).abs() < 1e-4);
        assert_eq!(
            render_label(&portrait, model, geometry).unwrap(),
            render_label(&landscape, model, geometry).unwrap()
        );
        // Round trip of the box mapping.
        landscape.elements[0].rect = Some(r.landscape_to_portrait(tape));
        assert_eq!(landscape.elements[0].rect, bar.rect);
    }

    #[test]
    fn decor_frame_sits_between_the_margins() {
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let label = Label {
            padding_mm: 3.0,
            min_length_mm: Some(40.0),
            fixed_length: true,
            decor: Some("basis:rounded".into()),
            ..Label::default()
        };
        let b = render_label(&label, model, geometry).unwrap();
        let lines = ink_lines(&b);
        assert!(
            lines[0].abs_diff(mm_to_dots(3.0)) <= 2,
            "first {}",
            lines[0]
        );
        let end_gap = b.height_dots() - 1 - lines[lines.len() - 1];
        assert!(end_gap.abs_diff(mm_to_dots(3.0)) <= 2, "end gap {end_gap}");
        // Unknown frames are ignored instead of failing the print.
        let unknown = Label {
            decor: Some("nope:nope".into()),
            ..label
        };
        assert!(ink_lines(&render_label(&unknown, model, geometry).unwrap()).is_empty());
    }

    #[test]
    fn fuse_box_renders_fields_and_separators() {
        if !has_font() {
            return;
        }
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let fuse = |separator, vertical| Element::FuseBox {
            fields: vec![
                FuseField::new("HAUPT", 2.0),
                FuseField::new("F1", 1.0),
                FuseField::new("F2", 1.0),
            ],
            pitch_mm: 10.0,
            separator,
            vertical,
            reverse: false,
            fixed: false,
            size_pt: None,
            align: TextAlign::Center,
            line_spacing: None,
            font: None,
            bold: false,
            italic: false,
        };
        // Flow layout: natural length = 4 modules of 10 mm.
        let flow = render_label(
            &Label {
                padding_mm: 0.0,
                ..Label::single(fuse(FuseSeparator::Frame, false))
            },
            model,
            geometry,
        )
        .unwrap();
        let len = flow.height_dots();
        assert!(
            (len as f32 - mm_to_dots(40.0) as f32).abs() <= 2.0,
            "length {len}"
        );
        // Frame: separator lines span the printable height at the field edges.
        let pins = geometry.left_offset_pins..geometry.left_offset_pins + geometry.printable_pins;
        let full = |line: u32| pins.clone().all(|p| flow.pixel(p, line));
        let edge = mm_to_dots(20.0);
        assert!((edge - 1..=edge + 1).any(full), "no separator at 20 mm");
        // Without separators only text ink remains; vertical text renders too.
        let plain = render_label(
            &Label {
                padding_mm: 0.0,
                ..Label::single(fuse(FuseSeparator::None, true))
            },
            model,
            geometry,
        )
        .unwrap();
        assert!(!(0..plain.height_dots()).any(|l| pins.clone().all(|p| plain.pixel(p, l))));
        assert!(!ink_lines(&plain).is_empty());
        let json = Label::single(fuse(FuseSeparator::Marks, false))
            .to_json()
            .unwrap();
        assert!(json.contains(r#""type": "fuse_box""#) && json.contains(r#""separator": "marks""#));
    }

    #[test]
    fn shapes_render_in_boxes_and_flow() {
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let shape = |filled| Element::Shape {
            shape: ShapeKind::Ellipse,
            stroke_mm: 0.5,
            filled,
        };
        let flow = render_label(&Label::single(shape(false)), model, geometry).unwrap();
        assert!(!ink_lines(&flow).is_empty());
        let json = Label::single(shape(true)).to_json().unwrap();
        assert!(json.contains(r#""type": "shape""#) && json.contains(r#""shape": "ellipse""#));
    }

    #[test]
    fn preview_reports_overflowing_text() {
        if !has_font() {
            return;
        }
        let model = p710();
        let boxed = |size_pt| Item {
            element: Element::Text {
                text: "Hallo".into(),
                size_pt,
                align: TextAlign::default(),
                font: None,
                bold: false,
                italic: false,
                line_spacing: None,
            },
            rect: Some(Rect {
                x_mm: 0.0,
                y_mm: 0.0,
                w_mm: 20.0,
                h_mm: 6.0,
            }),
            rotation: 0,
            locked: false,
            halign: None,
            valign: None,
            hidden: false,
            title: None,
        };
        let label = Label {
            elements: vec![boxed(None), boxed(Some(30.0))],
            ..Label::default()
        };
        let preview = render_label_preview(&label, model, 12, 1).unwrap();
        assert_eq!(preview.overflowing, [1]);
    }

    #[test]
    fn rotated_box_keeps_its_footprint() {
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let item = |rotation| Item {
            element: Element::Barcode {
                symbology: Symbology::Code128,
                data: "AB".into(),
            },
            rect: Some(Rect {
                x_mm: 0.0,
                y_mm: 0.0,
                w_mm: 8.0,
                h_mm: 9.0,
            }),
            rotation,
            locked: false,
            halign: None,
            valign: None,
            hidden: false,
            title: None,
        };
        for rotation in [0, 90, 180, 270] {
            let label = Label {
                elements: vec![item(rotation)],
                ..Label::default()
            };
            let bitmap = render_label(&label, model, geometry).unwrap();
            let lines = ink_lines(&bitmap);
            assert!(
                *lines.last().unwrap() < mm_to_dots(8.0),
                "rotation {rotation}"
            );
        }
        // 90°: bars now run along the tape -> one ink line has many more pins
        // inked than at 0° on the edge rows... simply check it differs.
        let a = render_label(
            &Label {
                elements: vec![item(0)],
                ..Label::default()
            },
            model,
            geometry,
        )
        .unwrap();
        let b = render_label(
            &Label {
                elements: vec![item(90)],
                ..Label::default()
            },
            model,
            geometry,
        )
        .unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn fill_element_is_solid() {
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let label = Label {
            elements: vec![Item {
                element: Element::Fill,
                rect: Some(Rect {
                    x_mm: 1.0,
                    y_mm: 0.0,
                    w_mm: 0.5,
                    h_mm: 9.0,
                }),
                rotation: 0,
                locked: false,
                halign: None,
                valign: None,
                hidden: false,
                title: None,
            }],
            ..Label::default()
        };
        let bitmap = render_label(&label, model, geometry).unwrap();
        let line = mm_to_dots(1.0) + 1;
        let inked = (0..model.head_pins)
            .filter(|&p| bitmap.pixel(p, line))
            .count();
        assert_eq!(
            inked as u32,
            mm_to_dots(9.0).min(geometry.printable_pins as u32)
        );
    }

    #[test]
    fn png_preview_has_tape_height() {
        let label = Label::single(Element::Qr { data: "A".into() });
        let png = render_label_png(&label, p710(), 12).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.height(), 70);
    }
}
