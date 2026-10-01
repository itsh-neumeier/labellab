//! QR code rendering into a [`Bitmap`], same orientation convention as
//! `text` (pins = tape-width axis, raster lines = feed/length axis).
//!
//! Linear barcodes (Code128/EAN/Code39/...) are still open M5 scope.

use qrcode::{EcLevel, QrCode};

use crate::{Bitmap, RenderError};

/// QR error correction level (higher = more redundant, bigger code).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrErrorCorrection {
    Low,
    Medium,
    Quartile,
    High,
}

impl From<QrErrorCorrection> for EcLevel {
    fn from(level: QrErrorCorrection) -> Self {
        match level {
            QrErrorCorrection::Low => EcLevel::L,
            QrErrorCorrection::Medium => EcLevel::M,
            QrErrorCorrection::Quartile => EcLevel::Q,
            QrErrorCorrection::High => EcLevel::H,
        }
    }
}

/// Modules of quiet (blank) border on each side. The QR spec calls for 4;
/// we use 2 to save tape, which is usually still scannable but TODO(verify)
/// against a real scanner/phone camera.
const QUIET_ZONE_MODULES: i32 = 2;

/// Renders `data` as a QR code for a tape with `head_pins` total
/// print-head pins, using `printable_pins` of them starting at
/// `left_offset_pins`. The module size (print dots per QR module) is
/// auto-fit to `printable_pins`.
pub fn render_qr(
    data: &str,
    head_pins: u16,
    printable_pins: u16,
    left_offset_pins: u16,
    ec_level: QrErrorCorrection,
) -> Result<Bitmap, RenderError> {
    let code = QrCode::with_error_correction_level(data, ec_level.into())
        .map_err(|e| RenderError::Barcode(e.to_string()))?;
    let modules = code.width() as i32;

    let module_px = (printable_pins as i32 / (modules + 2 * QUIET_ZONE_MODULES)).clamp(1, 8);
    let side_px = (modules + 2 * QUIET_ZONE_MODULES) * module_px;

    let mut bitmap = Bitmap::new(head_pins, side_px.max(1) as u32);
    let vertical_offset = left_offset_pins as i32 + ((printable_pins as i32 - side_px).max(0) / 2);

    for my in 0..modules {
        for mx in 0..modules {
            if code[(mx as usize, my as usize)] != qrcode::Color::Dark {
                continue;
            }
            for sy in 0..module_px {
                for sx in 0..module_px {
                    let line = (QUIET_ZONE_MODULES + mx) * module_px + sx;
                    let pin = vertical_offset + (QUIET_ZONE_MODULES + my) * module_px + sy;
                    if pin < 0 {
                        continue;
                    }
                    bitmap.set_pixel(pin as u16, line as u32, true);
                }
            }
        }
    }

    Ok(bitmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_nonempty_square_ish_code() {
        let bmp = render_qr(
            "https://example.com",
            128,
            50,
            39,
            QrErrorCorrection::Medium,
        )
        .unwrap();
        assert!(bmp.height_dots() > 0);
        let has_ink = (0..bmp.height_dots()).any(|y| bmp.row(y).iter().any(|&b| b != 0));
        assert!(has_ink, "rendered QR has no set pixels");
    }

    #[test]
    fn higher_ec_level_uses_more_or_equal_modules() {
        // Higher error correction adds redundancy, which can require a
        // bigger QR version (more modules) for the same data. Checked at
        // the qrcode-crate level since our rendering clamps module pixel
        // size to fit printable_pins, which isn't strictly monotonic in
        // module count.
        let low = QrCode::with_error_correction_level("test data", EcLevel::L).unwrap();
        let high = QrCode::with_error_correction_level("test data", EcLevel::H).unwrap();
        assert!(high.width() >= low.width());
    }

    #[test]
    fn rejects_data_too_long_for_a_qr_code() {
        let huge = "x".repeat(10_000);
        let err = render_qr(&huge, 128, 50, 39, QrErrorCorrection::Low).unwrap_err();
        assert!(matches!(err, RenderError::Barcode(_)));
    }
}
