//! Draws a border around a whole label [`Bitmap`]: solid or patterned
//! (dashed, dotted, double, diagonal stripes), any line thickness, on any
//! combination of sides, optionally inset from the edge.
//!
//! Sides are named as the label appears in the preview: `top` is the
//! tape edge at `pin_offset`, `bottom` the one at `pin_offset + pin_count`,
//! `left`/`right` are the start/end of the label along the feed axis.

use serde::{Deserialize, Serialize};

use crate::Bitmap;

/// Line pattern of a border.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BorderStyle {
    #[default]
    Solid,
    /// Dashes and gaps of `pattern` dots each.
    Dashed,
    /// Square dots of the line thickness with equal gaps.
    Dotted,
    /// Two thin lines (one third of the thickness each).
    Double,
    /// Diagonal stripes of `pattern` dots (warning-tape look).
    Striped,
}

/// Which edges get a border line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorderSides {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}

impl BorderSides {
    pub const ALL: Self = Self {
        top: true,
        bottom: true,
        left: true,
        right: true,
    };
}

impl Default for BorderSides {
    fn default() -> Self {
        Self::ALL
    }
}

/// A border in print dots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Border {
    pub style: BorderStyle,
    /// Line thickness.
    pub thickness: u16,
    pub sides: BorderSides,
    /// Dash length (`Dashed`) or stripe width (`Striped`).
    pub pattern: u32,
    /// Distance between the label edge and the border.
    pub inset: u32,
}

/// Draws a solid `thickness`-dot border on all four sides of the rectangle
/// spanning `pin_offset..pin_offset+pin_count` (tape-width axis) and the
/// bitmap's full length (`0..height_dots`, feed axis).
pub fn draw_border(bitmap: &mut Bitmap, pin_offset: u16, pin_count: u16, thickness: u16) {
    draw_border_styled(
        bitmap,
        pin_offset,
        pin_count,
        &Border {
            style: BorderStyle::Solid,
            thickness,
            sides: BorderSides::ALL,
            pattern: 1,
            inset: 0,
        },
    );
}

/// Whether the border pixel at `across` dots from the outer edge and
/// `along` dots along the edge is inked; `(pin, line)` are absolute
/// coordinates so diagonal stripes continue around corners.
fn inked(border: &Border, across: u32, along: u32, pin: u32, line: u32) -> bool {
    let t = border.thickness as u32;
    let pattern = border.pattern.max(1);
    match border.style {
        BorderStyle::Solid => true,
        BorderStyle::Dashed => (along / pattern).is_multiple_of(2),
        BorderStyle::Dotted => (along / t.max(1)).is_multiple_of(2),
        BorderStyle::Double => {
            if t < 3 {
                true
            } else {
                let w = t / 3;
                across < w || across >= t - w
            }
        }
        BorderStyle::Striped => ((pin + line) / pattern).is_multiple_of(2),
    }
}

/// Draws `border` around the rectangle spanning
/// `pin_offset..pin_offset+pin_count` and the bitmap's full length.
pub fn draw_border_styled(bitmap: &mut Bitmap, pin_offset: u16, pin_count: u16, border: &Border) {
    let height = bitmap.height_dots();
    let t = border.thickness as u32;
    let sides = border.sides;
    if height == 0 || pin_count == 0 || t == 0 {
        return;
    }
    // Inner rectangle the border lines run along: pins p0..p1, lines l0..l1.
    let (p0, p1) = (
        border.inset,
        (pin_count as u32).saturating_sub(border.inset),
    );
    let (l0, l1) = (border.inset, height.saturating_sub(border.inset));
    if p0 >= p1 || l0 >= l1 {
        return;
    }
    let in_band = |from_start: u32, from_end: u32, start: bool, end: bool| {
        if start && from_start < t {
            Some(from_start)
        } else if end && from_end < t {
            Some(from_end)
        } else {
            None
        }
    };
    for p in p0..p1 {
        let across_tb = in_band(p - p0, p1 - 1 - p, sides.top, sides.bottom);
        let put = |line: u32, bitmap: &mut Bitmap| {
            let across_lr = in_band(line - l0, l1 - 1 - line, sides.left, sides.right);
            let ink = across_tb.is_some_and(|a| inked(border, a, line - l0, p, line))
                || across_lr.is_some_and(|a| inked(border, a, p - p0, p, line));
            if ink {
                bitmap.set_pixel(pin_offset + p as u16, line, true);
            }
        };
        if across_tb.is_some() {
            for line in l0..l1 {
                put(line, bitmap);
            }
        } else {
            for line in (l0..(l0 + t).min(l1)).chain(l1.saturating_sub(t).max(l0 + t)..l1) {
                put(line, bitmap);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(bmp: &Bitmap, pin: u16, line: u32) -> bool {
        let byte = bmp.row(line)[(pin / 8) as usize];
        (byte >> (7 - pin % 8)) & 1 == 1
    }

    fn count_ink(bmp: &Bitmap, pins: std::ops::Range<u16>, lines: std::ops::Range<u32>) -> usize {
        pins.flat_map(|p| lines.clone().map(move |l| (p, l)))
            .filter(|&(p, l)| pixel(bmp, p, l))
            .count()
    }

    fn styled(style: BorderStyle, sides: BorderSides) -> Border {
        Border {
            style,
            thickness: 4,
            sides,
            pattern: 6,
            inset: 0,
        }
    }

    #[test]
    fn top_only_border_leaves_other_sides_blank() {
        let mut bmp = Bitmap::new(128, 100);
        let sides = BorderSides {
            top: true,
            bottom: false,
            left: false,
            right: false,
        };
        draw_border_styled(&mut bmp, 39, 50, &styled(BorderStyle::Solid, sides));
        assert!(pixel(&bmp, 39, 50) && pixel(&bmp, 42, 50));
        assert!(!pixel(&bmp, 43, 50));
        assert!(!pixel(&bmp, 88, 50), "bottom must stay blank");
        assert!(!pixel(&bmp, 60, 0), "left must stay blank");
    }

    #[test]
    fn patterns_ink_part_of_the_band() {
        let sides = BorderSides {
            top: true,
            bottom: false,
            left: false,
            right: false,
        };
        let band = |style| {
            let mut bmp = Bitmap::new(128, 120);
            draw_border_styled(&mut bmp, 39, 50, &styled(style, sides));
            count_ink(&bmp, 39..43, 0..120)
        };
        let solid = band(BorderStyle::Solid);
        assert_eq!(solid, 4 * 120);
        for style in [
            BorderStyle::Dashed,
            BorderStyle::Dotted,
            BorderStyle::Double,
            BorderStyle::Striped,
        ] {
            let n = band(style);
            assert!(n > solid / 4 && n < solid, "{style:?}: {n}");
        }
    }

    #[test]
    fn inset_moves_the_border_inwards() {
        let mut bmp = Bitmap::new(128, 100);
        let mut border = styled(BorderStyle::Solid, BorderSides::ALL);
        border.inset = 5;
        draw_border_styled(&mut bmp, 39, 50, &border);
        assert!(!pixel(&bmp, 39, 50));
        assert!(pixel(&bmp, 44, 50));
        assert!(!pixel(&bmp, 60, 0));
        assert!(pixel(&bmp, 60, 5));
    }

    #[test]
    fn draws_caps_at_start_and_end() {
        let mut bmp = Bitmap::new(128, 20);
        draw_border(&mut bmp, 39, 50, 2);
        assert!(pixel(&bmp, 50, 0), "top cap missing");
        assert!(pixel(&bmp, 50, 19), "bottom cap missing");
        assert!(!pixel(&bmp, 50, 10), "middle should be unbordered");
    }

    #[test]
    fn draws_sides_along_full_length() {
        let mut bmp = Bitmap::new(128, 20);
        draw_border(&mut bmp, 39, 50, 2);
        assert!(pixel(&bmp, 39, 10), "left side missing at pin_offset");
        assert!(
            pixel(&bmp, 88, 10),
            "right side missing at pin_offset+pin_count-1"
        );
        assert!(
            !pixel(&bmp, 63, 10),
            "middle of tape width should be unbordered"
        );
    }

    #[test]
    fn zero_thickness_draws_nothing() {
        let mut bmp = Bitmap::new(128, 20);
        draw_border(&mut bmp, 39, 50, 0);
        for line in 0..20 {
            assert_eq!(bmp.row(line), &vec![0u8; 16][..]);
        }
    }

    #[test]
    fn empty_bitmap_does_not_panic() {
        let mut bmp = Bitmap::new(128, 0);
        draw_border(&mut bmp, 39, 50, 2); // must not panic/index out of bounds
        assert_eq!(bmp.height_dots(), 0);
    }
}
