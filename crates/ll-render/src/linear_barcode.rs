//! Linear (1D) barcode rendering into a [`Bitmap`]. Code128 only for now;
//! EAN-13/8, UPC-A, Code39, ITF are still open M5 scope.
//!
//! Orientation differs from `text`/QR: a 1D barcode's bars run across the
//! *whole* printable tape width for every module, varying only along the
//! raster-line (length) axis — there's no vertical structure to encode.

use barcoders::sym::code128::Code128;

use crate::{Bitmap, RenderError};

/// Bar/space width in print dots. TODO(verify): a real scanner test would
/// confirm whether this is wide enough to read reliably at 180 dpi.
const MODULE_PX: u32 = 3;

/// Renders `data` as a Code128 barcode for a tape with `head_pins` total
/// print-head pins, filling `printable_pins` starting at
/// `left_offset_pins` for every bar.
///
/// `data` may use barcoders' character-set-switch syntax (starting with
/// `À`/`Ɓ`/`Ć`); if it doesn't, character-set B (general alphanumeric) is
/// assumed.
pub fn render_code128(
    data: &str,
    head_pins: u16,
    printable_pins: u16,
    left_offset_pins: u16,
) -> Result<Bitmap, RenderError> {
    let prefixed = ensure_start_charset(data);
    let code = Code128::new(&prefixed).map_err(|e| RenderError::Barcode(e.to_string()))?;
    let modules = code.encode();

    let total_lines = modules.len() as u32 * MODULE_PX;
    let mut bitmap = Bitmap::new(head_pins, total_lines);

    for (i, &module) in modules.iter().enumerate() {
        if module == 0 {
            continue;
        }
        for s in 0..MODULE_PX {
            let line = i as u32 * MODULE_PX + s;
            for p in 0..printable_pins {
                bitmap.set_pixel(left_offset_pins + p, line, true);
            }
        }
    }

    Ok(bitmap)
}

/// Code128 data must start with a character-set switch (`À`/`Ɓ`/`Ć`).
/// Defaults to character-set B (general alphanumeric) if the caller didn't
/// already pick one.
fn ensure_start_charset(data: &str) -> String {
    match data.chars().next() {
        Some('À' | 'Ɓ' | 'Ć') => data.to_string(),
        _ => format!("\u{0181}{data}"), // Ɓ = start character-set B
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_nonempty_barcode() {
        let bmp = render_code128("ABC-123", 128, 50, 39).unwrap();
        assert!(bmp.height_dots() > 0);
        let has_ink = (0..bmp.height_dots()).any(|y| bmp.row(y).iter().any(|&b| b != 0));
        assert!(has_ink, "rendered barcode has no set pixels");
    }

    #[test]
    fn bars_span_the_full_printable_width() {
        let bmp = render_code128("A", 128, 10, 20).unwrap();
        // Find a raster line that has ink (a "bar" column) and check it
        // spans pins 20..30 (left_offset_pins..+printable_pins).
        let bar_line = (0..bmp.height_dots())
            .find(|&y| bmp.row(y).iter().any(|&b| b != 0))
            .expect("at least one bar expected");
        for pin in 20..30 {
            let byte = bmp.row(bar_line)[(pin / 8) as usize];
            let bit = (byte >> (7 - pin % 8)) & 1;
            assert_eq!(bit, 1, "pin {pin} should be ink on a bar column");
        }
    }

    #[test]
    fn rejects_unencodable_character() {
        // Code128 character-sets A/B don't cover arbitrary Unicode.
        let err = render_code128("🎉", 128, 50, 39).unwrap_err();
        assert!(matches!(err, RenderError::Barcode(_)));
    }
}
