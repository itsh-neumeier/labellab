//! Label layout model and the `.llabel` template format (JSON).
//!
//! A label is a row of elements laid out one after another along the tape
//! (feed direction), each filling the tape's printable height. This is the
//! model the GUI editor (M6) edits and the CLI's `--template` loads.
//! [`render_label`] turns it into the one [`Bitmap`] that both the preview
//! ([`render_label_png`]) and the print job (`print::print_label`) use
//! (`AGENTS.md`: "Vorschau und Druck nutzen denselben Renderpfad").
//!
//! Image paths in a template are stored as written; relative ones are
//! resolved against the template's directory by [`Label::load`].

use std::path::{Path, PathBuf};

use ll_protocol::model::{mm_to_dots, ModelInfo, TapeGeometry};
use ll_render::{Bitmap, QrErrorCorrection, Symbology};
use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Current `.llabel` format version, written by [`Label::to_json`].
pub const LABEL_FORMAT_VERSION: u32 = 1;

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
    /// Elements, laid out left to right along the tape.
    #[serde(default)]
    pub elements: Vec<Element>,
    /// Blank space between two elements, in mm.
    #[serde(default = "default_gap_mm")]
    pub gap_mm: f32,
    /// Blank space before the first and after the last element, in mm.
    #[serde(default)]
    pub padding_mm: f32,
    /// Fixed minimum label length in mm; shorter content is centered.
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

/// One element of a [`Label`]. Serialized with a `"type"` tag, e.g.
/// `{"type": "text", "text": "Hallo"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Element {
    Text {
        text: String,
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

impl Label {
    /// A label with just `element` and default spacing.
    pub fn single(element: Element) -> Self {
        Self {
            elements: vec![element],
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

    /// Serializes as pretty-printed `.llabel` JSON.
    pub fn to_json(&self) -> Result<String, CoreError> {
        serde_json::to_string_pretty(self).map_err(|e| CoreError::Template(e.to_string()))
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
        for element in &mut self.elements {
            if let Element::Image { path, .. } = element {
                if path.is_relative() {
                    *path = base.join(&*path);
                }
            }
        }
    }
}

/// Renders one element to fill the tape's printable height.
fn render_element(
    element: &Element,
    model: &ModelInfo,
    geometry: &TapeGeometry,
) -> Result<Bitmap, CoreError> {
    let (head, pins, offset) = (
        model.head_pins,
        geometry.printable_pins,
        geometry.left_offset_pins,
    );
    Ok(match element {
        Element::Text { text } => ll_render::render_text(text, head, pins, offset)?,
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

/// Renders `label` for `model` with the tape described by `geometry`: the
/// single bitmap shared by preview and print.
pub fn render_label(
    label: &Label,
    model: &ModelInfo,
    geometry: &TapeGeometry,
) -> Result<Bitmap, CoreError> {
    let gap = mm_to_dots(label.gap_mm);
    let padding = mm_to_dots(label.padding_mm);

    let mut bitmap = Bitmap::new(model.head_pins, 0);
    bitmap.extend_blank(padding);
    for (i, element) in label.elements.iter().enumerate() {
        if i > 0 {
            bitmap.extend_blank(gap);
        }
        // Every element renders at `model.head_pins` wide, so this can't
        // mismatch; checked anyway rather than silently dropping content.
        if !bitmap.append(&render_element(element, model, geometry)?) {
            return Err(CoreError::Template("element width mismatch".into()));
        }
    }
    bitmap.extend_blank(padding);

    if let Some(min_mm) = label.min_length_mm {
        let missing = mm_to_dots(min_mm).saturating_sub(bitmap.height_dots());
        bitmap.prepend_blank(missing / 2);
        bitmap.extend_blank(missing - missing / 2);
    }

    if label.frame {
        ll_render::draw_border(
            &mut bitmap,
            geometry.left_offset_pins,
            geometry.printable_pins,
            BORDER_THICKNESS,
        );
    }
    Ok(bitmap)
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

    #[test]
    fn json_round_trip() {
        let label = Label {
            elements: vec![
                Element::Text {
                    text: "Server 1".into(),
                },
                Element::Qr {
                    data: "https://example.org".into(),
                },
                Element::Barcode {
                    symbology: Symbology::Ean13,
                    data: "400638133393".into(),
                },
                Element::Image {
                    path: "icon.svg".into(),
                    invert: false,
                },
            ],
            frame: true,
            min_length_mm: Some(40.0),
            ..Label::default()
        };
        let json = label.to_json().unwrap();
        assert!(json.contains(r#""type": "barcode""#));
        assert!(json.contains(r#""symbology": "ean13""#));
        assert_eq!(Label::from_json(&json).unwrap(), label);
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
            label.elements[0],
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
            elements: vec![qr.clone(), qr],
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
    fn png_preview_has_tape_height() {
        let label = Label::single(Element::Qr { data: "A".into() });
        let png = render_label_png(&label, p710(), 12).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.height(), 70);
    }
}
