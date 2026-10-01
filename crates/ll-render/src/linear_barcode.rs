//! Linear (1D) barcode rendering into a [`Bitmap`]. Code128, EAN-13/8,
//! UPC-A, Code39, ITF (interleaved 2-of-5).
//!
//! Orientation differs from `text`/QR: a 1D barcode's bars run across the
//! *whole* printable tape width for every module, varying only along the
//! raster-line (length) axis — there's no vertical structure to encode.

use barcoders::sym::code128::Code128;
use barcoders::sym::code39::Code39;
use barcoders::sym::ean13::{EAN13, UPCA};
use barcoders::sym::ean8::EAN8;
use barcoders::sym::tf::TF;

use crate::{Bitmap, RenderError};

/// Bar/space width in print dots. TODO(verify): a real scanner test would
/// confirm whether this is wide enough to read reliably at 180 dpi.
const MODULE_PX: u32 = 3;

/// Which linear barcode symbology to encode with. Serialized in
/// `.llabel` templates as `"code128"`, `"ean13"`, `"upc_a"`, …
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Symbology {
    /// Any ASCII text/data. Defaults to character-set B if `data` doesn't
    /// already start with a charset-switch character.
    Code128,
    /// 12 or 13 digits (12 without, 13 with a trailing check digit).
    Ean13,
    /// 7 or 8 digits (7 without, 8 with a trailing check digit).
    Ean8,
    /// 12 or 13 digits, i.e. an EAN-13 that starts with `0`.
    UpcA,
    /// Digits, uppercase letters and a handful of symbols (`-. $/+%` and space).
    Code39,
    /// Digits only (interleaved 2-of-5); odd-length input gets an
    /// auto-computed trailing check digit.
    Itf,
}

/// Renders `data` as a barcode of the given `symbology` for a tape with
/// `head_pins` total print-head pins, filling `printable_pins` starting
/// at `left_offset_pins` for every bar.
pub fn render_barcode(
    symbology: Symbology,
    data: &str,
    head_pins: u16,
    printable_pins: u16,
    left_offset_pins: u16,
) -> Result<Bitmap, RenderError> {
    let modules = encode_modules(symbology, data)?;
    Ok(render_modules(
        &modules,
        head_pins,
        printable_pins,
        left_offset_pins,
    ))
}

/// Encodes `data` to one entry per barcode module (`1` = bar, `0` = space).
pub(crate) fn encode_modules(symbology: Symbology, data: &str) -> Result<Vec<u8>, RenderError> {
    Ok(match symbology {
        Symbology::Code128 => {
            let prefixed = ensure_start_charset(data);
            Code128::new(&prefixed)
                .map_err(|e| RenderError::Barcode(e.to_string()))?
                .encode()
        }
        Symbology::Ean13 => EAN13::new(data)
            .map_err(|e| RenderError::Barcode(e.to_string()))?
            .encode(),
        Symbology::Ean8 => EAN8::new(data)
            .map_err(|e| RenderError::Barcode(e.to_string()))?
            .encode(),
        Symbology::UpcA => UPCA::new(data)
            .map_err(|e| RenderError::Barcode(e.to_string()))?
            .encode(),
        Symbology::Code39 => Code39::new(data)
            .map_err(|e| RenderError::Barcode(e.to_string()))?
            .encode(),
        Symbology::Itf => TF::interleaved(data)
            .map_err(|e| RenderError::Barcode(e.to_string()))?
            .encode(),
    })
}

/// Renders `data` as a Code128 barcode. Shorthand for
/// `render_barcode(Symbology::Code128, ...)`.
pub fn render_code128(
    data: &str,
    head_pins: u16,
    printable_pins: u16,
    left_offset_pins: u16,
) -> Result<Bitmap, RenderError> {
    render_barcode(
        Symbology::Code128,
        data,
        head_pins,
        printable_pins,
        left_offset_pins,
    )
}

fn render_modules(
    modules: &[u8],
    head_pins: u16,
    printable_pins: u16,
    left_offset_pins: u16,
) -> Bitmap {
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

    bitmap
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

    fn has_ink(bmp: &Bitmap) -> bool {
        (0..bmp.height_dots()).any(|y| bmp.row(y).iter().any(|&b| b != 0))
    }

    #[test]
    fn renders_nonempty_barcode() {
        let bmp = render_code128("ABC-123", 128, 50, 39).unwrap();
        assert!(bmp.height_dots() > 0);
        assert!(has_ink(&bmp), "rendered barcode has no set pixels");
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

    #[test]
    fn renders_ean13() {
        let bmp = render_barcode(Symbology::Ean13, "012345678905", 128, 50, 39).unwrap();
        assert!(has_ink(&bmp));
    }

    #[test]
    fn renders_ean8() {
        let bmp = render_barcode(Symbology::Ean8, "0123456", 128, 50, 39).unwrap();
        assert!(has_ink(&bmp));
    }

    #[test]
    fn renders_upca() {
        let bmp = render_barcode(Symbology::UpcA, "012345612345", 128, 50, 39).unwrap();
        assert!(has_ink(&bmp));
    }

    #[test]
    fn renders_code39() {
        let bmp = render_barcode(Symbology::Code39, "LABELLAB-123", 128, 50, 39).unwrap();
        assert!(has_ink(&bmp));
    }

    #[test]
    fn renders_itf() {
        let bmp = render_barcode(Symbology::Itf, "123456", 128, 50, 39).unwrap();
        assert!(has_ink(&bmp));
    }

    #[test]
    fn ean13_rejects_wrong_length() {
        let err = render_barcode(Symbology::Ean13, "123", 128, 50, 39).unwrap_err();
        assert!(matches!(err, RenderError::Barcode(_)));
    }
}
