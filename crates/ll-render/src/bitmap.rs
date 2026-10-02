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

    /// Mirrored along the label length (mirror print, e.g. for reading
    /// through clear tape from the back).
    pub fn mirrored(&self) -> Bitmap {
        let mut rows = self.rows.clone();
        rows.reverse();
        Bitmap { rows, ..*self }
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

    /// Cuts the bitmap to at most `lines` raster lines (drops the rest).
    pub fn truncate(&mut self, lines: u32) {
        if lines < self.height_dots {
            self.rows.truncate(lines as usize);
            self.height_dots = lines;
        }
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

    /// Rotates by `quarter_turns` x 90° clockwise as seen in the preview
    /// (pins = rows top to bottom, lines = columns left to right).
    pub fn rotated(&self, quarter_turns: u8) -> Bitmap {
        // Preview coordinates: column c = line, row r = pin.
        let (w, h) = (self.height_dots, self.width_pins as u32);
        match quarter_turns % 4 {
            0 => self.clone(),
            2 => {
                let mut out = Bitmap::new(self.width_pins, self.height_dots);
                for line in 0..w {
                    for pin in 0..self.width_pins {
                        if self.pixel(pin, line) {
                            out.set_pixel(self.width_pins - 1 - pin, w - 1 - line, true);
                        }
                    }
                }
                out
            }
            q => {
                // 90° cw: (c, r) -> (h-1-r, c); 270° cw: (c, r) -> (r, w-1-c).
                let mut out = Bitmap::new(w.min(u16::MAX as u32) as u16, h);
                for line in 0..w {
                    for pin in 0..self.width_pins {
                        if !self.pixel(pin, line) {
                            continue;
                        }
                        let (c, r) = (line, pin as u32);
                        let (nc, nr) = if q == 1 {
                            (h - 1 - r, c)
                        } else {
                            (r, w - 1 - c)
                        };
                        out.set_pixel(nr as u16, nc, true);
                    }
                }
                out
            }
        }
    }

    /// Sets every pixel (a filled box).
    pub fn fill(&mut self) {
        let full = 0xFFu8;
        for row in &mut self.rows {
            row.iter_mut().for_each(|b| *b = full);
        }
        // Clear padding bits beyond width_pins in the last byte.
        let rem = self.width_pins % 8;
        if rem != 0 {
            let mask = 0xFFu8 << (8 - rem);
            for row in &mut self.rows {
                if let Some(last) = row.last_mut() {
                    *last &= mask;
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
    fn rotation_maps_corners() {
        // 3 lines (columns) x 2 pins (rows); ink at column 0, row 0.
        let mut b = Bitmap::new(2, 3);
        b.set_pixel(0, 0, true);
        let r90 = b.rotated(1);
        assert_eq!((r90.width_pins(), r90.height_dots()), (3, 2));
        assert!(r90.pixel(0, 1), "top-left goes to top-right");
        let r180 = b.rotated(2);
        assert!(r180.pixel(1, 2), "top-left goes to bottom-right");
        let r270 = b.rotated(3);
        assert!(r270.pixel(2, 0), "top-left goes to bottom-left");
        assert_eq!(b.rotated(4), b);
    }

    #[test]
    fn fill_sets_only_real_pins() {
        let mut b = Bitmap::new(10, 2);
        b.fill();
        assert!(b.pixel(9, 1));
        assert_eq!(b.row(0), &[0xFF, 0xC0]);
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
