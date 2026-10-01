//! Imported-image rendering: PNG/JPEG/BMP/SVG -> 1-bit [`Bitmap`] via
//! Floyd-Steinberg dithering. A bundled symbol library is still open M5
//! scope.
//!
//! Same orientation convention as `text`/QR: image height maps to the
//! pin (tape-width) axis, image width maps to the raster-line
//! (feed/length) axis.

use std::path::Path;

use image::{GrayImage, Luma};

use crate::{Bitmap, RenderError};

/// Loads `path` (PNG/JPEG/BMP via `image`, SVG via `resvg`/`usvg`/
/// `tiny-skia` — dispatched on the `.svg` extension), scales it to fill
/// `printable_pins` of tape height (keeping aspect ratio) and dithers it
/// to 1-bit.
pub fn render_image(
    path: &Path,
    head_pins: u16,
    printable_pins: u16,
    left_offset_pins: u16,
    invert: bool,
) -> Result<Bitmap, RenderError> {
    let gray = load_gray(path, printable_pins)?;
    render_gray(&gray, head_pins, printable_pins, left_offset_pins, invert)
}

/// Loads `path` as grayscale: PNG/JPEG/BMP via `image`, SVG (by
/// extension) rasterized `svg_height_px` tall via `resvg`.
pub(crate) fn load_gray(path: &Path, svg_height_px: u16) -> Result<GrayImage, RenderError> {
    let is_svg = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"));

    if is_svg {
        render_svg_to_gray(path, svg_height_px)
    } else {
        Ok(image::open(path)
            .map_err(|e| RenderError::Image(e.to_string()))?
            .to_luma8())
    }
}

/// Rasterizes an SVG file to a grayscale image `target_height_px` pixels
/// tall (aspect ratio preserved), white background. `render_gray` handles
/// any further scaling/dithering, same as raster formats.
fn render_svg_to_gray(path: &Path, target_height_px: u16) -> Result<GrayImage, RenderError> {
    let data = std::fs::read(path)?;
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default())
        .map_err(|e| RenderError::Image(e.to_string()))?;

    let svg_size = tree.size();
    if svg_size.width() <= 0.0 || svg_size.height() <= 0.0 || target_height_px == 0 {
        return Ok(GrayImage::new(0, 0));
    }

    let scale = target_height_px as f32 / svg_size.height();
    let px_w = (svg_size.width() * scale).round().max(1.0) as u32;
    let px_h = target_height_px as u32;

    let mut pixmap = tiny_skia::Pixmap::new(px_w, px_h)
        .ok_or_else(|| RenderError::Image(format!("invalid SVG raster size {px_w}x{px_h}")))?;
    pixmap.fill(tiny_skia::Color::WHITE);
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    // Rendered onto an opaque white background, so every pixel is already
    // fully opaque (alpha 255) — straight RGB->luma, no alpha compositing
    // needed.
    let mut gray = GrayImage::new(px_w, px_h);
    for (i, px) in pixmap.pixels().iter().enumerate() {
        let (r, g, b) = (px.red() as u32, px.green() as u32, px.blue() as u32);
        let luma = ((r * 299 + g * 587 + b * 114) / 1000) as u8;
        gray.put_pixel(i as u32 % px_w, i as u32 / px_w, Luma([luma]));
    }
    Ok(gray)
}

/// Core logic, separated from file I/O so it's testable without touching
/// the filesystem (see tests below).
pub fn render_gray(
    gray: &GrayImage,
    head_pins: u16,
    printable_pins: u16,
    left_offset_pins: u16,
    invert: bool,
) -> Result<Bitmap, RenderError> {
    let (src_w, src_h) = gray.dimensions();
    if src_w == 0 || src_h == 0 || printable_pins == 0 {
        return Ok(Bitmap::new(head_pins, 0));
    }

    let scale = printable_pins as f32 / src_h as f32;
    let new_h = printable_pins as u32;
    let new_w = ((src_w as f32) * scale).round().max(1.0) as u32;

    let resized =
        image::imageops::resize(gray, new_w, new_h, image::imageops::FilterType::Triangle);
    let bits = floyd_steinberg_dither(&resized, invert);

    let mut bitmap = Bitmap::new(head_pins, new_w);
    for y in 0..new_h {
        for x in 0..new_w {
            if bits[(y * new_w + x) as usize] {
                bitmap.set_pixel(left_offset_pins + y as u16, x, true);
            }
        }
    }
    Ok(bitmap)
}

/// Classic Floyd-Steinberg error diffusion. Returns one `bool` per pixel
/// (row-major), `true` = ink.
pub(crate) fn floyd_steinberg_dither(img: &GrayImage, invert: bool) -> Vec<bool> {
    let (w, h) = img.dimensions();
    let (w, h) = (w as usize, h as usize);
    let mut errors: Vec<f32> = img.pixels().map(|Luma([v])| *v as f32).collect();
    let mut ink = vec![false; w * h];

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let old = errors[idx];
            let is_dark = old < 128.0;
            ink[idx] = is_dark != invert;
            let new = if is_dark { 0.0 } else { 255.0 };
            let err = old - new;

            if x + 1 < w {
                errors[idx + 1] += err * 7.0 / 16.0;
            }
            if y + 1 < h {
                if x > 0 {
                    errors[idx + w - 1] += err * 3.0 / 16.0;
                }
                errors[idx + w] += err * 5.0 / 16.0;
                if x + 1 < w {
                    errors[idx + w + 1] += err * 1.0 / 16.0;
                }
            }
        }
    }
    ink
}

#[cfg(test)]
mod tests {
    use image::ImageBuffer;

    use super::*;

    fn checkerboard(w: u32, h: u32) -> GrayImage {
        ImageBuffer::from_fn(w, h, |x, y| {
            if (x + y) % 2 == 0 {
                Luma([0u8])
            } else {
                Luma([255u8])
            }
        })
    }

    #[test]
    fn empty_image_yields_empty_bitmap() {
        let gray = GrayImage::new(0, 0);
        let bmp = render_gray(&gray, 128, 50, 39, false).unwrap();
        assert_eq!(bmp.height_dots(), 0);
    }

    #[test]
    fn all_black_image_is_fully_inked() {
        let gray = GrayImage::from_pixel(10, 10, Luma([0u8]));
        let bmp = render_gray(&gray, 128, 10, 39, false).unwrap();
        for y in 0..bmp.height_dots() {
            let has_ink = bmp.row(y).iter().any(|&b| b != 0);
            assert!(has_ink, "column {y} of an all-black image has no ink");
        }
    }

    #[test]
    fn all_white_image_is_blank() {
        let gray = GrayImage::from_pixel(10, 10, Luma([255u8]));
        let bmp = render_gray(&gray, 128, 10, 39, false).unwrap();
        for y in 0..bmp.height_dots() {
            assert!(bmp.row(y).iter().all(|&b| b == 0));
        }
    }

    #[test]
    fn invert_flips_all_black_to_blank() {
        let gray = GrayImage::from_pixel(10, 10, Luma([0u8]));
        let bmp = render_gray(&gray, 128, 10, 39, true).unwrap();
        for y in 0..bmp.height_dots() {
            assert!(bmp.row(y).iter().all(|&b| b == 0));
        }
    }

    #[test]
    fn checkerboard_produces_mixed_ink() {
        let gray = checkerboard(20, 20);
        let bmp = render_gray(&gray, 128, 20, 39, false).unwrap();
        let has_ink = (0..bmp.height_dots()).any(|y| bmp.row(y).iter().any(|&b| b != 0));
        let has_blank = (0..bmp.height_dots()).any(|y| bmp.row(y).contains(&0));
        assert!(has_ink && has_blank, "checkerboard should dither to a mix");
    }

    #[test]
    fn scales_to_fill_printable_pins_height() {
        let gray = GrayImage::from_pixel(40, 20, Luma([0u8]));
        let bmp = render_gray(&gray, 128, 50, 0, false).unwrap();
        // width scales proportionally: src 40x20 -> height 50 means width 100
        assert_eq!(bmp.height_dots(), 100);
    }

    #[test]
    fn renders_svg_file() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="40">
            <rect x="0" y="0" width="100" height="40" fill="black"/>
        </svg>"#;
        let path = std::env::temp_dir().join("labellab_render_svg_test.svg");
        std::fs::write(&path, svg).unwrap();

        let bmp = render_image(&path, 128, 50, 39, false).unwrap();
        std::fs::remove_file(&path).ok();

        assert!(bmp.height_dots() > 0);
        let has_ink = (0..bmp.height_dots()).any(|y| bmp.row(y).iter().any(|&b| b != 0));
        assert!(has_ink, "solid black SVG rect rendered no ink");
    }

    #[test]
    fn empty_svg_does_not_panic() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0"></svg>"#;
        let path = std::env::temp_dir().join("labellab_render_empty_svg_test.svg");
        std::fs::write(&path, svg).unwrap();

        let result = render_image(&path, 128, 50, 39, false);
        std::fs::remove_file(&path).ok();

        // Either an empty bitmap or a clean error is acceptable; must not panic.
        if let Ok(bmp) = result {
            assert_eq!(bmp.height_dots(), 0);
        }
    }
}
