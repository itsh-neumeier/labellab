//! Renders text with a real TrueType/OpenType font ([`fontdue`]) into a
//! [`Bitmap`] oriented for the printer: pins (tape width) run across the
//! bitmap's `x` axis, raster lines (feed direction / label length) run
//! down its `y` axis.
//!
//! No bold/italic/alignment/multi-line layout yet (M6 editor scope); this
//! renders one line, auto-sized to fill most of the tape's printable
//! height. Font family/fallback selection is `fontsrc`'s job.

use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use fontdue::{Font, FontSettings};

use crate::{fontsrc, Bitmap, RenderError};

/// Alpha threshold (0-255) above which a rasterized pixel counts as ink.
const INK_THRESHOLD: u8 = 128;

/// Renders `text` using the default system font (see [`fontsrc`]).
pub fn render_text(
    text: &str,
    head_pins: u16,
    printable_pins: u16,
    left_offset_pins: u16,
) -> Result<Bitmap, RenderError> {
    let font_data = fontsrc::load_default_font()?;
    render_text_with_font(
        text,
        &font_data,
        head_pins,
        printable_pins,
        left_offset_pins,
    )
}

/// Renders `text` with an explicit font (`font_data`: raw TTF/OTF bytes)
/// for a tape with `head_pins` total print-head pins, using
/// `printable_pins` of them starting at `left_offset_pins` (see
/// `ll_protocol::model::TapeGeometry`).
pub fn render_text_with_font(
    text: &str,
    font_data: &[u8],
    head_pins: u16,
    printable_pins: u16,
    left_offset_pins: u16,
) -> Result<Bitmap, RenderError> {
    if text.is_empty() {
        return Ok(Bitmap::new(head_pins, 0));
    }

    let font = Font::from_bytes(font_data, FontSettings::default())
        .map_err(|e| RenderError::Font(e.to_string()))?;

    // Leave headroom so ascenders/descenders aren't clipped against the
    // tape edges.
    let font_px = (printable_pins as f32 * 0.8).max(6.0);

    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings::default());
    layout.append(&[&font], &TextStyle::new(text, font_px, 0));

    let glyphs = layout.glyphs();
    let raster_lines = glyphs
        .iter()
        .map(|g| g.x as i32 + g.width as i32)
        .max()
        .unwrap_or(0)
        .max(1) as u32;

    let mut bitmap = Bitmap::new(head_pins, raster_lines);
    let vertical_offset =
        left_offset_pins as i32 + ((printable_pins as i32 - font_px.round() as i32).max(0) / 2);

    for glyph in glyphs {
        if glyph.width == 0 || glyph.height == 0 {
            continue; // space and other zero-area glyphs
        }
        let (_, coverage) = font.rasterize_config(glyph.key);
        for gy in 0..glyph.height {
            for gx in 0..glyph.width {
                if coverage[gy * glyph.width + gx] < INK_THRESHOLD {
                    continue;
                }
                let line = glyph.x as i32 + gx as i32;
                let pin = vertical_offset + glyph.y as i32 + gy as i32;
                if line < 0 || pin < 0 {
                    continue;
                }
                bitmap.set_pixel(pin as u16, line as u32, true);
            }
        }
    }

    Ok(bitmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Skips (without failing) on a machine with no candidate system font
    /// instead of asserting — CI installs one on `ubuntu-latest`, but this
    /// keeps local runs on an unusual setup from going red for an
    /// environment issue rather than a code bug.
    macro_rules! require_font {
        () => {
            match fontsrc::load_default_font() {
                Ok(data) => data,
                Err(_) => {
                    eprintln!("skip: no system font found, see ll_render::fontsrc");
                    return;
                }
            }
        };
    }

    #[test]
    fn empty_text_is_zero_width() {
        let bmp = render_text("", 128, 50, 39).unwrap();
        assert_eq!(bmp.height_dots(), 0);
    }

    #[test]
    fn nonempty_text_produces_ink() {
        let font = require_font!();
        let bmp = render_text_with_font("A", &font, 128, 50, 39).unwrap();
        assert!(bmp.height_dots() > 0);
        let has_ink = (0..bmp.height_dots()).any(|y| bmp.row(y).iter().any(|&b| b != 0));
        assert!(has_ink, "rendered 'A' has no set pixels");
    }

    #[test]
    fn longer_text_is_wider() {
        let font = require_font!();
        let short = render_text_with_font("A", &font, 128, 50, 39).unwrap();
        let long = render_text_with_font("A LABEL", &font, 128, 50, 39).unwrap();
        assert!(long.height_dots() > short.height_dots());
    }

    #[test]
    fn rendered_pixels_stay_within_head_pins() {
        let font = require_font!();
        let bmp = render_text_with_font("Hg", &font, 128, 50, 39).unwrap();
        // set_pixel silently drops out-of-range writes, so this mainly
        // guards against a future change breaking that invariant.
        assert_eq!(bmp.width_pins(), 128);
    }
}
