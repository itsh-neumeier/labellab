//! Draws a rectangular border around a whole label [`Bitmap`].
//!
//! No per-element frames/arbitrary shapes yet — that needs a composition
//! model for positioning multiple elements on one label, which is M6
//! editor scope. This covers the common case of a single bordered label
//! (e.g. a warning sign).

use crate::Bitmap;

/// Draws a `thickness`-dot border around the rectangle spanning
/// `pin_offset..pin_offset+pin_count` (tape-width axis) and the bitmap's
/// full length (`0..height_dots`, feed axis).
pub fn draw_border(bitmap: &mut Bitmap, pin_offset: u16, pin_count: u16, thickness: u16) {
    let height = bitmap.height_dots();
    if height == 0 || pin_count == 0 || thickness == 0 {
        return;
    }

    // Caps: short edges at the start/end of the label, spanning the pins.
    let cap = (thickness as u32).min(height);
    for line in 0..cap {
        for p in 0..pin_count {
            bitmap.set_pixel(pin_offset + p, line, true);
        }
    }
    for line in height.saturating_sub(thickness as u32)..height {
        for p in 0..pin_count {
            bitmap.set_pixel(pin_offset + p, line, true);
        }
    }

    // Sides: long edges along the tape-width boundaries, spanning the length.
    let side = thickness.min(pin_count);
    for p in 0..side {
        for line in 0..height {
            bitmap.set_pixel(pin_offset + p, line, true);
        }
    }
    for p in pin_count.saturating_sub(thickness)..pin_count {
        for line in 0..height {
            bitmap.set_pixel(pin_offset + p, line, true);
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
