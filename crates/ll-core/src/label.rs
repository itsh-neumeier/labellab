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

use std::path::{Path, PathBuf};

use ll_protocol::model::{dots_to_mm, mm_to_dots, pt_to_dots, ModelInfo, TapeGeometry};
use ll_render::{boxed, Bitmap, QrErrorCorrection, Symbology, TextAlign};
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

/// Lazily loaded default system font (only needed if there's text).
struct FontCache(Option<Vec<u8>>);

impl FontCache {
    fn get(&mut self) -> Result<&[u8], CoreError> {
        if self.0.is_none() {
            self.0 = Some(ll_render::fontsrc::load_default_font()?);
        }
        Ok(self.0.as_deref().unwrap_or_default())
    }
}

/// Renders one element for the flow layout: full head width, filling the
/// tape's printable height, natural length.
fn render_flow_element(
    element: &Element,
    model: &ModelInfo,
    geometry: &TapeGeometry,
    fonts: &mut FontCache,
) -> Result<Bitmap, CoreError> {
    let (head, pins, offset) = (
        model.head_pins,
        geometry.printable_pins,
        geometry.left_offset_pins,
    );
    Ok(match element {
        Element::Text {
            text,
            size_pt: None,
            ..
        } if !text.contains('\n') => {
            ll_render::render_text_with_font(text, fonts.get()?, head, pins, offset)?
        }
        Element::Text {
            text,
            size_pt,
            align,
        } => {
            let font = fonts.get()?;
            let size_px = size_pt.map(pt_to_dots);
            let width = boxed::text_natural_width(text, font, pins, size_px)?;
            let local = boxed::text_in_box(text, font, width, pins, size_px, *align)?;
            let mut out = Bitmap::new(head, width);
            out.blit(&local, offset as i32, 0, offset..offset + pins);
            out
        }
        Element::Qr { data } => {
            ll_render::render_qr(data, head, pins, offset, QrErrorCorrection::Medium)?
        }
        Element::Barcode { symbology, data } => {
            ll_render::render_barcode(*symbology, data, head, pins, offset)?
        }
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
    fonts: &mut FontCache,
) -> Result<Bitmap, CoreError> {
    Ok(match element {
        Element::Text {
            text,
            size_pt,
            align,
        } => boxed::text_in_box(text, fonts.get()?, w, h, size_pt.map(pt_to_dots), *align)?,
        Element::Qr { data } => boxed::qr_in_box(data, w, h, QrErrorCorrection::Medium)?,
        Element::Barcode { symbology, data } => boxed::barcode_in_box(*symbology, data, w, h)?,
        Element::Image { path, invert } => boxed::image_in_box(path, w, h, *invert)?,
    })
}

/// A box in print dots: (x, y, w, h).
fn rect_dots(rect: &Rect) -> (i32, i32, u32, u16) {
    let signed = |mm: f32| {
        let d = mm_to_dots(mm.abs()) as i32;
        if mm < 0.0 {
            -d
        } else {
            d
        }
    };
    (
        signed(rect.x_mm),
        signed(rect.y_mm),
        mm_to_dots(rect.w_mm),
        mm_to_dots(rect.h_mm).min(u16::MAX as u32) as u16,
    )
}

/// The rendered label plus, per element, its resolved box in dots
/// `(x, y, w, h)` (flow elements included).
struct Composed {
    bitmap: Bitmap,
    boxes: Vec<(i32, i32, u32, u32)>,
}

fn compose(
    label: &Label,
    model: &ModelInfo,
    geometry: &TapeGeometry,
) -> Result<Composed, CoreError> {
    let mut fonts = FontCache(None);
    if label.has_text() {
        fonts.get()?; // fail early with a clear "no font" error
    }
    let pins = geometry.printable_pins;
    let gap = mm_to_dots(label.gap_mm);
    let padding = mm_to_dots(label.padding_mm);

    // Flow pass: elements without a box, one after another.
    let mut bitmap = Bitmap::new(model.head_pins, 0);
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
        // Every flow element renders at `model.head_pins` wide, so this
        // can't mismatch; checked anyway rather than dropping content.
        if !bitmap.append(&render_flow_element(
            &item.element,
            model,
            geometry,
            &mut fonts,
        )?) {
            return Err(CoreError::Template("element width mismatch".into()));
        }
        boxes[i] = Some((start as i32, 0, bitmap.height_dots() - start, pins as u32));
    }
    bitmap.extend_blank(padding);

    let has_boxes = label.elements.iter().any(|i| i.rect.is_some());
    let min_len = label.min_length_mm.map(mm_to_dots).unwrap_or(0);
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
            .filter_map(|i| i.rect.as_ref().map(rect_dots))
            .map(|(x, _, w, _)| (x + w as i32).max(0) as u32 + padding)
            .max()
            .unwrap_or(0);
        let length = content_end.max(min_len);
        bitmap.extend_blank(length.saturating_sub(bitmap.height_dots()));

        let clip = geometry.left_offset_pins..geometry.left_offset_pins + pins;
        for (i, item) in label.elements.iter().enumerate() {
            let Some(rect) = &item.rect else { continue };
            let (x, y, w, h) = rect_dots(rect);
            if w > 0 && h > 0 {
                let local = render_boxed_element(&item.element, w, h, &mut fonts)?;
                bitmap.blit(
                    &local,
                    geometry.left_offset_pins as i32 + y,
                    x,
                    clip.clone(),
                );
            }
            boxes[i] = Some((x, y, w, h as u32));
        }
    }

    if label.frame {
        ll_render::draw_border(
            &mut bitmap,
            geometry.left_offset_pins,
            pins,
            BORDER_THICKNESS,
        );
    }
    Ok(Composed {
        bitmap,
        boxes: boxes.into_iter().map(Option::unwrap_or_default).collect(),
    })
}

/// Renders `label` for `model` with the tape described by `geometry`: the
/// single bitmap shared by preview and print.
pub fn render_label(
    label: &Label,
    model: &ModelInfo,
    geometry: &TapeGeometry,
) -> Result<Bitmap, CoreError> {
    Ok(compose(label, model, geometry)?.bitmap)
}

/// Every element's box in mm as rendered — flow elements get the box the
/// flow layout gave them. The GUI uses this to turn flow elements into
/// freely movable boxes without changing how the label looks.
pub fn resolved_rects(
    label: &Label,
    model: &ModelInfo,
    geometry: &TapeGeometry,
) -> Result<Vec<Rect>, CoreError> {
    let composed = compose(label, model, geometry)?;
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
/// as PNG (live preview in the GUI, `labellab render`).
pub fn render_label_png(
    label: &Label,
    model: &ModelInfo,
    width_mm: u8,
) -> Result<Vec<u8>, CoreError> {
    let geometry = geometry_for(model, width_mm)?;
    let bitmap = render_label(label, model, geometry)?;
    Ok(ll_render::png::to_png(
        &bitmap,
        geometry.left_offset_pins,
        geometry.printable_pins,
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
        });
        let bitmap = render_label(&label, model, geometry).unwrap();
        assert!(!ink_lines(&bitmap).is_empty());
    }

    #[test]
    fn png_preview_has_tape_height() {
        let label = Label::single(Element::Qr { data: "A".into() });
        let png = render_label_png(&label, p710(), 12).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.height(), 70);
    }
}
