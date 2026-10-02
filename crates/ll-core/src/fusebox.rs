//! Fuse box / distribution board element: one element holding a row of
//! fields along its box (one per device), each with its own text and
//! relative width, separated by lines in one of several styles. The text
//! is rendered by `label::render_fuse_box`; this module holds the model
//! and the geometry (field positions, separators).

use ll_render::Bitmap;
use serde::{Deserialize, Serialize};

/// Smallest and largest relative field width.
pub const MIN_RATIO: f32 = 0.25;
pub const MAX_RATIO: f32 = 20.0;

/// Separator line width in mm (thin and bold styles).
pub const LINE_MM: f32 = 0.25;
pub const BOLD_MM: f32 = 0.7;
/// Dash and gap length in mm of the dashed style.
pub const DASH_MM: f32 = 1.0;
pub const DASH_GAP_MM: f32 = 0.8;
/// Length of the marks of the marks style, as a fraction of the height.
const MARK_FRACTION: f32 = 0.2;

fn one() -> f32 {
    1.0
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}

/// One field (column) of a fuse box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuseField {
    #[serde(default)]
    pub text: String,
    /// Width relative to the other fields (1 = one module, 2 = a device
    /// spanning two modules).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub ratio: f32,
    /// Text direction for this field; `None` = the element's `vertical`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical: Option<bool>,
}

impl FuseField {
    pub fn new(text: impl Into<String>, ratio: f32) -> Self {
        Self {
            text: text.into(),
            ratio,
            vertical: None,
        }
    }

    pub fn ratio(&self) -> f32 {
        if self.ratio.is_finite() {
            self.ratio.clamp(MIN_RATIO, MAX_RATIO)
        } else {
            1.0
        }
    }
}

/// How fields are separated.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FuseSeparator {
    /// Short marks at the top and bottom edge.
    Marks,
    /// Dashed lines.
    Dashed,
    /// Thin lines between and at the ends of the fields.
    Line,
    /// Bold lines.
    Bold,
    /// Thin lines plus a line along the top and bottom (a table).
    #[default]
    Frame,
    None,
}

/// Field positions along a box of `len` dots: `(field index, start, end)`
/// in drawing order (`reverse` = last field first).
pub fn field_spans(fields: &[FuseField], len: u32, reverse: bool) -> Vec<(usize, u32, u32)> {
    let total: f32 = fields.iter().map(FuseField::ratio).sum();
    if fields.is_empty() || total <= 0.0 {
        return Vec::new();
    }
    let order: Vec<usize> = if reverse {
        (0..fields.len()).rev().collect()
    } else {
        (0..fields.len()).collect()
    };
    let mut acc = 0.0f32;
    order
        .into_iter()
        .map(|i| {
            let start = (acc / total * len as f32).round() as u32;
            acc += fields[i].ratio();
            let end = (acc / total * len as f32).round() as u32;
            (i, start, end.min(len))
        })
        .collect()
}

/// Separator geometry in dots.
#[derive(Debug, Clone, Copy)]
pub struct SeparatorDots {
    /// Line thickness.
    pub line: u32,
    pub dash: u32,
    pub gap: u32,
}

/// Thickness in dots of the lines `style` draws (0 for none).
pub fn line_dots(style: FuseSeparator, thin: u32, bold: u32) -> u32 {
    match style {
        FuseSeparator::None => 0,
        FuseSeparator::Bold => bold,
        _ => thin,
    }
}

/// Draws the separators at `edges` (dots along the box, ends included)
/// into `out` (box-local: `width_pins` across, `height_dots` along).
pub fn draw_separators(out: &mut Bitmap, edges: &[u32], style: FuseSeparator, d: SeparatorDots) {
    let (h, len) = (out.width_pins(), out.height_dots());
    if style == FuseSeparator::None || h == 0 || len == 0 || d.line == 0 {
        return;
    }
    let t = d.line.min(len);
    let mark = ((h as f32 * MARK_FRACTION).round() as u16).max(1);
    let ink_at = |pin: u16| -> bool {
        match style {
            FuseSeparator::Marks => pin < mark || pin >= h.saturating_sub(mark),
            FuseSeparator::Dashed => {
                let period = (d.dash + d.gap).max(1);
                (pin as u32) % period < d.dash.max(1)
            }
            _ => true,
        }
    };
    for &edge in edges {
        // Centered on the edge, kept inside the box at the ends.
        let start = edge.saturating_sub(t / 2).min(len - t);
        for line in start..start + t {
            for pin in 0..h {
                if ink_at(pin) {
                    out.set_pixel(pin, line, true);
                }
            }
        }
    }
    if style == FuseSeparator::Frame {
        let t = (t as u16).min(h);
        for line in 0..len {
            for pin in (0..t).chain(h - t..h) {
                out.set_pixel(pin, line, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_follow_the_ratios_and_reverse() {
        let fields = [
            FuseField::new("A", 2.0),
            FuseField::new("B", 1.0),
            FuseField::new("C", 1.0),
        ];
        assert_eq!(
            field_spans(&fields, 100, false),
            vec![(0, 0, 50), (1, 50, 75), (2, 75, 100)]
        );
        assert_eq!(
            field_spans(&fields, 100, true),
            vec![(2, 0, 25), (1, 25, 50), (0, 50, 100)]
        );
        assert!(field_spans(&[], 100, false).is_empty());
    }

    #[test]
    fn separator_styles_draw_where_expected() {
        let d = SeparatorDots {
            line: 2,
            dash: 4,
            gap: 4,
        };
        let draw = |style| {
            let mut b = Bitmap::new(40, 100);
            draw_separators(&mut b, &[0, 50, 100], style, d);
            b
        };
        let frame = draw(FuseSeparator::Frame);
        assert!(frame.pixel(20, 50) && frame.pixel(0, 25) && frame.pixel(39, 25));
        assert!(frame.pixel(20, 0) && frame.pixel(20, 99));
        let line = draw(FuseSeparator::Line);
        assert!(line.pixel(20, 50) && !line.pixel(0, 25));
        let marks = draw(FuseSeparator::Marks);
        assert!(marks.pixel(1, 50) && marks.pixel(38, 50) && !marks.pixel(20, 50));
        let dashed = draw(FuseSeparator::Dashed);
        assert!(dashed.pixel(1, 50) && !dashed.pixel(5, 50));
        let none = draw(FuseSeparator::None);
        assert!((0..100).all(|l| (0..40).all(|p| !none.pixel(p, l))));
    }

    #[test]
    fn ratio_is_clamped_and_json_is_compact() {
        assert_eq!(FuseField::new("x", 100.0).ratio(), MAX_RATIO);
        let json = serde_json::to_string(&FuseField::new("F1", 1.0)).unwrap();
        assert_eq!(json, r#"{"text":"F1"}"#);
    }
}
