//! Generators for typical cable and network labels (M7). Each returns an
//! ordinary [`Label`] with boxes, so the result can be edited further in
//! the GUI, saved as `.llabel`, used with CSV/numbering and printed through
//! the normal path.
//!
//! Lengths are in mm. `tape_mm` is the printable height of the loaded tape
//! (see `ll_protocol::model::TapeGeometry`), i.e. the box height.

use std::f32::consts::PI;

use serde::{Deserialize, Serialize};

use crate::label::{Element, Item, Label, Rect};

/// Width of separator lines in patch panel labels, in mm (about 2 dots).
const SEPARATOR_MM: f32 = 0.3;

/// Cable flag: the same text twice, separated by the part that wraps
/// around the cable (π × diameter), so the flag reads from both sides
/// once the two ends are stuck together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CableFlag {
    pub text: String,
    /// Cable diameter in mm.
    pub diameter_mm: f32,
    /// Length of each flag end in mm.
    pub flag_mm: f32,
}

/// Cable wrap: text repeated along the whole circumference so it can be
/// read from every side once wrapped around the cable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CableWrap {
    pub text: String,
    /// Cable diameter in mm.
    pub diameter_mm: f32,
    /// Number of repetitions; `None` = about one per 15 mm.
    pub repeats: Option<u32>,
    /// Text across the tape (along the cable) instead of along the tape.
    pub vertical: bool,
}

/// Patch panel / port label: `count` fields of `pitch_mm` each, numbered
/// `prefix` + running number, optionally with separator lines.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchPanel {
    pub count: u32,
    /// Port pitch in mm (distance between port centers).
    pub pitch_mm: f32,
    pub start: i64,
    pub step: i64,
    pub prefix: String,
    /// Zero-pad numbers to this many digits (0 = no padding).
    pub digits: usize,
    pub separators: bool,
    /// Blank space before the first and after the last field, in mm.
    pub margin_mm: f32,
}

/// Which generator to run, tagged for JSON (GUI, CLI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Layout {
    CableFlag(CableFlag),
    CableWrap(CableWrap),
    PatchPanel(PatchPanel),
}

fn text_item(text: &str, rect: Rect, rotation: u16) -> Item {
    Item {
        element: Element::text(text),
        rect: Some(rect),
        rotation,
    }
}

/// A label whose length is exactly `length_mm` (boxes define the content,
/// no extra padding).
fn fixed_length(elements: Vec<Item>, length_mm: f32) -> Label {
    Label {
        elements,
        padding_mm: 0.0,
        min_length_mm: Some(length_mm),
        ..Label::default()
    }
}

/// Wrap length for a cable of `diameter_mm` (its circumference).
pub fn circumference(diameter_mm: f32) -> f32 {
    PI * diameter_mm.max(0.0)
}

pub fn cable_flag(spec: &CableFlag, tape_mm: f32) -> Label {
    let flag = spec.flag_mm.max(1.0);
    let wrap = circumference(spec.diameter_mm);
    let rect = |x_mm| Rect {
        x_mm,
        y_mm: 0.0,
        w_mm: flag,
        h_mm: tape_mm,
    };
    fixed_length(
        vec![
            text_item(&spec.text, rect(0.0), 0),
            text_item(&spec.text, rect(flag + wrap), 0),
        ],
        2.0 * flag + wrap,
    )
}

pub fn cable_wrap(spec: &CableWrap, tape_mm: f32) -> Label {
    let length = circumference(spec.diameter_mm).max(1.0);
    let repeats = spec
        .repeats
        .unwrap_or_else(|| (length / 15.0).round() as u32)
        .max(1);
    let w = length / repeats as f32;
    let elements = (0..repeats)
        .map(|i| {
            text_item(
                &spec.text,
                Rect {
                    x_mm: i as f32 * w,
                    y_mm: 0.0,
                    w_mm: w,
                    h_mm: tape_mm,
                },
                if spec.vertical { 90 } else { 0 },
            )
        })
        .collect();
    fixed_length(elements, length)
}

pub fn patch_panel(spec: &PatchPanel, tape_mm: f32) -> Label {
    let count = spec.count.max(1);
    let pitch = spec.pitch_mm.max(1.0);
    let margin = spec.margin_mm.max(0.0);
    let mut elements = Vec::new();
    for i in 0..count {
        let number = spec.start + i as i64 * spec.step;
        let text = format!(
            "{}{}",
            spec.prefix,
            if number < 0 {
                format!("-{:0w$}", -number, w = spec.digits)
            } else {
                format!("{number:0w$}", w = spec.digits)
            }
        );
        let x = margin + i as f32 * pitch;
        elements.push(text_item(
            &text,
            Rect {
                x_mm: x,
                y_mm: 0.0,
                w_mm: pitch,
                h_mm: tape_mm,
            },
            0,
        ));
    }
    if spec.separators {
        for i in 0..=count {
            let x = margin + i as f32 * pitch - SEPARATOR_MM / 2.0;
            elements.push(Item {
                element: Element::Fill,
                rect: Some(Rect {
                    x_mm: x.max(0.0),
                    y_mm: 0.0,
                    w_mm: SEPARATOR_MM,
                    h_mm: tape_mm,
                }),
                rotation: 0,
            });
        }
    }
    fixed_length(elements, 2.0 * margin + count as f32 * pitch)
}

/// Runs the generator selected by `layout`.
pub fn generate(layout: &Layout, tape_mm: f32) -> Label {
    match layout {
        Layout::CableFlag(s) => cable_flag(s, tape_mm),
        Layout::CableWrap(s) => cable_wrap(s, tape_mm),
        Layout::PatchPanel(s) => patch_panel(s, tape_mm),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::label::{geometry_for, render_label};
    use ll_protocol::model::{dots_to_mm, mm_to_dots};

    fn p710() -> &'static ll_protocol::model::ModelInfo {
        ll_protocol::model::find_by_name("PT-P710BT").unwrap()
    }

    fn texts(label: &Label) -> Vec<String> {
        label
            .elements
            .iter()
            .filter_map(|i| match &i.element {
                Element::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn cable_flag_has_wrap_gap_of_the_circumference() {
        let label = cable_flag(
            &CableFlag {
                text: "LAN 12".into(),
                diameter_mm: 6.0,
                flag_mm: 20.0,
            },
            9.9,
        );
        assert_eq!(texts(&label), ["LAN 12", "LAN 12"]);
        let second = label.elements[1].rect.unwrap();
        assert!((second.x_mm - (20.0 + PI * 6.0)).abs() < 1e-3);
        assert!((label.min_length_mm.unwrap() - (40.0 + PI * 6.0)).abs() < 1e-3);
    }

    #[test]
    fn cable_wrap_repeats_across_the_circumference() {
        let label = cable_wrap(
            &CableWrap {
                text: "PE".into(),
                diameter_mm: 10.0,
                repeats: None,
                vertical: true,
            },
            9.9,
        );
        // 31.4 mm / 15 mm ≈ 2 repetitions, rotated.
        assert_eq!(label.elements.len(), 2);
        assert!(label.elements.iter().all(|i| i.rotation == 90));
    }

    #[test]
    fn patch_panel_fields_have_exact_pitch_and_numbers() {
        let spec = PatchPanel {
            count: 24,
            pitch_mm: 12.7,
            start: 1,
            step: 1,
            prefix: "P".into(),
            digits: 2,
            separators: true,
            margin_mm: 2.0,
        };
        let label = patch_panel(&spec, 9.9);
        let t = texts(&label);
        assert_eq!(t.first().map(String::as_str), Some("P01"));
        assert_eq!(t.last().map(String::as_str), Some("P24"));
        // 24 fields + 25 separators.
        assert_eq!(label.elements.len(), 49);

        // Rendered length matches margins + 24 × pitch within a dot.
        let model = p710();
        let geometry = geometry_for(model, 12).unwrap();
        let bitmap = render_label(&label, model, geometry).unwrap();
        let expected = mm_to_dots(2.0 * 2.0 + 24.0 * 12.7);
        assert!((bitmap.height_dots() as i64 - expected as i64).abs() <= 1);
        assert!(dots_to_mm(bitmap.height_dots()) > 300.0);
    }

    #[test]
    fn layouts_round_trip_as_json() {
        let layout = Layout::PatchPanel(PatchPanel {
            count: 8,
            pitch_mm: 11.0,
            start: 1,
            step: 1,
            prefix: String::new(),
            digits: 0,
            separators: false,
            margin_mm: 0.0,
        });
        let json = serde_json::to_string(&layout).unwrap();
        assert!(json.contains(r#""kind":"patch_panel""#));
        assert_eq!(serde_json::from_str::<Layout>(&json).unwrap(), layout);
    }
}
