//! Renders a [`Bitmap`] to PNG bytes for preview (CLI `render`, GUI live
//! preview later) — the same bitmap the print path sends, per
//! `AGENTS.md`: "Vorschau und Druck nutzen denselben Renderpfad".

use image::{GrayImage, Luma, Rgba, RgbaImage};

use crate::{Bitmap, RenderError};

/// Encodes the printable pin range `pin_offset..pin_offset+pin_count` of
/// `bitmap` as a PNG. Oriented naturally: image width = label length
/// (raster lines), image height = tape width (printable pins). White
/// background, black ink.
pub fn to_png(bitmap: &Bitmap, pin_offset: u16, pin_count: u16) -> Result<Vec<u8>, RenderError> {
    let width = bitmap.height_dots().max(1);
    let height = (pin_count as u32).max(1);
    let mut img = GrayImage::from_pixel(width, height, Luma([255u8]));

    for line in 0..bitmap.height_dots() {
        let row = bitmap.row(line);
        for p in 0..pin_count {
            let pin = (pin_offset + p) as usize;
            let byte = row[pin / 8];
            let bit = (byte >> (7 - pin % 8)) & 1;
            if bit == 1 {
                img.put_pixel(line, p as u32, Luma([0u8]));
            }
        }
    }

    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| RenderError::Encode(e.to_string()))?;
    Ok(out)
}

/// Like [`to_png`], but RGBA with ink opaque black and the background
/// fully transparent, so the GUI can show it in any tape/ink color (as a
/// CSS mask over the tape color).
pub fn to_png_mask(
    bitmap: &Bitmap,
    pin_offset: u16,
    pin_count: u16,
) -> Result<Vec<u8>, RenderError> {
    let width = bitmap.height_dots().max(1);
    let height = (pin_count as u32).max(1);
    let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    for line in 0..bitmap.height_dots() {
        for p in 0..pin_count {
            if bitmap.pixel(pin_offset + p, line) {
                img.put_pixel(line, p as u32, Rgba([0, 0, 0, 255]));
            }
        }
    }
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| RenderError::Encode(e.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bitmap_encodes_to_valid_png() {
        let bmp = Bitmap::new(128, 0);
        let png = to_png(&bmp, 39, 50).unwrap();
        assert_eq!(
            &png[..8],
            &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n']
        );
    }

    #[test]
    fn ink_pixel_shows_up_black() {
        let mut bmp = Bitmap::new(128, 1);
        bmp.set_pixel(39, 0, true); // first printable pin at offset 39
        let png = to_png(&bmp, 39, 50).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_luma8();
        assert_eq!(img.get_pixel(0, 0).0[0], 0);
        assert_eq!(img.get_pixel(0, 1).0[0], 255);
    }
}
