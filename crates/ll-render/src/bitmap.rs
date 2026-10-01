//! 1-bit raster bitmap: the single representation shared by live preview
//! and the actual print job (see `AGENTS.md`: "Vorschau und Druck nutzen
//! denselben Renderpfad").

/// A 1-bit-per-pixel bitmap, one bit per print dot, MSB first per row byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    width_pins: u16,
    height_dots: u32,
    /// Row-major, `ceil(width_pins / 8)` bytes per row.
    rows: Vec<Vec<u8>>,
}

impl Bitmap {
    pub fn new(width_pins: u16, height_dots: u32) -> Self {
        let row_bytes = width_pins.div_ceil(8) as usize;
        Self {
            width_pins,
            height_dots,
            rows: vec![vec![0u8; row_bytes]; height_dots as usize],
        }
    }

    pub fn width_pins(&self) -> u16 {
        self.width_pins
    }

    pub fn height_dots(&self) -> u32 {
        self.height_dots
    }

    pub fn set_pixel(&mut self, x: u16, y: u32, black: bool) {
        if x >= self.width_pins || y >= self.height_dots {
            return;
        }
        let byte = &mut self.rows[y as usize][(x / 8) as usize];
        let mask = 0x80 >> (x % 8);
        if black {
            *byte |= mask;
        } else {
            *byte &= !mask;
        }
    }

    pub fn row(&self, y: u32) -> &[u8] {
        &self.rows[y as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_bitmap_is_all_white() {
        let bmp = Bitmap::new(24, 10);
        assert_eq!(bmp.row(0), &[0u8; 3]);
    }

    #[test]
    fn set_pixel_sets_correct_bit() {
        let mut bmp = Bitmap::new(16, 1);
        bmp.set_pixel(0, 0, true);
        bmp.set_pixel(15, 0, true);
        assert_eq!(bmp.row(0), &[0x80, 0x01]);
    }
}
