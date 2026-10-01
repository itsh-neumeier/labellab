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

    /// Whether the pixel at (`x`, `y`) is ink. Out-of-range is white.
    pub fn pixel(&self, x: u16, y: u32) -> bool {
        if x >= self.width_pins || y >= self.height_dots {
            return false;
        }
        self.rows[y as usize][(x / 8) as usize] & (0x80 >> (x % 8)) != 0
    }

    /// Appends `lines` blank raster lines at the end (label length axis).
    pub fn extend_blank(&mut self, lines: u32) {
        let row_bytes = self.width_pins.div_ceil(8) as usize;
        self.rows
            .extend(std::iter::repeat_n(vec![0u8; row_bytes], lines as usize));
        self.height_dots += lines;
    }

    /// Inserts `lines` blank raster lines at the start.
    pub fn prepend_blank(&mut self, lines: u32) {
        let row_bytes = self.width_pins.div_ceil(8) as usize;
        self.rows.splice(
            0..0,
            std::iter::repeat_n(vec![0u8; row_bytes], lines as usize),
        );
        self.height_dots += lines;
    }

    /// ORs `src` into this bitmap with `src`'s pin 0 at `pin_offset` and
    /// its line 0 at `line_offset`. Ink landing outside `clip_pins` (or
    /// outside this bitmap) is dropped.
    pub fn blit(
        &mut self,
        src: &Bitmap,
        pin_offset: i32,
        line_offset: i32,
        clip_pins: std::ops::Range<u16>,
    ) {
        for line in 0..src.height_dots {
            let y = line_offset + line as i32;
            if y < 0 || y >= self.height_dots as i32 {
                continue;
            }
            for pin in 0..src.width_pins {
                let x = pin_offset + pin as i32;
                if x < clip_pins.start as i32 || x >= clip_pins.end as i32 {
                    continue;
                }
                if src.pixel(pin, line) {
                    self.set_pixel(x as u16, y as u32, true);
                }
            }
        }
    }

    /// Appends `other`'s raster lines after this bitmap's. Both must have
    /// the same `width_pins` (one print head); returns `false` and leaves
    /// `self` unchanged otherwise.
    pub fn append(&mut self, other: &Bitmap) -> bool {
        if other.width_pins != self.width_pins {
            return false;
        }
        self.rows.extend(other.rows.iter().cloned());
        self.height_dots += other.height_dots;
        true
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

    #[test]
    fn append_and_blank_lines_grow_length() {
        let mut a = Bitmap::new(16, 1);
        a.set_pixel(3, 0, true);
        let mut b = Bitmap::new(16, 2);
        b.set_pixel(5, 1, true);

        a.extend_blank(2);
        assert!(a.append(&b));
        a.prepend_blank(1);

        assert_eq!(a.height_dots(), 6);
        assert!(a.pixel(3, 1));
        assert!(a.pixel(5, 5));
        assert!(!a.pixel(5, 4));
        assert!(!a.append(&Bitmap::new(8, 1)));
    }

    #[test]
    fn blit_offsets_and_clips() {
        let mut src = Bitmap::new(4, 2);
        for pin in 0..4 {
            src.set_pixel(pin, 1, true);
        }
        let mut dst = Bitmap::new(16, 5);
        dst.blit(&src, 2, 3, 0..4);
        assert!(dst.pixel(2, 4) && dst.pixel(3, 4));
        assert!(!dst.pixel(4, 4), "clipped at pin 4");
        assert!(!dst.pixel(2, 3));
        dst.blit(&src, -2, -1, 0..16);
        assert!(dst.pixel(0, 0) && dst.pixel(1, 0));
    }
}
