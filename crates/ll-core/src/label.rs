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

use ll_protocol::model::{dots_to_mm, mm_to_dots, pt_to_dots, ModelInfo, TapeGeometry};
use ll_render::{boxed, Bitmap, Face, QrErrorCorrection, Symbology, TextAlign};
use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Current `.llabel` format version, written by [`Label::to_json`].
/// Version 1 (no boxes, no text size/alignment) is read unchanged.
pub const LABEL_FORMAT_VERSION: u32 = 2;

/// Border thickness in print dots when [`Label::frame`] is set.
pub const BORDER_THICKNESS: u16 = 2;

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
    /// Blank space before the first flow element and after the content's
    /// end, in mm.
    #[serde(default)]
    pub padding_mm: f32,
    /// Minimum label length in mm. Flow-only labels are centered in it;
    /// with boxes it's a fixed minimum (boxes keep their positions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length_mm: Option<f32>,
    /// Draw a border around the whole label.
    #[serde(default)]
    pub frame: bool,
    /// Number of tape strips stacked on top of each other (multi-tape
    /// label): the design is `strips` x the printable height tall and is
    /// printed as one strip per slice, top first.
    #[serde(default = "one_strip", skip_serializing_if = "is_one_strip")]
    pub strips: u8,
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
            min_length_mm: None,
            frame: false,
            strips: 1,
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
}

impl From<Element> for Item {
    fn from(element: Element) -> Self {
        Self {
            element,
            rect: None,
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
    },
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
        }
    }
}

impl Label {
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
#[derive(Default)]
struct FontCache {
    default_bytes: Option<Vec<u8>>,
    faces: HashMap<(Option<String>, bool, bool), Face>,
}

impl FontCache {
    fn default_bytes(&mut self) -> Result<&[u8], CoreError> {
        if self.default_bytes.is_none() {
            self.default_bytes = Some(ll_render::fontsrc::load_default_font()?);
        }
        Ok(self.default_bytes.as_deref().unwrap_or_default())
    }

    fn face(
        &mut self,
        family: &Option<String>,
        bold: bool,
        italic: bool,
    ) -> Result<&Face, CoreError> {
        let key = (family.clone(), bold, italic);
        if !self.faces.contains_key(&key) {
            let face = Face::load(family.as_deref(), bold, italic)?;
            self.faces.insert(key.clone(), face);
        }
        self.faces
            .get(&key)
            .ok_or(CoreError::Template("font cache".into()))
    }
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
fn render_flow_element(
    element: &Element,
    canvas: &Canvas,
    fonts: &mut FontCache,
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
        } if !text.contains('\n') => {
            ll_render::render_text_with_font(text, fonts.default_bytes()?, head, pins, offset)?
        }
        Element::Text {
            text,
            size_pt,
            align,
            font,
            bold,
            italic,
        } => {
            let font = fonts.face(font, *bold, *italic)?;
            let size_px = size_pt.map(|pt| canvas.pt(pt));
            let width = boxed::text_natural_width(text, font, pins, size_px)?;
            let local = boxed::text_in_box(text, font, width, pins, size_px, *align)?;
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
        Element::Image { path, invert } => {
            ll_render::render_image(path, head, pins, offset, *invert)?
        }
    })
}

/// Renders one element into a box-local bitmap of `w` x `h` dots.
fn render_boxed_element(
    element: &Element,
    w: u32,
    h: u16,
    canvas: &Canvas,
    fonts: &mut FontCache,
) -> Result<Bitmap, CoreError> {
    Ok(match element {
        Element::Text {
            text,
            size_pt,
            align,
            font,
            bold,
            italic,
        } => boxed::text_in_box(
            text,
            fonts.face(font, *bold, *italic)?,
            w,
            h,
            size_pt.map(|pt| canvas.pt(pt)),
            *align,
        )?,
        Element::Qr { data } => boxed::qr_in_box(data, w, h, QrErrorCorrection::Medium)?,
        Element::Barcode { symbology, data } => boxed::barcode_in_box(*symbology, data, w, h)?,
        Element::Image { path, invert } => boxed::image_in_box(path, w, h, *invert)?,
    })
}

/// The rendered label plus, per element, its resolved box in canvas dots
/// `(x, y, w, h)` (flow elements included).
struct Composed {
    bitmap: Bitmap,
    boxes: Vec<(i32, i32, u32, u32)>,
}

fn compose(label: &Label, canvas: &Canvas) -> Result<Composed, CoreError> {
    let mut fonts = FontCache::default();
    if label.has_text() {
        fonts.default_bytes()?; // fail early with a clear "no font" error
    }
    let pins = canvas.pins;
    let gap = canvas.mm(label.gap_mm);
    let padding = canvas.mm(label.padding_mm);

    // Flow pass: elements without a box, one after another.
    let mut bitmap = Bitmap::new(canvas.head_pins, 0);
    let mut boxes: Vec<Option<(i32, i32, u32, u32)>> = vec![None; label.elements.len()];
    bitmap.extend_blank(padding);
    let mut first = true;
    for (i, item) in label.elements.iter().enumerate() {
        if item.rect.is_some() {
            continue;
        }
        if !first {
            bitmap.extend_blank(gap);
        }
        first = false;
        let start = bitmap.height_dots();
        // Every flow element renders at `canvas.head_pins` wide, so this
        // can't mismatch; checked anyway rather than dropping content.
        if !bitmap.append(&render_flow_element(&item.element, canvas, &mut fonts)?) {
            return Err(CoreError::Template("element width mismatch".into()));
        }
        boxes[i] = Some((start as i32, 0, bitmap.height_dots() - start, pins as u32));
    }
    bitmap.extend_blank(padding);

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
            .map(|(x, _, w, _)| (x + w as i32).max(0) as u32 + padding)
            .max()
            .unwrap_or(0);
        let length = content_end.max(min_len);
        bitmap.extend_blank(length.saturating_sub(bitmap.height_dots()));

        let clip = canvas.offset..canvas.offset + pins;
        for (i, item) in label.elements.iter().enumerate() {
            let Some(rect) = &item.rect else { continue };
            let (x, y, w, h) = canvas.rect(rect);
            if w > 0 && h > 0 {
                let local = render_boxed_element(&item.element, w, h, canvas, &mut fonts)?;
                bitmap.blit(&local, canvas.offset as i32 + y, x, clip.clone());
            }
            boxes[i] = Some((x, y, w, h as u32));
        }
    }

    if label.frame {
        let thickness = (BORDER_THICKNESS as u32 * canvas.scale).min(u16::MAX as u32) as u16;
        ll_render::draw_border(&mut bitmap, canvas.offset, pins, thickness);
    }
    Ok(Composed {
        bitmap,
        boxes: boxes.into_iter().map(Option::unwrap_or_default).collect(),
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
    let composed = compose(label, &Canvas::new(label, model, geometry, 1))?;
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
        .map(|(x, y, w, h)| Rect {
            x_mm: mm(x),
            y_mm: mm(y),
            w_mm: dots_to_mm(w),
            h_mm: dots_to_mm(h),
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

/// GUI preview: the printable area (all strips) as a transparent PNG mask
/// (ink opaque, background clear) at `scale` x the print resolution.
/// `scale = 1` is the exact print raster; higher values render the same
/// layout through the same code at finer resolution for a smoother view.
pub fn render_label_preview(
    label: &Label,
    model: &ModelInfo,
    width_mm: u8,
    scale: u32,
) -> Result<Vec<u8>, CoreError> {
    let geometry = geometry_for(model, width_mm)?;
    let canvas = Canvas::new(label, model, geometry, scale);
    let bitmap = compose(label, &canvas)?.bitmap;
    Ok(ll_render::png::to_png_mask(
        &bitmap,
        canvas.offset,
        canvas.pins,
    )?)
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
                    },
                    rect: Some(Rect {
                        x_mm: 1.0,
                        y_mm: 0.5,
                        w_mm: 30.0,
                        h_mm: 8.0,
                    }),
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
        });
        label.resolve_paths(Path::new("/tmp/labels"));
        assert_eq!(
            label.elements[0].element,
            Element::Image {
                path: "/tmp/labels/a.png".into(),
                invert: false
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
            }],
            padding_mm: 1.0,
            ..Label::default()
        };
        let normal =
            image::load_from_memory(&render_label_preview(&label, model, 12, 1).unwrap()).unwrap();
        let hires =
            image::load_from_memory(&render_label_preview(&label, model, 12, 4).unwrap()).unwrap();
        assert_eq!(hires.height(), 4 * normal.height());
        assert!((hires.width() as i64 - 4 * normal.width() as i64).abs() <= 4);
    }

    #[test]
    fn png_preview_has_tape_height() {
        let label = Label::single(Element::Qr { data: "A".into() });
        let png = render_label_png(&label, p710(), 12).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.height(), 70);
    }
}
