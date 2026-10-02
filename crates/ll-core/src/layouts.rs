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

/// Single cable flag: wrap area around the cable, then one flag with the
/// text (the flag sticks out to one side).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SingleFlag {
    pub text: String,
    /// Cable diameter in mm.
    pub diameter_mm: f32,
    /// Flag length in mm.
    pub flag_mm: f32,
}

/// Terminal / punch-down block: `count` columns of `pitch_mm`, one or two
/// rows. With two rows, column `i` shows number `2i` below and `2i + 1`
/// above (counting from `start` in `step`s), like a punch-down block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerminalBlock {
    pub count: u32,
    pub pitch_mm: f32,
    pub start: i64,
    pub step: i64,
    pub prefix: String,
    pub digits: usize,
    /// 1 or 2.
    pub rows: u8,
    pub separators: bool,
    pub margin_mm: f32,
}

/// Fuse box / distribution board: `count` module fields of `pitch_mm`
/// (e.g. 17.5 mm DIN module), numbered text, optionally written across
/// the tape (vertical), plus an optional wider main switch field at the
/// start or end.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuseBox {
    pub count: u32,
    pub pitch_mm: f32,
    pub start: i64,
    pub step: i64,
    pub prefix: String,
    pub digits: usize,
    /// Text across the tape (90°) instead of along it.
    pub vertical: bool,
    /// Text of the main switch field; empty = no main switch field.
    #[serde(default)]
    pub main_switch: String,
    /// Width of the main switch field in mm.
    #[serde(default = "default_main_switch_mm")]
    pub main_switch_mm: f32,
    /// Main switch field at the end instead of the start.
    #[serde(default)]
    pub main_switch_right: bool,
    pub separators: bool,
    pub margin_mm: f32,
    /// Modules per field: a device spanning several modules becomes one
    /// field (column). Empty = `count` fields of one module each;
    /// otherwise `count` is ignored and the fields follow `spans`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spans: Vec<u32>,
    /// Text per field; missing or empty = automatic number.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub texts: Vec<String>,
}

impl FuseBox {
    /// Modules per field (see [`FuseBox::spans`]).
    pub fn field_spans(&self) -> Vec<u32> {
        if self.spans.is_empty() {
            vec![1; self.count.max(1) as usize]
        } else {
            self.spans.iter().map(|s| (*s).max(1)).collect()
        }
    }
}

fn default_main_switch_mm() -> f32 {
    35.0
}

/// Which generator to run, tagged for JSON (GUI, CLI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Layout {
    CableFlag(CableFlag),
    CableWrap(CableWrap),
    PatchPanel(PatchPanel),
    SingleFlag(SingleFlag),
    TerminalBlock(TerminalBlock),
    FuseBox(FuseBox),
}

fn text_item(text: &str, rect: Rect, rotation: u16) -> Item {
    Item {
        element: Element::text(text),
        rect: Some(rect),
        rotation,
        locked: false,
        halign: None,
        valign: None,
        hidden: false,
        title: None,
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

/// `prefix` + `number`, zero-padded to `digits`.
fn numbered(prefix: &str, number: i64, digits: usize) -> String {
    if number < 0 {
        format!("{prefix}-{:0digits$}", -number)
    } else {
        format!("{prefix}{number:0digits$}")
    }
}

/// A full-height separator line centered on `x`.
fn separator(x: f32, y: f32, h: f32) -> Item {
    fill_item(Rect {
        x_mm: (x - SEPARATOR_MM / 2.0).max(0.0),
        y_mm: y,
        w_mm: SEPARATOR_MM,
        h_mm: h,
    })
}

fn fill_item(rect: Rect) -> Item {
    Item {
        element: Element::Fill,
        rect: Some(rect),
        rotation: 0,
        locked: false,
        halign: None,
        valign: None,
        hidden: false,
        title: None,
    }
}

pub fn patch_panel(spec: &PatchPanel, tape_mm: f32) -> Label {
    let count = spec.count.max(1);
    let pitch = spec.pitch_mm.max(1.0);
    let margin = spec.margin_mm.max(0.0);
    let mut elements = Vec::new();
    for i in 0..count {
        let number = spec.start + i as i64 * spec.step;
        let text = numbered(&spec.prefix, number, spec.digits);
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
            elements.push(separator(margin + i as f32 * pitch, 0.0, tape_mm));
        }
    }
    fixed_length(elements, 2.0 * margin + count as f32 * pitch)
}

pub fn single_flag(spec: &SingleFlag, tape_mm: f32) -> Label {
    let flag = spec.flag_mm.max(1.0);
    let wrap = circumference(spec.diameter_mm);
    let text = text_item(
        &spec.text,
        Rect {
            x_mm: wrap,
            y_mm: 0.0,
            w_mm: flag,
            h_mm: tape_mm,
        },
        0,
    );
    fixed_length(vec![text], wrap + flag)
}

pub fn terminal_block(spec: &TerminalBlock, tape_mm: f32) -> Label {
    let count = spec.count.max(1);
    let pitch = spec.pitch_mm.max(1.0);
    let margin = spec.margin_mm.max(0.0);
    let rows = spec.rows.clamp(1, 2) as u32;
    let row_h = tape_mm / rows as f32;
    let mut elements = Vec::new();
    for i in 0..count {
        let x = margin + i as f32 * pitch;
        for r in 0..rows {
            // Two rows: the lower row gets the first number of the column.
            let k = i * rows + if rows == 2 { 1 - r } else { r };
            let number = spec.start + k as i64 * spec.step;
            elements.push(text_item(
                &numbered(&spec.prefix, number, spec.digits),
                Rect {
                    x_mm: x,
                    y_mm: r as f32 * row_h,
                    w_mm: pitch,
                    h_mm: row_h,
                },
                0,
            ));
        }
    }
    if spec.separators {
        for i in 0..=count {
            elements.push(separator(margin + i as f32 * pitch, 0.0, tape_mm));
        }
        if rows == 2 {
            elements.push(fill_item(Rect {
                x_mm: margin,
                y_mm: row_h - SEPARATOR_MM / 2.0,
                w_mm: count as f32 * pitch,
                h_mm: SEPARATOR_MM,
            }));
        }
    }
    fixed_length(elements, 2.0 * margin + count as f32 * pitch)
}

pub fn fuse_box(spec: &FuseBox, tape_mm: f32) -> Label {
    let spans = spec.field_spans();
    let modules: u32 = spans.iter().sum();
    let pitch = spec.pitch_mm.max(1.0);
    let margin = spec.margin_mm.max(0.0);
    let main = !spec.main_switch.trim().is_empty();
    let main_w = if main {
        spec.main_switch_mm.max(1.0)
    } else {
        0.0
    };
    let fields_x = margin
        + if main && !spec.main_switch_right {
            main_w
        } else {
            0.0
        };
    // Vertical text reads bottom to top, as usual on distribution boards.
    let rotation = if spec.vertical { 270 } else { 0 };
    let mut elements = Vec::new();
    // Field edges in modules from the first field.
    let mut edges = vec![0u32];
    for (i, span) in spans.iter().enumerate() {
        let start = *edges.last().unwrap_or(&0);
        edges.push(start + span);
        let number = spec.start + i as i64 * spec.step;
        let text = spec
            .texts
            .get(i)
            .filter(|t| !t.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| numbered(&spec.prefix, number, spec.digits));
        elements.push(text_item(
            &text,
            Rect {
                x_mm: fields_x + start as f32 * pitch,
                y_mm: 0.0,
                w_mm: *span as f32 * pitch,
                h_mm: tape_mm,
            },
            rotation,
        ));
    }
    let fields_end = fields_x + modules as f32 * pitch;
    if main {
        let x = if spec.main_switch_right {
            fields_end
        } else {
            margin
        };
        elements.push(text_item(
            &spec.main_switch,
            Rect {
                x_mm: x,
                y_mm: 0.0,
                w_mm: main_w,
                h_mm: tape_mm,
            },
            0,
        ));
    }
    if spec.separators {
        let mut edges: Vec<f32> = edges.iter().map(|e| fields_x + *e as f32 * pitch).collect();
        if main {
            edges.push(if spec.main_switch_right {
                fields_end + main_w
            } else {
                margin
            });
        }
        for x in edges {
            elements.push(separator(x, 0.0, tape_mm));
        }
    }
    fixed_length(elements, 2.0 * margin + modules as f32 * pitch + main_w)
}

/// Runs the generator selected by `layout`.
/// Generates the label for `layout` and remembers the layout in it
/// ([`Label::source`]), so the template can be edited again later.
pub fn generate(layout: &Layout, tape_mm: f32) -> Label {
    let mut label = generate_plain(layout, tape_mm);
    label.source = Some(Box::new(layout.clone()));
    label
}

fn generate_plain(layout: &Layout, tape_mm: f32) -> Label {
    match layout {
        Layout::CableFlag(s) => cable_flag(s, tape_mm),
        Layout::CableWrap(s) => cable_wrap(s, tape_mm),
        Layout::PatchPanel(s) => patch_panel(s, tape_mm),
        Layout::SingleFlag(s) => single_flag(s, tape_mm),
        Layout::TerminalBlock(s) => terminal_block(s, tape_mm),
        Layout::FuseBox(s) => fuse_box(s, tape_mm),
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
    fn single_flag_puts_text_after_the_wrap_area() {
        let label = single_flag(
            &SingleFlag {
                text: "X1".into(),
                diameter_mm: 5.0,
                flag_mm: 25.0,
            },
            9.9,
        );
        assert_eq!(texts(&label), ["X1"]);
        assert!((label.elements[0].rect.unwrap().x_mm - PI * 5.0).abs() < 1e-3);
        assert!((label.min_length_mm.unwrap() - (25.0 + PI * 5.0)).abs() < 1e-3);
    }

    #[test]
    fn terminal_block_numbers_two_rows_like_a_punch_down_block() {
        let label = terminal_block(
            &TerminalBlock {
                count: 6,
                pitch_mm: 15.0,
                start: 1,
                step: 1,
                prefix: "1A-A".into(),
                digits: 2,
                rows: 2,
                separators: true,
                margin_mm: 1.0,
            },
            18.0,
        );
        let t = texts(&label);
        // Column 1: top A02, bottom A01; last column: top A12, bottom A11.
        assert_eq!(&t[..2], ["1A-A02", "1A-A01"]);
        assert_eq!(&t[10..], ["1A-A12", "1A-A11"]);
        // 12 fields + 7 vertical + 1 horizontal separator.
        assert_eq!(label.elements.len(), 20);
    }

    #[test]
    fn fuse_box_has_vertical_fields_and_main_switch() {
        let spec = FuseBox {
            count: 4,
            pitch_mm: 17.5,
            start: 1,
            step: 1,
            prefix: "F".into(),
            digits: 0,
            vertical: true,
            main_switch: "HAUPTSCHALTER".into(),
            main_switch_mm: 35.0,
            main_switch_right: false,
            separators: true,
            margin_mm: 0.0,
            spans: Vec::new(),
            texts: Vec::new(),
        };
        let label = fuse_box(&spec, 9.9);
        assert_eq!(texts(&label), ["F1", "F2", "F3", "F4", "HAUPTSCHALTER"]);
        assert!(generate(&Layout::FuseBox(spec.clone()), 9.9)
            .source
            .is_some());
        // Merged modules: a 3-module device is one field; custom text.
        let merged = FuseBox {
            spans: vec![1, 3, 2],
            texts: vec![String::new(), "FI".into()],
            main_switch: String::new(),
            ..spec.clone()
        };
        let m = fuse_box(&merged, 9.9);
        assert_eq!(texts(&m), ["F1", "FI", "F3"]);
        let w: Vec<f32> = m.elements[..3]
            .iter()
            .map(|i| i.rect.unwrap().w_mm)
            .collect();
        assert_eq!(w, [17.5, 52.5, 35.0]);
        assert!((m.min_length_mm.unwrap() - 6.0 * 17.5).abs() < 1e-3);
        assert_eq!(label.elements[0].rotation, 270);
        assert!((label.elements[0].rect.unwrap().x_mm - 35.0).abs() < 1e-3);
        assert!((label.min_length_mm.unwrap() - (35.0 + 4.0 * 17.5)).abs() < 1e-3);
        let right = fuse_box(
            &FuseBox {
                main_switch_right: true,
                ..spec
            },
            9.9,
        );
        assert!(right.elements[0].rect.unwrap().x_mm.abs() < 1e-3);
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
