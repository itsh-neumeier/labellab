//! Renders text with the placeholder font ([`crate::font`]) into a
//! [`Bitmap`] oriented for the printer: pins (tape width) run across the
//! bitmap's `x` axis, raster lines (feed direction / label length) run
//! down its `y` axis — one bitmap column per printed raster line.

use crate::{font, Bitmap};

/// Renders `text` for a tape with `head_pins` total print-head pins, using
/// `printable_pins` of them starting at `left_offset_pins` (see
/// `ll_protocol::model::TapeGeometry`). The glyph height is scaled to fill
/// most of `printable_pins` (capped so large tapes don't get absurdly
/// chunky placeholder text).
pub fn render_text(
    text: &str,
    head_pins: u16,
    printable_pins: u16,
    left_offset_pins: u16,
) -> Bitmap {
    let scale = (printable_pins as u32 / (font::GLYPH_HEIGHT + 2)).clamp(1, 8);
    let glyph_h = font::GLYPH_HEIGHT * scale;
    let glyph_w = font::GLYPH_WIDTH * scale;
    let spacing = scale;

    let chars: Vec<char> = text.chars().collect();
    let total_cols = if chars.is_empty() {
        0
    } else {
        chars.len() as u32 * (glyph_w + spacing) - spacing
    };

    let mut bitmap = Bitmap::new(head_pins, total_cols);
    let vertical_offset =
        left_offset_pins as u32 + (printable_pins as u32).saturating_sub(glyph_h) / 2;

    let mut col_cursor = 0u32;
    for ch in chars {
        let rows = font::glyph(ch);
        for (gy, row_bits) in rows.iter().enumerate() {
            for gx in 0..font::GLYPH_WIDTH {
                let bit_set = (row_bits >> (font::GLYPH_WIDTH - 1 - gx)) & 1 == 1;
                if !bit_set {
                    continue;
                }
                for sx in 0..scale {
                    let y = col_cursor + gx * scale + sx;
                    for sy in 0..scale {
                        let x = vertical_offset + gy as u32 * scale + sy;
                        bitmap.set_pixel(x as u16, y, true);
                    }
                }
            }
        }
        col_cursor += glyph_w + spacing;
    }

    bitmap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_zero_width() {
        let bmp = render_text("", 128, 50, 39);
        assert_eq!(bmp.height_dots(), 0);
    }

    #[test]
    fn single_char_width_matches_scaled_glyph() {
        let bmp = render_text("I", 128, 50, 39);
        // printable_pins=50 -> scale = 50/7 = 7 (clamped to 8)
        let scale = (50u32 / 7).clamp(1, 8);
        assert_eq!(bmp.height_dots(), font::GLYPH_WIDTH * scale);
    }

    #[test]
    fn rendered_pixels_stay_within_printable_region() {
        let bmp = render_text("H", 128, 9, 0);
        for y in 0..bmp.height_dots() {
            let row = bmp.row(y);
            // Only the first byte can have bits (printable_pins=9 -> all ink
            // within pins 0..9, well inside the first 2 row bytes).
            assert!(row[2..].iter().all(|&b| b == 0));
        }
    }
}
